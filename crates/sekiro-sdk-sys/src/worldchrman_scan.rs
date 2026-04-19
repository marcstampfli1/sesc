//! Runtime discovery of the `WorldChrMan` inner tables.
//!
//! We don't have reliable offsets for Sekiro's ChrSet arrays in any of
//! our imported research data, so we find them at runtime.
//!
//! The approach:
//!
//!   1. Read the Hero `ChrIns` pointer (well-known: `WorldChrMan + 0x88`).
//!   2. The first 8 bytes of any `ChrIns` are its vtable pointer, shared
//!      across every instance of the same class.
//!   3. Sweep the first few KB of the `WorldChrMan` struct looking for
//!      inner pointers.  For each inner ptr `p`, treat it as either:
//!      - a direct `ChrIns**` array (DS3/ER-style), or
//!      - a `ChrSet*` whose own `+0x18` points to a `ChrIns**` array.
//!   4. For each candidate array, read the first few slots and check
//!      whether they look like `ChrIns*` (non-null, vtable matches the
//!      Hero's, all pointers land in the same heap region).
//!   5. Score and return the best candidate.
//!
//! This runs once per session after the Hero pointer is captured.
//! Purely read-only; safe to call from the tick hook.

use crate::memory::RawPtr;

/// How much of the `WorldChrMan` struct we sweep for inner pointers.
/// Kept conservative so we don't read off the end of an allocation if
/// the struct turns out to be smaller than we guessed.  FromSoft ChrSet
/// pointers are normally in the first few hundred bytes.
const SCAN_BYTES: usize = 0x800;

/// How many candidate array slots we probe when validating a table.
const PROBE_SLOTS: usize = 8;

/// Common ChrSet-style intermediary offsets to try for the `ChrIns**`
/// pointer.  Matches DS3 / ER shapes observed in libsekiro analogues.
const CHRSET_INNER_OFFSETS: &[usize] = &[0x10, 0x18, 0x20, 0x28, 0x30, 0x38];

#[derive(Debug, Clone, Copy)]
pub struct ChrSetCandidate {
    /// Offset in `WorldChrMan` where the inner pointer lives.
    pub worldchr_offset: usize,
    /// True iff we went `WorldChrMan+off → struct+inner_off → array`.
    /// False iff we went `WorldChrMan+off → array` directly.
    pub via_intermediary: bool,
    /// When via_intermediary, the offset used inside the ChrSet struct.
    pub inner_offset: usize,
    /// The resolved array base (first slot is `array[0]`).
    pub array_addr: usize,
    /// How many consecutive non-null, vtable-matching slots we saw.
    pub matching_slots: u32,
    /// Confidence score, 0..=1.
    pub confidence: f32,
    /// True iff Hero ChrIns is among the first PROBE_SLOTS entries.
    pub contains_hero: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub hero_vtable: usize,
    pub hero_ptr: usize,
    pub candidates: Vec<ChrSetCandidate>,
}

impl ScanReport {
    /// Best candidate by confidence.
    pub fn best(&self) -> Option<&ChrSetCandidate> {
        self.candidates.iter().max_by(|a, b| {
            a.confidence
                .partial_cmp(&b.confidence)
                .unwrap_or(core::cmp::Ordering::Equal)
        })
    }
}

/// Heuristic heap-address predicate — Windows user-mode allocations land
/// in a wide range.  Rejects obvious garbage (small ints, code addresses
/// below 0x1_0000_0000_0000 are still plausible, but anything under 1 MB
/// or equal to specific sentinels is not a pointer).
fn looks_like_heap_ptr(p: usize) -> bool {
    // Below 1 MB → almost certainly a scalar field, not a pointer.
    // Above 0x0000_7fff_ffff_ffff → invalid on current x64 Windows.
    p >= 0x100_000 && p < 0x0000_7fff_ffff_ffff && (p & 0x7) == 0
}

/// Read a pointer at `(base + off)` with guarded deref.  Returns None on
/// address values that obviously aren't pointers so the caller doesn't
/// follow into a segfault.
unsafe fn read_ptr_guarded(base: RawPtr, off: usize) -> Option<usize> {
    let raw: usize = base.offset(off as isize).read();
    if looks_like_heap_ptr(raw) {
        Some(raw)
    } else {
        None
    }
}

