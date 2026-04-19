//! Live param-table access via `SoloParamRepository`.
//!
//! Closes **P0 gap #5** (SPEC §11): the runtime param-table anchor.
//!
//! Walk formula (from ElaDiDu Cheat Engine table, ported from
//! `ct_aa_scripts.json`'s `paramUtils` + `Fill Param Data` scripts):
//!
//! ```text
//! repo         = *(sekiro.exe + SoloParamRepository_RVA)
//! param_record = *(repo + 0x78 + 0x48 * index)
//! param_data   = *(*(param_record + 0x70) + 0x70)
//! row_count    = u16 at param_data + 0x0A
//! rows[i] = {
//!     id:     u32 at param_data + 0x40 + 0x18 * i
//!     offset: u32 at param_data + 0x48 + 0x18 * i
//! }
//! row_data    = param_data + rows[i].offset
//! ```
//!
//! Stride-per-entry is 0x18 (24 bytes): each entry in the index array
//! has `(id: u32 at +0, offset: u32 at +8, extra: u64 at +16)`.
//!
//! Param-name → index table is 1.06 only.  Ground truth: ElaDiDu CT.

use crate::memory::RawPtr;
use crate::offsets::BaseAddrs;

/// Canonical param-table index (per the `SoloParamRepository` array
/// indexing used by the CE table).  v1.06 authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ParamKind {
    EquipParamWeapon = 0,
    EquipParamProtector = 1,
    EquipParamAccessory = 2,
    EquipParamGoods = 3,
    ReinforceParamWeapon = 4,
    ReinforceParamProtector = 5,
    NpcParam = 6,
    AtkParamNpc = 7,
    AtkParamPc = 8,
    NpcThinkParam = 9,
    ObjectParam = 10,
    Bullet = 11,
    BehaviorParam = 12,
    BehaviorParamPc = 13,
    Magic = 14,
    SpEffectParam = 15,
    SpEffectVfxParam = 16,
    TalkParam = 17,
    MenuColorTableParam = 18,
    ItemLotParam = 19,
    MoveParam = 20,
    CharaInitParam = 21,
    EquipMtrlSetParam = 22,
    FaceGenParam = 23,
    FaceParam = 24,
    FaceRangeParam = 25,
    RagdollParam = 26,
}

impl ParamKind {
    /// Human-friendly name used in logs.
    pub const fn name(self) -> &'static str {
        match self {
            ParamKind::EquipParamWeapon => "EquipParamWeapon",
            ParamKind::EquipParamProtector => "EquipParamProtector",
            ParamKind::EquipParamAccessory => "EquipParamAccessory",
            ParamKind::EquipParamGoods => "EquipParamGoods",
            ParamKind::ReinforceParamWeapon => "ReinforceParamWeapon",
            ParamKind::ReinforceParamProtector => "ReinforceParamProtector",
            ParamKind::NpcParam => "NpcParam",
            ParamKind::AtkParamNpc => "AtkParam_Npc",
            ParamKind::AtkParamPc => "AtkParam_Pc",
            ParamKind::NpcThinkParam => "NpcThinkParam",
            ParamKind::ObjectParam => "ObjectParam",
            ParamKind::Bullet => "Bullet",
            ParamKind::BehaviorParam => "BehaviorParam",
            ParamKind::BehaviorParamPc => "BehaviorParam_PC",
            ParamKind::Magic => "Magic",
            ParamKind::SpEffectParam => "SpEffectParam",
            ParamKind::SpEffectVfxParam => "SpEffectVfxParam",
            ParamKind::TalkParam => "TalkParam",
            ParamKind::MenuColorTableParam => "MenuColorTableParam",
            ParamKind::ItemLotParam => "ItemLotParam",
            ParamKind::MoveParam => "MoveParam",
            ParamKind::CharaInitParam => "CharaInitParam",
            ParamKind::EquipMtrlSetParam => "EquipMtrlSetParam",
            ParamKind::FaceGenParam => "FaceGenParam",
            ParamKind::FaceParam => "FaceParam",
            ParamKind::FaceRangeParam => "FaceRangeParam",
            ParamKind::RagdollParam => "RagdollParam",
        }
    }

    pub const fn index(self) -> u32 {
        self as u32
    }
}

/// Offsets within the paramrepo walk formula.
pub mod off {
    pub const RECORDS_START: usize = 0x78;
    pub const RECORD_STRIDE: usize = 0x48;
    pub const RECORD_TO_INNER_A: usize = 0x70;
    pub const INNER_A_TO_PARAM_DATA: usize = 0x70;
    pub const ROW_COUNT: usize = 0x0A;
    pub const ROW_IDS: usize = 0x40;
    pub const ROW_OFFSETS: usize = 0x48;
    pub const ROW_STRIDE: usize = 0x18;
}

/// Handle on a single live param table.
#[derive(Debug, Clone, Copy)]
pub struct ParamTable {
    pub kind: ParamKind,
    /// Base address of the param data blob.  Row ids live at
    /// `base + 0x40`, row offsets at `base + 0x48`, each entry 0x18
    /// stride; row data lives at `base + row_offset`.
    pub base: RawPtr,
}

impl ParamTable {
    /// # Safety
    /// `self.base` must be live for this frame.
    #[inline]
    pub unsafe fn row_count(&self) -> u16 {
        self.base.offset(off::ROW_COUNT as isize).read::<u16>()
    }

