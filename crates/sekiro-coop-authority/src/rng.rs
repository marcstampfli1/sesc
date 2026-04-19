//! Seeded PCG RNG.  SPEC §6.4.
//!
//! RNG state derives from `(match_seed, frame_counter, call_site_id)`;
//! both peers produce identical results for the same key triple.

use serde::{Deserialize, Serialize};

/// A PCG-XSH-RR 64-bit state.  Small, fast, no external deps.
#[derive(Debug, Clone, Copy)]
pub struct SeededRng {
    state: u64,
    inc: u64,
}

/// u64 seed for a match.  Host generates, broadcasts to client at
/// session establishment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSeed(pub u64);

impl MatchSeed {
    pub fn new(v: u64) -> Self {
        Self(v)
    }
}

/// AOB-derived identifier for a hook site that needs an RNG stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SiteId(pub u32);

impl SiteId {
    pub const EMEVD_RANDOMLY_SET_FLAG: SiteId = SiteId(0x01);
    pub const EMEVD_INIT_SEED: SiteId = SiteId(0x02);
}

impl SeededRng {
    /// Build an RNG whose state is fully determined by the triple.
    pub fn new(seed: MatchSeed, frame: u64, site: SiteId) -> Self {
        let mix = splitmix64(
            seed.0 ^ frame.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (site.0 as u64),
        );
        let mut rng = Self {
            state: 0,
            inc: 0xda3e_39cb_94b9_5bdbu64,
        };
        // PCG seeding ritual: set state to 0, inc odd, advance, add seed, advance.
        rng.next_u32();
        rng.state = rng.state.wrapping_add(mix);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    pub fn next_u64(&mut self) -> u64 {
        (self.next_u32() as u64) | ((self.next_u32() as u64) << 32)
    }

    pub fn range_u32(&mut self, min: u32, max_inclusive: u32) -> u32 {
        if max_inclusive <= min {
            return min;
        }
        let span = max_inclusive - min + 1;
        min + (self.next_u32() % span)
    }

    pub fn range_u64(&mut self, min: u64, max_inclusive: u64) -> u64 {
        if max_inclusive <= min {
            return min;
        }
        let span = max_inclusive - min + 1;
        min + (self.next_u64() % span)
    }

    /// Pick one of the flags in `[start..=end]` to set, matching Sekiro's
    /// `RandomlySetEventFlagInRange` semantics (SPEC §4.3).
    pub fn pick_flag_in_range(&mut self, start: u32, end: u32) -> u32 {
        self.range_u32(start, end)
    }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_key() {
        let mut a = SeededRng::new(MatchSeed(42), 100, SiteId(7));
        let mut b = SeededRng::new(MatchSeed(42), 100, SiteId(7));
        for _ in 0..32 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_site_differs() {
        let mut a = SeededRng::new(MatchSeed(42), 100, SiteId(7));
        let mut b = SeededRng::new(MatchSeed(42), 100, SiteId(8));
        let mut differed = false;
        for _ in 0..8 {
            if a.next_u32() != b.next_u32() {
                differed = true;
                break;
            }
        }
        assert!(differed);
    }
}
