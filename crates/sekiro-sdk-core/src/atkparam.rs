//! AtkParam row — schema from `AtkParams.md`.
//!
//! Every documented field has an offset + typed accessor.  Offsets are
//! computed transitively from the field ordering in `AtkParams.md`:
//! `FIELD_B = FIELD_A + sizeof(FIELD_A)`, which keeps them in one
//! consistent source of truth even when we're wrong about a specific
//! value (fixing one base offset propagates correctly through the
//! dependent ones).
//!
//! Used by the combat bridge for deflect detection and by the damage
//! broadcast path for routing hit information to non-authority peers.
//! SPEC §4.1, OSINT §3.2.

use sekiro_sdk_sys::memory::RawPtr;

/// Byte-offsets of every documented AtkParam field, computed from the
/// wiki's ordered field list.  All f32/s32 are 4-aligned; s16 is
/// 2-aligned; u8/s8/b8 is 1-aligned.  Field order is the
/// `AtkParams.md` canonical order.
#[rustfmt::skip]
pub mod off {
    // Block 1 — 16 bytes of hit radii
    pub const HIT0_RADIUS: usize = 0x00;
    pub const HIT1_RADIUS: usize = HIT0_RADIUS + 4;
    pub const HIT2_RADIUS: usize = HIT1_RADIUS + 4;
    pub const HIT3_RADIUS: usize = HIT2_RADIUS + 4;
    pub const KNOCKBACK_DIST: usize = HIT3_RADIUS + 4;
    pub const HIT_STOP_TIME: usize = KNOCKBACK_DIST + 4;
    pub const UNK01_DEFLECT_KNOCKBACK: usize = HIT_STOP_TIME + 4;
    pub const UNK02: usize = UNK01_DEFLECT_KNOCKBACK + 4;

    // SpEffects (5 × s32)
    pub const SP_EFFECT_0: usize = UNK02 + 4;
    pub const SP_EFFECT_1: usize = SP_EFFECT_0 + 4;
    pub const SP_EFFECT_2: usize = SP_EFFECT_1 + 4;
    pub const SP_EFFECT_3: usize = SP_EFFECT_2 + 4;
    pub const SP_EFFECT_4: usize = SP_EFFECT_3 + 4;

    // Hit 0-3 DmyPoly pairs (s16)
    pub const HIT0_DMY_POLY1: usize = SP_EFFECT_4 + 4;
    pub const HIT1_DMY_POLY1: usize = HIT0_DMY_POLY1 + 2;
    pub const HIT2_DMY_POLY1: usize = HIT1_DMY_POLY1 + 2;
    pub const HIT3_DMY_POLY1: usize = HIT2_DMY_POLY1 + 2;
    pub const HIT0_DMY_POLY2: usize = HIT3_DMY_POLY1 + 2;
    pub const HIT1_DMY_POLY2: usize = HIT0_DMY_POLY2 + 2;
    pub const HIT2_DMY_POLY2: usize = HIT1_DMY_POLY2 + 2;
    pub const HIT3_DMY_POLY2: usize = HIT2_DMY_POLY2 + 2;

    // Correction rates (s16)
    pub const BLOWING_CORRECTION: usize = HIT3_DMY_POLY2 + 2;
    pub const ATK_PHYS_CORRECTION: usize = BLOWING_CORRECTION + 2;
    pub const ATK_MAG_CORRECTION: usize = ATK_PHYS_CORRECTION + 2;
    pub const ATK_FIRE_CORRECTION: usize = ATK_MAG_CORRECTION + 2;
    pub const ATK_THUN_CORRECTION: usize = ATK_FIRE_CORRECTION + 2;
    pub const ATK_STAM_CORRECTION: usize = ATK_THUN_CORRECTION + 2;
    pub const GUARD_ATK_RATE_CORRECTION: usize = ATK_STAM_CORRECTION + 2;
    pub const GUARD_BREAK_CORRECTION: usize = GUARD_ATK_RATE_CORRECTION + 2;
    pub const ATK_THROW_ESCAPE_CORRECTION: usize = GUARD_BREAK_CORRECTION + 2;
    pub const ATK_SUPER_ARMOR_CORRECTION: usize = ATK_THROW_ESCAPE_CORRECTION + 2;

