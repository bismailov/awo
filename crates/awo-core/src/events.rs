use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    CommandReceived {
        command: String,
    },
    NoOpCompleted {
        label: String,
        config_dir: String,
        state_db_path: String,
    },
    RepoRegistered {
        id: String,
        name: String,
        repo_root: String,
        default_base_branch: String,
        worktree_root: String,
    },
    RepoRemoved {
        id: String,
        name: String,
    },
    RepoListLoaded {
        count: usize,
    },
    ContextLoaded {
        repo_id: String,
        entrypoints: usize,
        packs: usize,
    },
    ContextDoctorCompleted {
        repo_id: String,
        errors: usize,
        warnings: usize,
    },
    SkillsCatalogLoaded {
        repo_id: String,
        skills: usize,
    },
    SkillsDoctorCompleted {
        repo_id: String,
        runtimes: usize,
        warnings: usize,
    },
    SkillsLinked {
        repo_id: String,
        runtime: String,
        linked: usize,
    },
    SkillsSynced {
        repo_id: String,
        runtime: String,
        linked: usize,
    },
    SlotAcquired {
        slot_id: String,
        repo_id: String,
        branch_name: String,
        slot_path: String,
        strategy: String,
    },
    SlotListLoaded {
        count: usize,
    },
    SlotReleased {
        slot_id: String,
        strategy: String,
    },
    SlotDeleted {
        slot_id: String,
        had_worktree: bool,
    },
    SlotPruned {
        repo_id: Option<String>,
        pruned: usize,
        skipped: usize,
    },
    SlotRefreshed {
        slot_id: String,
        dirty: bool,
        fingerprint_status: String,
    },
    SessionContextPrepared {
        slot_id: String,
        files: usize,
        packs: Vec<String>,
    },
    SessionStarted {
        session_id: String,
        slot_id: String,
        runtime: String,
        supervisor: Option<String>,
        status: String,
    },
    SessionCancelled {
        session_id: String,
        slot_id: String,
    },
    SessionDeleted {
        session_id: String,
    },
    SessionListLoaded {
        count: usize,
    },
    ReviewStatusLoaded {
        dirty: usize,
        stale: usize,
    },
    ReviewDiffLoaded {
        slot_id: String,
        changed_files: usize,
    },
    SessionLogLoaded {
        session_id: String,
        stream: String,
        lines_returned: usize,
        log_path: String,
        content: String,
    },
    SessionTerminalCaptured {
        session_id: String,
        lines_returned: usize,
        max_lines: usize,
    },
    SessionTerminalInputSent {
        session_id: String,
        input_kind: String,
    },
    TeamArchived {
        team_id: String,
    },
    TeamReset {
        team_id: String,
        tasks_reset: usize,
        slots_unbound: usize,
    },
    TeamTaskStarted {
        team_id: String,
        task_id: String,
        routing_reason: String,
    },
    TeamTaskDelegated {
        team_id: String,
        task_id: String,
        target_member_id: String,
        auto_started: bool,
    },
    TeamListLoaded {
        repo_id: Option<String>,
        count: usize,
    },
    TeamLoaded {
        team_id: String,
    },
    TeamCreated {
        team_id: String,
        repo_id: String,
    },
    TeamMemberAdded {
        team_id: String,
        member_id: String,
    },
    TeamMemberUpdated {
        team_id: String,
        member_id: String,
    },
    TeamMemberRemoved {
        team_id: String,
        member_id: String,
    },
    TeamMemberSlotAssigned {
        team_id: String,
        member_id: String,
        slot_id: String,
    },
    TeamLeadReplaced {
        team_id: String,
        member_id: String,
    },
    TeamPlanAdded {
        team_id: String,
        plan_id: String,
    },
    TeamPlanApproved {
        team_id: String,
        plan_id: String,
    },
    TeamPlanGenerated {
        team_id: String,
        plan_id: String,
        task_id: String,
    },
    TeamTaskAdded {
        team_id: String,
        task_id: String,
    },
    TeamTaskSlotBound {
        team_id: String,
        task_id: String,
        slot_id: String,
    },
    TeamTaskAccepted {
        team_id: String,
        task_id: String,
    },
    TeamTaskReworkRequested {
        team_id: String,
        task_id: String,
    },
    TeamTaskCancelled {
        team_id: String,
        task_id: String,
    },
    TeamTaskSuperseded {
        team_id: String,
        task_id: String,
        replacement_task_id: String,
    },
    TeamReportGenerated {
        team_id: String,
        report_path: String,
    },
    TeamDeleted {
        team_id: String,
    },
}

