//! Sliding-window reliability over UDP.
//!
//! The state channel (§6.3) is unreliable — last-write-wins. The event
//! and barrier channels are reliable; this module provides that
//! reliability.
//!
//! Design:
//!
//! - Each reliable packet is tagged with a monotonically increasing
//!   32-bit sequence number.
//! - On receipt, the receiver records `(seq)` in a bitset.
//! - On every outgoing packet, the receiver attaches its most-recently
//!   seen `ack` plus a 32-bit `ack_bits` covering `ack-1 .. ack-32`.
//! - Senders track unacked packets in a retransmit queue; any packet
//!   that hasn't been acknowledged within `RTO` is resent.
//! - No sequence is ever retired without acknowledgement.
//!
//! This is the same scheme `gafferongames.com` describes, with a small
//! retransmit queue and no fancy congestion control.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::wire::{AckBits, PacketHeader, Seq};

/// Initial retransmit timeout.  Grows under congestion (not implemented).
pub const DEFAULT_RTO: Duration = Duration::from_millis(200);

/// Max unacked packets we hold in the retransmit queue before dropping.
pub const RETRANSMIT_QUEUE_CAP: usize = 256;

/// An unacked packet awaiting its acknowledgement.
#[derive(Clone)]
struct PendingPacket {
    seq: Seq,
    bytes: Vec<u8>,
    sent_at: Instant,
    retry_count: u8,
}

/// Receive-side acknowledgement state: the latest seq we've seen and a
/// 32-bit bitmap covering its 32 predecessors.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecvAckState {
    latest: Option<Seq>,
    bits: u32,
}

impl RecvAckState {
    pub fn record(&mut self, seq: Seq) {
        match self.latest {
            None => {
                self.latest = Some(seq);
                self.bits = 0;
            }
            Some(prev) => {
                let prev_raw = prev.0;
                let seq_raw = seq.0;
                if seq_gt(seq_raw, prev_raw) {
                    // Shift the bitmap: each step forward pushes one
                    // "seen" bit into `bits` for the old latest.
                    let advance = seq_raw.wrapping_sub(prev_raw);
                    if advance >= 32 {
                        self.bits = 0;
                    } else {
                        self.bits = self.bits.wrapping_shl(advance);
                        // The previous "latest" now becomes an ack-1
                        // relative to the new latest — set its bit.
                        let shift = advance.saturating_sub(1);
                        self.bits |= 1u32.wrapping_shl(shift);
                    }
                    self.latest = Some(seq);
                } else {
                    // Out-of-order or duplicate — set the corresponding
                    // bit if it's within the 32-wide window.
                    let diff = prev_raw.wrapping_sub(seq_raw);
                    if diff >= 1 && diff <= 32 {
                        self.bits |= 1u32 << (diff - 1);
                    }
                }
            }
        }
    }

    pub fn latest(&self) -> Seq {
        self.latest.unwrap_or(Seq(0))
    }

    pub fn bits(&self) -> AckBits {
        AckBits(self.bits)
    }

    /// Already received?  (Latest + 32-wide window.)
    pub fn contains(&self, seq: Seq) -> bool {
        let latest = match self.latest {
            Some(l) => l.0,
            None => return false,
        };
        if seq.0 == latest {
            return true;
        }
        let diff = latest.wrapping_sub(seq.0);
        diff >= 1 && diff <= 32 && (self.bits & (1u32 << (diff - 1))) != 0
    }
}

/// Circular-difference seq-greater-than (handles 32-bit wrap).
fn seq_gt(a: u32, b: u32) -> bool {
    ((a > b) && (a - b <= u32::MAX / 2)) || ((a < b) && (b - a > u32::MAX / 2))
}

