//! Shared-entity set identification.
//!
//! Decision tree implementation from SPEC Appendix B.

use sekiro_sdk_core::entity::{EntityId, EntityKind};
use std::collections::HashSet;

pub const ROLLBACK_PROXIMITY_RADIUS_M: f32 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedDecision {
    Shared,
    NotShared,
}

/// Lightweight snapshot-like type the band needs to classify an entity.
/// Produced from `ChrInsSnapshot` + engagement state.
#[derive(Debug, Clone, Copy)]
pub struct BandInput {
    pub id: EntityId,
    pub char_id: u32,
    pub position: [f32; 3],
    pub kind: EntityKind,
    pub hp_bar_visible: bool,
    pub any_player_lockon: bool,
    pub owner_is_shared: Option<bool>, // for projectiles
    pub scripted_interaction_active: bool,
}

/// Decide whether an entity is in `S(t)` given both players' positions.
pub fn classify(
    e: BandInput,
    host_pos: [f32; 3],
    client_pos: [f32; 3],
    radius: f32,
) -> SharedDecision {
    match e.kind {
        EntityKind::Player => SharedDecision::Shared,
        EntityKind::Boss => {
            if e.hp_bar_visible || e.any_player_lockon {
                SharedDecision::Shared
            } else {
                SharedDecision::NotShared
            }
        }
        EntityKind::Npc => {
            if e.scripted_interaction_active {
                SharedDecision::Shared
            } else {
                SharedDecision::NotShared
            }
        }
        EntityKind::Enemy => {
            let dh = distance(e.position, host_pos);
            let dc = distance(e.position, client_pos);
            if dh.min(dc) < radius {
                SharedDecision::Shared
            } else {
                SharedDecision::NotShared
            }
        }
        EntityKind::Invisible | EntityKind::Unknown => SharedDecision::NotShared,
        EntityKind::Object => {
            // Projectiles & bullets inherit from owner; regular objects are
            // host-owned and not part of the rollback band.
            match e.owner_is_shared {
                Some(true) => SharedDecision::Shared,
                _ => SharedDecision::NotShared,
            }
        }
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// The live shared set.  Updated each tick; reports additions/removals
/// so higher layers can emit `EntitySpawned`/`EntityDespawned`.
#[derive(Debug, Default, Clone)]
pub struct SharedBand {
    pub members: HashSet<EntityId>,
    pub radius: f32,
}

impl SharedBand {
    pub fn new() -> Self {
        Self {
            members: HashSet::new(),
            radius: ROLLBACK_PROXIMITY_RADIUS_M,
        }
    }

    /// Recompute membership for this tick; return (added, removed).
    pub fn recompute<I>(
        &mut self,
        inputs: I,
        host_pos: [f32; 3],
        client_pos: [f32; 3],
    ) -> (Vec<EntityId>, Vec<EntityId>)
    where
        I: IntoIterator<Item = BandInput>,
    {
        let mut next: HashSet<EntityId> = HashSet::new();
        for e in inputs {
            if classify(e, host_pos, client_pos, self.radius) == SharedDecision::Shared {
                next.insert(e.id);
            }
        }
        let added: Vec<EntityId> = next.difference(&self.members).copied().collect();
        let removed: Vec<EntityId> = self.members.difference(&next).copied().collect();
        self.members = next;
        (added, removed)
    }

    pub fn contains(&self, e: EntityId) -> bool {
        self.members.contains(&e)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}