    // Raw damage values (s16)
    pub const ATK_PHYS: usize = ATK_SUPER_ARMOR_CORRECTION + 2;
    pub const ATK_MAG: usize = ATK_PHYS + 2;
    pub const ATK_FIRE: usize = ATK_MAG + 2;
    pub const ATK_THUN: usize = ATK_FIRE + 2;
    pub const ATK_STAM: usize = ATK_THUN + 2;
    pub const GUARD_ATK_RATE: usize = ATK_STAM + 2;
    pub const GUARD_BREAK_RATE: usize = GUARD_ATK_RATE + 2;
    pub const ATK_SUPER_ARMOR: usize = GUARD_BREAK_RATE + 2;
    pub const ATK_THROW_ESCAPE: usize = ATK_SUPER_ARMOR + 2;
    pub const ATK_OBJ: usize = ATK_THROW_ESCAPE + 2;
    pub const GUARD_STAMINA_CUT_RATE: usize = ATK_OBJ + 2;
    pub const GUARD_RATE: usize = GUARD_STAMINA_CUT_RATE + 2;
    pub const THROW_TYPE_ID: usize = GUARD_RATE + 2;

    // Hit type / priority (u8) and damage-level
    pub const HIT0_HIT_TYPE: usize = THROW_TYPE_ID + 2;
    pub const HIT1_HIT_TYPE: usize = HIT0_HIT_TYPE + 1;
    pub const HIT2_HIT_TYPE: usize = HIT1_HIT_TYPE + 1;
    pub const HIT3_HIT_TYPE: usize = HIT2_HIT_TYPE + 1;
    pub const HIT0_PRIORITY: usize = HIT3_HIT_TYPE + 1;
    pub const HIT1_PRIORITY: usize = HIT0_PRIORITY + 1;
    pub const HIT2_PRIORITY: usize = HIT1_PRIORITY + 1;
    pub const HIT3_PRIORITY: usize = HIT2_PRIORITY + 1;
    pub const DAMAGE_LEVEL: usize = HIT3_PRIORITY + 1;
    pub const MAP_HIT_TYPE1: usize = DAMAGE_LEVEL + 1;
    pub const GUARD_CUT_CANCEL_RATE: usize = MAP_HIT_TYPE1 + 1;

    // s8 block
    pub const ATK_ATTRIBUTE: usize = GUARD_CUT_CANCEL_RATE + 1;
    pub const SP_ATTRIBUTE: usize = ATK_ATTRIBUTE + 1;
    pub const ATK_TYPE: usize = SP_ATTRIBUTE + 1;
    pub const ATK_MATERIAL: usize = ATK_TYPE + 1;
    pub const ATK_SIZE: usize = ATK_MATERIAL + 1;
    pub const DEF_MATERIAL: usize = ATK_SIZE + 1;
    pub const DEF_SFX_MATERIAL: usize = DEF_MATERIAL + 1;

    // u8 flags + more
    pub const HIT_SOURCE_TYPE: usize = DEF_SFX_MATERIAL + 1;
    pub const THROW_FLAG: usize = HIT_SOURCE_TYPE + 1;
    /// 8 b8 bits packed into one byte: disableGuard, disableStaminaAttack,
    /// disableHitSpEffect, IgnoreNotifyMissSwingForAI, repeatHitSfx,
    /// IsArrowAtk, IsGhostAtk, isDisableNoDamage (LSB-first).
    pub const FLAGS_A: usize = THROW_FLAG + 1;
    pub const ATK_POW_FOR_SFX_SE: usize = FLAGS_A + 1;
    pub const ATK_DIR_FOR_SFX_SE: usize = ATK_POW_FOR_SFX_SE + 1;
    /// Second byte of packed b8 flags.
    pub const FLAGS_B: usize = ATK_DIR_FOR_SFX_SE + 1;
    pub const PAD1: usize = FLAGS_B + 1;
    pub const REGAINABLE_SLOT_ID: usize = PAD1 + 1;

    // s32 block (aligned to 4)
    pub const DEATH_CAUSE_ID: usize = align4(REGAINABLE_SLOT_ID + 1);
    pub const DECAL_ID1: usize = DEATH_CAUSE_ID + 4;
    pub const DECAL_ID2: usize = DECAL_ID1 + 4;
    pub const SPAWN_AI_SOUND_ID: usize = DECAL_ID2 + 4;
    pub const HIT_AI_SOUND_ID: usize = SPAWN_AI_SOUND_ID + 4;
    pub const RUMBLE_ID0: usize = HIT_AI_SOUND_ID + 4;
    pub const RUMBLE_ID1: usize = RUMBLE_ID0 + 4;
    pub const RUMBLE_ID2: usize = RUMBLE_ID1 + 4;
    pub const RUMBLE_ID3: usize = RUMBLE_ID2 + 4;