    /// Iterate every `(id, row_pointer)` in the param table.
    ///
    /// # Safety
    /// `self.base` + param backing memory must be live.
    pub unsafe fn rows(&self) -> RowIter {
        RowIter {
            base: self.base,
            count: self.row_count() as usize,
            i: 0,
        }
    }

    /// Binary-ish lookup (FromSoft tables are sorted by ID, but we
    /// linear-scan for robustness).
    ///
    /// # Safety
    /// See [`Self::rows`].
    pub unsafe fn row(&self, id: u32) -> Option<RawPtr> {
        self.rows().find(|(rid, _)| *rid == id).map(|(_, p)| p)
    }
}

/// Iterator over `(id, row_ptr)` pairs inside a `ParamTable`.
pub struct RowIter {
    base: RawPtr,
    count: usize,
    i: usize,
}

impl Iterator for RowIter {
    type Item = (u32, RawPtr);
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.count {
            return None;
        }
        let i = self.i;
        self.i += 1;
        unsafe {
            let ids_slot = self
                .base
                .offset((off::ROW_IDS + off::ROW_STRIDE * i) as isize);
            let off_slot = self
                .base
                .offset((off::ROW_OFFSETS + off::ROW_STRIDE * i) as isize);
            let id: u32 = ids_slot.read();
            let row_off: u32 = off_slot.read();
            let row_ptr = self.base.offset(row_off as isize);
            Some((id, row_ptr))
        }
    }
}

/// Resolve the `SoloParamRepository`-rooted param table for a given
/// kind.  Returns `None` if the repo isn't live yet (e.g. before
/// game load complete).
///
/// # Safety
/// `addrs.solo_param_repository_rva` must be valid for the loaded
/// Sekiro; module must be loaded.
pub unsafe fn open_param(
    solo_param_repository_rva: usize,
    module_base: usize,
    kind: ParamKind,
) -> Option<ParamTable> {
    let repo_sym = RawPtr(module_base.wrapping_add(solo_param_repository_rva));
    let repo: usize = repo_sym.read();
    if repo == 0 {
        return None;
    }
    let record_slot = repo + off::RECORDS_START + off::RECORD_STRIDE * (kind.index() as usize);
    let record: usize = RawPtr(record_slot).read();
    if record == 0 {
        return None;
    }
    let inner_a: usize = RawPtr(record + off::RECORD_TO_INNER_A).read();
    if inner_a == 0 {
        return None;
    }
    let param_data: usize = RawPtr(inner_a + off::INNER_A_TO_PARAM_DATA).read();
    if param_data == 0 {
        return None;
    }
    Some(ParamTable {
        kind,
        base: RawPtr(param_data),
    })
}

/// Default SoloParamRepository RVA for v1.06.  Falls back to this when
/// the `BaseAddrs` table doesn't have an explicit entry.
pub const SOLO_PARAM_REPOSITORY_RVA_V1_06: usize = 0x3d978b0;

/// Convenience: pull the SoloParamRepository RVA off the base-address
/// table (if present) or the v1.06 constant otherwise.
pub fn rva(addrs: &BaseAddrs) -> usize {
    let _ = addrs; // BaseAddrs doesn't carry SoloParamRepository yet;
                    // scanner (`natives::Natives::scan`) supplies it.
    SOLO_PARAM_REPOSITORY_RVA_V1_06
}

/// Sample a few key param-table rows for a liveness log.
///
/// # Safety
/// See [`open_param`].
pub unsafe fn sample_param_summary(
    solo_param_repository_rva: usize,
    module_base: usize,
) -> Vec<(ParamKind, Option<u16>)> {
    let kinds = [
        ParamKind::AtkParamPc,
        ParamKind::AtkParamNpc,
        ParamKind::SpEffectParam,
        ParamKind::BehaviorParam,
        ParamKind::NpcParam,
        ParamKind::ItemLotParam,
    ];
    kinds
        .into_iter()
        .map(|k| {
            let table = open_param(solo_param_repository_rva, module_base, k);
            let count = table.map(|t| t.row_count());
            (k, count)
        })
        .collect()
}

/// Look up one AtkParam row by ID.  Tries `AtkParam_Pc` first (player-
/// space attacks); falls back to `AtkParam_Npc` (enemy-space).
///
/// # Safety
/// See [`open_param`].
pub unsafe fn atk_param_row(
    solo_param_repository_rva: usize,
    module_base: usize,
    atk_param_id: u32,
) -> Option<crate::memory::RawPtr> {
    if let Some(t) = open_param(solo_param_repository_rva, module_base, ParamKind::AtkParamPc) {
        if let Some(row) = t.row(atk_param_id) {
            return Some(row);
        }
    }
    if let Some(t) = open_param(solo_param_repository_rva, module_base, ParamKind::AtkParamNpc) {
        if let Some(row) = t.row(atk_param_id) {
            return Some(row);
        }
    }
    None
}

/// Look up one SpEffectParam row by ID.
///
/// # Safety
/// See [`open_param`].
pub unsafe fn speffect_param_row(
    solo_param_repository_rva: usize,
    module_base: usize,
    speffect_id: u32,
) -> Option<crate::memory::RawPtr> {
    open_param(solo_param_repository_rva, module_base, ParamKind::SpEffectParam)
        .and_then(|t| t.row(speffect_id))
}