impl DomainEvent {
    pub fn to_message(&self) -> String {
        match self {
            Self::CommandReceived { command } => format!("Command received: {command}"),
            Self::NoOpCompleted {
                label,
                config_dir,
                state_db_path,
            } => format!(
                "No-op finished for `{label}`. Config: {config_dir}. State DB: {state_db_path}"
            ),
            Self::RepoRegistered {
                id,
                name,
                repo_root,
                default_base_branch,
                worktree_root,
            } => format!(
                "Registered repo `{name}` ({id}) at {repo_root}. Base branch: {default_base_branch}. Worktrees: {worktree_root}"
            ),
            Self::RepoRemoved { id, name } => {
                format!("Removed repo `{name}` ({id}).")
            }
            Self::RepoListLoaded { count } => format!("Loaded {count} registered repo(s)."),
            Self::ContextLoaded {
                repo_id,
                entrypoints,
                packs,
            } => format!(
                "Loaded context for repo `{repo_id}`: {entrypoints} entrypoint(s), {packs} pack(s)"
            ),
            Self::ContextDoctorCompleted {
                repo_id,
                errors,
                warnings,
            } => format!(
                "Context doctor for repo `{repo_id}` finished with {errors} error(s) and {warnings} warning(s)"
            ),
            Self::SkillsCatalogLoaded { repo_id, skills } => {
                format!("Loaded {skills} shared skill(s) for repo `{repo_id}`")
            }
            Self::SkillsDoctorCompleted {
                repo_id,
                runtimes,
                warnings,
            } => format!(
                "Skills doctor for repo `{repo_id}` finished across {runtimes} runtime(s) with {warnings} warning(s)"
            ),
            Self::SkillsLinked {
                repo_id,
                runtime,
                linked,
            } => format!("Linked {linked} skill(s) for repo `{repo_id}` into `{runtime}`"),
            Self::SkillsSynced {
                repo_id,
                runtime,
                linked,
            } => format!("Synced {linked} skill(s) for repo `{repo_id}` into `{runtime}`"),
            Self::SlotAcquired {
                slot_id,
                repo_id,
                branch_name,
                slot_path,
                strategy,
            } => format!(
                "Acquired {strategy} slot `{slot_id}` for repo `{repo_id}` on branch `{branch_name}` at {slot_path}"
            ),
            Self::SlotListLoaded { count } => format!("Loaded {count} slot(s)."),
            Self::SlotReleased { slot_id, strategy } => {
                format!("Released {strategy} slot `{slot_id}`")
            }
            Self::SlotDeleted {
                slot_id,
                had_worktree,
            } => {
                if *had_worktree {
                    format!("Deleted slot `{slot_id}` and removed its worktree")
                } else {
                    format!("Deleted slot `{slot_id}` from local state")
                }
            }
            Self::SlotPruned {
                repo_id,
                pruned,
                skipped,
            } => match repo_id {
                Some(repo_id) => format!(
                    "Pruned {pruned} released slot(s) for repo `{repo_id}`; skipped {skipped}"
                ),
                None => format!("Pruned {pruned} released slot(s); skipped {skipped}"),
            },
            Self::SlotRefreshed {
                slot_id,
                dirty,
                fingerprint_status,
            } => format!(
                "Refreshed slot `{slot_id}`. dirty={dirty} fingerprint={fingerprint_status}"
            ),
            Self::SessionContextPrepared {
                slot_id,
                files,
                packs,
            } => format!(
                "Prepared launch context for slot `{slot_id}` with {files} file(s) and packs [{}]",
                if packs.is_empty() {
                    "-".to_string()
                } else {
                    packs.join(", ")
                }
            ),
            Self::SessionStarted {
                session_id,
                slot_id,
                runtime,
                supervisor,
                status,
            } => {
                format!(
                    "Session `{session_id}` for slot `{slot_id}` using `{runtime}`{} is {status}",
                    supervisor
                        .as_deref()
                        .map(|value| format!(" via `{value}`"))
                        .unwrap_or_default()
                )
            }
            Self::SessionCancelled {
                session_id,
                slot_id,
            } => format!("Session `{session_id}` for slot `{slot_id}` was cancelled"),
            Self::SessionDeleted { session_id } => {
                format!("Session `{session_id}` was deleted from local state")
            }
            Self::SessionListLoaded { count } => format!("Loaded {count} session(s)."),
            Self::ReviewStatusLoaded { dirty, stale } => {
                format!("Review status: {dirty} dirty slot(s), {stale} stale slot(s)")
            }
            Self::ReviewDiffLoaded {
                slot_id,
                changed_files,
            } => {
                format!(
                    "Loaded review diff for slot `{slot_id}` with {changed_files} changed file(s)"
                )
            }
            Self::SessionLogLoaded {
                session_id,
                stream,
                lines_returned,
                ..
            } => {
                format!("Loaded {lines_returned} line(s) of {stream} for session `{session_id}`")
            }
            Self::SessionTerminalCaptured {
                session_id,
                lines_returned,
                max_lines,
            } => format!(
                "Captured {lines_returned} line(s) of live terminal output for session `{session_id}` (max {max_lines})"
            ),
            Self::SessionTerminalInputSent {
                session_id,
                input_kind,
            } => format!("Sent embedded terminal {input_kind} input to session `{session_id}`"),
            Self::TeamArchived { team_id } => {
                format!("Team `{team_id}` archived")
            }
            Self::TeamReset {
                team_id,
                tasks_reset,
                slots_unbound,
            } => {
                format!(
                    "Team `{team_id}` reset to planning: {tasks_reset} task(s) reset, {slots_unbound} slot binding(s) cleared"
                )
            }
            Self::TeamTaskStarted {
                team_id,
                task_id,
                routing_reason,
            } => {
                format!("Team task `{task_id}` started on team `{team_id}`: {routing_reason}")
            }
            Self::TeamTaskDelegated {
                team_id,
                task_id,
                target_member_id,
                auto_started,
            } => {
                format!(
                    "Team task `{task_id}` delegated to `{target_member_id}` on team `{team_id}` (auto_start={auto_started})"
                )
            }
            Self::TeamListLoaded { repo_id, count } => {
                format!(
                    "Loaded {count} team manifest(s){}",
                    repo_id
                        .as_ref()
                        .map(|id| format!(" for repo `{id}`"))
                        .unwrap_or_default()
                )
            }
            Self::TeamLoaded { team_id } => {
                format!("Loaded team `{team_id}`")
            }
            Self::TeamCreated { team_id, repo_id } => {
                format!("Created team `{team_id}` for repo `{repo_id}`")
            }
            Self::TeamMemberAdded { team_id, member_id } => {
                format!("Added member `{member_id}` to team `{team_id}`")
            }
            Self::TeamMemberUpdated { team_id, member_id } => {
                format!("Updated member `{member_id}` in team `{team_id}`")
            }
            Self::TeamMemberRemoved { team_id, member_id } => {
                format!("Removed member `{member_id}` from team `{team_id}`")
            }
            Self::TeamMemberSlotAssigned {
                team_id,
                member_id,
                slot_id,
            } => {
                format!("Assigned slot `{slot_id}` to member `{member_id}` in team `{team_id}`")
            }
            Self::TeamLeadReplaced { team_id, member_id } => {
                format!("Current lead for team `{team_id}` is now `{member_id}`")
            }
            Self::TeamPlanAdded { team_id, plan_id } => {
                format!("Added plan item `{plan_id}` to team `{team_id}`")
            }
            Self::TeamPlanApproved { team_id, plan_id } => {
                format!("Approved plan item `{plan_id}` in team `{team_id}`")
            }
            Self::TeamPlanGenerated {
                team_id,
                plan_id,
                task_id,
            } => {
                format!("Generated task `{task_id}` from plan item `{plan_id}` in team `{team_id}`")
            }
            Self::TeamTaskAdded { team_id, task_id } => {
                format!("Added task `{task_id}` to team `{team_id}`")
            }
            Self::TeamTaskSlotBound {
                team_id,
                task_id,
                slot_id,
            } => {
                format!("Bound slot `{slot_id}` to task `{task_id}` in team `{team_id}`")
            }
            Self::TeamTaskAccepted { team_id, task_id } => {
                format!("Accepted task `{task_id}` in team `{team_id}`")
            }
            Self::TeamTaskReworkRequested { team_id, task_id } => {
                format!("Sent task `{task_id}` back for rework in team `{team_id}`")
            }
            Self::TeamTaskCancelled { team_id, task_id } => {
                format!("Cancelled task `{task_id}` in team `{team_id}`")
            }
            Self::TeamTaskSuperseded {
                team_id,
                task_id,
                replacement_task_id,
            } => {
                format!(
                    "Superseded task `{task_id}` in team `{team_id}` with `{replacement_task_id}`"
                )
            }
            Self::TeamReportGenerated {
                team_id,
                report_path,
            } => {
                format!("Generated report for team `{team_id}` at `{report_path}`")
            }
            Self::TeamDeleted { team_id } => {
                format!("Deleted team `{team_id}`")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Event bus — sequence-numbered ring buffer with poll-based consumption
// ---------------------------------------------------------------------------

/// A timestamped, sequence-numbered event entry stored in the ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub seq: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event: DomainEvent,
}

/// The result of polling the event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPollResult {
    /// Events newer than the requested sequence number.
    pub entries: Vec<EventEntry>,
    /// The current head sequence number (use as `since_seq` for next poll).
    pub head_seq: u64,
}

const DEFAULT_RING_CAPACITY: usize = 1024;

struct EventBusInner {
    ring: VecDeque<EventEntry>,
    next_seq: u64,
    capacity: usize,
}

impl EventBusInner {
    fn new(capacity: usize) -> Self {
        Self {
            ring: VecDeque::with_capacity(capacity.min(256)),
            next_seq: 1,
            capacity,
        }
    }

    fn publish(&mut self, events: &[DomainEvent]) {
        let now = chrono::Utc::now();
        for event in events {
            let entry = EventEntry {
                seq: self.next_seq,
                timestamp: now,
                event: event.clone(),
            };
            self.next_seq += 1;

            if self.ring.len() >= self.capacity {
                self.ring.pop_front(); // O(1) instead of Vec::remove(0) which is O(n)
            }
            self.ring.push_back(entry);
        }
    }

    fn poll(&self, since_seq: u64, limit: usize) -> EventPollResult {
        let entries: Vec<EventEntry> = self
            .ring
            .iter()
            .filter(|entry| entry.seq > since_seq)
            .take(limit)
            .cloned()
            .collect();
        EventPollResult {
            entries,
            head_seq: self.head_seq(),
        }
    }

    fn head_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }
}

/// Thread-safe event bus backed by a bounded ring buffer.
///
/// Events are assigned monotonically increasing sequence numbers. Clients
/// poll with a `since_seq` cursor to receive only new events. When the
/// buffer is full, the oldest events are evicted.
#[derive(Clone)]
pub struct EventBus {
    shared: Arc<EventBusShared>,
}

struct EventBusShared {
    inner: Mutex<EventBusInner>,
    changed: Condvar,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.lock_inner("debug");
        f.debug_struct("EventBus")
            .field("head_seq", &inner.head_seq())
            .field("buffered", &inner.ring.len())
            .field("capacity", &inner.capacity)
            .finish()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    /// Create a new event bus with the default ring buffer capacity (1024).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_RING_CAPACITY)
    }

