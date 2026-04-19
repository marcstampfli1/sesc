//! Lobby creation/discovery.  SPEC §7.2 steps 1-2.
//!
//! The raw-UDP transport skips real lobby discovery; a peer is given an
//! ip:port out-of-band.  `Lobby` is the abstraction so the Steam impl
//! (behind the `steam` feature) can plug into the same session lifecycle.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

pub const MOD_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub lobby_id: String,
    pub host_addr: SocketAddr,
    pub version: String,
    pub game_cycle: u8,
}

#[derive(Debug, Clone)]
pub enum Lobby {
    /// Out-of-band address exchange (no matchmaking server).
    DirectUdp { info: LobbyInfo },
    /// Steam P2P lobby.  Implemented once `steam` feature is enabled.
    #[cfg(feature = "steam")]
    Steam { lobby_id: u64 },
}

impl Lobby {
    pub fn direct_udp(host_addr: SocketAddr, game_cycle: u8) -> Self {
        Lobby::DirectUdp {
            info: LobbyInfo {
                lobby_id: format!("{}", host_addr),
                host_addr,
                version: MOD_VERSION.into(),
                game_cycle,
            },
        }
    }

    pub fn info(&self) -> Option<&LobbyInfo> {
        match self {
            Lobby::DirectUdp { info } => Some(info),
            #[cfg(feature = "steam")]
            Lobby::Steam { .. } => None,
        }
    }
}
