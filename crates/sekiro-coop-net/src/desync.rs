//! Desync detection loop.  SPEC §9.3.
//!
//! Every `DESYNC_PERIOD_FRAMES` frames both peers hash the shared-set
//! snapshot and exchange hashes in a heartbeat tag.  A mismatch triggers:
//!
//! 1. Log the divergence frame + both hashes.
//! 2. Request `FullStateSnapshot` from host.
//! 3. If mismatch persists after restore, end session with
//!    `DesyncReport`.
//!
//! The net layer owns the detection state; higher layers feed it a
//! hash each tick and poll for "send a snapshot now" / "end session"
//! decisions.

use parking_lot::Mutex;
use std::collections::HashMap;

use crate::wire::Seq;

/// How often we hash + exchange.  60 frames = 1 second at 60 Hz.
pub const DESYNC_PERIOD_FRAMES: u64 = 60;

/// How many consecutive mismatches trigger session termination.
pub const DESYNC_KILL_STRIKES: u8 = 3;

/// Decision emitted each time a remote hash arrives for comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesyncAction {
    /// Hashes matched; continue normally.
    Ok,
    /// First divergence — request a `FullStateSnapshot` from host.
    RequestSnapshot { frame: u64 },
    /// Persistent divergence — end session.
    KillSession,
    /// Remote hash arrived late (frame already aged out of our local
    /// hash ring); no action.
    Stale,
}

#[derive(Debug, Default)]
pub struct DesyncDetector {
    local_hashes: Mutex<HashMap<u64, u64>>,
    strikes: Mutex<u8>,
    last_checked_frame: Mutex<u64>,
    /// Retain up to this many historical local hashes for comparison
    /// against late-arriving remote reports.
    history_cap: usize,
}

impl DesyncDetector {
    pub fn new() -> Self {
        Self {
            local_hashes: Mutex::new(HashMap::new()),
            strikes: Mutex::new(0),
            last_checked_frame: Mutex::new(0),
            history_cap: 16,
        }
    }

    /// Decide whether this frame is a check-point frame.
    pub fn should_hash(&self, frame: u64) -> bool {
        frame > 0 && frame % DESYNC_PERIOD_FRAMES == 0
    }

    /// Record our local hash for this frame.
    pub fn record_local(&self, frame: u64, hash: u64) {
        let mut h = self.local_hashes.lock();
        h.insert(frame, hash);
        // Trim history.
        if h.len() > self.history_cap {
            let oldest = *h.keys().min().unwrap();
            h.remove(&oldest);
        }
    }

    /// Feed a remote hash for a past frame and decide what to do.
    pub fn compare_remote(&self, frame: u64, remote_hash: u64) -> DesyncAction {
        let h = self.local_hashes.lock();
        let local = match h.get(&frame) {
            Some(v) => *v,
            None => return DesyncAction::Stale,
        };
        drop(h);
        *self.last_checked_frame.lock() = frame;
        if local == remote_hash {
            *self.strikes.lock() = 0;
            return DesyncAction::Ok;
        }
        // Mismatch.
        let mut strikes = self.strikes.lock();
        *strikes = strikes.saturating_add(1);
        if *strikes >= DESYNC_KILL_STRIKES {
            DesyncAction::KillSession
        } else {
            DesyncAction::RequestSnapshot { frame }
        }
    }

    pub fn reset(&self) {
        self.local_hashes.lock().clear();
        *self.strikes.lock() = 0;
    }

    pub fn strikes(&self) -> u8 {
        *self.strikes.lock()
    }

    pub fn last_checked(&self) -> u64 {
        *self.last_checked_frame.lock()
    }
}

/// Encode a `(frame, hash)` pair into the 32-bit `ping_tag` field of a
/// Heartbeat.  Actually we need 64+64 bits — callers should use a
/// dedicated `DesyncReport` packet for full exchange; the ping_tag is
/// purely for round-trip timing.  This helper is left for forward
/// compatibility.
pub fn tag_from_hash(hash: u64) -> u32 {
    ((hash >> 32) ^ (hash & 0xFFFF_FFFF)) as u32
}

/// Which `Seq` the last snapshot request used, so we can suppress
/// duplicate requests while waiting for the response.
#[derive(Debug, Default)]
pub struct InflightSnapshotRequest {
    pub for_frame: Option<u64>,
    pub request_seq: Option<Seq>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_ok() {
        let d = DesyncDetector::new();
        d.record_local(60, 0xdeadbeef);
        assert_eq!(d.compare_remote(60, 0xdeadbeef), DesyncAction::Ok);
        assert_eq!(d.strikes(), 0);
    }

    #[test]
    fn first_mismatch_requests_snapshot() {
        let d = DesyncDetector::new();
        d.record_local(60, 0x1111_1111);
        assert_eq!(
            d.compare_remote(60, 0x2222_2222),
            DesyncAction::RequestSnapshot { frame: 60 }
        );
        assert_eq!(d.strikes(), 1);
    }

    #[test]
    fn persistent_mismatch_kills_session() {
        let d = DesyncDetector::new();
        for f in [60, 120, 180] {
            d.record_local(f, 0x1111);
        }
        assert_eq!(
            d.compare_remote(60, 0x2222),
            DesyncAction::RequestSnapshot { frame: 60 }
        );
        assert_eq!(
            d.compare_remote(120, 0x3333),
            DesyncAction::RequestSnapshot { frame: 120 }
        );
        assert_eq!(d.compare_remote(180, 0x4444), DesyncAction::KillSession);
    }

    #[test]
    fn stale_when_frame_unknown() {
        let d = DesyncDetector::new();
        assert_eq!(d.compare_remote(60, 0), DesyncAction::Stale);
    }

    #[test]
    fn recovery_resets_strikes() {
        let d = DesyncDetector::new();
        d.record_local(60, 0xA);
        d.record_local(120, 0xB);
        d.compare_remote(60, 0xC); // strike 1
        assert_eq!(d.compare_remote(120, 0xB), DesyncAction::Ok);
        assert_eq!(d.strikes(), 0);
    }

    #[test]
    fn history_cap_evicts_oldest() {
        let d = DesyncDetector::new();
        for f in 0..20u64 {
            d.record_local(f, f);
        }
        // Oldest frames should be gone.
        assert_eq!(d.compare_remote(0, 0), DesyncAction::Stale);
    }

    #[test]
    fn should_hash_on_period() {
        let d = DesyncDetector::new();
        assert!(!d.should_hash(0));
        assert!(!d.should_hash(59));
        assert!(d.should_hash(60));
        assert!(d.should_hash(120));
        assert!(!d.should_hash(61));
    }
}
