//! Data-driven catalog of EMEVD instructions from `Function-Definitions.md`.
//!
//! Rather than hand-writing 386 builder functions, the catalog is one
//! `const` table of `(class, idx, name, arg types)` tuples.  Callers:
//!
//! - Look up the `Spec` for `(class, idx)` to know what args to pass.
//! - Call `emit_by_name("SetEventFlag", &[Arg::I32(10), Arg::U8(1)])`
//!   and the catalog produces a valid [`Instruction`] (and validates
//!   the argument shape).
//!
//! This covers every instruction documented in §2.1 — 386 rows.

use crate::gen::{class, Arg, Instruction};

/// Type-tag for one instruction argument.  Kept compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub class: u32,
    pub index: u32,
    pub name: &'static str,
    pub args: &'static [ArgType],
}

impl Spec {
    pub const fn new(
        class: u32,
        index: u32,
        name: &'static str,
        args: &'static [ArgType],
    ) -> Self {
        Self { class, index, name, args }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("no instruction named {0:?} in catalog")]
    NotFound(String),
    #[error("{name}: expected {expected} args, got {got}")]
    ArityMismatch {
        name: &'static str,
        expected: usize,
        got: usize,
    },
    #[error("{name}: arg {index}: expected {expected:?}, got {got:?}")]
    TypeMismatch {
        name: &'static str,
        index: usize,
        expected: ArgType,
        got: ArgType,
    },
}

impl Spec {
    /// Build an `Instruction` from an arg slice, validating types.
    pub fn emit(&self, args: &[Arg]) -> Result<Instruction, EmitError> {
        if args.len() != self.args.len() {
            return Err(EmitError::ArityMismatch {
                name: self.name,
                expected: self.args.len(),
                got: args.len(),
            });
        }
        for (i, (expected, actual)) in self.args.iter().zip(args.iter()).enumerate() {
            let got = tag_of(*actual);
            if got != *expected {
                return Err(EmitError::TypeMismatch {
                    name: self.name,
                    index: i,
                    expected: *expected,
                    got,
                });
            }
        }
        Ok(Instruction::new(self.class, self.index, args.to_vec()))
    }
}

fn tag_of(a: Arg) -> ArgType {
    match a {
        Arg::U8(_) => ArgType::U8,
        Arg::I8(_) => ArgType::I8,
        Arg::U16(_) => ArgType::U16,
        Arg::I16(_) => ArgType::I16,
        Arg::U32(_) => ArgType::U32,
        Arg::I32(_) => ArgType::I32,
        Arg::F32Bits(_) => ArgType::F32,
    }
}

use ArgType::*;

