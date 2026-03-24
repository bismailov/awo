# Job Card D: Git Status Caching for Snapshot Performance

## Objective

Eliminate redundant `git status --porcelain` calls from the TUI hot path. Currently, every `snapshot()` call runs `git status` per dirty slot to collect dirty file lists — in the TUI this happens every 200ms. With 5 dirty slots, that's 25 git processes per second. Move dirty file collection behind a time-based cache so git is called at most once per slot per N seconds.

## Scope

**Two files**:
- `crates/awo-core/src/snapshot.rs` — cache dirty file results
- `crates/awo-core/src/git.rs` — no changes needed, just consumed differently

No changes to `awo-app`. The cache is internal to snapshot building.

## What to build

### 1. Slot dirty file cache

Add a simple time-based cache for dirty file lists. Since `AppSnapshot::load()` is called from `AppCore::snapshot()`, and `AppCore` is long-lived in the TUI, we can store the cache on `AppCore` or pass it through.

The cleanest approach: add a `DirtyFileCache` that lives alongside the snapshot builder.

```rust
// In snapshot.rs
use std::collections::HashMap;
use std::time::{Duration, Instant};

const DIRTY_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct DirtyFileCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    files: Vec<String>,
    cached_at: Instant,
}

impl DirtyFileCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get_or_refresh(&mut self, slot_id: &str, slot_path: &str) -> Vec<String> {
        let now = Instant::now();
        if let Some(entry) = self.entries.get(slot_id) {
            if now.duration_since(entry.cached_at) < DIRTY_CACHE_TTL {
                return entry.files.clone();
            }
        }
        let files = git::dirty_files(Path::new(slot_path)).unwrap_or_else(|err| {
            tracing::warn!(slot_id, %err, "failed to list dirty files for slot");
            vec![]
        });
        self.entries.insert(
            slot_id.to_string(),
            CacheEntry {
                files: files.clone(),
                cached_at: now,
            },
        );
        files
    }

    /// Remove entries for slots that no longer exist.
    pub fn retain_slots(&mut self, active_slot_ids: &[&str]) {
        self.entries
            .retain(|id, _| active_slot_ids.contains(&id.as_str()));
    }

    /// Force-invalidate a specific slot (call after slot refresh or release).
    pub fn invalidate(&mut self, slot_id: &str) {
        self.entries.remove(slot_id);
    }
}
```

### 2. Thread the cache through snapshot building

Change `AppSnapshot::load()` signature to accept `&mut DirtyFileCache`:

```rust
pub fn load(
    config: &AppConfig,
    store: &Store,
    dirty_cache: &mut DirtyFileCache,
) -> AwoResult<AppSnapshot>
```

In `build_review_summary()`, replace the inline `git::dirty_files()` call with `dirty_cache.get_or_refresh(slot_id, slot_path)`.

### 3. Store cache on AppCore

Add `dirty_cache: DirtyFileCache` field to `AppCore`:

```rust
pub struct AppCore {
    config: AppConfig,
    store: Store,
    dirty_cache: DirtyFileCache,
}
```

Initialize in `from_config()`:
```rust
Ok(Self {
    config,
    store,
    dirty_cache: DirtyFileCache::new(),
})
```

Update `snapshot()` to pass `&mut self.dirty_cache`.

### 4. Invalidate on mutations

After `SlotRelease` and `SlotRefresh` commands complete, invalidate the slot's cache entry. The cleanest place is in `AppCore::dispatch()` — inspect the command and invalidate:

```rust
pub fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
    match &command {
        Command::SlotRelease { slot_id } => self.dirty_cache.invalidate(slot_id),
        Command::SlotRefresh { slot_id } => self.dirty_cache.invalidate(slot_id),
        _ => {}
    }
    Dispatcher::dispatch(self, command)
}
```

### 5. Prune stale cache entries

In `snapshot()`, after loading, prune cache entries for slots that no longer exist:

```rust
let slot_ids: Vec<&str> = snapshot.slots.iter().map(|s| s.id.as_str()).collect();
self.dirty_cache.retain_slots(&slot_ids);
```

## Constraints

- Do NOT change `git.rs` — the cache wraps the existing functions.
- Do NOT add any new crate dependencies.
- Keep the cache simple — `HashMap` + `Instant`, no async, no threads.
- The TTL (5 seconds) is a constant, not configurable. It can be tuned later.
- `DirtyFileCache` must derive `Debug` (required by `AppCore`).
- Do NOT change the `build_review_summary_from_summaries` function used in repo-scoped reviews from the CLI — only cache in the TUI-facing snapshot path.

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Existing tests should pass without changes — tests don't exercise the cache (they use fresh `AppCore` instances with no dirty slots).

Optionally verify performance:
1. Register a repo with 3+ active slots
2. Make some files dirty in each slot worktree
3. Run `awo tui` — should feel responsive (no visible lag)
4. Previously: 3 git processes × 5 per second = 15 git invocations/sec
5. After: 3 git processes × once per 5 seconds = 0.6 git invocations/sec

## What NOT to do

- Do not add file watching (inotify/kqueue)
- Do not add async or threading for cache refresh
- Do not cache `git status` for the dirty boolean — only cache the file list
- Do not modify `tui.rs`
- Do not modify test files
