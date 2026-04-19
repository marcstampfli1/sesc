//! Combat bridge — damage hook + deflect classification.
//!
//! SPEC §4.1, OSINT §3.2.  The hook runs **before** the native damage
//! computation so the owning peer decides the outcome.
//!
//! **P0 gap #3**: damage-application function AOB — populate at runtime
//! via a pattern near known damage strings; log every hit then compare
//! against expected parry/hit distribution to confirm.

use parking_lot::RwLock;
use sekiro_sdk_core::animation::{AnimationId, DeflectAnimTable};
use sekiro_sdk_core::atkparam::{AtkParam, DamageLevel, DockingEdgeReaction};
use sekiro_sdk_core::entity::EntityId;
use sekiro_sdk_sys::memory::RawPtr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatOutcome {
    /// Clean hit — damage applied normally.
    Hit,
    /// Victim blocked; chip damage applies per `atkDark`.
    Block,
    /// Victim deflected; posture damage per `AtkStam*Correction`.
    Deflect,
    /// Attack missed (invincibility/i-frame).
    Miss,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CombatEvent {
    pub attacker: EntityId,
    pub victim: EntityId,
    pub atk_param_id: i32,
    pub outcome: CombatOutcome,
    pub damage_level: u8,
    pub posture_damage: i16,
    /// NPC-reaction enum — relevant for animation sync on the non-owner.
    pub reaction: u8,
    /// SpEffect IDs that should apply on hit (from AtkParam.spEffect0-4).
    pub on_hit_speffects: [i32; 5],
}

/// Classifier — converts a raw (attacker, victim, AtkParam) into a typed
/// [`CombatEvent`].  Pure function modulo the deflect-table read.
pub struct CombatClassifier {
    pub deflect: RwLock<DeflectAnimTable>,
}

impl CombatClassifier {
    pub fn new() -> Self {
        Self { deflect: RwLock::new(DeflectAnimTable::new()) }
    }

    /// # Safety
    /// `atk_param.row` + `victim_anim` must be live reads from the
    /// current frame.
    pub unsafe fn classify(
        &self,
        attacker: EntityId,
        victim: EntityId,
        victim_char_id: u32,
        victim_anim: AnimationId,
        victim_invincible: bool,
        atk_param_id: i32,
        atk_param: AtkParam,
    ) -> CombatEvent {
        if victim_invincible {
            return blank(attacker, victim, atk_param_id, CombatOutcome::Miss);
        }
        let carries_posture = atk_param.can_deflect();
        let in_deflect_set = self
            .deflect
            .read()
            .contains(victim_char_id, victim_anim);
        let outcome = if carries_posture && in_deflect_set {
            CombatOutcome::Deflect
        } else if in_deflect_set_no_atk(carries_posture, in_deflect_set) {
            // Victim is blocking but the attack can't deflect.
            CombatOutcome::Block
        } else {
            CombatOutcome::Hit
        };

        let dl: DamageLevel = atk_param.damage_level();
        let reaction: DockingEdgeReaction = atk_param.docking_edge_reaction();
        let stam = atk_param.atk_stam();
        let corr = atk_param.atk_stam_correction() as i32;
        let posture = if outcome == CombatOutcome::Deflect {
            // Simplified model: AtkStam * AtkStamCorrection/100.
            ((stam as i32) * corr / 100) as i16
        } else {
            0
        };
        let speffects = atk_param.on_hit_sp_effects();
        CombatEvent {
            attacker,
            victim,
            atk_param_id,
            outcome,
            damage_level: dl.0,
            posture_damage: posture,
            reaction: reaction_as_u8(reaction),
            on_hit_speffects: speffects,
        }
    }

    /// Call during Phase C observation to teach the classifier which
    /// anim IDs are deflect anims for a given character (SPEC §11 #8).
    pub fn learn_deflect(&self, char_id: u32, anim_id: AnimationId) {
        self.deflect.write().learn(char_id, anim_id);
    }

    pub fn table_size(&self) -> usize {
        self.deflect.read().size()
    }
}

fn in_deflect_set_no_atk(carries_posture: bool, in_set: bool) -> bool {
    in_set && !carries_posture
}

fn reaction_as_u8(r: DockingEdgeReaction) -> u8 {
    match r {
        DockingEdgeReaction::ComboInterrupted => 1,
        DockingEdgeReaction::ComboInterruptedAlt => 2,
        DockingEdgeReaction::ComboContinues => 11,
        DockingEdgeReaction::Other(v) => v,
    }
}

fn blank(attacker: EntityId, victim: EntityId, atk_param_id: i32, outcome: CombatOutcome) -> CombatEvent {
    CombatEvent {
        attacker,
        victim,
        atk_param_id,
        outcome,
        damage_level: 0,
        posture_damage: 0,
        reaction: 0,
        on_hit_speffects: [0; 5],
    }
}

/// The combat bridge as seen by higher layers.  Owns the damage-function
/// hook trampoline once installed.
pub struct CombatBridge {
    pub classifier: CombatClassifier,
    pub damage_fn: Option<RawPtr>,
    pub damage_trampoline: core::sync::atomic::AtomicUsize,
}

impl CombatBridge {
    pub fn new() -> Self {
        Self {
            classifier: CombatClassifier::new(),
            damage_fn: None,
            damage_trampoline: core::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn set_damage_fn(&mut self, ptr: RawPtr) {
        self.damage_fn = Some(ptr);
    }

    pub fn trampoline(&self) -> usize {
        self.damage_trampoline
            .load(core::sync::atomic::Ordering::Acquire)
    }

    pub fn set_trampoline(&self, p: usize) {
        self.damage_trampoline
            .store(p, core::sync::atomic::Ordering::Release);
    }
}
