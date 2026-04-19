//! Reconnect-grace state + outgoing-packet buffer.  SPEC §7.4.
//!
//! When the peer goes silent (no packet for > `heartbeat_period`) but
//! before the `reconnect_grace` expires, we:
//!
//! - Stop sending unreliable state updates (they'd just pile up).
//! - Buffer reliable outgoing packets up to `MAX_BUFFERED_BYTES`.
//! - On next packet receipt, flush the buffer + request a
//!   `FullStateSnapshot` to re-sync.
//!
//! After the grace expires, the session ends and both peers revert to
//! solo (DLL warps to the last Sculptor's Idol).

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::wire::Seq;

pub const MAX_BUFFERED_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// Packets flowing normally.
    Up,
    /// No packet received for > `heartbeat_period`; drain queue only
    /// when reachable.
    Suspect,
    /// No contact for > `reconnect_grace`; session will end.
    Expired,
}

#[derive(Debug, Default)]
pub struct GraceBuffer {
    queue: Mutex<VecDeque<Vec<u8>>>,
    bytes: Mutex<usize>,
    dropped: Mutex<u64>,
    /// Whether we've already requested a snapshot during the current
    /// disconnect; stops the DLL from spamming requests.
    snapshot_requested: Mutex<Option<Seq>>,
}

impl GraceBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if a snapshot request is in flight.
    pub fn has_pending_snapshot_request(&self) -> bool {
        self.snapshot_requested.lock().is_some()
    }

    /// Mark a snapshot request as in-flight at sequence `seq`.
    pub fn mark_snapshot_request(&self, seq: Seq) {
        *self.snapshot_requested.lock() = Some(seq);
    }

    /// Called when a `FullStateSnapshot` arrives — clears pending flag.
    pub fn clear_snapshot_request(&self) {
        *self.snapshot_requested.lock() = None;
    }

    /// Push an outgoing packet into the buffer.  If the buffer is full
    /// (`MAX_BUFFERED_BYTES`), drop the oldest entries.
    pub fn push(&self, bytes: Vec<u8>) {
        let len = bytes.len();
        let mut q = self.queue.lock();
        let mut size = self.bytes.lock();
        let mut dropped = 0u64;
        while *size + len > MAX_BUFFERED_BYTES {
            let evicted = match q.pop_front() {
                Some(v) => v,
                None => break,
            };
            *size = size.saturating_sub(evicted.len());
            dropped += 1;
        }
        *size += len;
        q.push_back(bytes);
        drop(q);
        drop(size);
        if dropped > 0 {
            *self.dropped.lock() += dropped;
        }
    }

    /// Drain the buffer, returning all queued packets.
    pub fn drain(&self) -> Vec<Vec<u8>> {
        let mut q = self.queue.lock();
        let drained: Vec<Vec<u8>> = q.drain(..).collect();
        *self.bytes.lock() = 0;
        drained
    }

    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    pub fn bytes(&self) -> usize {
        *self.bytes.lock()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    pub fn dropped(&self) -> u64 {
        *self.dropped.lock()
    }
}

/// Classify a link based on the time since the last peer contact.
pub fn classify_link(
    last_contact: Instant,
    heartbeat_period: Duration,
    reconnect_grace: Duration,
) -> LinkState {
    let elapsed = Instant::now().saturating_duration_since(last_contact);
    if elapsed < heartbeat_period * 3 {
        LinkState::Up
    } else if elapsed < reconnect_grace {
        LinkState::Suspect
    } else {
        LinkState::Expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_transitions() {
        let now = Instant::now();
        let period = Duration::from_millis(500);
        let grace = Duration::from_secs(10);
        // Right now → Up.
        assert_eq!(classify_link(now, period, grace), LinkState::Up);
    }

    #[test]
    fn buffer_evicts_when_full() {
        let b = GraceBuffer::new();
        // Push (0.5*MAX) + (0.6*MAX) — second push evicts first.
        let half = vec![0u8; MAX_BUFFERED_BYTES / 2];
        let big = vec![0u8; (MAX_BUFFERED_BYTES * 6) / 10];
        b.push(half);
        b.push(big);
        assert!(b.bytes() <= MAX_BUFFERED_BYTES);
        assert!(b.dropped() >= 1);
    }

    #[test]
    fn drain_returns_in_order() {
        let b = GraceBuffer::new();
        b.push(vec![1]);
        b.push(vec![2]);
        b.push(vec![3]);
        let drained = b.drain();
        assert_eq!(drained, vec![vec![1], vec![2], vec![3]]);
        assert!(b.is_empty());
    }

    #[test]
    fn snapshot_request_lifecycle() {
        let b = GraceBuffer::new();
        assert!(!b.has_pending_snapshot_request());
        b.mark_snapshot_request(Seq(42));
        assert!(b.has_pending_snapshot_request());
        b.clear_snapshot_request();
        assert!(!b.has_pending_snapshot_request());
    }
}
