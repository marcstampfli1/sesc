//! Module-memory access and pointer-chain resolution.

use std::slice;

use anyhow::{Context, Result};

/// Raw pointer newtype; kept `usize` so math on offsets is explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawPtr(pub usize);

impl RawPtr {
    pub const NULL: RawPtr = RawPtr(0);

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// # Safety
    /// Caller must ensure `self` points to a valid, aligned, fully-initialised
    /// `T` that will not be concurrently mutated for the lifetime of the
    /// returned reference. Sekiro memory is owned by the game; treat these
    /// reads as snapshots of an unstable world.
    #[inline]
    pub unsafe fn read<T: Copy>(&self) -> T {
        core::ptr::read(self.0 as *const T)
    }

    /// # Safety
    /// As [`Self::read`], and caller must additionally ensure that writing
    /// through this pointer is safe given the game's current state.
    #[inline]
    pub unsafe fn write<T: Copy>(&self, v: T) {
        core::ptr::write(self.0 as *mut T, v);
    }

    #[inline]
    pub fn offset(self, by: isize) -> RawPtr {
        RawPtr((self.0 as isize).wrapping_add(by) as usize)
    }
}

/// A chain of offsets applied via repeated dereference, mirroring the
/// `*(*(*base + a) + b) + c` pattern documented in OSINT §1.1.
///
/// The last element is *added but not dereferenced* — it's the offset
/// within the final struct.
#[derive(Debug, Clone)]
pub struct PtrChain {
    pub base: RawPtr,
    pub offsets: Vec<isize>,
}

impl PtrChain {
    pub fn new(base: RawPtr, offsets: impl Into<Vec<isize>>) -> Self {
        Self { base, offsets: offsets.into() }
    }

    /// Resolve the chain to a final pointer.  Any null deref along the way
    /// returns `RawPtr::NULL`.
    ///
    /// Semantics match `libsekiro`'s pointer chains: `self.base` is the
    /// *address of a pointer variable* (a static-memory slot that holds
    /// the first node pointer).  The chain:
    ///
    /// 1. Reads `*self.base` to obtain the root node pointer.
    /// 2. For every intermediate offset, adds the offset and
    ///    dereferences again.
    /// 3. The **last** offset is added but not dereferenced — the
    ///    returned pointer addresses the field itself.
    ///
    /// Empty offset list: returns the root pointer (after one deref).
    ///
    /// # Safety
    /// Every intermediate pointer must be a valid readable pointer in the
    /// game process. Resolve each frame; do not cache across frames.
    pub unsafe fn resolve(&self) -> RawPtr {
        if self.base.is_null() {
            return RawPtr::NULL;
        }
        // Step 1: read the symbol itself to get the root node.
        let root: usize = self.base.read();
        let mut p = RawPtr(root);
        if self.offsets.is_empty() {
            return p;
        }
        for (idx, &off) in self.offsets.iter().enumerate() {
            if p.is_null() {
                return RawPtr::NULL;
            }
            if idx + 1 == self.offsets.len() {
                // Last offset: add to get the field address; don't deref.
                return p.offset(off);
            }
            // Intermediate: add the offset, then dereference.
            let next_addr: usize = p.offset(off).read();
            p = RawPtr(next_addr);
        }
        p
    }
}

/// Handle on a loaded module (our own process only, by design).
#[derive(Debug, Clone, Copy)]
pub struct Module {
    pub base: usize,
    pub size: usize,
}

impl Module {
    /// Byte-slice view of the module's image bytes.
    ///
    /// # Safety
    /// Module must be loaded and `size` accurate.
    pub unsafe fn as_bytes(&self) -> &'static [u8] {
        slice::from_raw_parts(self.base as *const u8, self.size)
    }

    /// Resolve an RVA to an absolute address.
    #[inline]
    pub fn rva(&self, rva: usize) -> RawPtr {
        RawPtr(self.base + rva)
    }
}

#[cfg(target_os = "windows")]
pub fn find_current_module(name: &str) -> Result<Module> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut name_c = name.to_string();
    name_c.push('\0');
    let h: HMODULE = unsafe { GetModuleHandleA(windows::core::PCSTR(name_c.as_ptr()))? };
    if h.is_invalid() {
        anyhow::bail!("module not loaded: {name}");
    }
    let mut info = MODULEINFO::default();
    let ok = unsafe {
        GetModuleInformation(
            GetCurrentProcess(),
            h,
            &mut info as *mut _,
            core::mem::size_of::<MODULEINFO>() as u32,
        )
    };
    ok.ok().context("GetModuleInformation failed")?;
    Ok(Module {
        base: info.lpBaseOfDll as usize,
        size: info.SizeOfImage as usize,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn find_current_module(_name: &str) -> Result<Module> {
    anyhow::bail!("find_current_module: windows-only")
}
