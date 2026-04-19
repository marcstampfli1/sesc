//! Snapshot delta compression.
//!
//! Per-tick state packets carry only the fields that changed since the
//! last-acked baseline.  A 16-bit dirty-field bitmask precedes each
//! entity's payload.  Full snapshots are sent when a peer requests
//! one (reconnect / desync recovery).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::snapshot::{EntitySnapshot, RollbackSnapshot};

/// Per-field bit assignments.
pub mod bit {
    pub const HP: u16 = 1 << 0;
    pub const MAX_HP: u16 = 1 << 1;
    pub const POSTURE: u16 = 1 << 2;
    pub const MAX_POSTURE: u16 = 1 << 3;
    pub const POSITION: u16 = 1 << 4;
    pub const ROTATION: u16 = 1 << 5;
    pub const VELOCITY: u16 = 1 << 6;
    pub const ANIMATION_ID: u16 = 1 << 7;
    pub const ANIMATION_FRAME: u16 = 1 << 8;
    pub const TEAM_TYPE: u16 = 1 << 9;
    pub const TARGET_LOCK: u16 = 1 << 10;
    pub const AI_COMMAND: u16 = 1 << 11;
    pub const AI_SLOT: u16 = 1 << 12;
    pub const IS_DEFLECTING: u16 = 1 << 13;
    pub const SPEFFECTS: u16 = 1 << 14;
    pub const PART_HP: u16 = 1 << 15;
}

/// Per-entity delta: the bitmask tells the reader which `Option`s are
/// populated.  Fields with `Some(v)` differ from the baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityDelta {
    pub entity_id: u32,
    pub char_id: u32,
    pub dirty: u16,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub posture: Option<f32>,
    pub max_posture: Option<f32>,
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub velocity: Option<[f32; 3]>,
    pub animation_id: Option<u32>,
    pub animation_frame: Option<f32>,
    pub team_type: Option<u8>,
    pub target_lock: Option<u32>,
    pub ai_command: Option<u32>,
    pub ai_slot: Option<u8>,
    pub is_deflecting: Option<bool>,
    pub active_speffects: Option<Vec<u32>>,
    pub npc_part_hp: Option<Vec<(u16, i32)>>,
}

/// A compressed snapshot relative to a baseline frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDelta {
    pub baseline_frame: u64,
    pub frame: u64,
    pub match_seed: u64,
    pub frame_counter: u64,
    pub entities: Vec<EntityDelta>,
    pub removed_entities: Vec<u32>,
    pub connected_flags: Option<Vec<u8>>,
}

impl SnapshotDelta {
    /// Compute `new - baseline`.
    pub fn compute(baseline: &RollbackSnapshot, new: &RollbackSnapshot) -> Self {
        let base_by_id: HashMap<u32, &EntitySnapshot> =
            baseline.entities.iter().map(|e| (e.entity_id, e)).collect();

        let mut deltas = Vec::new();
        let mut seen_ids = Vec::new();
        for e in &new.entities {
            seen_ids.push(e.entity_id);
            deltas.push(diff_entity(base_by_id.get(&e.entity_id).copied(), e));
        }
        let seen: std::collections::HashSet<u32> = seen_ids.into_iter().collect();
        let removed: Vec<u32> = baseline
            .entities
            .iter()
            .filter(|e| !seen.contains(&e.entity_id))
            .map(|e| e.entity_id)
            .collect();

        let flags = if baseline.connected_flags != new.connected_flags {
            Some(new.connected_flags.clone())
        } else {
            None
        };

        Self {
            baseline_frame: baseline.frame,
            frame: new.frame,
            match_seed: new.match_seed,
            frame_counter: new.frame_counter,
            entities: deltas,
            removed_entities: removed,
            connected_flags: flags,
        }
    }

