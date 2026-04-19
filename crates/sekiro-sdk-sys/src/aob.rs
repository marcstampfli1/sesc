//! AOB (array-of-bytes) scanner with wildcard + RIP-relative support.
//!
//! `aob_indirect_twice`: scan → read 4-byte RIP-relative from `scan + k` →
//! dereference the resulting pointer.  Matches the pattern used throughout
//! `libsekiro::codegen::aob_scans.rs` (OSINT §1.2).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("pattern not found in module")]
    NotFound,
    #[error("malformed pattern: {0}")]
    BadPattern(String),
    #[error("read past end of module")]
    OutOfBounds,
}

/// One byte of an AOB pattern.  `Wild` matches any byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternByte {
    Exact(u8),
    Wild,
}

#[derive(Debug, Clone)]
pub struct AobPattern {
    pub bytes: Vec<PatternByte>,
}

impl AobPattern {
    /// Parse a space-separated AOB string, e.g. `"48 8B 05 ?? ?? ?? ?? 48 63 C9"`.
    /// `??` or `?` match any byte.
    pub fn parse(spec: &str) -> Result<Self, ScanError> {
        let mut bytes = Vec::new();
        for tok in spec.split_whitespace() {
            match tok {
                "?" | "??" => bytes.push(PatternByte::Wild),
                hex if hex.len() == 2 => {
                    let b = u8::from_str_radix(hex, 16)
                        .map_err(|_| ScanError::BadPattern(tok.to_string()))?;
                    bytes.push(PatternByte::Exact(b));
                }
                _ => return Err(ScanError::BadPattern(tok.to_string())),
            }
        }
        if bytes.is_empty() {
            return Err(ScanError::BadPattern("empty pattern".into()));
        }
        Ok(Self { bytes })
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Linear-scan `haystack` for the first byte-sequence that matches this
    /// pattern. Returns the offset into `haystack`.
    pub fn scan(&self, haystack: &[u8]) -> Result<usize, ScanError> {
        if haystack.len() < self.bytes.len() {
            return Err(ScanError::NotFound);
        }
        let end = haystack.len() - self.bytes.len();
        'outer: for i in 0..=end {
            for (j, pb) in self.bytes.iter().enumerate() {
                if let PatternByte::Exact(b) = pb {
                    if haystack[i + j] != *b {
                        continue 'outer;
                    }
                }
            }
            return Ok(i);
        }
        Err(ScanError::NotFound)
    }
}

/// Read a 4-byte RIP-relative displacement at `base + offset` inside
/// `haystack` and resolve to the absolute offset inside the module.
///
/// `base` is the offset of the instruction that contains the 32-bit
/// displacement; `disp_offset` is how far into the instruction the
/// displacement starts; `instr_len` is the full length of the instruction
/// (since RIP points past the instruction end).
pub fn resolve_rip_relative(
    haystack: &[u8],
    base: usize,
    disp_offset: usize,
    instr_len: usize,
) -> Result<usize, ScanError> {
    let start = base + disp_offset;
    let end = start + 4;
    if end > haystack.len() {
        return Err(ScanError::OutOfBounds);
    }
    let disp = i32::from_le_bytes([
        haystack[start],
        haystack[start + 1],
        haystack[start + 2],
        haystack[start + 3],
    ]);
    let next_instr = (base + instr_len) as isize;
    let target = next_instr + disp as isize;
    if target < 0 {
        return Err(ScanError::OutOfBounds);
    }
    Ok(target as usize)
}

/// Scan + RIP-relative resolve in one shot ("aob_indirect" semantics).
///
/// Typical use: an instruction like `48 8B 05 ?? ?? ?? ??` (mov rax, [rip+disp])
/// where the three leading bytes are opcode+modrm and the next four are the
/// displacement.
pub fn scan_and_resolve(
    haystack: &[u8],
    pattern: &AobPattern,
    disp_offset: usize,
    instr_len: usize,
) -> Result<usize, ScanError> {
    let hit = pattern.scan(haystack)?;
    resolve_rip_relative(haystack, hit, disp_offset, instr_len)
}

/// Canonical Sekiro AOB patterns from OSINT §1.2.
///
/// Each entry: `(symbol-name, pattern, disp_offset, instr_len)`.  The
/// disp/instr-len pair encodes the "indirect" step used by `libsekiro`.
pub mod patterns {
    use super::AobPattern;

