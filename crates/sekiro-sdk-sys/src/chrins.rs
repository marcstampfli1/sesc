//! `ChrIns` — Sekiro's character-instance struct.
//!
//! **P0 gap #1** (SPEC §3.4, §11). Field offsets listed below are the
//! *minimum set* the mod requires.  They are populated at runtime from
//! the Cielos CE table or via live-dump validation (reading HP back out
//! of the player `ChrIns` and comparing to the known value).
//!
//! Any field marked `UNRESOLVED` (= `usize::MAX`) means we have not
//! validated it and reads/writes through it MUST be rejected by
//! [`ChrInsLayout::validate`].

use crate::memory::RawPtr;

/// Runtime-populated layout for the `ChrIns` struct on this patch version.
///
/// All offsets are byte offsets from the start of the `ChrIns` instance.
#[derive(Debug, Clone, Copy)]
pub struct ChrInsLayout {
    pub entity_id: usize,
    pub char_id: usize,
    pub hp: usize,
    pub max_hp: usize,
    pub posture: usize,
    pub max_posture: usize,
    pub animation_id: usize,
    pub animation_frame: usize,
    pub position: usize,
    pub rotation: usize,
    pub velocity: usize,
    pub team_type: usize,
    pub target_lock: usize,
    pub ai_command: usize,
    pub ai_slot: usize,
    pub is_deflecting: usize,
    pub network_authority: usize,
}

pub const UNRESOLVED: usize = usize::MAX;

impl ChrInsLayout {
    /// All-unresolved default. Every field must be filled in before use.
    pub const fn unresolved() -> Self {
        Self {
            entity_id: UNRESOLVED,
            char_id: UNRESOLVED,
            hp: UNRESOLVED,
            max_hp: UNRESOLVED,
            posture: UNRESOLVED,
            max_posture: UNRESOLVED,
            animation_id: UNRESOLVED,
            animation_frame: UNRESOLVED,
            position: UNRESOLVED,
            rotation: UNRESOLVED,
            velocity: UNRESOLVED,
            team_type: UNRESOLVED,
            target_lock: UNRESOLVED,
            ai_command: UNRESOLVED,
            ai_slot: UNRESOLVED,
            is_deflecting: UNRESOLVED,
            network_authority: UNRESOLVED,
        }
    }

    pub fn validate(&self) -> Result<(), Vec<&'static str>> {
        let mut missing = Vec::new();
        macro_rules! chk {
            ($field:ident) => {
                if self.$field == UNRESOLVED {
                    missing.push(stringify!($field));
                }
            };
        }
        chk!(entity_id);
        chk!(char_id);
        chk!(hp);
        chk!(max_hp);
        chk!(posture);
        chk!(max_posture);
        chk!(animation_id);
        chk!(animation_frame);
        chk!(position);
        chk!(rotation);
        chk!(velocity);
        chk!(team_type);
        chk!(target_lock);
        chk!(ai_command);
        chk!(ai_slot);
        chk!(is_deflecting);
        chk!(network_authority);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

/// Snapshot of a `ChrIns` read through a validated layout.  Pure-data;
/// safe to hand to higher layers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChrInsSnapshot {
    pub entity_id: u32,
    pub char_id: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub posture: f32,
    pub max_posture: f32,
    pub animation_id: u32,
    pub animation_frame: f32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub team_type: u8,
    pub target_lock: u32,
    pub ai_command: u32,
    pub ai_slot: u8,
    pub is_deflecting: bool,
    pub network_authority: u8,
}

/// # Safety
/// `ptr` must point to a valid `ChrIns` instance that matches `layout`
/// for the current patch version. Caller owns lifetime.
pub unsafe fn read_snapshot(ptr: RawPtr, layout: &ChrInsLayout) -> ChrInsSnapshot {
    #[inline]
    unsafe fn read_at<T: Copy + Default>(ptr: RawPtr, off: usize) -> T {
        if off == UNRESOLVED {
            return T::default();
        }
        ptr.offset(off as isize).read()
    }
    ChrInsSnapshot {
        entity_id: read_at(ptr, layout.entity_id),
        char_id: read_at(ptr, layout.char_id),
        hp: read_at(ptr, layout.hp),
        max_hp: read_at(ptr, layout.max_hp),
        posture: read_at(ptr, layout.posture),
        max_posture: read_at(ptr, layout.max_posture),
        animation_id: read_at(ptr, layout.animation_id),
        animation_frame: read_at(ptr, layout.animation_frame),
        position: read_at(ptr, layout.position),
        rotation: read_at(ptr, layout.rotation),
        velocity: read_at(ptr, layout.velocity),
        team_type: read_at(ptr, layout.team_type),
        target_lock: read_at(ptr, layout.target_lock),
        ai_command: read_at(ptr, layout.ai_command),
        ai_slot: read_at(ptr, layout.ai_slot),
        is_deflecting: read_at::<u8>(ptr, layout.is_deflecting) != 0,
        network_authority: read_at(ptr, layout.network_authority),
    }
}
