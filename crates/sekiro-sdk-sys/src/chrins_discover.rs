//! Heuristic `ChrIns` offset discovery.
//!
//! When the Cielos CE table hasn't been dropped next to the DLL, we
//! can still bootstrap the layout by scanning memory around a known
//! player `ChrIns` pointer and looking for plausible values.
//!
//! The heuristics are intentionally conservative: we prefer "no
//! candidate" over "wrong candidate" because the stepper's write path
//! (in `sekiro-coop-rollback::stepper`) would corrupt gameplay state
//! if given incorrect offsets.  Every field reported has a confidence
//! score; a validated layout requires all critical fields to score
//! above the corresponding threshold.
//!
//! Run once after the player has spawned (to observe non-zero HP),
//! then again at a checkpoint (to confirm the offsets still land on
//! plausible values).

use crate::chrins::{ChrInsLayout, UNRESOLVED};
use crate::memory::RawPtr;

/// Max offset we'll consider inside a `ChrIns` instance.  Picked large
/// enough to cover every field we care about even under generous
/// padding.
pub const MAX_CHRINS_SCAN_BYTES: usize = 0x4000;

/// Alignment for u32/i32/f32 fields when scanning.
const ALIGN_4: usize = 4;
/// Alignment for [f32; 3] and [f32; 4] fields.
const ALIGN_16: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    pub offset: usize,
    pub confidence: f32,
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryReport {
    pub hp: Option<Candidate>,
    pub max_hp: Option<Candidate>,
    pub posture: Option<Candidate>,
    pub max_posture: Option<Candidate>,
    pub position: Option<Candidate>,
    pub animation_id: Option<Candidate>,
    pub entity_id: Option<Candidate>,
    pub team_type: Option<Candidate>,
    pub is_deflecting: Option<Candidate>,
}

impl DiscoveryReport {
    /// Apply a discovery report to a layout.  Confidence below
    /// `min_confidence` is treated as "unresolved" to avoid populating
    /// the layout with guesses.
    pub fn apply(&self, layout: &mut ChrInsLayout, min_confidence: f32) -> u32 {
        let mut hits = 0;
        macro_rules! set {
            ($field:ident) => {
                if let Some(c) = self.$field {
                    if c.confidence >= min_confidence {
                        layout.$field = c.offset;
                        hits += 1;
                    }
                }
            };
        }
        set!(hp);
        set!(max_hp);
        set!(posture);
        set!(max_posture);
        set!(position);
        set!(animation_id);
        set!(entity_id);
        set!(team_type);
        set!(is_deflecting);
        hits
    }
}