    pub fn quitout() -> AobPattern {
        // 48 8B 05 ?? ?? ?? ?? 48 63 C9 89 54 88 20 C3
        AobPattern::parse("48 8B 05 ?? ?? ?? ?? 48 63 C9 89 54 88 20 C3").unwrap()
    }
    pub fn render_world() -> AobPattern {
        AobPattern::parse("80 3D ?? ?? ?? ?? 00 0F 10 00 0F 11 45 D0").unwrap()
    }
    pub fn debug_render() -> AobPattern {
        AobPattern::parse("44 0F B6 3D ?? ?? ?? ?? 0F 29 74 24 20 0F 28 F1 E8").unwrap()
    }
    pub fn igt() -> AobPattern {
        AobPattern::parse(
            "48 8B 0D ?? ?? ?? ?? 0F 28 C6 F3 0F 59 05 ?? ?? ?? ?? \
             F3 48 0F 2C C0 01 81 ?? ?? ?? ??",
        )
        .unwrap()
    }
    pub fn player_position() -> AobPattern {
        AobPattern::parse(
            "48 83 3D ?? ?? ?? ?? 00 0F 84 ?? ?? ?? ?? F3 41 0F 10 47 78 F3 0F 5C C7",
        )
        .unwrap()
    }
    pub fn debug_flags() -> AobPattern {
        AobPattern::parse("80 3D ?? ?? ?? ?? 00 75 08 32 C0 48 83 C4 20").unwrap()
    }
    pub fn show_cursor() -> AobPattern {
        AobPattern::parse(
            "40 38 3D ?? ?? ?? ?? 0F B6 DB 0F 44 DF 84 DB 0F 94 C3 83 7D 40 FF",
        )
        .unwrap()
    }
    pub fn no_logo() -> AobPattern {
        AobPattern::parse(
            "74 30 48 8D 54 24 30 48 8B CD E8 ?? ?? ?? ?? 90 BB 01 00 00 00 \
             89 5C 24 20 44 0F B6 4E 04",
        )
        .unwrap()
    }
    pub fn font_patch() -> AobPattern {
        AobPattern::parse("48 8B FA 49 8B F0 48 8B D9").unwrap()
    }

    // From OSINT §1.2 (Sekiro-Debug-Patch HookSites.h):
    pub fn activate_debug_menu() -> AobPattern {
        AobPattern::parse(
            "C3 CC CC CC CC CC CC CC CC 32 C0 C3 CC CC CC CC CC CC CC CC CC CC CC CC CC B8",
        )
        .unwrap()
    }
    pub fn menu_draw_hook() -> AobPattern {
        AobPattern::parse("38 4C 8B C0 F3 0F 11 44 24 2C").unwrap()
    }
    pub fn enable_3_areas() -> AobPattern {
        AobPattern::parse(
            "48 83 C4 70 5B C3 CC CC 32 C0 C3 CC CC CC CC CC CC CC CC CC CC CC CC CC 32",
        )
        .unwrap()
    }
    pub fn enable_freeze_cam() -> AobPattern {
        AobPattern::parse("8B 83 E0 00 00 00 FF C8 83").unwrap()
    }

    // --- Symbol-base AOBs (from SEKIRO_OFFSETS.md Part A).
    // Each resolves to a static global pointer at a known module RVA.

    pub fn world_chr_man() -> AobPattern {
        AobPattern::parse("48 8B C6 48 89 05 ?? ?? ?? ?? 48 85 C0").unwrap()
    }
    pub fn world_chr_man_dbg() -> AobPattern {
        AobPattern::parse("49 8B C4 48 89 05 ?? ?? ?? ?? 48 8B CF").unwrap()
    }
    pub fn world_ai_manager() -> AobPattern {
        AobPattern::parse(
            "F3 0F 58 C6 0F 2F F8 ?? ?? 48 8B 0D ?? ?? ?? ?? 48 85 C9",
        )
        .unwrap()
    }
    pub fn game_man() -> AobPattern {
        AobPattern::parse("66 0F 42 C8 66 3B CE ?? ?? 48 8B 05").unwrap()
    }
    pub fn field_area() -> AobPattern {
        AobPattern::parse("48 3B C7 48 0F 44 C5 48 89 05").unwrap()
    }
    pub fn solo_param_repository() -> AobPattern {
        AobPattern::parse("48 89 1D ?? ?? ?? ?? 48 83 C4 50 5B C3").unwrap()
    }
    pub fn game_data() -> AobPattern {
        AobPattern::parse("0F B6 DA 4C 8B F1 40 32 FF 48 8B 05").unwrap()
    }
    pub fn event_man() -> AobPattern {
        AobPattern::parse("0F B6 D8 48 8B 0D ?? ?? ?? ?? 48 85 C9").unwrap()
    }
    pub fn event_flag_man() -> AobPattern {
        AobPattern::parse("41 8B F5 90 48 8B 0D ?? ?? ?? ?? 48 85 C9").unwrap()
    }
    pub fn lock_tgt_man() -> AobPattern {
        AobPattern::parse("48 8B 35 ?? ?? ?? ?? 49 8B D7 49 8B CE").unwrap()
    }
    pub fn damage_management() -> AobPattern {
        AobPattern::parse("48 8B 1D ?? ?? ?? ?? 48 8B F0 48 85 DB").unwrap()
    }
    pub fn map_item_man() -> AobPattern {
        AobPattern::parse("45 33 C0 33 D2 48 8B 0D").unwrap()
    }
    pub fn dlc() -> AobPattern {
        AobPattern::parse("48 8B 0D ?? ?? ?? ?? 44 8B CF 4C 8B C6 48 8B D3").unwrap()
    }
    pub fn debug_menu() -> AobPattern {
        AobPattern::parse(
            "40 53 48 81 EC 80 00 00 00 48 83 3D ?? ?? ?? ?? 00 4C 8B D2 48 8B D9",
        )
        .unwrap()
    }
    pub fn render_flags() -> AobPattern {
        AobPattern::parse(
            "40 53 56 41 56 48 83 EC 40 44 0F B6 35 ?? ?? ?? ?? 48 8B F1",
        )
        .unwrap()
    }
    pub fn targeting_draw_flags() -> AobPattern {
        AobPattern::parse("0F B7 47 5A A8 02 75 ?? 80 3D F4").unwrap()
    }
    pub fn rend_man() -> AobPattern {
        AobPattern::parse(
            "48 8B 1D ?? ?? ?? ?? 48 8B 5B 30 48 63 43 20 48 8B 4C C3 10",
        )
        .unwrap()
    }

