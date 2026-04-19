//! MinHook wrapper with a central registry for enable/disable on unload.
//!
//! MinHook is pulled in via FFI. Link against `MinHook.x64.lib` at DLL
//! link time (mod loader / build script is responsible). SPEC §3.7.

use parking_lot::Mutex;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HookError {
    #[error("MinHook init failed (MH_Initialize returned {0})")]
    InitFailed(i32),
    #[error("MH_CreateHook failed (status {0}) at target {1:#x}")]
    CreateFailed(i32, usize),
    #[error("MH_EnableHook failed (status {0}) at target {1:#x}")]
    EnableFailed(i32, usize),
    #[error("MH_RemoveHook failed (status {0})")]
    RemoveFailed(i32),
    #[error("hook not found: {0:#x}")]
    NotFound(usize),
    #[error("hook target conflicts with existing hook: {0:#x}")]
    Conflict(usize),
}

/// Opaque hook handle returned by the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hook(pub u64);

/// Return slot for MinHook's `MH_CreateHook` — the trampoline to the
/// original function. Cast back to the native fn signature at the call
/// site.
pub type TrampolinePtr = *mut core::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Entry {
    target: usize,
    detour: usize,
    trampoline: usize,
    enabled: bool,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

#[derive(Debug, Default)]
struct Registry {
    next_id: u64,
    hooks: HashMap<u64, Entry>,
    by_target: HashMap<usize, u64>,
}

/// Initialise MinHook.  Idempotent.
pub fn init() -> Result<(), HookError> {
    let mut guard = REGISTRY.lock();
    if guard.is_some() {
        return Ok(());
    }
    // SAFETY: MH_Initialize is thread-safe per MinHook docs.
    let status = unsafe { sys::MH_Initialize() };
    if status != 0 {
        return Err(HookError::InitFailed(status));
    }
    *guard = Some(Registry::default());
    Ok(())
}

/// Create and enable a function hook.  Returns a handle plus the
/// trampoline address to call the original.
pub fn create_hook(target: usize, detour: usize) -> Result<(Hook, TrampolinePtr), HookError> {
    let mut guard = REGISTRY.lock();
    let reg = guard.as_mut().expect("call hook::init first");
    if reg.by_target.contains_key(&target) {
        return Err(HookError::Conflict(target));
    }
    let mut tramp: *mut core::ffi::c_void = core::ptr::null_mut();
    let status = unsafe {
        sys::MH_CreateHook(
            target as *mut _,
            detour as *mut _,
            &mut tramp as *mut _,
        )
    };
    if status != 0 {
        return Err(HookError::CreateFailed(status, target));
    }
    let status = unsafe { sys::MH_EnableHook(target as *mut _) };
    if status != 0 {
        // Best-effort cleanup.
        unsafe { sys::MH_RemoveHook(target as *mut _) };
        return Err(HookError::EnableFailed(status, target));
    }
    reg.next_id = reg.next_id.wrapping_add(1);
    let id = reg.next_id;
    reg.hooks.insert(
        id,
        Entry {
            target,
            detour,
            trampoline: tramp as usize,
            enabled: true,
        },
    );
    reg.by_target.insert(target, id);
    Ok((Hook(id), tramp))
}

/// Disable + remove a hook.  Safe to call on a hook that's already
/// been removed.
pub fn remove(h: Hook) -> Result<(), HookError> {
    let mut guard = REGISTRY.lock();
    let reg = guard.as_mut().expect("registry not initialised");
    let entry = match reg.hooks.remove(&h.0) {
        Some(e) => e,
        None => return Ok(()),
    };
    reg.by_target.remove(&entry.target);
    if entry.enabled {
        let _ = unsafe { sys::MH_DisableHook(entry.target as *mut _) };
    }
    let status = unsafe { sys::MH_RemoveHook(entry.target as *mut _) };
    if status != 0 {
        return Err(HookError::RemoveFailed(status));
    }
    Ok(())
}

/// Remove every registered hook (on DLL unload).
pub fn remove_all() {
    let mut guard = REGISTRY.lock();
    if let Some(reg) = guard.as_mut() {
        for (_, e) in reg.hooks.drain() {
            if e.enabled {
                unsafe { sys::MH_DisableHook(e.target as *mut _) };
            }
            unsafe { sys::MH_RemoveHook(e.target as *mut _) };
        }
        reg.by_target.clear();
    }
    unsafe { sys::MH_Uninitialize() };
    *guard = None;
}

/// Read-only registry view for diagnostics.
#[derive(Debug, Clone)]
pub struct HookInfo {
    pub id: u64,
    pub target: usize,
    pub trampoline: usize,
    pub enabled: bool,
}

/// Enumerate every live hook for diagnostics.
pub fn list() -> Vec<HookInfo> {
    let guard = REGISTRY.lock();
    guard
        .as_ref()
        .map(|r| {
            r.hooks
                .iter()
                .map(|(id, e)| HookInfo {
                    id: *id,
                    target: e.target,
                    trampoline: e.trampoline,
                    enabled: e.enabled,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Lightweight wrapper exposing the registry as a Layer-2-ergonomic
/// type; internal state still lives in the static.
pub struct HookRegistry;

impl HookRegistry {
    pub fn init() -> Result<Self, HookError> {
        init()?;
        Ok(Self)
    }

    pub fn hook(
        &self,
        target: usize,
        detour: usize,
    ) -> Result<(Hook, TrampolinePtr), HookError> {
        create_hook(target, detour)
    }

    pub fn remove(&self, h: Hook) -> Result<(), HookError> {
        remove(h)
    }

    pub fn list(&self) -> Vec<HookInfo> {
        list()
    }
}

impl Drop for HookRegistry {
    fn drop(&mut self) {
        // Intentionally no-op; call `remove_all()` from DllMain DETACH
        // so we can surface errors.
    }
}

// MinHook FFI bindings.  Caller must link `MinHook.x64.lib`.
//
// Gated on the `minhook` feature so tests + type-check runs that don't
// have the lib on the link path still compile.  When the feature is
// disabled, every MH_* call returns a non-zero sentinel (`-1`) to mean
// "not available" — upstream still compiles, the hook layer simply
// can't install detours.
#[cfg(feature = "minhook")]
#[allow(non_snake_case)]
mod sys {
    use core::ffi::c_void;
    #[link(name = "MinHook.x64", kind = "static")]
    extern "system" {
        pub fn MH_Initialize() -> i32;
        pub fn MH_Uninitialize() -> i32;
        pub fn MH_CreateHook(
            target: *mut c_void,
            detour: *mut c_void,
            trampoline: *mut *mut c_void,
        ) -> i32;
        pub fn MH_RemoveHook(target: *mut c_void) -> i32;
        pub fn MH_EnableHook(target: *mut c_void) -> i32;
        pub fn MH_DisableHook(target: *mut c_void) -> i32;
    }
}

#[cfg(not(feature = "minhook"))]
#[allow(non_snake_case)]
mod sys {
    use core::ffi::c_void;
    pub unsafe fn MH_Initialize() -> i32 { -1 }
    pub unsafe fn MH_Uninitialize() -> i32 { 0 }
    pub unsafe fn MH_CreateHook(
        _t: *mut c_void,
        _d: *mut c_void,
        _tr: *mut *mut c_void,
    ) -> i32 { -1 }
    pub unsafe fn MH_RemoveHook(_t: *mut c_void) -> i32 { 0 }
    pub unsafe fn MH_EnableHook(_t: *mut c_void) -> i32 { -1 }
    pub unsafe fn MH_DisableHook(_t: *mut c_void) -> i32 { 0 }
}
