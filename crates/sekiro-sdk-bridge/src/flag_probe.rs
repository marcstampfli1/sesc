//! Connected-event-flag range probe.  SPEC §6.5, P2 gap #12.
//!
//! Strategy: set a flag at a candidate ID; observe via a trait
//! whether the peer sees the change.  The probe algorithm is pure —
//! only the `PeerObserver` impl requires Sekiro running.
//!
//! We can't just sweep every flag ID (Sekiro has hundreds of
//! thousands).  Instead we use exponential probing + binary search
//! around DS3/ER-documented precedent:
//!
//! ```text
//!   Initial candidates: [30_000, 50_000, 70_000]  (from DS3 precedent)
//!   If any candidate syncs → binary-search left + right to find
//!   boundaries.
//! ```

use crate::world::ConnectedFlagRange;

/// Abstract "can the peer see my flag change" observer.  In production
/// this is implemented via a dedicated network probe protocol; for
/// tests we plug in a deterministic fake.
pub trait PeerObserver {
    /// Sets the flag `id` on our peer and returns true iff the remote
    /// peer sees it synced within `timeout_ms`.
    fn observe_flag_sync(&mut self, id: u32, timeout_ms: u32) -> bool;
}

/// Default seed candidates from DS3 precedent (`Souls-modding` wiki).
pub const DEFAULT_CANDIDATES: &[u32] = &[
    30_000, 40_000, 50_000, 60_000, 70_000, 80_000,
];

/// Probe the connected flag range.  Returns `Some(range)` if a
/// boundary pair was found; `None` if no candidate synced.
pub fn probe<O: PeerObserver>(
    observer: &mut O,
    candidates: &[u32],
    timeout_ms: u32,
) -> Option<ConnectedFlagRange> {
    let synced_candidate = candidates
        .iter()
        .copied()
        .find(|&id| observer.observe_flag_sync(id, timeout_ms))?;

    // Binary-search left boundary in [0, synced_candidate].
    let left = binary_search_boundary(
        observer,
        0,
        synced_candidate,
        true,
        timeout_ms,
    );
    // Binary-search right boundary in [synced_candidate, u32::MAX / 2].
    let right = binary_search_boundary(
        observer,
        synced_candidate,
        1_000_000,
        false,
        timeout_ms,
    );

    let mut range = ConnectedFlagRange::new();
    range.set(left, right);
    Some(range)
}

/// Find a boundary in `[lo, hi]` where `observer.observe_flag_sync`
/// transitions.  `find_left = true` searches for the smallest synced
/// ID; `false` searches for the largest.  Tracks the best known
/// synced value as we go to avoid off-by-one at the edges.
fn binary_search_boundary<O: PeerObserver>(
    observer: &mut O,
    mut lo: u32,
    mut hi: u32,
    find_left: bool,
    timeout_ms: u32,
) -> u32 {
    let mut best = if find_left { hi } else { lo };
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let synced = observer.observe_flag_sync(mid, timeout_ms);
        if synced {
            best = mid;
            if find_left {
                if mid == 0 {
                    return best;
                }
                hi = mid - 1;
            } else {
                if mid == u32::MAX {
                    return best;
                }
                lo = mid + 1;
            }
        } else if find_left {
            lo = mid + 1;
        } else {
            if mid == 0 {
                return best;
            }
            hi = mid - 1;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake observer: a fixed range `[lo..=hi]` is sync'd.
    struct FakeObserver {
        lo: u32,
        hi: u32,
    }

    impl PeerObserver for FakeObserver {
        fn observe_flag_sync(&mut self, id: u32, _timeout_ms: u32) -> bool {
            id >= self.lo && id <= self.hi
        }
    }

    #[test]
    fn probe_finds_range_with_standard_candidates() {
        let mut obs = FakeObserver { lo: 35_000, hi: 85_000 };
        let range = probe(&mut obs, DEFAULT_CANDIDATES, 10).expect("range");
        let (lo, hi) = range.inclusive.expect("present");
        assert_eq!(lo, 35_000);
        assert_eq!(hi, 85_000);
    }

    #[test]
    fn probe_returns_none_when_nothing_syncs() {
        let mut obs = FakeObserver { lo: 900_000, hi: 1_000_000 };
        let range = probe(&mut obs, DEFAULT_CANDIDATES, 10);
        assert!(range.is_none());
    }

    #[test]
    fn probe_finds_narrow_range() {
        let mut obs = FakeObserver { lo: 30_000, hi: 30_063 };
        let range = probe(&mut obs, DEFAULT_CANDIDATES, 10).expect("range");
        let (lo, hi) = range.inclusive.expect("present");
        assert_eq!(lo, 30_000);
        assert_eq!(hi, 30_063);
    }

    #[test]
    fn probe_finds_single_flag_range() {
        let mut obs = FakeObserver { lo: 40_000, hi: 40_000 };
        let range = probe(&mut obs, DEFAULT_CANDIDATES, 10).expect("range");
        let (lo, hi) = range.inclusive.expect("present");
        assert_eq!(lo, 40_000);
        assert_eq!(hi, 40_000);
    }
}