    // --- Native function AOBs (from SEKIRO_MULTIPLAYER.md §1.2, all
    // from SEKIRO_OFFSETS.md Part B).  Offset is the number of bytes to
    // SUBTRACT from the scan match to get the function start (for
    // patterns that match mid-function) or add (for offsets listed as
    // positive).  Callers use `scan` + pointer arithmetic.

    pub fn fn_give_item_debug() -> AobPattern {
        AobPattern::parse("83 FA 1C 0F 87 BE 0E 00 00").unwrap()
    }
    pub fn fn_lua_do_string() -> AobPattern {
        AobPattern::parse(
            "40 53 48 83 EC 30 48 8B D9 48 89 54 24 20 48 83 C8 FF 48 FF C0",
        )
        .unwrap()
    }
    pub fn fn_get_flag() -> AobPattern {
        AobPattern::parse(
            "8B DA 74 ?? E8 ?? ?? ?? ?? 4C 8B C0 48 85 C0 74 ?? B8 D3 4D 62 10",
        )
        .unwrap()
    }
    pub fn fn_set_flag() -> AobPattern {
        AobPattern::parse("45 0F B6 E1 45 0F B6 E8 44 8B F2 48 8B E9").unwrap()
    }
    pub fn fn_draw_debug_mesh() -> AobPattern {
        AobPattern::parse(
            "48 8B D9 80 B9 F0 02 00 00 00 75 ?? 80 B9 F1 02 00 00 00",
        )
        .unwrap()
    }
    pub fn fn_give_item() -> AobPattern {
        AobPattern::parse(
            "48 89 58 18 0F 29 70 B8 45 33 ED 44 89 6C 24 44",
        )
        .unwrap()
    }
    pub fn fn_apply_effect() -> AobPattern {
        AobPattern::parse("44 89 70 9C 45 33 C0 4C 89 70 A0").unwrap()
    }
    pub fn fn_special_effect_delete_effect() -> AobPattern {
        AobPattern::parse(
            "48 83 EC 28 8B C2 48 8B 51 08 48 85 D2 74 ?? 90 39 42 58",
        )
        .unwrap()
    }
    pub fn fn_upgrade_prosthetics_menu() -> AobPattern {
        AobPattern::parse(
            "C7 43 18 17 00 00 00 C7 05 ?? ?? ?? ?? 17 00 00 00",
        )
        .unwrap()
    }
    pub fn fn_open_skills_menu() -> AobPattern {
        AobPattern::parse(
            "C7 43 18 18 00 00 00 C7 05 ?? ?? ?? ?? 18 00 00 00",
        )
        .unwrap()
    }
    pub fn fn_warp_next_stage_bonfire() -> AobPattern {
        AobPattern::parse("85 D2 ?? ?? ?? ?? ?? ?? B8 89 B5 F8 14 F7 EB").unwrap()
    }
    pub fn fn_add_experience() -> AobPattern {
        AobPattern::parse(
            "48 89 58 F0 48 8B D9 48 89 68 E8 33 ED 48 89 78 E0 41 8D 0C 10",
        )
        .unwrap()
    }
    pub fn fn_enlarge_unk_hkb_array() -> AobPattern {
        AobPattern::parse(
            "C1 E2 04 48 8B 48 58 48 8B 01 FF 50 08 48 8B E8 48 85 C0",
        )
        .unwrap()
    }
}