    // VFX+dummypoly blocks for hits 0-3 (s32 × 3 each)
    pub const HIT0_VFX_ID: usize = RUMBLE_ID3 + 4;
    pub const HIT0_DUMMY_POLY_ID0: usize = HIT0_VFX_ID + 4;
    pub const HIT0_DUMMY_POLY_ID1: usize = HIT0_DUMMY_POLY_ID0 + 4;
    pub const HIT1_VFX_ID: usize = HIT0_DUMMY_POLY_ID1 + 4;
    pub const HIT1_DUMMY_POLY_ID0: usize = HIT1_VFX_ID + 4;
    pub const HIT1_DUMMY_POLY_ID1: usize = HIT1_DUMMY_POLY_ID0 + 4;
    pub const HIT2_VFX_ID: usize = HIT1_DUMMY_POLY_ID1 + 4;
    pub const HIT2_DUMMY_POLY_ID0: usize = HIT2_VFX_ID + 4;
    pub const HIT2_DUMMY_POLY_ID1: usize = HIT2_DUMMY_POLY_ID0 + 4;
    pub const HIT3_VFX_ID: usize = HIT2_DUMMY_POLY_ID1 + 4;
    pub const HIT3_DUMMY_POLY_ID0: usize = HIT3_VFX_ID + 4;
    pub const HIT3_DUMMY_POLY_ID1: usize = HIT3_DUMMY_POLY_ID0 + 4;
    pub const HIT4_VFX_ID: usize = HIT3_DUMMY_POLY_ID1 + 4;
    pub const HIT4_DUMMY_POLY_ID0: usize = HIT4_VFX_ID + 4;
    pub const HIT4_DUMMY_POLY_ID1: usize = HIT4_DUMMY_POLY_ID0 + 4;
    pub const HIT5_VFX_ID: usize = HIT4_DUMMY_POLY_ID1 + 4;
    pub const HIT5_DUMMY_POLY_ID0: usize = HIT5_VFX_ID + 4;
    pub const HIT5_DUMMY_POLY_ID1: usize = HIT5_DUMMY_POLY_ID0 + 4;
    pub const HIT6_VFX_ID: usize = HIT5_DUMMY_POLY_ID1 + 4;
    pub const HIT6_DUMMY_POLY_ID0: usize = HIT6_VFX_ID + 4;
    pub const HIT6_DUMMY_POLY_ID1: usize = HIT6_DUMMY_POLY_ID0 + 4;
    pub const HIT7_VFX_ID: usize = HIT6_DUMMY_POLY_ID1 + 4;
    pub const HIT7_DUMMY_POLY_ID0: usize = HIT7_VFX_ID + 4;
    pub const HIT7_DUMMY_POLY_ID1: usize = HIT7_DUMMY_POLY_ID0 + 4;

    // Extra-hit radii 4-15 (f32)
    pub const HIT4_RADIUS: usize = HIT7_DUMMY_POLY_ID1 + 4;
    pub const HIT5_RADIUS: usize = HIT4_RADIUS + 4;
    pub const HIT6_RADIUS: usize = HIT5_RADIUS + 4;
    pub const HIT7_RADIUS: usize = HIT6_RADIUS + 4;
    pub const HIT8_RADIUS: usize = HIT7_RADIUS + 4;
    pub const HIT9_RADIUS: usize = HIT8_RADIUS + 4;
    pub const HIT10_RADIUS: usize = HIT9_RADIUS + 4;
    pub const HIT11_RADIUS: usize = HIT10_RADIUS + 4;
    pub const HIT12_RADIUS: usize = HIT11_RADIUS + 4;
    pub const HIT13_RADIUS: usize = HIT12_RADIUS + 4;
    pub const HIT14_RADIUS: usize = HIT13_RADIUS + 4;
    pub const HIT15_RADIUS: usize = HIT14_RADIUS + 4;

    // Extra-hit DmyPoly1 4-15 (s16)
    pub const HIT4_DMY_POLY1: usize = HIT15_RADIUS + 4;
    pub const HIT5_DMY_POLY1: usize = HIT4_DMY_POLY1 + 2;
    pub const HIT6_DMY_POLY1: usize = HIT5_DMY_POLY1 + 2;
    pub const HIT7_DMY_POLY1: usize = HIT6_DMY_POLY1 + 2;
    pub const HIT8_DMY_POLY1: usize = HIT7_DMY_POLY1 + 2;
    pub const HIT9_DMY_POLY1: usize = HIT8_DMY_POLY1 + 2;
    pub const HIT10_DMY_POLY1: usize = HIT9_DMY_POLY1 + 2;
    pub const HIT11_DMY_POLY1: usize = HIT10_DMY_POLY1 + 2;
    pub const HIT12_DMY_POLY1: usize = HIT11_DMY_POLY1 + 2;
    pub const HIT13_DMY_POLY1: usize = HIT12_DMY_POLY1 + 2;
    pub const HIT14_DMY_POLY1: usize = HIT13_DMY_POLY1 + 2;
    pub const HIT15_DMY_POLY1: usize = HIT14_DMY_POLY1 + 2;

