//! Session lifecycle.  SPEC §7.2-§7.4.

use parking_lot::Mutex;
use sekiro_coop_authority::rng::MatchSeed;
use sekiro_coop_authority::table::PeerId;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::grace::{classify_link, GraceBuffer, LinkState};
use crate::lobby::{Lobby, MOD_VERSION};
use crate::reliability::{stamp_ack, RecvAckState, Reliability};
use crate::transport::{Transport, TransportError};
use crate::wire::{HandshakePayload, PacketBody, PacketHeader, PacketType, Seq};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("version mismatch: local={local}, remote={remote}")]
    VersionMismatch { local: String, remote: String },
    #[error("game-cycle mismatch: local={local}, remote={remote}")]
    GameCycleMismatch { local: u8, remote: u8 },
    #[error("handshake timed out after {0:?}")]
    HandshakeTimeout(Duration),
    #[error("serialisation: {0}")]
    Ser(String),
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub peer: PeerId,
    pub mod_version: String,
    pub game_version: String,
    pub save_hash: u64,
    pub game_cycle: u8,
    pub handshake_timeout: Duration,
    pub reconnect_grace: Duration,
    pub heartbeat_period: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            peer: PeerId::Host,
            mod_version: MOD_VERSION.to_string(),
            game_version: String::from("1.06"),
            save_hash: 0,
            game_cycle: 0,
            handshake_timeout: Duration::from_secs(5),
            reconnect_grace: Duration::from_secs(10),
            heartbeat_period: Duration::from_millis(500),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeOutcome {
    Ok(MatchSeed),
    RejectedVersion,
    RejectedGameCycle,
    TimedOut,
}

pub struct Session<T: Transport> {
    pub cfg: SessionConfig,
    pub transport: Mutex<T>,
    pub lobby: Lobby,
    pub match_seed: Option<MatchSeed>,
    pub reliability: Reliability,
    pub recv_ack: Mutex<RecvAckState>,
    pub grace: GraceBuffer,
    next_seq: Mutex<Seq>,
    last_heartbeat: Mutex<Instant>,
    last_peer_contact: Mutex<Instant>,
}

impl<T: Transport> Session<T> {
    pub fn new(cfg: SessionConfig, transport: T, lobby: Lobby) -> Self {
        Self {
            cfg,
            transport: Mutex::new(transport),
            lobby,
            match_seed: None,
            reliability: Reliability::new(),
            recv_ack: Mutex::new(RecvAckState::default()),
            grace: GraceBuffer::new(),
            next_seq: Mutex::new(Seq(0)),
            last_heartbeat: Mutex::new(Instant::now()),
            last_peer_contact: Mutex::new(Instant::now()),
        }
    }

    /// Classify the link based on the last peer-contact timestamp.
    pub fn link_state(&self) -> LinkState {
        classify_link(
            *self.last_peer_contact.lock(),
            self.cfg.heartbeat_period,
            self.cfg.reconnect_grace,
        )
    }

    /// On re-contact after a suspect link, flush any buffered packets.
    /// Returns how many were drained.
    pub fn flush_grace_buffer(&self) -> Result<usize, SessionError> {
        let drained = self.grace.drain();
        let count = drained.len();
        let mut t = self.transport.lock();
        for bytes in drained {
            t.send(&bytes)?;
        }
        Ok(count)
    }

    /// Request a full state snapshot from the peer.  Idempotent while
    /// one is already in flight.
    pub fn request_full_snapshot(&self, for_frame: u64) -> Result<(), SessionError> {
        if self.grace.has_pending_snapshot_request() {
            return Ok(());
        }
        let mut seq = self.next_seq.lock();
        let s = *seq;
        *seq = seq.next();
        drop(seq);
        let mut header = PacketHeader::new(PacketType::DesyncReport, s, true, true);
        stamp_ack(&mut header, &self.recv_ack.lock());
        let body = PacketBody::DesyncReport {
            frame: for_frame,
            my_hash: 0,
        };
        let bytes = encode(&header, &body).map_err(|e| SessionError::Ser(e.to_string()))?;
        self.transport.lock().send(&bytes)?;
        self.reliability.track(s, bytes);
        self.grace.mark_snapshot_request(s);
        Ok(())
    }

    /// Exchange handshake payloads with the peer.  Host generates the
    /// `MatchSeed` and broadcasts; Client validates and mirrors.
    pub fn handshake(&mut self) -> Result<HandshakeOutcome, SessionError> {
        let seed = if self.cfg.peer == PeerId::Host {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO);
            let entropy = (now.as_nanos() as u64)
                ^ (now.as_secs().wrapping_mul(0x9E37_79B9_7F4A_7C15));
            MatchSeed::new(entropy.wrapping_add(self.cfg.save_hash))
        } else {
            MatchSeed::new(0) // client fills in from remote handshake
        };

        let payload = HandshakePayload {
            mod_version: self.cfg.mod_version.clone(),
            game_version: self.cfg.game_version.clone(),
            save_hash: self.cfg.save_hash,
            game_cycle: self.cfg.game_cycle,
            match_seed: seed,
            peer: self.cfg.peer,
        };

        let body = PacketBody::Handshake(payload);
        self.send_reliable(PacketType::Handshake, &body)?;

