# Release Finalization Audit (March 28, 2026)

## Scope

This pass reviewed the current release candidate after the broker, runtime-truth, CI, and release-sweep work landed.

Covered in this pass:
- updated Windows daemon/supervision code paths
- runtime capability and recovery truth
- supply-chain tooling closure
- CLI/TUI smoke behavior in isolated Awo roots
- roadmap and public-doc alignment

## Overall Assessment

The local-first product is now in a strong release-candidate state on the current development platform:

- broker health, degraded-state reporting, and MCP subscriptions are materially improved
- the orchestration loop is present end to end:
  - planning
  - task cards
  - delegation/start
  - review/diff
  - immutable recovery
  - cleanup
- runtime/operator truth is more honest and more actionable
- security and license checks are now both configured and locally validated
- the full workspace verification remains green

I did not find a new blocking correctness bug in the code changed during this pass.

## What Was Validated

### Verification

Passed in this pass:
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo deny check`
- `cargo audit`

### Smoke Workflows

Validated in isolated `AWO_CONFIG_DIR` / `AWO_DATA_DIR` roots:
- repo registration against a fresh temporary Git repo
- runtime capability inspection for Claude, Codex, Gemini, and Shell
- warm-slot acquire / session start / log inspection / release / delete
- team init / member add / plan add / approve / generate
- task-card add / start
- immutable supersede flow
- team report / teardown / delete
- TUI startup and quit

## Findings

### Resolved In This Pass

1. Windows ConPTY exit handling was too lossy.
   The ConPTY path collapsed non-zero exits to `1`. It now preserves the actual child exit code and uses `/T` when killing the Windows process tree.

2. Runtime/operator truth was still conflating provider limits with token exhaustion.
   The runtime model now distinguishes `provider_limited` from `exhausted`, and the CLI/TUI/reconciliation guidance reflects that difference.

3. Capability metadata was too pessimistic for some installed CLIs.
   Based on real local CLI surfaces:
   - Claude now reports native budget-guardrail and structured-output support
   - Codex now reports native structured-output support for exec-mode JSON/schema flows
   - Gemini now reports native structured-output support for headless JSON/stream-json flows

4. `deny.toml` was not actually valid against the installed `cargo-deny`.
   Local validation found schema drift plus a missing `0BSD` allowance from the new Windows transport dependency chain. Both are now fixed.

5. Platform docs had drifted behind the code.
   The public docs no longer say Windows PTY supervision is entirely missing; they now describe the implemented ConPTY and Named Pipe paths and call out the remaining validation gap honestly.

### Residual Risks

1. Real Windows workflow validation is still outstanding.
   The Windows code paths have advanced materially, but this macOS machine still cannot complete a Windows-target build because bundled SQLite cross-compilation fails before the Rust workspace can be fully validated for that target. A real Windows environment is still required for final parity signoff.

2. Provider usage telemetry is still mostly advisory.
   The product is now more honest about what each runtime can and cannot expose, but adapter-fed spend/quota ingestion is still thin.

3. Broker live delivery is good enough for the local product, but not the final long-term design.
   MCP subscriptions and event-triggered refreshes are in place, yet the system still relies on bounded notifications and polling more than a richer future live-update model would.

## Release Recommendation

Recommended for a **local release candidate** with one explicit caveat:

- Unix/macOS local use is in strong shape
- Windows should still be labeled as implemented-but-needing-final native validation

That is now a narrow, concrete blocker rather than broad product uncertainty.
