//! Proximity-based authority handoff.  SPEC §6.2.

use parking_lot::Mutex;
use sekiro_sdk_core::entity::EntityId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::table::PeerId;

pub const HANDOFF_TIMEOUT_MS: u64 = 200;

/// Per-entity handoff state.
#[derive(Debug, Clone, Copy)]
pub enum HandoffOutcome {
    /// Not in flight.
    Idle,
    /// Waiting for `HandoffAck` from the new owner.
    Pending,
    /// Ack received; authority finalised.
    Acked,
    /// Timed out without ack; to be retried by caller.
    TimedOut,
}

#[derive(Debug, Clone, Copy)]
struct Tracker {
    new_owner: PeerId,
    started_at: Instant,
    outcome: HandoffOutcome,
}

/// Tracks in-flight handoffs so the authority layer can retry on
/// missing ack.
#[derive(Debug, Default)]
pub struct HandoffTracker {
    in_flight: Mutex<HashMap<EntityId, Tracker>>,
}

impl HandoffTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a handoff.  Stores the (entity, new_owner) pair and arms
    /// the timeout.
    pub fn start(&self, id: EntityId, new_owner: PeerId) {
        self.in_flight.lock().insert(
            id,
            Tracker {
                new_owner,
                started_at: Instant::now(),
                outcome: HandoffOutcome::Pending,
            },
        );
    }

    /// Ack received from the new owner.
    pub fn ack(&self, id: EntityId) -> HandoffOutcome {
        let mut table = self.in_flight.lock();
        match table.get_mut(&id) {
            Some(t) => {
                t.outcome = HandoffOutcome::Acked;
                HandoffOutcome::Acked
            }
            None => HandoffOutcome::Idle,
        }
    }

    /// Sweep the table; return entities whose handoffs have timed out
    /// (and clear them from the tracker).
    pub fn sweep_timeouts(&self) -> Vec<(EntityId, PeerId)> {
        let mut out = Vec::new();
        let mut table = self.in_flight.lock();
        let now = Instant::now();
        let timeout = Duration::from_millis(HANDOFF_TIMEOUT_MS);
        table.retain(|id, t| match t.outcome {
            HandoffOutcome::Pending if now.duration_since(t.started_at) >= timeout => {
                out.push((*id, t.new_owner));
                false
            }
            HandoffOutcome::Acked => false, // clean up
            _ => true,
        });
        out
    }

    pub fn is_pending(&self, id: EntityId) -> bool {
        matches!(
            self.in_flight.lock().get(&id).map(|t| t.outcome),
            Some(HandoffOutcome::Pending)
        )
    }

    pub fn len(&self) -> usize {
        self.in_flight.lock().len()
    }
}
