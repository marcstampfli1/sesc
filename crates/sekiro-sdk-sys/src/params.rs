//! Live param-table access — AtkParam, SpEffectParam, BehaviorParam.
//!
//! **P0 gap #5** (SPEC §3.6). The runtime param table sits somewhere in
//! game memory; its exact base is resolved via Yapped/Cielos CE table.
//! Row format is `(header, row_data[count])` with fixed row-stride per
//! param type.

use crate::memory::RawPtr;

/// Paramdex-style row-stride constants. Values are approximations from
/// the wiki field counts; exact stride is verified at runtime by reading
/// a known row (e.g. AtkParam row for a distinctive attack) and matching
/// field values.
pub mod stride {
    /// AtkParam stride from `AtkParams.md` (all fields 0x1CC+).
    pub const ATK_PARAM: usize = 0x1D0;
    /// SpEffectParam stride (~3297 documented fields, mix of small types).
    pub const SP_EFFECT_PARAM: usize = 0x2B0;
    /// BehaviorParam stride (~3009 fields, heavy u8).
    pub const BEHAVIOR_PARAM: usize = 0x40;
}

/// Runtime handle to a live param table.
#[derive(Debug, Clone, Copy)]
pub struct ParamTable {
    /// Base pointer to the first row.
    pub rows: RawPtr,
    /// Number of rows in the table.
    pub count: u32,
    /// Byte-stride per row.
    pub stride: usize,
    /// Parallel id array: `rows_id[i]` is the param ID of `rows[i]`.
    /// On FromSoft params the id array is sorted ascending.
    pub ids: RawPtr,
}

impl ParamTable {
    /// Binary-search for a row by ID.  Returns the base pointer of the row.
    ///
    /// # Safety
    /// `self.ids` and `self.rows` must be live, well-formed, and
    /// `stride`-consistent for the current frame.
    pub unsafe fn row(&self, id: u32) -> Option<RawPtr> {
        if self.ids.is_null() || self.rows.is_null() || self.count == 0 {
            return None;
        }
        // Linear first — param tables have O(thousands) entries and the
        // binary-search layout is only reliable once the ids array is
        // validated as sorted.
        for i in 0..self.count as usize {
            let slot = self
                .ids
                .offset((i * core::mem::size_of::<u32>()) as isize);
            let rid: u32 = slot.read();
            if rid == id {
                let row = self
                    .rows
                    .offset((i * self.stride) as isize);
                return Some(row);
            }
        }
        None
    }
}

/// The set of param tables the mod needs live access to.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParamIndex {
    pub atk_param: Option<ParamTable>,
    pub atk_param_pc: Option<ParamTable>,
    pub atk_param_npc: Option<ParamTable>,
    pub sp_effect_param: Option<ParamTable>,
    pub behavior_param: Option<ParamTable>,
    pub behavior_param_pc: Option<ParamTable>,
    pub npc_param: Option<ParamTable>,
}
