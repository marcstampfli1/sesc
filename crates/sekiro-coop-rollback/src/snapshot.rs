//! Snapshot format and snapshot ring.  SPEC §5.2.

use sekiro_sdk_core::entity::EntityId;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::ring::ROLLBACK_MAX_FRAMES;

/// Per-entity state we serialise into a rollback snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u32,
    pub char_id: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub posture: f32,
    pub max_posture: f32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub animation_id: u32,
    pub animation_frame: f32,
    pub team_type: u8,
    pub target_lock: u32,
    pub ai_command: u32,
    pub ai_slot: u8,
    pub is_deflecting: bool,
    pub active_speffects: Vec<u32>,
    pub npc_part_hp: Vec<(u16, i32)>,
}

/// A complete rollback snapshot — shared entities + connected-flag
/// bitmap + RNG state.  SPEC §5.2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackSnapshot {
    pub frame: u64,
    pub entities: Vec<EntitySnapshot>,
    pub connected_flags: Vec<u8>, // bitmap
    pub match_seed: u64,
    pub frame_counter: u64,
}

impl RollbackSnapshot {
    pub fn empty(frame: u64, match_seed: u64) -> Self {
        Self {
            frame,
            entities: Vec::new(),
            connected_flags: Vec::new(),
            match_seed,
            frame_counter: frame,
        }
    }

    pub fn byte_estimate(&self) -> usize {
        // Cheap approximation.
        core::mem::size_of::<RollbackSnapshot>()
            + self.entities.len() * core::mem::size_of::<EntitySnapshot>()
            + self.connected_flags.len()
            + self
                .entities
                .iter()
                .map(|e| e.active_speffects.len() * 4 + e.npc_part_hp.len() * 6)
                .sum::<usize>()
    }

    pub fn find(&self, id: EntityId) -> Option<&EntitySnapshot> {
        self.entities.iter().find(|e| e.entity_id == id.0)
    }
}

/// Ring of the last N snapshots, keyed by absolute frame.
#[derive(Debug, Default)]
pub struct SnapshotRing {
    capacity: usize,
    buf: VecDeque<RollbackSnapshot>,
}

impl SnapshotRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            buf: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    /// Default-sized ring (matches [`ROLLBACK_MAX_FRAMES`]).
    pub fn with_default_capacity() -> Self {
        Self::new(ROLLBACK_MAX_FRAMES as usize)
    }

    pub fn push(&mut self, snap: RollbackSnapshot) {
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(snap);
    }

    /// Fetch the snapshot for an exact frame, if still in the ring.
    pub fn at(&self, frame: u64) -> Option<&RollbackSnapshot> {
        self.buf.iter().find(|s| s.frame == frame)
    }

    pub fn newest(&self) -> Option<&RollbackSnapshot> {
        self.buf.back()
    }

    pub fn oldest(&self) -> Option<&RollbackSnapshot> {
        self.buf.front()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Byte-estimate for diagnostics.
    pub fn total_bytes(&self) -> usize {
        self.buf.iter().map(|s| s.byte_estimate()).sum()
    }
}

/// Hash the frozen shared-set state for desync detection.  SPEC §9.3.
/// Uses a simple FNV-1a over the serialised bytes.
pub fn hash_snapshot(s: &RollbackSnapshot) -> u64 {
    let bytes = bincode::serialize(s).expect("serialize snapshot");
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}