    // Extra-hit DmyPoly2 4-15 (s16)
    pub const HIT4_DMY_POLY2: usize = HIT15_DMY_POLY1 + 2;
    pub const HIT5_DMY_POLY2: usize = HIT4_DMY_POLY2 + 2;
    pub const HIT6_DMY_POLY2: usize = HIT5_DMY_POLY2 + 2;
    pub const HIT7_DMY_POLY2: usize = HIT6_DMY_POLY2 + 2;
    pub const HIT8_DMY_POLY2: usize = HIT7_DMY_POLY2 + 2;
    pub const HIT9_DMY_POLY2: usize = HIT8_DMY_POLY2 + 2;
    pub const HIT10_DMY_POLY2: usize = HIT9_DMY_POLY2 + 2;
    pub const HIT11_DMY_POLY2: usize = HIT10_DMY_POLY2 + 2;
    pub const HIT12_DMY_POLY2: usize = HIT11_DMY_POLY2 + 2;
    pub const HIT13_DMY_POLY2: usize = HIT12_DMY_POLY2 + 2;
    pub const HIT14_DMY_POLY2: usize = HIT13_DMY_POLY2 + 2;
    pub const HIT15_DMY_POLY2: usize = HIT14_DMY_POLY2 + 2;

    // Extra-hit type 4-15 (u8)
    pub const HIT4_HIT_TYPE: usize = HIT15_DMY_POLY2 + 2;
    pub const HIT5_HIT_TYPE: usize = HIT4_HIT_TYPE + 1;
    pub const HIT6_HIT_TYPE: usize = HIT5_HIT_TYPE + 1;
    pub const HIT7_HIT_TYPE: usize = HIT6_HIT_TYPE + 1;
    pub const HIT8_HIT_TYPE: usize = HIT7_HIT_TYPE + 1;
    pub const HIT9_HIT_TYPE: usize = HIT8_HIT_TYPE + 1;
    pub const HIT10_HIT_TYPE: usize = HIT9_HIT_TYPE + 1;
    pub const HIT11_HIT_TYPE: usize = HIT10_HIT_TYPE + 1;
    pub const HIT12_HIT_TYPE: usize = HIT11_HIT_TYPE + 1;
    pub const HIT13_HIT_TYPE: usize = HIT12_HIT_TYPE + 1;
    pub const HIT14_HIT_TYPE: usize = HIT13_HIT_TYPE + 1;
    pub const HIT15_HIT_TYPE: usize = HIT14_HIT_TYPE + 1;

    // Trailing block (from wiki 0x17C onwards)
    pub const UNK_0X17C: usize = align4(HIT15_HIT_TYPE + 1);
    pub const UNK_0X180: usize = UNK_0X17C + 4;
    pub const UNK_0X184: usize = UNK_0X180 + 4;
    pub const UNK_0X186: usize = UNK_0X184 + 2;
    pub const DEF_MATERIAL_VAL1: usize = UNK_0X186 + 2;
    pub const DEF_MATERIAL_VAL2: usize = DEF_MATERIAL_VAL1 + 2;
    pub const ATK_DARK_CORRECTION: usize = DEF_MATERIAL_VAL2 + 2;
    pub const ATK_DARK: usize = ATK_DARK_CORRECTION + 2;

    // Charge-attack + unknown flag bytes.  Wiki labels six bits at
    // "0x192" as a separate flag byte, distinct from IsChargeAtk2/3 at
    // 0x190.  Layout: {FLAGS_C byte @ 0x190, UNK_0x191 u8, FLAGS_D byte
    // @ 0x192, UNK_0x193 u8, NewStaminaDamage1 u16 @ 0x194, ...}.
    pub const FLAGS_C: usize = ATK_DARK + 2;      // 0x190
    pub const UNK_0X191: usize = FLAGS_C + 1;     // 0x191
    pub const FLAGS_D: usize = UNK_0X191 + 1;     // 0x192
    pub const UNK_0X193: usize = FLAGS_D + 1;     // 0x193
    pub const NEW_STAMINA_DAMAGE_1: usize = UNK_0X193 + 1; // 0x194
    pub const UNK_0X196: usize = NEW_STAMINA_DAMAGE_1 + 2;
    pub const SET_DOCKING_EDGE_VAR_ID: usize = UNK_0X196 + 2;
    pub const SET_GUARD_BEHAVIOR: usize = SET_DOCKING_EDGE_VAR_ID + 1;
    pub const SEKIRO_UNK1: usize = SET_GUARD_BEHAVIOR + 1;
    pub const NEW_DAMAGE_SYSTEM_1: usize = SEKIRO_UNK1 + 2;
    pub const NEW_DAMAGE_SYSTEM_2: usize = NEW_DAMAGE_SYSTEM_1 + 2;
    pub const NEW_DAMAGE_SYSTEM_3: usize = NEW_DAMAGE_SYSTEM_2 + 2;
    pub const DAMAGE_LEVEL_U16: usize = NEW_DAMAGE_SYSTEM_3 + 2;
    pub const ATK_ELEMENT_CORRECT_PARAM_ID: usize = DAMAGE_LEVEL_U16 + 2;
    pub const UNK_0X1A8: usize = ATK_ELEMENT_CORRECT_PARAM_ID + 4;
    pub const EXTRA_ATTRIBUTE: usize = UNK_0X1A8 + 1;
    pub const NEW_STAMINA_DAMAGE_2: usize = EXTRA_ATTRIBUTE + 1;
    pub const BLOCKED_POSTURE_DAMAGE: usize = NEW_STAMINA_DAMAGE_2 + 2;