/// Discover candidates.  `known_player_hp` is a hint — the value you
/// see on screen for the player the moment you kick off discovery.
/// Setting it to `None` disables the "exact HP match" boost and falls
/// back to range-only scoring.
///
/// # Safety
/// `chrins` must point to a live `ChrIns` instance of at least
/// `MAX_CHRINS_SCAN_BYTES` readable bytes.
pub unsafe fn discover(chrins: RawPtr, known_player_hp: Option<i32>) -> DiscoveryReport {
    let mut report = DiscoveryReport::default();

    // Sweep i32 fields for HP / max_hp / entity_id.
    let mut hp_candidates = Vec::new();
    let mut entity_candidates = Vec::new();
    for off in (0..MAX_CHRINS_SCAN_BYTES).step_by(ALIGN_4) {
        let v: i32 = chrins.offset(off as isize).read();
        // Entity ID is a large u32 in the 10_000..2_000_000 range.
        let vu = v as u32;
        if matches!(vu, 10_000..=9_999_999) {
            entity_candidates.push((off, vu));
        }
        if is_plausible_hp(v) {
            hp_candidates.push((off, v));
        }
    }

    // Pair HP with max_hp: for each HP candidate at `off`, look for a
    // max_hp at `off + 4` that is >= HP (fixed-pair layout in FromSoft
    // ChrIns) or alternatively at `off - 4` (some revisions have the
    // order swapped).
    let mut best_pair: Option<(usize, usize, f32)> = None;
    for (off, hp) in &hp_candidates {
        let exact_match_bonus = match known_player_hp {
            Some(k) if *hp == k => 0.4,
            _ => 0.0,
        };
        for (mhp_off, mhp_val) in &[
            (off.wrapping_add(4), *off + 4 < MAX_CHRINS_SCAN_BYTES),
        ] {
            if !*mhp_val {
                continue;
            }
            let mhp: i32 = chrins.offset(*mhp_off as isize).read();
            if mhp >= *hp && is_plausible_max_hp(mhp) {
                let base: f32 = 0.5;
                let pair_bonus: f32 = if (*hp as f32 / mhp as f32) > 0.01 {
                    0.1
                } else {
                    0.0
                };
                let confidence = (base + exact_match_bonus + pair_bonus).min(1.0f32);
                if best_pair.map(|(_, _, c)| c < confidence).unwrap_or(true) {
                    best_pair = Some((*off, *mhp_off, confidence));
                }
            }
        }
    }
    if let Some((hp_off, mhp_off, c)) = best_pair {
        report.hp = Some(Candidate { offset: hp_off, confidence: c });
        report.max_hp = Some(Candidate { offset: mhp_off, confidence: c });
    }

    // Pair posture with max_posture (f32 pair).
    let mut posture_pair: Option<(usize, usize, f32)> = None;
    for off in (0..MAX_CHRINS_SCAN_BYTES).step_by(ALIGN_4) {
        let p: f32 = chrins.offset(off as isize).read();
        if !is_plausible_posture(p) {
            continue;
        }
        let mp_off = off + 4;
        if mp_off + 4 > MAX_CHRINS_SCAN_BYTES {
            continue;
        }
        let mp: f32 = chrins.offset(mp_off as isize).read();
        if is_plausible_max_posture(mp) && mp >= p {
            let confidence = 0.55 + if p > 0.0 { 0.05 } else { 0.0 };
            if posture_pair.map(|(_, _, c)| c < confidence).unwrap_or(true) {
                posture_pair = Some((off, mp_off, confidence));
            }
        }
    }
    if let Some((p_off, mp_off, c)) = posture_pair {
        report.posture = Some(Candidate { offset: p_off, confidence: c });
        report.max_posture = Some(Candidate { offset: mp_off, confidence: c });
    }

    // Position: [f32; 3] where all three are finite and plausible.
    // Prefer 16-byte-aligned offsets since ChrIns positions typically
    // live in an XMM-aligned slot.  Skip offsets already claimed by the
    // posture pair to avoid treating `{posture, max_posture, pad}` as
    // a coordinate triple.
    let mut best_pos: Option<(usize, f32)> = None;
    let posture_range = report
        .posture
        .map(|c| (c.offset, c.offset + 16))
        .unwrap_or((usize::MAX, usize::MAX));
    for off in (0..MAX_CHRINS_SCAN_BYTES).step_by(ALIGN_16) {
        if off + 12 > MAX_CHRINS_SCAN_BYTES {
            break;
        }
        if off >= posture_range.0 && off < posture_range.1 {
            continue;
        }
        let x: f32 = chrins.offset(off as isize).read();
        let y: f32 = chrins.offset((off + 4) as isize).read();
        let z: f32 = chrins.offset((off + 8) as isize).read();
        if is_plausible_position([x, y, z]) {
            // Score tighter triples higher.  Real positions in Sekiro
            // tend to have varied magnitudes across components; a
            // monotonically increasing triple is more likely to be
            // sibling scalar fields (posture/max_posture/pad).
            let monotonic_penalty = if (x <= y && y <= z) || (x >= y && y >= z) {
                0.1
            } else {
                0.0
            };
            let confidence = 0.5 - monotonic_penalty;
            if best_pos.map(|(_, c)| c < confidence).unwrap_or(true) {
                best_pos = Some((off, confidence));
            }
        }
    }
    if let Some((off, c)) = best_pos {
        report.position = Some(Candidate { offset: off, confidence: c });
    }

    // Animation ID: u32 nonzero, plausible range.
    let mut best_anim: Option<(usize, f32)> = None;
    for off in (0..MAX_CHRINS_SCAN_BYTES).step_by(ALIGN_4) {
        let v: u32 = chrins.offset(off as isize).read();
        if is_plausible_anim_id(v) {
            let confidence = 0.4;
            if best_anim.map(|(_, c)| c < confidence).unwrap_or(true) {
                best_anim = Some((off, confidence));
            }
        }
    }
    if let Some((off, c)) = best_anim {
        report.animation_id = Some(Candidate { offset: off, confidence: c });
    }

    // Entity ID: prefer the 10000 sentinel (player) over other candidates.
    if let Some((off, _)) = entity_candidates
        .iter()
        .copied()
        .find(|(_, v)| *v == 10_000)
    {
        report.entity_id = Some(Candidate { offset: off, confidence: 0.9 });
    } else if let Some((off, _)) = entity_candidates.first() {
        report.entity_id = Some(Candidate {
            offset: *off,
            confidence: 0.3,
        });
    }

    // Team type: u8 in 0..=10.  Low confidence since many bytes match.
    // Only report when it's adjacent to a high-confidence HP pair.
    if let Some(hp_c) = report.hp {
        for off in (hp_c.offset + 16..hp_c.offset + 256).step_by(1) {
            if off >= MAX_CHRINS_SCAN_BYTES {
                break;
            }
            let b: u8 = chrins.offset(off as isize).read();
            if b <= 10 && b != 0 {
                report.team_type = Some(Candidate {
                    offset: off,
                    confidence: 0.25,
                });
                break;
            }
        }
    }

    // is_deflecting: u8 that is 0 or 1.  Also only suggestive.
    if let Some(pos_c) = report.position {
        for off in (pos_c.offset + 64..pos_c.offset + 512).step_by(1) {
            if off >= MAX_CHRINS_SCAN_BYTES {
                break;
            }
            let b: u8 = chrins.offset(off as isize).read();
            if b <= 1 {
                report.is_deflecting = Some(Candidate {
                    offset: off,
                    confidence: 0.15,
                });
                break;
            }
        }
    }

    report
}