    /// Apply to a baseline to reconstruct the full snapshot.
    pub fn apply(&self, baseline: &RollbackSnapshot) -> RollbackSnapshot {
        let mut out = baseline.clone();
        out.frame = self.frame;
        out.match_seed = self.match_seed;
        out.frame_counter = self.frame_counter;
        if let Some(flags) = &self.connected_flags {
            out.connected_flags = flags.clone();
        }

        // Remove despawns.
        out.entities
            .retain(|e| !self.removed_entities.contains(&e.entity_id));

        // Apply each entity delta.
        let mut index: HashMap<u32, usize> = out
            .entities
            .iter()
            .enumerate()
            .map(|(i, e)| (e.entity_id, i))
            .collect();
        for d in &self.entities {
            match index.get(&d.entity_id) {
                Some(&idx) => apply_entity(&mut out.entities[idx], d),
                None => {
                    // New entity — synth a blank base then apply.
                    let mut fresh = blank_entity(d.entity_id, d.char_id);
                    apply_entity(&mut fresh, d);
                    index.insert(d.entity_id, out.entities.len());
                    out.entities.push(fresh);
                }
            }
        }
        out
    }
}

fn diff_entity(base: Option<&EntitySnapshot>, new: &EntitySnapshot) -> EntityDelta {
    let mut d = EntityDelta {
        entity_id: new.entity_id,
        char_id: new.char_id,
        dirty: 0,
        hp: None,
        max_hp: None,
        posture: None,
        max_posture: None,
        position: None,
        rotation: None,
        velocity: None,
        animation_id: None,
        animation_frame: None,
        team_type: None,
        target_lock: None,
        ai_command: None,
        ai_slot: None,
        is_deflecting: None,
        active_speffects: None,
        npc_part_hp: None,
    };
    macro_rules! diff {
        ($bit:expr, $field:ident, $dest:ident) => {
            match base {
                Some(b) if b.$field == new.$field => {}
                _ => {
                    d.dirty |= $bit;
                    d.$dest = Some(new.$field.clone());
                }
            }
        };
    }
    diff!(bit::HP, hp, hp);
    diff!(bit::MAX_HP, max_hp, max_hp);
    diff!(bit::POSTURE, posture, posture);
    diff!(bit::MAX_POSTURE, max_posture, max_posture);
    diff!(bit::POSITION, position, position);
    diff!(bit::ROTATION, rotation, rotation);
    diff!(bit::VELOCITY, velocity, velocity);
    diff!(bit::ANIMATION_ID, animation_id, animation_id);
    diff!(bit::ANIMATION_FRAME, animation_frame, animation_frame);
    diff!(bit::TEAM_TYPE, team_type, team_type);
    diff!(bit::TARGET_LOCK, target_lock, target_lock);
    diff!(bit::AI_COMMAND, ai_command, ai_command);
    diff!(bit::AI_SLOT, ai_slot, ai_slot);
    diff!(bit::IS_DEFLECTING, is_deflecting, is_deflecting);
    diff!(bit::SPEFFECTS, active_speffects, active_speffects);
    diff!(bit::PART_HP, npc_part_hp, npc_part_hp);
    d
}

fn apply_entity(dest: &mut EntitySnapshot, d: &EntityDelta) {
    dest.char_id = d.char_id;
    if let Some(v) = d.hp {
        dest.hp = v;
    }
    if let Some(v) = d.max_hp {
        dest.max_hp = v;
    }
    if let Some(v) = d.posture {
        dest.posture = v;
    }
    if let Some(v) = d.max_posture {
        dest.max_posture = v;
    }
    if let Some(v) = d.position {
        dest.position = v;
    }
    if let Some(v) = d.rotation {
        dest.rotation = v;
    }
    if let Some(v) = d.velocity {
        dest.velocity = v;
    }
    if let Some(v) = d.animation_id {
        dest.animation_id = v;
    }
    if let Some(v) = d.animation_frame {
        dest.animation_frame = v;
    }
    if let Some(v) = d.team_type {
        dest.team_type = v;
    }
    if let Some(v) = d.target_lock {
        dest.target_lock = v;
    }
    if let Some(v) = d.ai_command {
        dest.ai_command = v;
    }
    if let Some(v) = d.ai_slot {
        dest.ai_slot = v;
    }
    if let Some(v) = d.is_deflecting {
        dest.is_deflecting = v;
    }
    if let Some(v) = &d.active_speffects {
        dest.active_speffects = v.clone();
    }
    if let Some(v) = &d.npc_part_hp {
        dest.npc_part_hp = v.clone();
    }
}