    // Trailing unknown block up to 0x1CC f32
    pub const UNK_0X1AE: usize = BLOCKED_POSTURE_DAMAGE + 2;
    pub const UNK_0X1B0: usize = UNK_0X1AE + 2;
    pub const UNK_0X1B2: usize = UNK_0X1B0 + 2;
    pub const UNK_0X1B4: usize = UNK_0X1B2 + 2;
    pub const UNK_0X1B6: usize = UNK_0X1B4 + 2;
    pub const ATK_TYPE_MATERIAL_NEW: usize = UNK_0X1B6 + 2;
    pub const UNK_0X1B9: usize = ATK_TYPE_MATERIAL_NEW + 1;
    pub const UNK_0X1BA: usize = UNK_0X1B9 + 1;
    pub const UNK_0X1BB: usize = UNK_0X1BA + 1;
    pub const UNK_0X1BC: usize = UNK_0X1BB + 1;
    pub const UNK_0X1C0: usize = UNK_0X1BC + 4;
    pub const UNK_0X1C1: usize = UNK_0X1C0 + 1;
    pub const UNK_0X1C2: usize = UNK_0X1C1 + 1;
    pub const UNK_0X1C4: usize = UNK_0X1C2 + 2;
    pub const UNK_0X1C8: usize = UNK_0X1C4 + 4;
    pub const MAP_HIT_TYPE2: usize = UNK_0X1C8 + 1;
    pub const MAP_HIT_TYPE3: usize = MAP_HIT_TYPE2 + 1;
    pub const UNK_0X1CB: usize = MAP_HIT_TYPE3 + 1;
    pub const UNK_0X1CC: usize = UNK_0X1CB + 1;

    /// Total stride.  Used when binary-searching the param row table.
    pub const ROW_STRIDE: usize = UNK_0X1CC + 4;

    const fn align4(n: usize) -> usize {
        (n + 3) & !3
    }
}

// --- Enums / typed values --------------------------------------------------

/// NPC reaction to having an attack deflected/blocked.  From
/// `AtkParams.md`:
/// - 1: combo interrupted
/// - 2: combo interrupted (different reaction)
/// - 11: no reaction, combo continues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockingEdgeReaction {
    ComboInterrupted,
    ComboInterruptedAlt,
    ComboContinues,
    Other(u8),
}

impl DockingEdgeReaction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => DockingEdgeReaction::ComboInterrupted,
            2 => DockingEdgeReaction::ComboInterruptedAlt,
            11 => DockingEdgeReaction::ComboContinues,
            other => DockingEdgeReaction::Other(other),
        }
    }

    pub fn combo_continues(self) -> bool {
        matches!(self, DockingEdgeReaction::ComboContinues)
    }
}

/// `damageLevel` enum from `AtkParams.md` 0-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageLevel(pub u8);

impl DamageLevel {
    pub fn is_katana_drag(self) -> bool {
        self.0 == 10
    }
    pub fn is_airborne_launch(self) -> bool {
        self.0 == 9
    }
    pub fn is_pancake(self) -> bool {
        self.0 == 6
    }
    pub fn is_no_interrupt(self) -> bool {
        self.0 == 8
    }
    pub fn stuns_heavily(self) -> bool {
        matches!(self.0, 6 | 9 | 10)
    }
}

/// The individual boolean flags packed into FLAGS_A (byte at `off::FLAGS_A`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagsA(pub u8);

