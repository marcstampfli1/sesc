//! Steam P2P transport stub.  SPEC §7.1.
//!
//! Enabled via the `steam` feature.  Full impl requires `steamworks-rs`;
//! this scaffold declares the API shape so higher layers can be written
//! against `Transport` uniformly.

#[cfg(feature = "steam")]
pub mod imp {
    use crate::transport::{Transport, TransportError};
    use std::net::SocketAddr;

    pub struct SteamTransport {
        // TODO(P2 gap #15, SPEC §11): wire up steamworks-rs handles.
        // Keep `peer_steam_id: u64` and a channel of queued packets.
    }

    impl SteamTransport {
        pub fn new() -> Result<Self, TransportError> {
            Err(TransportError::Closed)
        }
    }

    impl Transport for SteamTransport {
        fn send(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
            Err(TransportError::Closed)
        }
        fn recv(&mut self, _buf: &mut [u8]) -> Result<Option<usize>, TransportError> {
            Err(TransportError::Closed)
        }
        fn set_peer(&mut self, _peer: SocketAddr) -> Result<(), TransportError> {
            Err(TransportError::Closed)
        }
        fn local_addr(&self) -> Result<SocketAddr, TransportError> {
            Err(TransportError::Closed)
        }
    }
}
