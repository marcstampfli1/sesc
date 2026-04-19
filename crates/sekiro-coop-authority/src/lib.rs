//! Layer 4 — per-entity authority, proximity handoff, sync channels,
//! seeded RNG broadcast.  SPEC §6.

pub mod channels;
pub mod driver;
pub mod handoff;
pub mod rng;
pub mod table;

pub use channels::{SyncChannel, ChannelBudget};
pub use driver::{HandoffDecision, HandoffPolicy, ProximityDriver, ProximityObservation};
pub use handoff::{HandoffOutcome, HandoffTracker, HANDOFF_TIMEOUT_MS};
pub use rng::{MatchSeed, SeededRng, SiteId};
pub use table::{AuthorityLevel, AuthorityTable, PeerId};
