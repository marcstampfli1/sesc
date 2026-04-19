//! `SharedStepper` implementation that writes back through a
//! [`ChrInsLayout`].  Used during rollback re-simulation (SPEC §5.5)
//! to restore shared-entity state from a snapshot.
//!
//! Gating: the stepper no-ops when the layout still has unresolved
//! fields (`P0 gap #1`), so rollback simply cannot corrupt game state
//! until the Cielos CE table has been loaded.

use sekiro_sdk_core::entity::EntityId;
use sekiro_sdk_sys::chrins::{ChrInsLayout, UNRESOLVED};
use sekiro_sdk_sys::memory::RawPtr;

use crate::ring::Input;
use crate::resim::SharedStepper;
use crate::snapshot::{EntitySnapshot, RollbackSnapshot};

/// Writes snapshot data back to each entity's `ChrIns` instance.
///
/// Construct with a closure that resolves `EntityId → RawPtr` (typically
/// by walking `WorldChrMan`).  If the resolver returns `None` for any
/// entity, that entity's snapshot row is skipped silently.
pub struct ChrInsStepper<F> {
    pub layout: ChrInsLayout,
    pub resolve: F,
    /// Counts of writes per invocation; useful for integration tests.
    pub last_written_entities: usize,
}

impl<F> ChrInsStepper<F>
where
    F: FnMut(EntityId) -> Option<RawPtr>,
{
    pub fn new(layout: ChrInsLayout, resolve: F) -> Self {
        Self {
            layout,
            resolve,
            last_written_entities: 0,
        }
    }

    fn layout_ready(&self) -> bool {
        self.layout.validate().is_ok()
    }
}

impl<F> SharedStepper for ChrInsStepper<F>
where
    F: FnMut(EntityId) -> Option<RawPtr>,
{
    fn restore(&mut self, snap: &RollbackSnapshot) {
        self.last_written_entities = 0;
        if !self.layout_ready() {
            return;
        }
        for e in &snap.entities {
            let ptr = match (self.resolve)(EntityId(e.entity_id)) {
                Some(p) if !p.is_null() => p,
                _ => continue,
            };
            // SAFETY: the resolver promises `ptr` is a live, validly-
            // aligned `ChrIns` instance matching `self.layout`.  The
            // field offsets are all `UNRESOLVED` or valid — checked by
            // layout_ready above.
            unsafe {
                write_entity(ptr, &self.layout, e);
            }
            self.last_written_entities += 1;
        }
    }

    fn step(&mut self, _frame: u64, _local: Input, _remote: Input) {
        // Stepping shared entities is driven by replaying hook
        // invocations at Layer 2 — the stepper itself does no work.
    }
}

/// # Safety
/// `ptr` must be a valid `ChrIns` instance whose field layout matches
/// `layout`; all fields must be writable (which they are — Sekiro keeps
/// its game memory read-write).  Any field offset that is `UNRESOLVED`
/// is skipped.
pub unsafe fn write_entity(ptr: RawPtr, layout: &ChrInsLayout, s: &EntitySnapshot) {
    write_at::<i32>(ptr, layout.hp, s.hp);
    write_at::<i32>(ptr, layout.max_hp, s.max_hp);
    write_at::<f32>(ptr, layout.posture, s.posture);
    write_at::<f32>(ptr, layout.max_posture, s.max_posture);
    write_at::<[f32; 3]>(ptr, layout.position, s.position);
    write_at::<[f32; 4]>(ptr, layout.rotation, s.rotation);
    write_at::<[f32; 3]>(ptr, layout.velocity, s.velocity);
    write_at::<u32>(ptr, layout.animation_id, s.animation_id);
    write_at::<f32>(ptr, layout.animation_frame, s.animation_frame);
    write_at::<u8>(ptr, layout.team_type, s.team_type);
    write_at::<u32>(ptr, layout.target_lock, s.target_lock);
    write_at::<u32>(ptr, layout.ai_command, s.ai_command);
    write_at::<u8>(ptr, layout.ai_slot, s.ai_slot);
    write_at::<u8>(ptr, layout.is_deflecting, s.is_deflecting as u8);
}