        // Wait for peer handshake.
        let deadline = Instant::now() + self.cfg.handshake_timeout;
        let mut buf = [0u8; 8192];
        loop {
            if Instant::now() >= deadline {
                return Ok(HandshakeOutcome::TimedOut);
            }
            let mut t = self.transport.lock();
            match t.recv(&mut buf)? {
                None => {
                    drop(t);
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Some(n) => {
                    let (_hdr, remote) = decode(&buf[..n])
                        .map_err(|e| SessionError::Ser(e.to_string()))?;
                    if let PacketBody::Handshake(hp) = remote {
                        if hp.mod_version != self.cfg.mod_version
                            || hp.game_version != self.cfg.game_version
                        {
                            return Ok(HandshakeOutcome::RejectedVersion);
                        }
                        if hp.game_cycle != self.cfg.game_cycle {
                            return Ok(HandshakeOutcome::RejectedGameCycle);
                        }
                        let final_seed = match self.cfg.peer {
                            PeerId::Host => seed,
                            PeerId::Client => hp.match_seed,
                        };
                        self.match_seed = Some(final_seed);
                        return Ok(HandshakeOutcome::Ok(final_seed));
                    }
                    // Non-handshake packets before handshake complete
                    // are discarded.
                }
            }
        }
    }

    pub fn send_reliable(
        &self,
        ty: PacketType,
        body: &PacketBody,
    ) -> Result<(), SessionError> {
        let mut seq = self.next_seq.lock();
        let s = *seq;
        *seq = seq.next();
        drop(seq);
        let mut header = PacketHeader::new(ty, s, true, true);
        stamp_ack(&mut header, &self.recv_ack.lock());
        let bytes = encode(&header, body).map_err(|e| SessionError::Ser(e.to_string()))?;
        // Track for retransmit regardless — if the peer's back, they'll
        // ack and the tracker clears.
        self.reliability.track(s, bytes.clone());
        match self.link_state() {
            LinkState::Up => {
                self.transport.lock().send(&bytes)?;
            }
            LinkState::Suspect => {
                // Buffer for eventual flush; also attempt send in case
                // the peer just quietly came back.
                self.grace.push(bytes.clone());
                let _ = self.transport.lock().send(&bytes);
            }
            LinkState::Expired => {
                // Drop silently; caller should observe
                // `peer_timed_out()` and end the session.
            }
        }
        Ok(())
    }

    pub fn send_unreliable(
        &self,
        ty: PacketType,
        body: &PacketBody,
    ) -> Result<(), SessionError> {
        // Don't queue unreliable traffic (state channel) while suspect —
        // just drop it; next full snapshot will catch the peer up.
        if self.link_state() != LinkState::Up {
            return Ok(());
        }
        let mut seq = self.next_seq.lock();
        let s = *seq;
        *seq = seq.next();
        drop(seq);
        let mut header = PacketHeader::new(ty, s, false, false);
        stamp_ack(&mut header, &self.recv_ack.lock());
        let bytes = encode(&header, body).map_err(|e| SessionError::Ser(e.to_string()))?;
        self.transport.lock().send(&bytes)?;
        Ok(())
    }

    /// Walk the retransmit queue and re-send anything past its RTO.
    /// Callers invoke this each tick.
    pub fn drive_retransmits(&self) -> Result<usize, SessionError> {
        let due = self.reliability.due_for_retransmit(Instant::now());
        let n = due.len();
        let mut t = self.transport.lock();
        for bytes in due {
            t.send(&bytes)?;
        }
        Ok(n)
    }

    pub fn tick_heartbeat(&self, remote_frame: u64) -> Result<(), SessionError> {
        let now = Instant::now();
        let mut last = self.last_heartbeat.lock();
        if now.duration_since(*last) < self.cfg.heartbeat_period {
            return Ok(());
        }
        *last = now;
        drop(last);
        self.send_unreliable(
            PacketType::Heartbeat,
            &PacketBody::Heartbeat {
                remote_frame,
                ping_tag: 0,
            },
        )
    }

    pub fn poll_packet(&self, buf: &mut [u8]) -> Result<Option<(PacketHeader, PacketBody)>, SessionError> {
        let mut t = self.transport.lock();
        match t.recv(buf)? {
            None => Ok(None),
            Some(n) => {
                drop(t);
                let pair = decode(&buf[..n]).map_err(|e| SessionError::Ser(e.to_string()))?;
                let (header, _) = &pair;
                // Update peer-contact timestamp (reconnect grace).
                *self.last_peer_contact.lock() = Instant::now();
                // Apply the peer's ack to clear acked outstandings.
                self.reliability
                    .apply_remote_ack(header.ack, header.ack_bits);
                // Record this packet for our own ack state (reliable
                // packets only — we don't ack unreliable state ones).
                if header.reliable() {
                    self.recv_ack.lock().record(header.seq);
                }
                Ok(Some(pair))
            }
        }
    }

    /// True iff we haven't heard from the peer within the reconnect grace.
    pub fn peer_timed_out(&self) -> bool {
        Instant::now().duration_since(*self.last_peer_contact.lock())
            > self.cfg.reconnect_grace
    }
}

pub fn encode(header: &PacketHeader, body: &PacketBody) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&(header, body))
}

pub fn decode(bytes: &[u8]) -> bincode::Result<(PacketHeader, PacketBody)> {
    bincode::deserialize(bytes)
}