fn blank_entity(id: u32, char_id: u32) -> EntitySnapshot {
    EntitySnapshot {
        entity_id: id,
        char_id,
        hp: 0,
        max_hp: 0,
        posture: 0.0,
        max_posture: 0.0,
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        velocity: [0.0; 3],
        animation_id: 0,
        animation_frame: 0.0,
        team_type: 0,
        target_lock: 0,
        ai_command: 0,
        ai_slot: 0,
        is_deflecting: false,
        active_speffects: Vec::new(),
        npc_part_hp: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u32, hp: i32, pos: [f32; 3]) -> EntitySnapshot {
        EntitySnapshot {
            entity_id: id,
            char_id: 1010,
            hp,
            max_hp: 1000,
            posture: 0.0,
            max_posture: 500.0,
            position: pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            animation_id: 1,
            animation_frame: 0.5,
            team_type: 2,
            target_lock: 0,
            ai_command: 0,
            ai_slot: 0,
            is_deflecting: false,
            active_speffects: Vec::new(),
            npc_part_hp: Vec::new(),
        }
    }

    #[test]
    fn no_change_delta_is_empty() {
        let base = RollbackSnapshot {
            frame: 10,
            entities: vec![entity(1, 100, [0.0, 0.0, 0.0])],
            connected_flags: vec![0xAA],
            match_seed: 42,
            frame_counter: 10,
        };
        let d = SnapshotDelta::compute(&base, &base);
        let only = &d.entities[0];
        assert_eq!(only.dirty, 0);
        assert!(d.connected_flags.is_none());
    }

    #[test]
    fn partial_change_delta() {
        let base = entity(1, 100, [0.0, 0.0, 0.0]);
        let mut new = base.clone();
        new.hp = 85;
        new.position = [1.0, 0.0, 0.0];

        let base_snap = RollbackSnapshot {
            frame: 10,
            entities: vec![base.clone()],
            connected_flags: vec![],
            match_seed: 42,
            frame_counter: 10,
        };
        let new_snap = RollbackSnapshot {
            frame: 11,
            entities: vec![new.clone()],
            connected_flags: vec![],
            match_seed: 42,
            frame_counter: 11,
        };

        let d = SnapshotDelta::compute(&base_snap, &new_snap);
        let e = &d.entities[0];
        assert_eq!(e.dirty, bit::HP | bit::POSITION);
        assert_eq!(e.hp, Some(85));
        assert_eq!(e.position, Some([1.0, 0.0, 0.0]));
        assert_eq!(e.posture, None);
        assert_eq!(e.animation_id, None);
    }

    #[test]
    fn apply_reconstructs_identically() {
        let base_snap = RollbackSnapshot {
            frame: 10,
            entities: vec![entity(1, 100, [0.0; 3]), entity(2, 50, [5.0, 0.0, 0.0])],
            connected_flags: vec![0x01, 0x02],
            match_seed: 7,
            frame_counter: 10,
        };
        let mut new_snap = base_snap.clone();
        new_snap.frame = 11;
        new_snap.frame_counter = 11;
        new_snap.entities[0].hp = 75;
        new_snap.entities[1].position = [6.0, 0.0, 0.0];
        new_snap.connected_flags = vec![0x01, 0x03];

        let d = SnapshotDelta::compute(&base_snap, &new_snap);
        let reconstructed = d.apply(&base_snap);
        assert_eq!(reconstructed, new_snap);
    }

    #[test]
    fn spawn_and_despawn() {
        let base_snap = RollbackSnapshot {
            frame: 10,
            entities: vec![entity(1, 100, [0.0; 3])],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 10,
        };
        let new_snap = RollbackSnapshot {
            frame: 11,
            entities: vec![entity(2, 200, [3.0, 0.0, 0.0])],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 11,
        };

        let d = SnapshotDelta::compute(&base_snap, &new_snap);
        assert_eq!(d.removed_entities, vec![1]);
        assert_eq!(d.entities.len(), 1);
        assert_eq!(d.entities[0].entity_id, 2);

        let reconstructed = d.apply(&base_snap);
        assert_eq!(reconstructed.entities.len(), 1);
        assert_eq!(reconstructed.entities[0].entity_id, 2);
        assert_eq!(reconstructed.entities[0].hp, 200);
    }
}