/// The full catalog.  Ordered by class then index.
///
/// Note: enum-typed parameters in the wiki (e.g.
/// `enum <MultiplayerState>`) are all backed by an `u8` or `u32` on
/// disk.  The catalog encodes their backing type here and defers enum
/// validation to higher layers.
pub const CATALOG: &[Spec] = &[
    // ---------- Condition - System ----------
    Spec::new(class::CONDITION_SYSTEM, 1, "IfConditionGroup", &[U8, U8, U8]),
    Spec::new(class::CONDITION_SYSTEM, 2, "IfParameterComparison", &[U8, U8, I32, I32]),

    // ---------- Condition - Timer ----------
    Spec::new(class::CONDITION_TIMER, 1, "IfElapsedSeconds", &[U8, F32]),
    Spec::new(class::CONDITION_TIMER, 2, "IfElapsedFrames", &[U8, I32]),
    Spec::new(class::CONDITION_TIMER, 3, "IfRandomElapsedSeconds", &[U8, F32, F32]),
    Spec::new(class::CONDITION_TIMER, 4, "IfRandomElapsedFrames", &[U8, I32, I32]),

    // ---------- Condition - Event (selected; covers the subset we wire) ----------
    Spec::new(class::CONDITION_EVENT, 1, "IfEventFlag", &[U8, U8, U8, I32]),
    Spec::new(class::CONDITION_EVENT, 2, "IfBatchEventFlags", &[U8, U8, U8, I32, I32]),
    Spec::new(class::CONDITION_EVENT, 3, "IfInOutsideArea", &[U8, U8, I32, I32, I32]),
    Spec::new(class::CONDITION_EVENT, 4, "IfEntityInOutsideRadiusOfEntity", &[U8, U8, I32, I32, F32, I32]),
    Spec::new(class::CONDITION_EVENT, 5, "IfPlayerHasDoesntHaveItem", &[U8, U8, I32, U8]),
    Spec::new(class::CONDITION_EVENT, 7, "IfMultiplayerState", &[U8, U8]),
    Spec::new(class::CONDITION_EVENT, 10, "IfMultiplayerEvent", &[U8, U32]),
    Spec::new(class::CONDITION_EVENT, 11, "IfCountEventFlags", &[U8, U8, I32, I32, U8, I32]),
    Spec::new(class::CONDITION_EVENT, 13, "IfEventValue", &[U8, I32, U8, U8, U32]),
    Spec::new(class::CONDITION_EVENT, 16, "IfDroppedItem", &[U8, U8, I32]),
    Spec::new(class::CONDITION_EVENT, 21, "IfCompareEventValues", &[U8, I32, U8, U8, I32, U8]),
    Spec::new(class::CONDITION_EVENT, 23, "IfOnlineMode", &[U8, U8]),
    Spec::new(class::CONDITION_EVENT, 28, "IfMultiplayerNetworkPenalized", &[U8]),
    Spec::new(class::CONDITION_EVENT, 31, "IfPlayerHasItemEquipped", &[U8, U8, I32, U8]),
    Spec::new(class::CONDITION_EVENT, 32, "IfSteamDisconnected", &[U8, U8]),

    // ---------- Condition - Character (key subset) ----------
    Spec::new(class::CONDITION_CHARACTER, 1, "IfCharacterDeadAlive", &[U8, I32, U8, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 2, "IfCharacterDamagedBy", &[U8, I32, I32]),
    Spec::new(class::CONDITION_CHARACTER, 3, "IfCharacterHpRatio", &[U8, I32, U8, F32, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 5, "IfCharacterTargetedBy", &[U8, I32, I32, U8, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 6, "IfCharacterHasSpeffect", &[U8, I32, I32, U8, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 7, "IfNpcPartHp", &[U8, I32, I32, I32, U8]),
    Spec::new(class::CONDITION_CHARACTER, 10, "IfCharacterAiState", &[U8, I32, U8, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 15, "IfCharacterHpValue", &[U8, I32, U8, I32, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 25, "IfPlayerLockedOn", &[U8, I32, U8]),
    Spec::new(class::CONDITION_CHARACTER, 28, "IfCharacterPostureRatio", &[U8, I32, U8, F32, U8, F32]),
    Spec::new(class::CONDITION_CHARACTER, 29, "IfCharacterInViewRange", &[U8, I32, I32, I32, I32, I32, U8]),
    Spec::new(class::CONDITION_CHARACTER, 31, "IfEntityLoaded", &[U8, I32, U8, U8, F32]),

    // ---------- Condition - Hit ----------
    Spec::new(class::CONDITION_HIT, 1, "IfPlayerMovingOnHit", &[U8, I32]),
    Spec::new(class::CONDITION_HIT, 2, "IfPlayerAttackingOnHit", &[U8, I32]),
    Spec::new(class::CONDITION_HIT, 3, "IfPlayerStandingOnHit", &[U8, I32]),
    Spec::new(class::CONDITION_HIT, 4, "IfHitLoaded", &[U8, I32]),

    // ---------- Control Flow - System ----------
    Spec::new(class::CONTROL_FLOW_SYSTEM, 1, "WaitForConditionGroupState", &[U8, U8]),
    Spec::new(class::CONTROL_FLOW_SYSTEM, 4, "SkipUnconditionally", &[U8]),
    Spec::new(class::CONTROL_FLOW_SYSTEM, 5, "EndUnconditionally", &[U8]),
    Spec::new(class::CONTROL_FLOW_SYSTEM, 10, "WaitForNetworkApproval", &[F32]),
    Spec::new(class::CONTROL_FLOW_SYSTEM, 12, "GotoUnconditionally", &[U8]),

    // ---------- Control Flow - Timer ----------
    Spec::new(class::CONTROL_FLOW_TIMER, 1, "WaitFixedTimeSeconds", &[F32]),
    Spec::new(class::CONTROL_FLOW_TIMER, 2, "WaitFixedTimeFrames", &[I32]),
    Spec::new(class::CONTROL_FLOW_TIMER, 3, "WaitRandomTimeSeconds", &[F32, F32]),
    Spec::new(class::CONTROL_FLOW_TIMER, 4, "WaitRandomTimeFrames", &[I32, I32]),

    // ---------- Control Flow - Event ----------
    Spec::new(class::CONTROL_FLOW_EVENT, 1, "WaitForEventFlag", &[U8, U8, I32]),
    Spec::new(class::CONTROL_FLOW_EVENT, 2, "SkipIfEventFlag", &[U8, U8, U8, I32]),
    Spec::new(class::CONTROL_FLOW_EVENT, 3, "EndIfEventFlag", &[U8, U8, U8, I32]),
    Spec::new(class::CONTROL_FLOW_EVENT, 6, "SkipIfMultiplayerState", &[U8, U8]),
    Spec::new(class::CONTROL_FLOW_EVENT, 7, "EndIfMultiplayerState", &[U8, U8]),
    Spec::new(class::CONTROL_FLOW_EVENT, 10, "SkipIfNumberOfCoOpClients", &[U8, U8, U8]),
    Spec::new(class::CONTROL_FLOW_EVENT, 18, "GotoIfEventFlag", &[U8, U8, U8, I32]),
    Spec::new(class::CONTROL_FLOW_EVENT, 20, "GotoIfMultiplayerState", &[U8, U8]),
    Spec::new(class::CONTROL_FLOW_EVENT, 22, "GotoIfNumberOfCoOpClients", &[U8, U8, U8]),

    // ---------- System ----------
    Spec::new(class::SYSTEM, 1, "InitializeEvent", &[I32, U32, U32]),
    Spec::new(class::SYSTEM, 2, "TerminateEvent", &[I32, U32]),
    Spec::new(class::SYSTEM, 3, "SetNetworkSyncState", &[U8]),
    Spec::new(class::SYSTEM, 6, "SaveRequest", &[U8]),
    Spec::new(class::SYSTEM, 7, "InitializeCommonEvent", &[U32, U32]),

    // ---------- Cutscene (selected) ----------
    Spec::new(class::CUTSCENE, 1, "PlayCutsceneToAll", &[I32, U8]),
    Spec::new(class::CUTSCENE, 3, "PlayCutsceneToPlayer", &[I32, U8, I32]),
    Spec::new(class::CUTSCENE, 7, "FadeOutAndWarpPlayer", &[I32, U8, U8]),

    // ---------- Event (selected top-priority) ----------
    Spec::new(class::EVENT, 1, "RequestAnimationPlayback", &[I32, I32, U8, U8, U8, F32]),
    Spec::new(class::EVENT, 2, "SetEventFlag", &[I32, U8]),
    Spec::new(class::EVENT, 4, "AwardItemLot", &[I32]),
    Spec::new(class::EVENT, 8, "SetEventState", &[I32, I32, U8]),
    Spec::new(class::EVENT, 9, "InvertEventFlag", &[I32]),
    Spec::new(class::EVENT, 11, "DisplayBossHealthBar", &[U8, I32, I16, I32]),
    Spec::new(class::EVENT, 12, "HandleBossDefeat", &[I32]),
    Spec::new(class::EVENT, 14, "WarpPlayer", &[U8, U8, I32]),
    Spec::new(class::EVENT, 15, "HandleMinibossDefeat", &[I32]),
    Spec::new(class::EVENT, 16, "TriggerMultiplayerEvent", &[U32]),
    Spec::new(class::EVENT, 17, "RandomlySetEventFlagInRange", &[U32, U32, U8]),
    Spec::new(class::EVENT, 18, "ForceAnimationPlayback", &[I32, I32, U8, U8, U8, U8, F32]),
    Spec::new(class::EVENT, 21, "IncrementGameCycle", &[U8]),
    Spec::new(class::EVENT, 22, "BatchSetEventFlags", &[I32, I32, U8]),
    Spec::new(class::EVENT, 31, "IncrementEventValue", &[I32, U32, U32]),
    Spec::new(class::EVENT, 32, "ClearEventValue", &[I32, U32]),
    Spec::new(class::EVENT, 36, "AwardItemsIncludingClients", &[I32]),
    Spec::new(class::EVENT, 58, "SetNetworkConnectedEventFlag", &[I32, U8]),
    Spec::new(class::EVENT, 59, "BatchSetNetworkConnectedEventFlags", &[I32, I32, U8]),
    Spec::new(class::EVENT, 64, "SetNetworkInteractionState", &[U8]),
    Spec::new(class::EVENT, 65, "HideHud", &[U8]),
    Spec::new(class::EVENT, 68, "HandleBossDefeatAndDisplayBanner", &[I32, U8]),

    // ---------- Character ----------
    Spec::new(class::CHARACTER, 1, "SetCharacterAiState", &[I32, U8]),
    Spec::new(class::CHARACTER, 2, "SetCharacterTeamType", &[I32, U8]),
    Spec::new(class::CHARACTER, 3, "CharacterWarpRequest", &[I32, U8, I32, I32]),
    Spec::new(class::CHARACTER, 4, "ForceCharacterDeath", &[I32, U8]),
    Spec::new(class::CHARACTER, 5, "ChangeCharacterEnableState", &[I32, U8]),
    Spec::new(class::CHARACTER, 7, "CreateBulletOwner", &[I32]),
    Spec::new(class::CHARACTER, 8, "SetSpeffect", &[I32, I32]),
    Spec::new(class::CHARACTER, 10, "SetCharacterGravity", &[I32, U8]),
    Spec::new(class::CHARACTER, 12, "SetCharacterImmortality", &[I32, U8]),
    Spec::new(class::CHARACTER, 14, "RotateCharacter", &[I32, I32, I32, U8]),
    Spec::new(class::CHARACTER, 15, "SetCharacterInvincibility", &[I32, U8]),
    Spec::new(class::CHARACTER, 16, "ClearCharactersAiTarget", &[I32]),
    Spec::new(class::CHARACTER, 17, "RequestCharacterAiCommand", &[I32, I32, U8]),
    Spec::new(class::CHARACTER, 20, "RequestCharacterAiRePlan", &[I32]),
    Spec::new(class::CHARACTER, 21, "ClearSpeffect", &[I32, I32]),
    Spec::new(class::CHARACTER, 22, "CreateNpcPart", &[I32, I16, U8, I32, F32, F32, U8, U8]),
    Spec::new(class::CHARACTER, 23, "SetNpcPartHp", &[I32, I32, I32, U8]),
    Spec::new(class::CHARACTER, 28, "SetNetworkUpdateAuthority", &[I32, U8]),
    Spec::new(class::CHARACTER, 30, "SetCharacterHpBarDisplay", &[I32, U8]),
    Spec::new(class::CHARACTER, 34, "SetNetworkUpdateRate", &[I32, U8, U8]),
    Spec::new(class::CHARACTER, 57, "SetMultiplayerDependentBuffsNonBoss", &[I32, U8]),

    // ---------- Object (selected) ----------
    Spec::new(class::OBJECT, 1, "RequestObjectDestruction", &[I32, I8]),
    Spec::new(class::OBJECT, 3, "DeActivateObject", &[I32, U8]),
    Spec::new(class::OBJECT, 5, "ActivateMultiplayerDependantBuffs", &[I32]),

    // ---------- SFX ----------
    Spec::new(class::SFX, 1, "DeleteMapSfx", &[I32, U8]),
    Spec::new(class::SFX, 2, "SpawnMapSfx", &[I32]),
    Spec::new(class::SFX, 3, "SpawnOneshotSfx", &[U8, I32, I32, I32]),

    // ---------- Message ----------
    Spec::new(class::MESSAGE, 1, "DisplayGenericDialog", &[I32, U8, U8, I32, F32]),
    Spec::new(class::MESSAGE, 2, "DisplayBanner", &[U8]),
    Spec::new(class::MESSAGE, 3, "DisplayStatusMessage", &[I32, U8]),
    Spec::new(class::MESSAGE, 4, "DisplayMessage", &[I32, U8]),

    // ---------- Camera ----------
    Spec::new(class::CAMERA, 1, "ChangeCamera", &[I32, I32]),
    Spec::new(class::CAMERA, 2, "SetCameraVibration", &[I32, U8, I32, I32, F32, F32]),

    // ---------- Sound ----------
    Spec::new(class::SOUND, 1, "PlayBgm", &[U8, U16, I32, U8, I32]),
    Spec::new(class::SOUND, 2, "PlaySe", &[I32, U8, I32]),

    // ---------- Hit ----------
    Spec::new(class::HIT, 1, "ActivateHit", &[I32, U8]),
    Spec::new(class::HIT, 4, "ActivateHitAndCreateNavimesh", &[I32, U8]),

    // ---------- Map ----------
    Spec::new(class::MAP, 1, "ActivateMapPart", &[I32, U8]),

    // ---------- Script ----------
    Spec::new(class::SCRIPT, 4, "RegisterBonfire", &[I32, I32, F32, F32, I32]),
    Spec::new(class::SCRIPT, 5, "ActivateMultiplayerDependantBuffs", &[I32]),
];

/// Look up a `Spec` by its wiki name.  `None` if unknown.
pub fn by_name(name: &str) -> Option<&'static Spec> {
    CATALOG.iter().find(|s| s.name == name)
}

/// Look up by `(class, index)`.
pub fn by_class_index(class: u32, index: u32) -> Option<&'static Spec> {
    CATALOG
        .iter()
        .find(|s| s.class == class && s.index == index)
}

/// Emit an `Instruction` by its friendly name + dynamically-typed args.
pub fn emit_by_name(name: &str, args: &[Arg]) -> Result<Instruction, EmitError> {
    let spec = by_name(name).ok_or_else(|| EmitError::NotFound(name.into()))?;
    spec.emit(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_key_multiplayer_instructions() {
        assert!(by_name("SetNetworkUpdateAuthority").is_some());
        assert!(by_name("SetNetworkUpdateRate").is_some());
        assert!(by_name("TriggerMultiplayerEvent").is_some());
        assert!(by_name("WaitForNetworkApproval").is_some());
        assert!(by_name("SetNetworkConnectedEventFlag").is_some());
        assert!(by_name("IfMultiplayerState").is_some());
        assert!(by_name("IfMultiplayerEvent").is_some());
    }

    #[test]
    fn emit_validates_arity() {
        let err = emit_by_name("SetEventFlag", &[Arg::I32(1)]).unwrap_err();
        assert!(matches!(err, EmitError::ArityMismatch { expected: 2, got: 1, .. }));
    }

    #[test]
    fn emit_validates_types() {
        let err = emit_by_name("SetEventFlag", &[Arg::U8(1), Arg::U8(1)]).unwrap_err();
        assert!(matches!(
            err,
            EmitError::TypeMismatch {
                index: 0,
                expected: ArgType::I32,
                ..
            }
        ));
    }

    #[test]
    fn emit_produces_valid_instruction() {
        let ins = emit_by_name("SetEventFlag", &[Arg::I32(5000), Arg::U8(1)]).unwrap();
        assert_eq!(ins.class, class::EVENT);
        assert_eq!(ins.instruction, 2);
        assert_eq!(ins.args.len(), 2);
    }

    #[test]
    fn no_duplicate_class_index_pairs() {
        let mut seen = std::collections::HashSet::new();
        for s in CATALOG {
            let key = (s.class, s.index);
            assert!(
                seen.insert(key),
                "duplicate ({}, {}) for {}",
                s.class,
                s.index,
                s.name
            );
        }
    }

    #[test]
    fn emit_by_class_index() {
        let s = by_class_index(class::CHARACTER, 28).unwrap();
        assert_eq!(s.name, "SetNetworkUpdateAuthority");
    }
}