    /// Create a new event bus with a custom ring buffer capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            shared: Arc::new(EventBusShared {
                inner: Mutex::new(EventBusInner::new(capacity)),
                changed: Condvar::new(),
            }),
        }
    }

    fn lock_inner(&self, context: &'static str) -> MutexGuard<'_, EventBusInner> {
        match self.shared.inner.lock() {
            Ok(inner) => inner,
            Err(poisoned) => {
                warn!(
                    context = context,
                    "event bus mutex poisoned; recovering inner state"
                );
                poisoned.into_inner()
            }
        }
    }

    fn wait_for_change<'a>(
        &self,
        inner: MutexGuard<'a, EventBusInner>,
        since_seq: u64,
        timeout: Duration,
    ) -> MutexGuard<'a, EventBusInner> {
        match self
            .shared
            .changed
            .wait_timeout_while(inner, timeout, |state| state.head_seq() <= since_seq)
        {
            Ok((locked, _)) => locked,
            Err(poisoned) => {
                warn!(
                    context = "wait",
                    "event bus condvar wait encountered poisoned state; recovering"
                );
                let (locked, _) = poisoned.into_inner();
                locked
            }
        }
    }

    /// Publish a batch of events to the bus, assigning sequence numbers.
    pub fn publish(&self, events: &[DomainEvent]) {
        if events.is_empty() {
            return;
        }
        let mut inner = self.lock_inner("publish");
        inner.publish(events);
        drop(inner);
        self.shared.changed.notify_all();
    }

    /// Poll for events newer than `since_seq`, up to `limit` entries.
    pub fn poll(&self, since_seq: u64, limit: usize) -> EventPollResult {
        self.lock_inner("poll").poll(since_seq, limit)
    }

    /// Wait for events newer than `since_seq`, up to `limit` entries, or until `timeout`.
    pub fn wait(&self, since_seq: u64, limit: usize, timeout: Duration) -> EventPollResult {
        let mut inner = self.lock_inner("wait");
        if inner.head_seq() <= since_seq && !timeout.is_zero() {
            inner = self.wait_for_change(inner, since_seq, timeout);
        }

        inner.poll(since_seq, limit)
    }

    /// Return the current head sequence number without fetching events.
    pub fn head_seq(&self) -> u64 {
        self.lock_inner("head_seq").head_seq()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_bus_publish_and_poll() {
        let bus = EventBus::new();
        assert_eq!(bus.head_seq(), 0);

        bus.publish(&[DomainEvent::CommandReceived {
            command: "noop".to_string(),
        }]);
        assert_eq!(bus.head_seq(), 1);

        let result = bus.poll(0, 100);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].seq, 1);
        assert_eq!(result.head_seq, 1);

        // Polling with head_seq returns nothing new
        let result2 = bus.poll(1, 100);
        assert!(result2.entries.is_empty());
        assert_eq!(result2.head_seq, 1);
    }

    #[test]
    fn event_bus_sequence_numbers_are_monotonic() {
        let bus = EventBus::new();
        bus.publish(&[
            DomainEvent::CommandReceived {
                command: "first".to_string(),
            },
            DomainEvent::CommandReceived {
                command: "second".to_string(),
            },
        ]);

        let result = bus.poll(0, 100);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].seq, 1);
        assert_eq!(result.entries[1].seq, 2);
    }

    #[test]
    fn event_bus_wait_returns_existing_events_without_blocking() {
        let bus = EventBus::new();
        bus.publish(&[DomainEvent::CommandReceived {
            command: "ready".to_string(),
        }]);

        let result = bus.wait(0, 100, Duration::from_secs(1));
        assert_eq!(result.entries.len(), 1);
        assert_eq!(
            result.entries[0].event.to_message(),
            "Command received: ready"
        );
    }

    #[test]
    fn event_bus_wait_blocks_until_new_event_arrives() {
        let bus = EventBus::new();
        let publisher = bus.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            publisher.publish(&[DomainEvent::CommandReceived {
                command: "later".to_string(),
            }]);
        });

        let result = bus.wait(0, 100, Duration::from_secs(1));
        assert_eq!(result.entries.len(), 1);
        assert_eq!(
            result.entries[0].event.to_message(),
            "Command received: later"
        );
        assert_eq!(result.head_seq, 1);
    }

    #[test]
    fn event_bus_wait_times_out_without_new_events() {
        let bus = EventBus::new();

        let result = bus.wait(0, 100, Duration::from_millis(10));
        assert!(result.entries.is_empty());
        assert_eq!(result.head_seq, 0);
    }

    #[test]
    fn event_bus_respects_limit() {
        let bus = EventBus::new();
        bus.publish(&[
            DomainEvent::CommandReceived {
                command: "a".to_string(),
            },
            DomainEvent::CommandReceived {
                command: "b".to_string(),
            },
            DomainEvent::CommandReceived {
                command: "c".to_string(),
            },
        ]);

        let result = bus.poll(0, 2);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[1].seq, 2);
    }

    #[test]
    fn event_bus_ring_evicts_oldest() {
        let bus = EventBus::with_capacity(3);
        for i in 1..=5 {
            bus.publish(&[DomainEvent::CommandReceived {
                command: format!("cmd-{i}"),
            }]);
        }

        let result = bus.poll(0, 100);
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].seq, 3);
        assert_eq!(result.entries[2].seq, 5);
    }

    #[test]
    fn event_bus_poll_with_cursor_after_eviction() {
        let bus = EventBus::with_capacity(3);
        for i in 1..=5 {
            bus.publish(&[DomainEvent::CommandReceived {
                command: format!("cmd-{i}"),
            }]);
        }

        let result = bus.poll(4, 100);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].seq, 5);
    }

    #[test]
    fn event_bus_empty_publish_is_noop() {
        let bus = EventBus::new();
        bus.publish(&[]);
        assert_eq!(bus.head_seq(), 0);
    }

    #[test]
    fn event_bus_clone_shares_state() {
        let bus = EventBus::new();
        let bus2 = bus.clone();

        bus.publish(&[DomainEvent::CommandReceived {
            command: "from-bus1".to_string(),
        }]);

        let result = bus2.poll(0, 100);
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn event_bus_recovers_after_poisoned_mutex() {
        let bus = EventBus::new();
        let poisoned_bus = bus.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned_bus.shared.inner.lock().unwrap();
            panic!("poison event bus");
        })
        .join();

        bus.publish(&[DomainEvent::CommandReceived {
            command: "recovered".to_string(),
        }]);

        assert_eq!(bus.head_seq(), 1);
        let result = bus.poll(0, 10);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(
            result.entries[0].event.to_message(),
            "Command received: recovered"
        );
    }
}
