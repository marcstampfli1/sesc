//! Proximity handoff driver.
//!
//! Tick-rate loop: scan shared entities, detect proximity ownership
//! flips, dispatch `HandoffPacket` requests via the caller-provided
//! channel, and sweep the retransmit/timeout state each tick.  SPEC
//! §6.2, EMEVD event 99002.

use parking_lot::Mutex;
use sekiro_sdk_core::entity::{EntityId, EntityKind};
use std::collections::HashMap;

use crate::handoff::HandoffTracker;
use crate::table::{AuthorityLevel, AuthorityTable, PeerId};

/// One entity's proximity snapshot for this tick.
#[derive(Debug, Clone, Copy)]
pub struct ProximityObservation {
    pub id: EntityId,
    pub kind: EntityKind,
    pub host_distance_m: f32,
    pub client_distance_m: f32,
}

/// Decision emitted by the driver.  Higher layers turn these into
/// wire packets / table mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffDecision {
    /// Transfer authority from us to the remote peer.
    TransferOut { entity: EntityId, to: PeerId },
    /// We should claim authority (remote has lost proximity).
    ClaimHere { entity: EntityId },
    /// Nothing to do.
    NoOp,
}

#[derive(Debug, Clone, Copy)]
pub struct HandoffPolicy {
    pub radius_m: f32,
    /// Hysteresis window (metres) to prevent thrashing when a
    /// player orbits the enemy at exactly `radius_m`.
    pub hysteresis_m: f32,
}

impl HandoffPolicy {
    pub const DEFAULT: Self = HandoffPolicy {
        radius_m: 50.0,
        hysteresis_m: 5.0,
    };
}

/// Stateful driver.  Construct once; call [`Self::tick`] per game tick.
pub struct ProximityDriver {
    pub policy: HandoffPolicy,
    last_observation: Mutex<HashMap<EntityId, ProximityState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProximityState {
    HostOnly,
    ClientOnly,
    Both,
    Neither,
}

impl ProximityDriver {
    pub fn new(policy: HandoffPolicy) -> Self {
        Self {
            policy,
            last_observation: Mutex::new(HashMap::new()),
        }
    }

    /// Classify a single observation with hysteresis.
    fn classify(&self, obs: ProximityObservation) -> ProximityState {
        let radius = self.policy.radius_m;
        let hys = self.policy.hysteresis_m;
        let host_in = obs.host_distance_m < radius;
        let client_in = obs.client_distance_m < radius;
        let host_out = obs.host_distance_m > radius + hys;
        let client_out = obs.client_distance_m > radius + hys;
        match (host_in, client_in, host_out, client_out) {
            (true, true, _, _) => ProximityState::Both,
            (true, false, _, true) => ProximityState::HostOnly,
            (false, true, true, _) => ProximityState::ClientOnly,
            (false, false, true, true) => ProximityState::Neither,
            _ => {
                // Inside hysteresis — keep previous classification.
                self.last_observation
                    .lock()
                    .get(&obs.id)
                    .copied()
                    .unwrap_or(ProximityState::Neither)
            }
        }
    }

    /// Process a tick.  Produces decisions for every entity observed.
    /// Callers should apply them via `HandoffTracker::start` + wire
    /// packet emission; when the ack arrives, call `handoffs.ack()`.
    pub fn tick(
        &self,
        me: PeerId,
        observations: impl IntoIterator<Item = ProximityObservation>,
        table: &AuthorityTable,
        handoffs: &HandoffTracker,
    ) -> Vec<HandoffDecision> {
        let mut decisions = Vec::new();
        let mut obs_by_id = HashMap::new();
        for obs in observations {
            obs_by_id.insert(obs.id, obs);
            let classed = self.classify(obs);
            let mut prev = self.last_observation.lock();
            let last = prev.insert(obs.id, classed);
            drop(prev);
            // Don't re-queue a handoff that's already in flight.
            if handoffs.is_pending(obs.id) {
                continue;
            }
            let current_auth = table.get(obs.id);
            let decision = propose(me, obs.id, last, classed, current_auth);
            if decision != HandoffDecision::NoOp {
                decisions.push(decision);
            }
        }
        decisions
    }

