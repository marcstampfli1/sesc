//! TAE (Time-based Animation Event) block IDs.
//!
//! Source: `Animations.md` wiki reference.  TAEs are embedded in each
//! character's `.anibnd` → `.tae` files; during an animation's playback
//! the engine dispatches timed events (spawn hitbox, add SpEffect,
//! blend into next animation, etc.) at specified frames.
//!
//! SPEC §4.2 "TAE event dispatch": when an authority peer plays an
//! animation, the peer's engine fires the animation's TAE events; the
//! non-authority peer also plays the animation (via ForceAnim replay)
//! and its engine fires the same TAE events locally — closing the loop
//! deterministically.

use serde::{Deserialize, Serialize};

/// A known TAE block type.  Use `raw()` to get the on-disk index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaeBlock {
    /// Spawns a hitbox (as defined by the attached AtkParam).  SPEC §4.1.
    InvokeAttackBehavior,
    /// Spawns a bullet/projectile/effect via BulletParam.
    InvokeBulletBehavior,
    /// Frame blending at the animation's start.
    Blend,
    /// Applies a SpEffect for the block's duration.
    AddSpEffect,
    /// Spawns a one-shot visual effect (FFX).
    SpawnOneShotFfx,
    /// Sets the character's turn rate for the block's duration.
    SetTurnSpeed,
    /// Sets a movement speed multiplier.
    SetMovementMultiplier,
    /// Unknown / unmapped block — preserves the raw id.
    Other(u16),
}

impl TaeBlock {
    pub fn raw(self) -> u16 {
        match self {
            TaeBlock::InvokeAttackBehavior => 1,
            TaeBlock::InvokeBulletBehavior => 2,
            TaeBlock::Blend => 16,
            TaeBlock::AddSpEffect => 67,
            TaeBlock::SpawnOneShotFfx => 96,
            TaeBlock::SetTurnSpeed => 224,
            TaeBlock::SetMovementMultiplier => 760,
            TaeBlock::Other(v) => v,
        }
    }

    pub fn from_raw(id: u16) -> Self {
        match id {
            1 => TaeBlock::InvokeAttackBehavior,
            2 => TaeBlock::InvokeBulletBehavior,
            16 => TaeBlock::Blend,
            67 => TaeBlock::AddSpEffect,
            96 => TaeBlock::SpawnOneShotFfx,
            224 => TaeBlock::SetTurnSpeed,
            760 => TaeBlock::SetMovementMultiplier,
            other => TaeBlock::Other(other),
        }
    }

    /// True iff this block is a *hitbox-spawning* block — the ones the
    /// combat bridge (SPEC §4.1) treats as authority-sensitive.
    pub fn is_hitbox_spawner(self) -> bool {
        matches!(
            self,
            TaeBlock::InvokeAttackBehavior | TaeBlock::InvokeBulletBehavior
        )
    }

    /// True iff this block applies a SpEffect (AddSpEffect[67]).
    pub fn is_speffect_applier(self) -> bool {
        matches!(self, TaeBlock::AddSpEffect)
    }
}

/// A TAE event occurrence in an animation timeline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TaeEvent {
    pub block: TaeBlock,
    /// Start time of the block in seconds.
    pub start_s: f32,
    /// End time of the block in seconds.
    pub end_s: f32,
    /// First block-specific argument (AtkParam ID, SpEffect ID, etc.).
    pub arg0: u32,
    pub arg1: u32,
}

impl TaeEvent {
    /// Duration in seconds.
    pub fn duration_s(&self) -> f32 {
        self.end_s - self.start_s
    }

    /// Convenience: build an InvokeAttackBehavior block pointing at an
    /// AtkParam row.
    pub fn hitbox(atk_param_id: u32, start_s: f32, end_s: f32) -> Self {
        Self {
            block: TaeBlock::InvokeAttackBehavior,
            start_s,
            end_s,
            arg0: atk_param_id,
            arg1: 0,
        }
    }

    pub fn speffect(speffect_id: u32, start_s: f32, end_s: f32) -> Self {
        Self {
            block: TaeBlock::AddSpEffect,
            start_s,
            end_s,
            arg0: speffect_id,
            arg1: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known_blocks() {
        for b in [
            TaeBlock::InvokeAttackBehavior,
            TaeBlock::InvokeBulletBehavior,
            TaeBlock::Blend,
            TaeBlock::AddSpEffect,
            TaeBlock::SpawnOneShotFfx,
            TaeBlock::SetTurnSpeed,
            TaeBlock::SetMovementMultiplier,
        ] {
            let raw = b.raw();
            assert_eq!(TaeBlock::from_raw(raw), b, "round-trip {:?}", b);
        }
    }

    #[test]
    fn unknown_id_becomes_other() {
        let b = TaeBlock::from_raw(999);
        assert_eq!(b, TaeBlock::Other(999));
        assert_eq!(b.raw(), 999);
    }

    #[test]
    fn hitbox_spawner_classification() {
        assert!(TaeBlock::InvokeAttackBehavior.is_hitbox_spawner());
        assert!(TaeBlock::InvokeBulletBehavior.is_hitbox_spawner());
        assert!(!TaeBlock::AddSpEffect.is_hitbox_spawner());
        assert!(!TaeBlock::Blend.is_hitbox_spawner());
    }

    #[test]
    fn speffect_classification() {
        assert!(TaeBlock::AddSpEffect.is_speffect_applier());
        assert!(!TaeBlock::InvokeAttackBehavior.is_speffect_applier());
    }

    #[test]
    fn event_constructors() {
        let h = TaeEvent::hitbox(1234, 0.1, 0.3);
        assert_eq!(h.block, TaeBlock::InvokeAttackBehavior);
        assert_eq!(h.arg0, 1234);
        assert!((h.duration_s() - 0.2).abs() < 1e-6);

        let s = TaeEvent::speffect(9001, 0.0, 1.0);
        assert_eq!(s.block, TaeBlock::AddSpEffect);
        assert!((s.duration_s() - 1.0).abs() < 1e-6);
    }
}
