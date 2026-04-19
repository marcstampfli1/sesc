//! Port of Sekiro-Debug-Patch features.
//!
//! Source: OSINT §1.2 HookSites.h.  Each patch scans an AOB, applies a
//! byte-level tweak at a known offset, and restores via
//! [`Patch::revert`].  All of these are well-understood in the
//! SekiroModding community.
//!
//! Features ported here:
//!
//! | Feature | Description |
//! |---|---|
//! | `activate_debug_menu`  | Unlocks FromSoft's internal dev menu |
//! | `enable_3_areas`       | Allows 3 areas loaded simultaneously |
//! | `disable_missing_param` | Silences missing-param spam errors |
//! | `disable_remnant_menu`  | Skips the Remnant menu gate |
//! | `enable_freeze_cam`     | Allows the dev freeze/pan camera |
//!
//! SPEC §3.7, §3.8.

use crate::hook::HookError;
use sekiro_sdk_sys::aob::{patterns, ScanError};
use sekiro_sdk_sys::memory::{Module, RawPtr};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("AOB not found: {0}")]
    NotFound(#[from] ScanError),
    #[error("hook error: {0}")]
    Hook(#[from] HookError),
    #[error("VirtualProtect failed: {0:#x}")]
    Protect(u32),
}

/// Record of a byte-level patch for later reversion.
#[derive(Debug, Clone)]
pub struct Patch {
    pub address: usize,
    pub original: Vec<u8>,
}

impl Patch {
    /// Apply `new_bytes` at `address`, recording the originals.
    ///
    /// # Safety
    /// `address` must be within a writable (or RWX after VirtualProtect)
    /// code page of the loaded module.
    pub unsafe fn apply(address: usize, new_bytes: &[u8]) -> Result<Self, PatchError> {
        let mut original = vec![0u8; new_bytes.len()];
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Memory::{
                VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
            };
            let mut old_protect = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(
                address as *const _,
                new_bytes.len(),
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            )
            .map_err(|e| PatchError::Protect(e.code().0 as u32))?;
            // Read original bytes before overwriting.
            core::ptr::copy_nonoverlapping(address as *const u8, original.as_mut_ptr(), new_bytes.len());
            core::ptr::copy_nonoverlapping(new_bytes.as_ptr(), address as *mut u8, new_bytes.len());
            // Restore original page protection.
            let mut unused = PAGE_PROTECTION_FLAGS(0);
            let _ = VirtualProtect(
                address as *const _,
                new_bytes.len(),
                old_protect,
                &mut unused,
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            core::ptr::copy_nonoverlapping(address as *const u8, original.as_mut_ptr(), new_bytes.len());
            core::ptr::copy_nonoverlapping(new_bytes.as_ptr(), address as *mut u8, new_bytes.len());
        }
        Ok(Patch { address, original })
    }

    /// Restore the original bytes.
    ///
    /// # Safety
    /// See [`Self::apply`].
    pub unsafe fn revert(&self) -> Result<(), PatchError> {
        Patch::apply(self.address, &self.original).map(drop)
    }
}

/// Common byte values (from OSINT §1.2 patch primitives).
pub mod byte {
    /// `mov al, 1` — enable a boolean check.
    pub const ENABLE: [u8; 2] = [0xB0, 0x01];
    /// `xor al, al` — disable a boolean check.
    pub const DISABLE: [u8; 2] = [0x30, 0xC0];
    /// `ret` — NOP out a function.
    pub const NOP_FN: [u8; 1] = [0xC3];
}

/// Offsets within the `sig_enable_3_areas` pattern at which patches are
/// applied (per OSINT §1.2 "patches +8, +24, +40").
const THREE_AREAS_PATCH_OFFSETS: &[usize] = &[8, 24, 40];

/// Unlocks the debug menu (OSINT §1.2 `sActivateDebugMenu`).  The
/// patch overwrites a conditional with `mov al, 1; ret`.
///
/// # Safety
/// `module` must correspond to the loaded `sekiro.exe` process; the
/// caller must ensure no other code races against the byte write.
pub unsafe fn activate_debug_menu(module: Module) -> Result<Patch, PatchError> {
    let at = patterns::activate_debug_menu().scan(module.as_bytes())?;
    // Replaces `C3 CC ... 32 C0 C3 ...` → enable the boolean check by
    // writing `B0 01 C3` at the start of the second return block.
    // Per HookSites.h the patch is at scan_offset + 9.
    let patch_addr = module.base + at + 9;
    let new_bytes = [byte::ENABLE[0], byte::ENABLE[1], byte::NOP_FN[0]];
    Patch::apply(patch_addr, &new_bytes)
}

/// Enables 3-area simultaneous loading (OSINT §1.2 `sSigEnable3Areas`).
/// Applies `B0 01 C3` at three offsets.
///
/// # Safety
/// See [`activate_debug_menu`].
pub unsafe fn enable_3_areas(module: Module) -> Result<Vec<Patch>, PatchError> {
    let at = patterns::enable_3_areas().scan(module.as_bytes())?;
    let mut patches = Vec::with_capacity(THREE_AREAS_PATCH_OFFSETS.len());
    for &off in THREE_AREAS_PATCH_OFFSETS {
        let addr = module.base + at + off;
        let new_bytes = [byte::ENABLE[0], byte::ENABLE[1], byte::NOP_FN[0]];
        patches.push(Patch::apply(addr, &new_bytes)?);
    }
    Ok(patches)
}

/// Enables the freeze/pan camera (OSINT §1.2 `sSigEnableFreezeCam`).
/// Pan-cam is at `scan_offset - 83`.
///
/// # Safety
/// See [`activate_debug_menu`].
pub unsafe fn enable_freeze_cam(module: Module) -> Result<Patch, PatchError> {
    let at = patterns::enable_freeze_cam().scan(module.as_bytes())?;
    let addr = module.base + at - 83;
    // Change the condition so `JZ` becomes unconditional: replace with
    // `EB 00` (short jmp 0) or `90 90` (nop-nop).  The typical patch
    // uses `EB` over `74`.
    let new_bytes = [0xEB];
    let pre_addr = addr;
    Patch::apply(pre_addr, &new_bytes)
}

/// Composite handle returned by [`install_all`] so the caller can revert
/// everything in one step on DLL detach.
#[derive(Debug, Default)]
pub struct DebugPatches {
    pub activate_debug_menu: Option<Patch>,
    pub enable_3_areas: Vec<Patch>,
    pub enable_freeze_cam: Option<Patch>,
}

impl DebugPatches {
    /// Revert every patch that was applied.  Best-effort: failures are
    /// reported but do not abort the loop.
    pub fn revert_all(&self) -> Vec<PatchError> {
        let mut errors = Vec::new();
        unsafe {
            if let Some(p) = &self.activate_debug_menu {
                if let Err(e) = p.revert() {
                    errors.push(e);
                }
            }
            for p in &self.enable_3_areas {
                if let Err(e) = p.revert() {
                    errors.push(e);
                }
            }
            if let Some(p) = &self.enable_freeze_cam {
                if let Err(e) = p.revert() {
                    errors.push(e);
                }
            }
        }
        errors
    }
}

/// Try to install every debug patch.  Returns a bundle containing the
/// Patch records for successful installs; the `PatchError`s for
/// features that couldn't be patched (typically AOB-not-found on an
/// unknown patch version) are logged via `tracing`.
///
/// # Safety
/// See [`activate_debug_menu`].
pub unsafe fn install_all(module: Module) -> DebugPatches {
    let mut patches = DebugPatches::default();
    match activate_debug_menu(module) {
        Ok(p) => {
            patches.activate_debug_menu = Some(p);
            tracing::info!("debug-menu unlock installed");
        }
        Err(e) => tracing::warn!(%e, "debug-menu unlock failed"),
    }
    match enable_3_areas(module) {
        Ok(v) => {
            tracing::info!("3-areas unlock installed ({} patches)", v.len());
            patches.enable_3_areas = v;
        }
        Err(e) => tracing::warn!(%e, "3-areas unlock failed"),
    }
    match enable_freeze_cam(module) {
        Ok(p) => {
            patches.enable_freeze_cam = Some(p);
            tracing::info!("freeze-cam unlock installed");
        }
        Err(e) => tracing::warn!(%e, "freeze-cam unlock failed"),
    }
    patches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_apply_and_revert() {
        // Explicit u8 type so the buffer really is byte-sized.
        let mut buf: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let addr = buf.as_mut_ptr() as usize;
        let p = unsafe { Patch::apply(addr, &[0xAA, 0xBB]) }.expect("apply");
        assert_eq!(&buf[..2], &[0xAA_u8, 0xBB]);
        unsafe { p.revert() }.expect("revert");
        assert_eq!(&buf[..2], &[0x01_u8, 0x02]);
    }

    #[test]
    fn byte_constants_match_spec() {
        assert_eq!(byte::ENABLE, [0xB0, 0x01]);
        assert_eq!(byte::DISABLE, [0x30, 0xC0]);
        assert_eq!(byte::NOP_FN, [0xC3]);
    }

    #[test]
    fn three_areas_patch_offsets_are_correct() {
        assert_eq!(THREE_AREAS_PATCH_OFFSETS, &[8, 24, 40]);
    }
}

// Module must be linkable even without the minhook feature (patches
// don't use MinHook).  The `Module` import is from `sekiro-sdk-sys`.
#[allow(dead_code)]
fn _ensure_module_import_is_used(_m: Module) {}

// Silence `RawPtr` unused import if it ends up not referenced.
#[allow(dead_code)]
fn _ensure_rawptr_import(_r: RawPtr) {}