impl FlagsA {
    pub fn disable_guard(self) -> bool { self.0 & (1 << 0) != 0 }
    pub fn disable_stamina_attack(self) -> bool { self.0 & (1 << 1) != 0 }
    pub fn disable_hit_speffect(self) -> bool { self.0 & (1 << 2) != 0 }
    pub fn ignore_notify_miss_swing_for_ai(self) -> bool { self.0 & (1 << 3) != 0 }
    pub fn repeat_hit_sfx(self) -> bool { self.0 & (1 << 4) != 0 }
    pub fn is_arrow_atk(self) -> bool { self.0 & (1 << 5) != 0 }
    pub fn is_ghost_atk(self) -> bool { self.0 & (1 << 6) != 0 }
    pub fn is_disable_no_damage(self) -> bool { self.0 & (1 << 7) != 0 }
}

/// FLAGS_B at `off::FLAGS_B`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagsB(pub u8);

impl FlagsB {
    pub fn oppose_target(self) -> bool { self.0 & (1 << 0) != 0 }
    pub fn friendly_target(self) -> bool { self.0 & (1 << 1) != 0 }
    pub fn self_target(self) -> bool { self.0 & (1 << 2) != 0 }
    pub fn is_charge_atk(self) -> bool { self.0 & (1 << 3) != 0 }
    pub fn is_share_hit_list(self) -> bool { self.0 & (1 << 4) != 0 }
    pub fn is_check_obj_penetration(self) -> bool { self.0 & (1 << 5) != 0 }
}

/// Field tag used by a typed reader API.  Keep in lockstep with `off::*`.
#[derive(Debug, Clone, Copy)]
pub enum AtkParamField {
    AtkStam,
    AtkStamCorrection,
    DamageLevel,
    SetDockingEdgeVarId,
    SpEffect(u8), // 0..=4
    NewStaminaDamage1,
    NewStaminaDamage2,
    BlockedPostureDamage,
    AtkPhys,
    AtkMag,
    AtkFire,
    AtkThun,
    AtkDark,
    ThrowTypeId,
    FlagsA,
    FlagsB,
}

// --- Accessor type --------------------------------------------------------

/// Typed read-only view over a single AtkParam row.
#[derive(Debug, Clone, Copy)]
pub struct AtkParam {
    pub row: RawPtr,
}

macro_rules! reader_s16 {
    ($name:ident, $off:ident) => {
        #[inline]
        pub unsafe fn $name(&self) -> i16 {
            self.row.offset(off::$off as isize).read::<i16>()
        }
    };
}

macro_rules! reader_s32 {
    ($name:ident, $off:ident) => {
        #[inline]
        pub unsafe fn $name(&self) -> i32 {
            self.row.offset(off::$off as isize).read::<i32>()
        }
    };
}

macro_rules! reader_u8 {
    ($name:ident, $off:ident) => {
        #[inline]
        pub unsafe fn $name(&self) -> u8 {
            self.row.offset(off::$off as isize).read::<u8>()
        }
    };
}

macro_rules! reader_f32 {
    ($name:ident, $off:ident) => {
        #[inline]
        pub unsafe fn $name(&self) -> f32 {
            self.row.offset(off::$off as isize).read::<f32>()
        }
    };
}

impl AtkParam {
    pub fn new(row: RawPtr) -> Self {
        Self { row }
    }

    // -- Hit radii --
    reader_f32!(hit0_radius, HIT0_RADIUS);
    reader_f32!(hit1_radius, HIT1_RADIUS);
    reader_f32!(hit2_radius, HIT2_RADIUS);
    reader_f32!(hit3_radius, HIT3_RADIUS);
    reader_f32!(knockback_dist, KNOCKBACK_DIST);
    reader_f32!(hit_stop_time, HIT_STOP_TIME);
    reader_f32!(deflect_knockback, UNK01_DEFLECT_KNOCKBACK);

    /// # Safety
    /// Caller guarantees `self.row` points to a live AtkParam row
    /// matching this crate's offset model for the current frame.
    #[inline]
    pub unsafe fn sp_effect(&self, slot: u8) -> Option<i32> {
        let off = match slot {
            0 => off::SP_EFFECT_0,
            1 => off::SP_EFFECT_1,
            2 => off::SP_EFFECT_2,
            3 => off::SP_EFFECT_3,
            4 => off::SP_EFFECT_4,
            _ => return None,
        };
        Some(self.row.offset(off as isize).read::<i32>())
    }

