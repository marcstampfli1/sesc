//! Debug-flag setters — the 20-byte structure at `debug_flags`.
//!
//! SPEC §3.8, OSINT §1.1 debug-flag table.

use sekiro_sdk_sys::memory::RawPtr;
use sekiro_sdk_sys::offsets::DebugFlagOffsets;
use sekiro_sdk_sys::version::GameVersion;

/// Handle to the 20-byte debug-flag block at the known base.
#[derive(Debug, Clone, Copy)]
pub struct DebugFlags {
    pub base: RawPtr,
    pub offsets: DebugFlagOffsets,
}

impl DebugFlags {
    pub fn new(base: RawPtr, version: GameVersion) -> Self {
        Self {
            base,
            offsets: DebugFlagOffsets::for_version(version),
        }
    }

    /// # Safety
    /// `self.base` must point to the live debug-flag block.
    pub unsafe fn set_all_no_update_ai(&self, on: bool) {
        self.write_flag(self.offsets.all_no_update_ai, on);
    }

    /// # Safety
    /// See [`Self::set_all_no_update_ai`].
    pub unsafe fn set_all_no_damage(&self, on: bool) {
        self.write_flag(self.offsets.all_no_damage, on);
    }

    /// # Safety
    /// See [`Self::set_all_no_update_ai`].
    pub unsafe fn set_all_no_move(&self, on: bool) {
        self.write_flag(self.offsets.all_no_move, on);
    }

    /// # Safety
    /// See [`Self::set_all_no_update_ai`].
    pub unsafe fn set_player_no_dead(&self, on: bool) {
        self.write_flag(self.offsets.player_no_dead, on);
    }

    /// # Safety
    /// See [`Self::set_all_no_update_ai`].
    pub unsafe fn set_player_exterminate(&self, on: bool) {
        self.write_flag(self.offsets.player_exterminate, on);
    }

    #[inline]
    unsafe fn write_flag(&self, offset: isize, on: bool) {
        let v: u8 = if on { 1 } else { 0 };
        self.base.offset(offset).write::<u8>(v);
    }
}