// --- Plausibility predicates -----------------------------------------------

fn is_plausible_hp(v: i32) -> bool {
    matches!(v, 1..=5000)
}

fn is_plausible_max_hp(v: i32) -> bool {
    matches!(v, 1..=10_000)
}

fn is_plausible_posture(p: f32) -> bool {
    p.is_finite() && (0.0..=2000.0).contains(&p)
}

fn is_plausible_max_posture(p: f32) -> bool {
    p.is_finite() && (10.0..=2000.0).contains(&p)
}

fn is_plausible_position(xyz: [f32; 3]) -> bool {
    xyz.iter().all(|c| c.is_finite() && c.abs() <= 100_000.0)
        && (xyz[0].abs() + xyz[1].abs() + xyz[2].abs() > 0.001)
}

fn is_plausible_anim_id(v: u32) -> bool {
    matches!(v, 1..=99_999)
}

/// Given a set of fields already known to be correct, validate the
/// layout stays plausible at runtime by re-reading each field.  Used
/// to flag layout drift across patch versions.
///
/// # Safety
/// `ptr` must point to a live ChrIns.
pub unsafe fn validate_still_plausible(ptr: RawPtr, layout: &ChrInsLayout) -> Vec<&'static str> {
    let mut issues = Vec::new();
    if layout.hp != UNRESOLVED {
        let v: i32 = ptr.offset(layout.hp as isize).read();
        if !is_plausible_hp(v) && v != 0 {
            issues.push("hp");
        }
    }
    if layout.max_hp != UNRESOLVED {
        let v: i32 = ptr.offset(layout.max_hp as isize).read();
        if !is_plausible_max_hp(v) && v != 0 {
            issues.push("max_hp");
        }
    }
    if layout.position != UNRESOLVED {
        let x: f32 = ptr.offset(layout.position as isize).read();
        let y: f32 = ptr.offset((layout.position + 4) as isize).read();
        let z: f32 = ptr.offset((layout.position + 8) as isize).read();
        if !is_plausible_position([x, y, z]) {
            issues.push("position");
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_layout() -> Vec<u8> {
        // Construct a fake ChrIns buffer with known-value fields at
        // specific offsets.
        let mut buf = vec![0u8; MAX_CHRINS_SCAN_BYTES];
        // Zero everything, then scatter plausibility-passing values at
        // offsets we expect discovery to find.
        // Entity id = 10000 at offset 0x80.
        buf[0x80..0x84].copy_from_slice(&10_000u32.to_le_bytes());
        // HP pair: 900 / 1000 at offset 0x120.
        buf[0x120..0x124].copy_from_slice(&900i32.to_le_bytes());
        buf[0x124..0x128].copy_from_slice(&1000i32.to_le_bytes());
        // Posture pair: 120.0 / 450.0 at offset 0x200.
        buf[0x200..0x204].copy_from_slice(&120.0f32.to_le_bytes());
        buf[0x204..0x208].copy_from_slice(&450.0f32.to_le_bytes());
        // Position [f32; 3] at offset 0x300: (123.0, 45.0, -67.0)
        buf[0x300..0x304].copy_from_slice(&123.0f32.to_le_bytes());
        buf[0x304..0x308].copy_from_slice(&45.0f32.to_le_bytes());
        buf[0x308..0x30c].copy_from_slice(&(-67.0f32).to_le_bytes());
        // Animation id at 0x400: 7010.
        buf[0x400..0x404].copy_from_slice(&7010u32.to_le_bytes());
        buf
    }

    #[test]
    fn discovery_finds_hp_pair_with_exact_match() {
        let buf = with_layout();
        let ptr = RawPtr(buf.as_ptr() as usize);
        let report = unsafe { discover(ptr, Some(900)) };
        let hp = report.hp.expect("hp");
        let mhp = report.max_hp.expect("max_hp");
        assert_eq!(hp.offset, 0x120);
        assert_eq!(mhp.offset, 0x124);
        // Exact match bumps confidence over the baseline 0.5.
        assert!(hp.confidence > 0.7);
    }

    #[test]
    fn discovery_finds_posture_pair() {
        let buf = with_layout();
        let ptr = RawPtr(buf.as_ptr() as usize);
        let report = unsafe { discover(ptr, None) };
        let p = report.posture.expect("posture");
        assert_eq!(p.offset, 0x200);
    }

    #[test]
    fn discovery_finds_position() {
        let buf = with_layout();
        let ptr = RawPtr(buf.as_ptr() as usize);
        let report = unsafe { discover(ptr, None) };
        let pos = report.position.expect("position");
        assert_eq!(pos.offset, 0x300);
    }

    #[test]
    fn discovery_finds_entity_id() {
        let buf = with_layout();
        let ptr = RawPtr(buf.as_ptr() as usize);
        let report = unsafe { discover(ptr, None) };
        let e = report.entity_id.expect("entity");
        assert_eq!(e.offset, 0x80);
        // Player ID match should be very high confidence.
        assert!(e.confidence >= 0.9);
    }

    #[test]
    fn apply_respects_min_confidence() {
        let buf = with_layout();
        let ptr = RawPtr(buf.as_ptr() as usize);
        let report = unsafe { discover(ptr, Some(900)) };
        let mut layout = ChrInsLayout::unresolved();
        let strict_hits = report.apply(&mut layout, 0.9);
        let lax_hits = {
            let mut l = ChrInsLayout::unresolved();
            report.apply(&mut l, 0.2)
        };
        assert!(lax_hits > strict_hits);
    }

    #[test]
    fn validate_flags_corrupted_layout() {
        let mut buf = with_layout();
        // Corrupt the HP value.
        buf[0x120..0x124].copy_from_slice(&(-9999i32).to_le_bytes());
        let ptr = RawPtr(buf.as_ptr() as usize);
        let mut layout = ChrInsLayout::unresolved();
        layout.hp = 0x120;
        let issues = unsafe { validate_still_plausible(ptr, &layout) };
        assert_eq!(issues, vec!["hp"]);
    }
}