    // -- Correction rates (s16) --
    reader_s16!(blowing_correction, BLOWING_CORRECTION);
    reader_s16!(atk_phys_correction, ATK_PHYS_CORRECTION);
    reader_s16!(atk_mag_correction, ATK_MAG_CORRECTION);
    reader_s16!(atk_fire_correction, ATK_FIRE_CORRECTION);
    reader_s16!(atk_thun_correction, ATK_THUN_CORRECTION);
    reader_s16!(atk_stam_correction, ATK_STAM_CORRECTION);
    reader_s16!(guard_atk_rate_correction, GUARD_ATK_RATE_CORRECTION);
    reader_s16!(guard_break_correction, GUARD_BREAK_CORRECTION);
    reader_s16!(atk_throw_escape_correction, ATK_THROW_ESCAPE_CORRECTION);
    reader_s16!(atk_super_armor_correction, ATK_SUPER_ARMOR_CORRECTION);

    // -- Raw damage (s16) --
    reader_s16!(atk_phys, ATK_PHYS);
    reader_s16!(atk_mag, ATK_MAG);
    reader_s16!(atk_fire, ATK_FIRE);
    reader_s16!(atk_thun, ATK_THUN);
    reader_s16!(atk_stam, ATK_STAM);
    reader_s16!(guard_atk_rate, GUARD_ATK_RATE);
    reader_s16!(guard_break_rate, GUARD_BREAK_RATE);
    reader_s16!(atk_super_armor, ATK_SUPER_ARMOR);
    reader_s16!(atk_throw_escape, ATK_THROW_ESCAPE);
    reader_s16!(atk_obj, ATK_OBJ);
    reader_s16!(guard_stamina_cut_rate, GUARD_STAMINA_CUT_RATE);
    reader_s16!(guard_rate, GUARD_RATE);
    reader_s16!(throw_type_id, THROW_TYPE_ID);

    // -- Hit-type/priority + damage-level --
    reader_u8!(hit0_hit_type, HIT0_HIT_TYPE);
    reader_u8!(hit1_hit_type, HIT1_HIT_TYPE);
    reader_u8!(hit2_hit_type, HIT2_HIT_TYPE);
    reader_u8!(hit3_hit_type, HIT3_HIT_TYPE);
    reader_u8!(hit0_priority, HIT0_PRIORITY);
    reader_u8!(hit1_priority, HIT1_PRIORITY);
    reader_u8!(hit2_priority, HIT2_PRIORITY);
    reader_u8!(hit3_priority, HIT3_PRIORITY);

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn damage_level(&self) -> DamageLevel {
        DamageLevel(self.row.offset(off::DAMAGE_LEVEL as isize).read::<u8>())
    }

    // -- s8 block --
    reader_u8!(atk_attribute, ATK_ATTRIBUTE);
    reader_u8!(sp_attribute, SP_ATTRIBUTE);
    reader_u8!(atk_type, ATK_TYPE);
    reader_u8!(atk_material, ATK_MATERIAL);
    reader_u8!(atk_size, ATK_SIZE);
    reader_u8!(def_material, DEF_MATERIAL);
    reader_u8!(def_sfx_material, DEF_SFX_MATERIAL);

    // -- u8 flags --
    reader_u8!(hit_source_type, HIT_SOURCE_TYPE);
    reader_u8!(throw_flag, THROW_FLAG);

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn flags_a(&self) -> FlagsA {
        FlagsA(self.row.offset(off::FLAGS_A as isize).read::<u8>())
    }

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn flags_b(&self) -> FlagsB {
        FlagsB(self.row.offset(off::FLAGS_B as isize).read::<u8>())
    }

    reader_u8!(atk_pow_for_sfx_se, ATK_POW_FOR_SFX_SE);
    reader_u8!(atk_dir_for_sfx_se, ATK_DIR_FOR_SFX_SE);
    reader_u8!(regainable_slot_id, REGAINABLE_SLOT_ID);

    // -- s32 audio/vfx block --
    reader_s32!(death_cause_id, DEATH_CAUSE_ID);
    reader_s32!(decal_id1, DECAL_ID1);
    reader_s32!(decal_id2, DECAL_ID2);
    reader_s32!(spawn_ai_sound_id, SPAWN_AI_SOUND_ID);
    reader_s32!(hit_ai_sound_id, HIT_AI_SOUND_ID);
    reader_s32!(rumble_id0, RUMBLE_ID0);
    reader_s32!(rumble_id1, RUMBLE_ID1);
    reader_s32!(rumble_id2, RUMBLE_ID2);
    reader_s32!(rumble_id3, RUMBLE_ID3);

    // -- Hit 4-15 radii --
    reader_f32!(hit4_radius, HIT4_RADIUS);
    reader_f32!(hit5_radius, HIT5_RADIUS);
    reader_f32!(hit6_radius, HIT6_RADIUS);
    reader_f32!(hit7_radius, HIT7_RADIUS);
    reader_f32!(hit8_radius, HIT8_RADIUS);
    reader_f32!(hit9_radius, HIT9_RADIUS);
    reader_f32!(hit10_radius, HIT10_RADIUS);
    reader_f32!(hit11_radius, HIT11_RADIUS);
    reader_f32!(hit12_radius, HIT12_RADIUS);
    reader_f32!(hit13_radius, HIT13_RADIUS);
    reader_f32!(hit14_radius, HIT14_RADIUS);
    reader_f32!(hit15_radius, HIT15_RADIUS);

