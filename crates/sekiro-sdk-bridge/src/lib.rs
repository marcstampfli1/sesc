//! Layer 2 — game-state bridges.
//!
//! Intercept every native mutator we care about; emit typed
//! `BridgeEvent`s to higher layers.  SPEC §4.

pub mod ai;
pub mod combat;
pub mod events;
pub mod flag_probe;
pub mod world;

pub use ai::{AiBridge, AnimEvent};
pub use combat::{CombatBridge, CombatEvent, CombatOutcome};
pub use events::{BridgeDispatcher, BridgeEvent, Subscriber, TickBoundary};
pub use flag_probe::{probe as probe_connected_flag_range, PeerObserver, DEFAULT_CANDIDATES};
pub use world::{ConnectedFlagRange, EventFlagEvent, MultiplayerEvent, WorldBridge};
