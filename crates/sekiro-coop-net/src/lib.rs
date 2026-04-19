//! Layer 5 — session, matchmaking, transport, wire protocol.
//!
//! SPEC §7, §9.

pub mod desync;
pub mod grace;
pub mod lobby;
pub mod reliability;
pub mod session;
pub mod steam;
pub mod transport;
pub mod wire;

pub use desync::{DesyncAction, DesyncDetector, DESYNC_KILL_STRIKES, DESYNC_PERIOD_FRAMES};
pub use grace::{classify_link, GraceBuffer, LinkState, MAX_BUFFERED_BYTES};
pub use lobby::{Lobby, LobbyInfo, MOD_VERSION};
pub use reliability::{RecvAckState, Reliability, ReliabilityStats, DEFAULT_RTO};
pub use session::{HandshakeOutcome, Session, SessionConfig};
pub use transport::{Transport, TransportError, UdpTransport};
pub use wire::{AckBits, PacketBody, PacketHeader, PacketType, Seq, WIRE_MAGIC};