    // -- Trailing block --
    reader_s16!(atk_dark_correction, ATK_DARK_CORRECTION);
    reader_s16!(atk_dark, ATK_DARK);
    reader_u8!(flags_c_raw, FLAGS_C);

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn docking_edge_reaction(&self) -> DockingEdgeReaction {
        DockingEdgeReaction::from_u8(
            self.row
                .offset(off::SET_DOCKING_EDGE_VAR_ID as isize)
                .read::<u8>(),
        )
    }

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn guard_behavior(&self) -> DockingEdgeReaction {
        DockingEdgeReaction::from_u8(
            self.row
                .offset(off::SET_GUARD_BEHAVIOR as isize)
                .read::<u8>(),
        )
    }

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn new_stamina_damage_1(&self) -> u16 {
        self.row
            .offset(off::NEW_STAMINA_DAMAGE_1 as isize)
            .read::<u16>()
    }

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn new_stamina_damage_2(&self) -> u16 {
        self.row
            .offset(off::NEW_STAMINA_DAMAGE_2 as isize)
            .read::<u16>()
    }

    /// # Safety
    /// See [`Self::sp_effect`].
    #[inline]
    pub unsafe fn blocked_posture_damage(&self) -> u16 {
        self.row
            .offset(off::BLOCKED_POSTURE_DAMAGE as isize)
            .read::<u16>()
    }

    reader_s32!(atk_element_correct_param_id, ATK_ELEMENT_CORRECT_PARAM_ID);

    // -- Convenience predicates --

    /// True iff this attack carries posture damage (i.e. can trigger
    /// deflect).  SPEC §4.1 deflect detection condition #1.
    ///
    /// # Safety
    /// See [`Self::sp_effect`].
    pub unsafe fn can_deflect(&self) -> bool {
        self.atk_stam() > 0
    }

    /// Collect non-zero spEffect IDs (to apply to the victim on hit).
    /// Returns a fixed-size array with trailing zero-slots; callers
    /// should stop on the first zero.
    ///
    /// # Safety
    /// See [`Self::sp_effect`].
    pub unsafe fn on_hit_sp_effects(&self) -> [i32; 5] {
        let mut out = [0i32; 5];
        let mut next = 0;
        for slot in 0..=4u8 {
            if let Some(id) = self.sp_effect(slot) {
                if id != 0 {
                    out[next] = id;
                    next += 1;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stride_fits_wiki_0x1d0() {
        // AtkParams.md's trailing field is `0x1CC` (f32 at 0x1CC..0x1D0).
        // Stride should be exactly 0x1D0.
        assert_eq!(off::ROW_STRIDE, 0x1D0);
    }

    #[test]
    fn key_offsets_match_published_positions() {
        assert_eq!(off::DAMAGE_LEVEL, 0x7A);
        assert_eq!(off::ATK_STAM, 0x60);
        assert_eq!(off::ATK_STAM_CORRECTION, 0x4E);
        assert_eq!(off::SET_DOCKING_EDGE_VAR_ID, 0x198);
        assert_eq!(off::BLOCKED_POSTURE_DAMAGE, 0x1AC);
    }

    #[test]
    fn flags_a_bit_semantics() {
        let f = FlagsA(0b0000_0001);
        assert!(f.disable_guard());
        assert!(!f.disable_stamina_attack());

        let f = FlagsA(0b1000_0100);
        assert!(f.disable_hit_speffect());
        assert!(f.is_disable_no_damage());
    }

    #[test]
    fn docking_edge_reaction_roundtrip() {
        for (v, expected) in [
            (1u8, DockingEdgeReaction::ComboInterrupted),
            (2, DockingEdgeReaction::ComboInterruptedAlt),
            (11, DockingEdgeReaction::ComboContinues),
            (99, DockingEdgeReaction::Other(99)),
        ] {
            assert_eq!(DockingEdgeReaction::from_u8(v), expected);
        }
    }

    #[test]
    fn damage_level_predicates() {
        assert!(DamageLevel(10).is_katana_drag());
        assert!(DamageLevel(9).is_airborne_launch());
        assert!(DamageLevel(6).is_pancake());
        assert!(DamageLevel(8).is_no_interrupt());
        assert!(DamageLevel(10).stuns_heavily());
        assert!(!DamageLevel(2).stuns_heavily());
    }
}
