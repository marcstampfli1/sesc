//! World / event bridge — event flags, scripted triggers, boss fog, loot.
//!
//! SPEC §4.3.  Covers `SetEventFlag`, `SetNetworkConnectedEventFlag`,
//! `BatchSetNetworkConnectedEventFlags`, `RandomlySetEventFlagInRange`,
//! `TriggerMultiplayerEvent`, `SetEventState`, `AwardItemLot`.

use serde::{Deserialize, Serialize};

/// The "connected" event-flag ID range auto-sync'd by the engine (SPEC
/// §6.5). Starts as `None`; populated at startup by the probe strategy
/// (runtime detection or DS3 precedent).
#[derive(Debug, Clone, Copy, Default)]
pub struct ConnectedFlagRange {
    pub inclusive: Option<(u32, u32)>,
}

impl ConnectedFlagRange {
    pub const fn new() -> Self {
        Self { inclusive: None }
    }

    pub fn set(&mut self, start: u32, end: u32) {
        self.inclusive = Some((start, end));
    }

    /// Is this flag ID in the connected (auto-sync) range?
    pub fn contains(&self, id: u32) -> bool {
        match self.inclusive {
            Some((lo, hi)) => (lo..=hi).contains(&id),
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EventFlagEvent {
    pub flag_id: u32,
    pub state: bool,
    /// True iff the mod decided this needs cross-peer sync (the original
    /// `SetEventFlag` was promoted to `SetNetworkConnectedEventFlag`).
    pub synced: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MultiplayerEvent {
    /// The EMEVD multiplayer-event ID (see `TriggerMultiplayerEvent`).
    pub event_id: u32,
}

/// The world/event bridge state.
#[derive(Default)]
pub struct WorldBridge {
    pub connected_flags: ConnectedFlagRange,
    pub tramp_set_event_flag: core::sync::atomic::AtomicUsize,
    pub tramp_set_network_flag: core::sync::atomic::AtomicUsize,
    pub tramp_batch_network_flags: core::sync::atomic::AtomicUsize,
    pub tramp_randomly_set_flag: core::sync::atomic::AtomicUsize,
    pub tramp_trigger_multi: core::sync::atomic::AtomicUsize,
    pub tramp_set_event_state: core::sync::atomic::AtomicUsize,
    pub tramp_award_item_lot: core::sync::atomic::AtomicUsize,
}

impl WorldBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Given a raw `SetEventFlag` write, decide whether to promote it to
    /// `SetNetworkConnectedEventFlag`.  Non-allocating.
    pub fn should_sync(&self, flag_id: u32) -> bool {
        self.connected_flags.contains(flag_id)
    }
}
