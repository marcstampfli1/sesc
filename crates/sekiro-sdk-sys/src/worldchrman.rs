//! `WorldChrMan` — the table holding every loaded `ChrIns`.
//!
//! **P0 gap #2** (SPEC §3.5). On DS3/ER this is an array-of-pointers
//! with a terminator; Sekiro is assumed to follow the same shape and is
//! validated by walking until a null/sentinel and verifying the player
//! entity is among the pointers yielded.

use crate::chrins::{ChrInsLayout, ChrInsSnapshot, read_snapshot};
use crate::memory::RawPtr;

/// Runtime layout of the `WorldChrMan` table.
#[derive(Debug, Clone, Copy)]
pub struct WorldChrManLayout {
    /// Offset from the `WorldChrMan` base to the `ChrIns**` table pointer.
    pub table_ptr: usize,
    /// Offset from the base to the count field (if the table is array+count).
    /// Set to [`crate::chrins::UNRESOLVED`] if this version uses a null-terminator instead.
    pub table_count: usize,
    /// Max entries to walk as a safety bound if no count/terminator is found.
    pub max_walk: usize,
}

impl WorldChrManLayout {
    /// Sensible defaults for a walker that terminates on null.  Offsets
    /// remain unresolved until validated against the running game.
    pub const fn unresolved() -> Self {
        Self {
            table_ptr: crate::chrins::UNRESOLVED,
            table_count: crate::chrins::UNRESOLVED,
            max_walk: 512,
        }
    }
}

/// Iterator over `ChrIns*` pointers in `WorldChrMan`.
///
/// # Safety
/// Constructed only from a validated `WorldChrMan` pointer; the game must
/// not be concurrently despawning entities (run from the tick hook, not
/// from an async thread).
pub struct ChrInsIter {
    table: RawPtr,
    index: usize,
    count: usize,
    max: usize,
}

impl ChrInsIter {
    /// # Safety
    /// See type-level docs.
    pub unsafe fn from_world(world_ptr: RawPtr, layout: &WorldChrManLayout) -> Self {
        if world_ptr.is_null() || layout.table_ptr == crate::chrins::UNRESOLVED {
            return Self { table: RawPtr::NULL, index: 0, count: 0, max: 0 };
        }
        let table_addr: usize = world_ptr.offset(layout.table_ptr as isize).read();
        let count = if layout.table_count != crate::chrins::UNRESOLVED {
            let c: u32 = world_ptr.offset(layout.table_count as isize).read();
            c as usize
        } else {
            usize::MAX // null-terminated walk
        };
        Self {
            table: RawPtr(table_addr),
            index: 0,
            count,
            max: layout.max_walk,
        }
    }
}

impl Iterator for ChrInsIter {
    type Item = RawPtr;
    fn next(&mut self) -> Option<RawPtr> {
        if self.table.is_null() || self.index >= self.max || self.index >= self.count {
            return None;
        }
        let slot = self.table.offset((self.index * core::mem::size_of::<usize>()) as isize);
        let entry: usize = unsafe { slot.read() };
        self.index += 1;
        if entry == 0 {
            return None;
        }
        Some(RawPtr(entry))
    }
}

/// Snapshot every currently-loaded character.  Performs one `ChrIns` read
/// per entry; use sparingly (once per tick is plenty).
///
/// # Safety
/// See [`ChrInsIter`].
pub unsafe fn snapshot_all(
    world_ptr: RawPtr,
    wcm: &WorldChrManLayout,
    chrins: &ChrInsLayout,
) -> Vec<ChrInsSnapshot> {
    ChrInsIter::from_world(world_ptr, wcm)
        .map(|p| read_snapshot(p, chrins))
        .collect()
}
