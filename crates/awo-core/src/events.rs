use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
    SessionLogLoaded {
        session_id: String,
        stream: String,
        lines_returned: usize,
        log_path: String,
        content: String,
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
    TeamTaskAdded {
        team_id: String,
        task_id: String,
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
            Self::SessionLogLoaded {
                session_id,
                stream,
                lines_returned,
                ..
            } => {
                format!("Loaded {lines_returned} line(s) of {stream} for session `{session_id}`")
            }
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
            Self::TeamTaskAdded { team_id, task_id } => {
                format!("Added task `{task_id}` to team `{team_id}`")
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
    ring: Vec<EventEntry>,
    next_seq: u64,
    capacity: usize,
}

impl EventBusInner {
    fn new(capacity: usize) -> Self {
        Self {
            ring: Vec::with_capacity(capacity.min(256)),
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
                self.ring.remove(0);
            }
            self.ring.push(entry);
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
    inner: Arc<Mutex<EventBusInner>>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.lock().unwrap();
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
            inner: Arc::new(Mutex::new(EventBusInner::new(capacity))),
        }
    }

    /// Publish a batch of events to the bus, assigning sequence numbers.
    pub fn publish(&self, events: &[DomainEvent]) {
        if events.is_empty() {
            return;
        }
        self.inner.lock().unwrap().publish(events);
    }

    /// Poll for events newer than `since_seq`, up to `limit` entries.
    pub fn poll(&self, since_seq: u64, limit: usize) -> EventPollResult {
        self.inner.lock().unwrap().poll(since_seq, limit)
    }

    /// Return the current head sequence number without fetching events.
    pub fn head_seq(&self) -> u64 {
        self.inner.lock().unwrap().head_seq()
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
}
