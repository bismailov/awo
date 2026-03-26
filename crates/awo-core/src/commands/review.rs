use super::{CommandOutcome, CommandRunner};
use crate::error::AwoResult;
use crate::events::DomainEvent;
use crate::slot::{FingerprintStatus, SlotStatus};

impl<'a> CommandRunner<'a> {
    pub(super) fn run_review_status(
        &mut self,
        repo_id: Option<String>,
    ) -> AwoResult<CommandOutcome> {
        self.sync_runtime_state(repo_id.as_deref())?;
        let mut slots = self.store.list_slots(repo_id.as_deref())?;
        let mut dirty = 0usize;
        let mut stale = 0usize;

        for slot in &mut slots {
            if slot.status == SlotStatus::Active || slot.status == SlotStatus::Released {
                self.refresh_slot_state(slot)?;
                self.store.upsert_slot(slot)?;
            }
            if slot.dirty {
                dirty += 1;
            }
            if slot.fingerprint_status == FingerprintStatus::Stale {
                stale += 1;
            }
        }

        self.store
            .insert_action("review_status", &format!("dirty={} stale={}", dirty, stale))?;

        let events = vec![
            DomainEvent::CommandReceived {
                command: "review_status".to_string(),
            },
            DomainEvent::ReviewStatusLoaded { dirty, stale },
        ];

        Ok(CommandOutcome::with_events(
            format!("Review status updated: {dirty} dirty, {stale} stale."),
            events,
        ))
    }
}