    /// Forget entities that disappeared (e.g. despawn + respawn should
    /// not reuse an old classification).
    pub fn forget(&self, entity: EntityId) {
        self.last_observation.lock().remove(&entity);
    }
}

fn propose(
    me: PeerId,
    entity: EntityId,
    prev: Option<ProximityState>,
    now: ProximityState,
    current_auth: AuthorityLevel,
) -> HandoffDecision {
    if prev == Some(now) {
        return HandoffDecision::NoOp;
    }
    let target_owner = match now {
        ProximityState::HostOnly => PeerId::Host,
        ProximityState::ClientOnly => PeerId::Client,
        ProximityState::Both | ProximityState::Neither => PeerId::Host, // tiebreak
    };
    if target_owner == me {
        // We should own it.
        match current_auth {
            AuthorityLevel::Local => HandoffDecision::NoOp,
            _ => HandoffDecision::ClaimHere { entity },
        }
    } else {
        // Remote peer should own it.
        match current_auth {
            AuthorityLevel::Remote => HandoffDecision::NoOp,
            _ => HandoffDecision::TransferOut {
                entity,
                to: target_owner,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: u32, host_d: f32, client_d: f32) -> ProximityObservation {
        ProximityObservation {
            id: EntityId(id),
            kind: EntityKind::Enemy,
            host_distance_m: host_d,
            client_distance_m: client_d,
        }
    }

    #[test]
    fn enemy_near_host_only_stays_host() {
        let driver = ProximityDriver::new(HandoffPolicy::DEFAULT);
        let table = AuthorityTable::new(PeerId::Host);
        table.set(EntityId(1), AuthorityLevel::Local);
        let ho = HandoffTracker::new();

        let out = driver.tick(PeerId::Host, [obs(1, 10.0, 80.0)], &table, &ho);
        // No transition, no decisions.
        assert!(out.is_empty() || !matches!(out[0], HandoffDecision::TransferOut { .. }));
    }

    #[test]
    fn transition_host_to_client_triggers_transfer() {
        let driver = ProximityDriver::new(HandoffPolicy::DEFAULT);
        let table = AuthorityTable::new(PeerId::Host);
        table.set(EntityId(1), AuthorityLevel::Local);
        let ho = HandoffTracker::new();

        // Tick 1: HostOnly.
        let _ = driver.tick(PeerId::Host, [obs(1, 10.0, 80.0)], &table, &ho);

        // Tick 2: ClientOnly — host should transfer out.
        let out = driver.tick(PeerId::Host, [obs(1, 80.0, 10.0)], &table, &ho);
        assert_eq!(
            out,
            vec![HandoffDecision::TransferOut {
                entity: EntityId(1),
                to: PeerId::Client,
            }]
        );
    }

    #[test]
    fn transition_claim_when_we_become_owner() {
        let driver = ProximityDriver::new(HandoffPolicy::DEFAULT);
        let table = AuthorityTable::new(PeerId::Client);
        table.set(EntityId(1), AuthorityLevel::Remote);
        let ho = HandoffTracker::new();

        // Tick 1: host-only → we're remote; that's correct.
        let _ = driver.tick(PeerId::Client, [obs(1, 10.0, 80.0)], &table, &ho);
        // Tick 2: client-only → we should claim.
        let out = driver.tick(PeerId::Client, [obs(1, 80.0, 10.0)], &table, &ho);
        assert_eq!(out, vec![HandoffDecision::ClaimHere { entity: EntityId(1) }]);
    }

    #[test]
    fn inflight_handoff_suppresses_duplicate() {
        let driver = ProximityDriver::new(HandoffPolicy::DEFAULT);
        let table = AuthorityTable::new(PeerId::Host);
        table.set(EntityId(1), AuthorityLevel::Local);
        let ho = HandoffTracker::new();

        let _ = driver.tick(PeerId::Host, [obs(1, 10.0, 80.0)], &table, &ho);
        let out = driver.tick(PeerId::Host, [obs(1, 80.0, 10.0)], &table, &ho);
        assert_eq!(out.len(), 1);

        // Simulate us having started the handoff already.
        ho.start(EntityId(1), PeerId::Client);
        let out = driver.tick(PeerId::Host, [obs(1, 80.0, 10.0)], &table, &ho);
        assert!(out.is_empty(), "should suppress dup while in flight");
    }

    #[test]
    fn hysteresis_prevents_thrash() {
        let driver = ProximityDriver::new(HandoffPolicy {
            radius_m: 50.0,
            hysteresis_m: 5.0,
        });
        let table = AuthorityTable::new(PeerId::Host);
        table.set(EntityId(1), AuthorityLevel::Local);
        let ho = HandoffTracker::new();

        // Start firmly host-only.
        let _ = driver.tick(PeerId::Host, [obs(1, 10.0, 80.0)], &table, &ho);
        // Slightly crossing the threshold — still in hysteresis window.
        let out = driver.tick(PeerId::Host, [obs(1, 52.0, 48.0)], &table, &ho);
        assert!(out.is_empty(), "hysteresis should hold previous state");
    }
}
