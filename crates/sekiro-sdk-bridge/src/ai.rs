//! AI & animation bridge — suppression on non-authority peers,
//! authoritative replay of incoming events.  SPEC §4.2.

use sekiro_sdk_core::entity::EntityId;
use serde::{Deserialize, Serialize};

/// Which native function the event came from.  Determines which
/// trampoline the non-owner calls when replaying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimRequestKind {
    Request,       // RequestAnimationPlayback (Event #1)
    Force,         // ForceAnimationPlayback (Event #18)
    AiState,       // SetCharacterAiState (Char #1)
    AiCommand,     // RequestCharacterAiCommand (Char #17)
    AiReplan,      // RequestCharacterAiRePlan (Char #20)
    Speffect,      // SetSpeffect (Char #8)
    Invincibility, // SetCharacterInvincibility (Char #15)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnimEvent {
    pub kind: AnimRequestKind,
    pub entity: EntityId,
    pub anim_id: u32,
    pub arg_a: u32,
    pub arg_b: u32,
    pub flag: bool,
}

impl AnimEvent {
    pub fn request(entity: EntityId, anim_id: u32, should_loop: bool, wait: bool) -> Self {
        Self {
            kind: AnimRequestKind::Request,
            entity,
            anim_id,
            arg_a: should_loop as u32,
            arg_b: wait as u32,
            flag: false,
        }
    }
    pub fn force(entity: EntityId, anim_id: u32, should_loop: bool, wait: bool, ignore_wait: bool) -> Self {
        Self {
            kind: AnimRequestKind::Force,
            entity,
            anim_id,
            arg_a: should_loop as u32,
            arg_b: wait as u32,
            flag: ignore_wait,
        }
    }
    pub fn ai_state(entity: EntityId, enabled: bool) -> Self {
        Self {
            kind: AnimRequestKind::AiState,
            entity,
            anim_id: 0,
            arg_a: 0,
            arg_b: 0,
            flag: enabled,
        }
    }
    pub fn ai_command(entity: EntityId, cmd: u32, slot: u8) -> Self {
        Self {
            kind: AnimRequestKind::AiCommand,
            entity,
            anim_id: 0,
            arg_a: cmd,
            arg_b: slot as u32,
            flag: false,
        }
    }
    pub fn ai_replan(entity: EntityId) -> Self {
        Self {
            kind: AnimRequestKind::AiReplan,
            entity,
            anim_id: 0,
            arg_a: 0,
            arg_b: 0,
            flag: false,
        }
    }
    pub fn speffect(entity: EntityId, id: i32) -> Self {
        Self {
            kind: AnimRequestKind::Speffect,
            entity,
            anim_id: 0,
            arg_a: id as u32,
            arg_b: 0,
            flag: false,
        }
    }
    pub fn invincibility(entity: EntityId, enabled: bool) -> Self {
        Self {
            kind: AnimRequestKind::Invincibility,
            entity,
            anim_id: 0,
            arg_a: 0,
            arg_b: 0,
            flag: enabled,
        }
    }
}

/// Layer-2 handle to all AI/anim hooks.
#[derive(Default)]
pub struct AiBridge {
    /// Trampolines are stored as raw usizes; the detours cast back to
    /// native fn pointers. Set to 0 when unhooked.
    pub tramp_request_anim: core::sync::atomic::AtomicUsize,
    pub tramp_force_anim: core::sync::atomic::AtomicUsize,
    pub tramp_ai_state: core::sync::atomic::AtomicUsize,
    pub tramp_ai_command: core::sync::atomic::AtomicUsize,
    pub tramp_ai_replan: core::sync::atomic::AtomicUsize,
    pub tramp_speffect: core::sync::atomic::AtomicUsize,
    pub tramp_invincibility: core::sync::atomic::AtomicUsize,
}

impl AiBridge {
    pub fn new() -> Self {
        Self::default()
    }
}
