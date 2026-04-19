//! Sekiro patch-version detection.
//!
//! Source: OSINT §1.1 base-address table (5 versions: 1.02, 1.03/04 byte-identical, 1.05, 1.06).
//! Detection strategy: inspect a discriminator AOB or module timestamp.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameVersion {
    V1_02,
    V1_03_04,
    V1_05,
    V1_06,
    Unknown,
}

impl fmt::Display for GameVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameVersion::V1_02 => f.write_str("1.02"),
            GameVersion::V1_03_04 => f.write_str("1.03/1.04"),
            GameVersion::V1_05 => f.write_str("1.05"),
            GameVersion::V1_06 => f.write_str("1.06"),
            GameVersion::Unknown => f.write_str("unknown"),
        }
    }
}

/// Observed image size of the loaded `sekiro.exe` module per patch.
/// Source: `SizeOfImage` from `GetModuleInformation` on the running
/// process (empirically verified for 1.06).  Values for earlier patches
/// are left as approximate placeholders since the exact figures are not
/// in hand; the primary version check is AOB-based and uses these only
/// as a cheap pre-filter.
const SIZE_V1_02: u32 = 0x04B5_0000;
const SIZE_V1_03_04: u32 = 0x04B5_8000;
const SIZE_V1_05: u32 = 0x042D_0000;
const SIZE_V1_06: u32 = 0x042D_8000; // = 70066176 (measured)

/// Detect the running Sekiro patch version.  Prefers AOB scanning
/// (definitive); falls back to image-size heuristic.
///
/// # Safety
/// `module_bytes` must point to the loaded module's image.
pub unsafe fn detect_version_live(module_bytes: &[u8], image_size: u32) -> GameVersion {
    if let Some(v) = detect_version_by_aob(module_bytes) {
        return v;
    }
    detect_version(image_size)
}

/// Scan a small, canonical AOB set; resolve their RVAs; match against
/// every known version's offset table.  Returns the version whose
/// table is consistent with every resolved RVA.
pub fn detect_version_by_aob(module_bytes: &[u8]) -> Option<GameVersion> {
    use crate::aob::{patterns, resolve_rip_relative};
    use crate::offsets::BaseAddrs;

    // Symbol → (pattern, disp_offset, instr_len, |BaseAddrs| -> usize).
    let probes: Vec<(&'static str, _, usize, usize, fn(&BaseAddrs) -> usize)> = vec![
        ("quitout",         patterns::quitout(),         3, 7, |a| a.quitout),
        ("render_world",    patterns::render_world(),    2, 7, |a| a.render_world),
        ("igt",             patterns::igt(),             3, 7, |a| a.igt),
        ("player_position", patterns::player_position(), 3, 8, |a| a.player_position),
    ];

    // Resolve each symbol from the module.
    let mut resolved: Vec<(&'static str, usize, fn(&BaseAddrs) -> usize)> = Vec::new();
    for (name, pat, disp, ilen, field) in &probes {
        let hit = match pat.scan(module_bytes) {
            Ok(o) => o,
            Err(_) => continue,
        };
        if *ilen == 0 {
            continue;
        }
        if let Ok(rva) = resolve_rip_relative(module_bytes, hit, *disp, *ilen) {
            resolved.push((name, rva, *field));
        }
    }
    if resolved.is_empty() {
        return None;
    }

    // Find the one version whose table agrees with every resolved symbol.
    for v in [
        GameVersion::V1_06,
        GameVersion::V1_05,
        GameVersion::V1_03_04,
        GameVersion::V1_02,
    ] {
        if let Some(addrs) = BaseAddrs::for_version(v) {
            if resolved
                .iter()
                .all(|(_, rva, field)| field(&addrs) == *rva)
            {
                return Some(v);
            }
        }
    }
    None
}

/// Detect the running Sekiro patch version from the loaded module's
/// base address and image size.
///
/// The definitive check is a version-specific byte at `debug_flags + N`
/// since the layout moved between 1.04 → 1.05. This function performs
/// a best-effort match; callers should validate using a known
/// pointer chain (e.g. player HP read).
pub fn detect_version(image_size: u32) -> GameVersion {
    // Nearest-match (tolerates small delta between point releases).
    let candidates = [
        (SIZE_V1_02, GameVersion::V1_02),
        (SIZE_V1_03_04, GameVersion::V1_03_04),
        (SIZE_V1_05, GameVersion::V1_05),
        (SIZE_V1_06, GameVersion::V1_06),
    ];

    let mut best: Option<(u32, GameVersion)> = None;
    for (size, ver) in candidates {
        let delta = size.abs_diff(image_size);
        match best {
            Some((prev, _)) if delta >= prev => {}
            _ => best = Some((delta, ver)),
        }
    }

    match best {
        // Require delta ≤ 1MB to count as a match.
        Some((delta, ver)) if delta < 0x10_0000 => ver,
        _ => GameVersion::Unknown,
    }
}