/// Scan WorldChrMan for ChrSet/ChrIns-array candidates using Hero as
/// ground truth.
///
/// # Safety
/// Caller must have valid pointers:
/// - `world_chr_man` is a live WorldChrMan struct.
/// - `hero_chrins` is a live ChrIns (from `WorldChrMan+0x88`).
pub unsafe fn scan(world_chr_man: RawPtr, hero_chrins: RawPtr) -> ScanReport {
    let mut report = ScanReport::default();

    if world_chr_man.is_null() || hero_chrins.is_null() {
        return report;
    }

    // ChrIns vtable is at the very start.  Every ChrIns of the same
    // game-class shares this pointer.
    let hero_vtable: usize = hero_chrins.offset(0).read();
    report.hero_vtable = hero_vtable;
    report.hero_ptr = hero_chrins.0;
    if !looks_like_heap_ptr(hero_vtable) {
        return report;
    }

    // Sweep WorldChrMan for inner pointers.
    for off in (0..SCAN_BYTES).step_by(8) {
        let Some(inner_ptr) = read_ptr_guarded(world_chr_man, off) else {
            continue;
        };
        if inner_ptr == hero_chrins.0 {
            // Skip the direct Hero slot at +0x88 — that's not a ChrSet.
            continue;
        }

        // Case A: inner_ptr is itself a ChrIns** array head.
        if let Some(c) = try_array(inner_ptr, hero_vtable, hero_chrins.0, off, None) {
            report.candidates.push(c);
        }

        // Case B: inner_ptr is a ChrSet struct with an inner pointer.
        for &chrset_off in CHRSET_INNER_OFFSETS {
            let probe = RawPtr(inner_ptr);
            let Some(array_ptr) = read_ptr_guarded(probe, chrset_off) else {
                continue;
            };
            if let Some(c) =
                try_array(array_ptr, hero_vtable, hero_chrins.0, off, Some(chrset_off))
            {
                report.candidates.push(c);
            }
        }
    }

    // Sort descending by confidence for log readability.
    report.candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    // Keep the top 10 to avoid spamming the log.
    report.candidates.truncate(10);

    report
}

/// Probe a candidate `ChrIns**` array.  Returns a scored candidate if
/// the first few slots look like ChrIns pointers sharing Hero's vtable.
unsafe fn try_array(
    array_addr: usize,
    hero_vtable: usize,
    hero_ptr: usize,
    worldchr_offset: usize,
    inner_offset: Option<usize>,
) -> Option<ChrSetCandidate> {
    if !looks_like_heap_ptr(array_addr) {
        return None;
    }
    let array_base = RawPtr(array_addr);

    let mut matching = 0u32;
    let mut nulls_seen = 0u32;
    let mut contains_hero = false;
    for i in 0..PROBE_SLOTS {
        let slot_off = i * 8;
        let slot_val: usize = array_base.offset(slot_off as isize).read();
        if slot_val == 0 {
            // Null-terminated arrays are common — a trailing null doesn't
            // invalidate earlier matches.  Count and keep going.
            nulls_seen += 1;
            continue;
        }
        if !looks_like_heap_ptr(slot_val) {
            return None;
        }
        // Dereference and compare vtable.
        let candidate_ptr = RawPtr(slot_val);
        let vtbl: usize = candidate_ptr.offset(0).read();
        if vtbl == hero_vtable {
            matching += 1;
            if slot_val == hero_ptr {
                contains_hero = true;
            }
        } else if !looks_like_heap_ptr(vtbl) {
            // First 8 bytes aren't a pointer → not a ChrIns.
            return None;
        }
    }

    if matching == 0 {
        return None;
    }

    // Confidence: matching_slots / probed.  Bonus if Hero is in the array.
    let base_score = matching as f32 / PROBE_SLOTS as f32;
    let hero_bonus = if contains_hero { 0.2 } else { 0.0 };
    let _ = nulls_seen;
    let confidence = (base_score + hero_bonus).min(1.0);

    Some(ChrSetCandidate {
        worldchr_offset,
        via_intermediary: inner_offset.is_some(),
        inner_offset: inner_offset.unwrap_or(0),
        array_addr,
        matching_slots: matching,
        confidence,
        contains_hero,
    })
}