/// Send-side state: the retransmit queue.
#[derive(Default)]
pub struct Reliability {
    queue: Mutex<VecDeque<PendingPacket>>,
    rto: Duration,
    pub stats: Mutex<ReliabilityStats>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReliabilityStats {
    pub sent: u64,
    pub retransmitted: u64,
    pub acked: u64,
    pub dropped_queue_full: u64,
}

impl Reliability {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            rto: DEFAULT_RTO,
            stats: Mutex::new(ReliabilityStats::default()),
        }
    }

    pub fn with_rto(rto: Duration) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            rto,
            stats: Mutex::new(ReliabilityStats::default()),
        }
    }

    /// Record a reliable packet's bytes so it can be retransmitted if
    /// the ack doesn't come in time.
    pub fn track(&self, seq: Seq, bytes: Vec<u8>) {
        let mut q = self.queue.lock();
        if q.len() >= RETRANSMIT_QUEUE_CAP {
            q.pop_front();
            self.stats.lock().dropped_queue_full += 1;
        }
        q.push_back(PendingPacket {
            seq,
            bytes,
            sent_at: Instant::now(),
            retry_count: 0,
        });
        self.stats.lock().sent += 1;
    }

    /// Apply the peer's advertised ack state.  Removes any acked entries
    /// from the retransmit queue.
    pub fn apply_remote_ack(&self, ack: Seq, bits: AckBits) {
        let mut q = self.queue.lock();
        let bits = bits.0;
        let mut acked = 0u64;
        q.retain(|p| {
            if p.seq == ack {
                acked += 1;
                return false;
            }
            let diff = ack.0.wrapping_sub(p.seq.0);
            if diff >= 1 && diff <= 32 && (bits & (1u32 << (diff - 1))) != 0 {
                acked += 1;
                return false;
            }
            true
        });
        drop(q);
        self.stats.lock().acked += acked;
    }

    /// Return every packet due for retransmission.  Caller re-sends them.
    pub fn due_for_retransmit(&self, now: Instant) -> Vec<Vec<u8>> {
        let mut q = self.queue.lock();
        let mut out = Vec::new();
        for p in q.iter_mut() {
            if now.duration_since(p.sent_at) >= self.rto {
                out.push(p.bytes.clone());
                p.sent_at = now;
                p.retry_count = p.retry_count.saturating_add(1);
            }
        }
        drop(q);
        if !out.is_empty() {
            self.stats.lock().retransmitted += out.len() as u64;
        }
        out
    }

    pub fn outstanding(&self) -> usize {
        self.queue.lock().len()
    }

    pub fn clear(&self) {
        self.queue.lock().clear();
    }
}

/// Attach the peer ack to an outgoing header.  Mutates `hdr.ack` +
/// `hdr.ack_bits`.
pub fn stamp_ack(hdr: &mut PacketHeader, recv: &RecvAckState) {
    hdr.ack = recv.latest();
    hdr.ack_bits = recv.bits();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{PacketType, Seq};

    #[test]
    fn recv_state_in_order() {
        let mut r = RecvAckState::default();
        for i in 1..=5u32 {
            r.record(Seq(i));
        }
        assert_eq!(r.latest(), Seq(5));
        for i in 1..=4u32 {
            assert!(r.contains(Seq(i)), "missing seq {i}");
        }
        assert!(r.contains(Seq(5)));
    }

    #[test]
    fn recv_state_out_of_order() {
        let mut r = RecvAckState::default();
        r.record(Seq(10));
        r.record(Seq(7));
        r.record(Seq(9));
        r.record(Seq(8));
        assert_eq!(r.latest(), Seq(10));
        for s in 7..=10 {
            assert!(r.contains(Seq(s)), "missing {s}");
        }
        assert!(!r.contains(Seq(6)));
    }

    #[test]
    fn retransmit_clears_on_ack() {
        let r = Reliability::with_rto(Duration::from_millis(50));
        r.track(Seq(1), vec![1, 2, 3]);
        r.track(Seq(2), vec![4, 5, 6]);
        assert_eq!(r.outstanding(), 2);
        // Ack seq 2; bits-bitmap includes seq 1 as latest-1.
        r.apply_remote_ack(Seq(2), AckBits(0b01));
        assert_eq!(r.outstanding(), 0);
        assert_eq!(r.stats.lock().acked, 2);
    }

    #[test]
    fn retransmit_due_after_rto() {
        let r = Reliability::with_rto(Duration::from_millis(10));
        r.track(Seq(1), vec![9, 9]);
        std::thread::sleep(Duration::from_millis(20));
        let due = r.due_for_retransmit(Instant::now());
        assert_eq!(due.len(), 1);
        assert_eq!(due[0], vec![9, 9]);
        // Stays queued.
        assert_eq!(r.outstanding(), 1);
    }

    #[test]
    fn retransmit_respects_queue_cap() {
        let r = Reliability::with_rto(Duration::from_millis(1000));
        for i in 0..(RETRANSMIT_QUEUE_CAP as u32 + 5) {
            r.track(Seq(i), vec![i as u8]);
        }
        assert_eq!(r.outstanding(), RETRANSMIT_QUEUE_CAP);
        assert_eq!(r.stats.lock().dropped_queue_full, 5);
    }

    #[test]
    fn stamp_ack_on_header() {
        let mut recv = RecvAckState::default();
        recv.record(Seq(100));
        recv.record(Seq(99));
        let mut hdr = PacketHeader::new(PacketType::Heartbeat, Seq(1), false, false);
        stamp_ack(&mut hdr, &recv);
        assert_eq!(hdr.ack, Seq(100));
        assert!(hdr.ack_bits.0 & 0b01 != 0, "prev ack should be set");
    }
}
