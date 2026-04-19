//! Sync channels — the three packet classes.  SPEC §6.3.

/// Which channel a packet uses.  Drives reliability policy in the net layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncChannel {
    /// Per-tick snapshot deltas for owned entities.  Unreliable,
    /// last-write-wins.
    State,
    /// Discrete, ordered events (deflects, deathblows, SpEffect applies,
    /// flag sets, AI commands).  Reliable.
    Event,
    /// Lockstep barriers (`WaitForNetworkApproval` semantics).  Reliable.
    Barrier,
}

impl SyncChannel {
    pub fn reliable(self) -> bool {
        matches!(self, SyncChannel::Event | SyncChannel::Barrier)
    }

    pub fn ordered(self) -> bool {
        // State is unordered (latest wins); Event is ordered across ticks;
        // Barrier is a single request/ack — order irrelevant at the
        // transport level.
        matches!(self, SyncChannel::Event)
    }
}

/// Per-channel bandwidth budget in bytes-per-second.  Used by the
/// net layer to throttle state updates when the remote reports congestion.
#[derive(Debug, Clone, Copy)]
pub struct ChannelBudget {
    pub state_bps: u32,
    pub event_bps: u32,
    pub barrier_bps: u32,
}

impl ChannelBudget {
    /// Defaults from SPEC §6.3 ("~96 KB/s per peer" state channel).
    pub const DEFAULT: Self = ChannelBudget {
        state_bps: 96 * 1024,
        event_bps: 32 * 1024,
        barrier_bps: 4 * 1024,
    };
}