#[inline]
unsafe fn write_at<T: Copy>(ptr: RawPtr, off: usize, v: T) {
    if off == UNRESOLVED {
        return;
    }
    ptr.offset(off as isize).write(v);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fake_layout(total_size: usize) -> ChrInsLayout {
        // Lay out fields in a deterministic packed order, big enough
        // that none exceed `total_size`.  Caller sizes the buffer to
        // match.
        let _ = total_size;
        ChrInsLayout {
            entity_id: 0,     // u32
            char_id: 4,       // u32
            hp: 8,            // i32
            max_hp: 12,       // i32
            posture: 16,      // f32
            max_posture: 20,  // f32
            animation_id: 24, // u32
            animation_frame: 28,
            position: 32, // [f32;3]
            rotation: 44, // [f32;4]
            velocity: 60, // [f32;3]
            team_type: 72, // u8
            target_lock: 76,
            ai_command: 80,
            ai_slot: 84,
            is_deflecting: 88,
            network_authority: 92,
        }
    }

    fn snap_for(id: u32) -> EntitySnapshot {
        EntitySnapshot {
            entity_id: id,
            char_id: 5080, // Gyoubu
            hp: 1234,
            max_hp: 2000,
            posture: 150.0,
            max_posture: 600.0,
            position: [1.5, 2.5, 3.5],
            rotation: [0.0, 0.7071, 0.0, 0.7071],
            velocity: [0.0; 3],
            animation_id: 7010,
            animation_frame: 0.25,
            team_type: 3,
            target_lock: 10000,
            ai_command: 42,
            ai_slot: 1,
            is_deflecting: true,
            active_speffects: Vec::new(),
            npc_part_hp: Vec::new(),
        }
    }

    #[test]
    fn restore_no_ops_when_layout_unresolved() {
        let mut buf = [0u8; 96];
        let buf_ptr = RawPtr(buf.as_mut_ptr() as usize);
        let snap = RollbackSnapshot {
            frame: 1,
            entities: vec![snap_for(10000)],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 1,
        };
        let mut resolve = |_id: EntityId| Some(buf_ptr);
        let mut stepper = ChrInsStepper::new(ChrInsLayout::unresolved(), &mut resolve);
        stepper.restore(&snap);
        assert_eq!(stepper.last_written_entities, 0);
        // Buffer untouched.
        assert_eq!(buf, [0u8; 96]);
    }

    #[test]
    fn restore_writes_every_field() {
        let mut buf = vec![0u8; 96];
        let buf_ptr = RawPtr(buf.as_mut_ptr() as usize);

        let snap = RollbackSnapshot {
            frame: 1,
            entities: vec![snap_for(10000)],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 1,
        };

        let map: HashMap<EntityId, RawPtr> =
            [(EntityId(10000), buf_ptr)].into_iter().collect();
        let mut resolve = |id: EntityId| map.get(&id).copied();
        let layout = fake_layout(buf.len());
        let mut stepper = ChrInsStepper::new(layout, &mut resolve);
        stepper.restore(&snap);
        assert_eq!(stepper.last_written_entities, 1);

        // Confirm a few well-known fields were written.
        let hp = i32::from_le_bytes(buf[8..12].try_into().unwrap());
        assert_eq!(hp, 1234);
        let posture = f32::from_le_bytes(buf[16..20].try_into().unwrap());
        assert!((posture - 150.0).abs() < 0.001);
        let team = buf[72];
        assert_eq!(team, 3);
        let is_deflecting = buf[88];
        assert_eq!(is_deflecting, 1);
    }

    #[test]
    fn restore_skips_unknown_entity() {
        let mut resolve = |_id: EntityId| None;
        let layout = fake_layout(96);
        let mut stepper = ChrInsStepper::new(layout, &mut resolve);
        let snap = RollbackSnapshot {
            frame: 1,
            entities: vec![snap_for(10000)],
            connected_flags: vec![],
            match_seed: 0,
            frame_counter: 1,
        };
        stepper.restore(&snap);
        assert_eq!(stepper.last_written_entities, 0);
    }
}
