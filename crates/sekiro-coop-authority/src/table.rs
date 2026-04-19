//! Per-entity authority table.  SPEC §6.1.
//!
//! The mod's table is the source of truth; every change is mirrored to
//! the engine by calling the native `SetNetworkUpdateAuthority`
//! function (EMEVD Character #28).

use parking_lot::RwLock;
use sekiro_sdk_core::entity::{EntityId, EntityKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which of the two peers this process represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PeerId {
    Host,
    Client,
}

impl PeerId {
    pub fn other(self) -> Self {
        match self {
            PeerId::Host => PeerId::Client,
            PeerId::Client => PeerId::Host,
        }
    }
}

/// The authority level for an entity from THIS peer's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityLevel {
    /// This peer owns the entity — runs its simulation authoritatively.
    Local,
    /// The other peer owns the entity — we receive state/events.
    Remote,
    /// Symmetric; both peers compute and host tiebreaks.  Avoid if
    /// possible (see SPEC §6.1).
    Shared,
}

impl AuthorityLevel {
    pub fn is_local(self) -> bool {
        matches!(self, AuthorityLevel::Local | AuthorityLevel::Shared)
    }

    pub fn is_remote(self) -> bool {
        matches!(self, AuthorityLevel::Remote)
    }
}

/// Central table.  Keyed by entity ID.
#[derive(Debug)]
pub struct AuthorityTable {
    pub peer: PeerId,
    table: RwLock<HashMap<EntityId, AuthorityLevel>>,
}

impl AuthorityTable {
    pub fn new(peer: PeerId) -> Self {
        Self {
            peer,
            table: RwLock::new(HashMap::new()),
        }
    }

    /// Look up authority for an entity; unknown entities default to
    /// [`AuthorityLevel::Remote`] (fail-safe: don't speak for things we
    /// haven't explicitly claimed).
    pub fn get(&self, id: EntityId) -> AuthorityLevel {
        self.table
            .read()
            .get(&id)
            .copied()
            .unwrap_or(AuthorityLevel::Remote)
    }

    pub fn set(&self, id: EntityId, level: AuthorityLevel) {
        self.table.write().insert(id, level);
    }

    /// Convenience predicates mirroring SPEC §4.1 pseudocode.
    pub fn owns(&self, id: EntityId) -> bool {
        self.get(id).is_local()
    }

    /// The deciding peer for a (attacker, victim) pair — host tiebreaks.
    pub fn deciding_peer(&self, attacker: EntityId, victim: EntityId) -> PeerId {
        let a = self.get(attacker);
        let v = self.get(victim);
        match (a, v) {
            (AuthorityLevel::Local, _) | (_, AuthorityLevel::Local) => self.peer,
            _ => PeerId::Host, // tiebreak
        }
    }

    pub fn is_local(&self, peer: PeerId) -> bool {
        self.peer == peer
    }

    pub fn len(&self) -> usize {
        self.table.read().len()
    }

    /// Apply the default assignments from SPEC §6.1 for known entity
    /// kinds.  Callers pass the list of entities currently loaded.
    pub fn apply_defaults(&self, entities: &[(EntityId, EntityKind, DefaultHint)]) {
        for (id, kind, hint) in entities {
            let level = default_level(self.peer, *kind, *hint);
            self.set(*id, level);
        }
    }
}

/// Side-channel facts needed to pick a default authority level.
#[derive(Debug, Clone, Copy)]
pub struct DefaultHint {
    pub host_targets: bool,
    pub client_targets: bool,
    pub host_in_proximity: bool,
    pub client_in_proximity: bool,
    pub is_host_player: bool,
    pub is_client_player: bool,
}

impl DefaultHint {
    pub const NONE: DefaultHint = DefaultHint {
        host_targets: false,
        client_targets: false,
        host_in_proximity: false,
        client_in_proximity: false,
        is_host_player: false,
        is_client_player: false,
    };
}

fn default_level(me: PeerId, kind: EntityKind, h: DefaultHint) -> AuthorityLevel {
    if h.is_host_player {
        return if me == PeerId::Host { AuthorityLevel::Local } else { AuthorityLevel::Remote };
    }
    if h.is_client_player {
        return if me == PeerId::Client { AuthorityLevel::Local } else { AuthorityLevel::Remote };
    }
    let host = match kind {
        EntityKind::Boss => match (h.host_targets, h.client_targets) {
            (true, false) => PeerId::Host,
            (false, true) => PeerId::Client,
            (true, true) | (false, false) => PeerId::Host, // tiebreak
        },
        EntityKind::Enemy => match (h.host_in_proximity, h.client_in_proximity) {
            (true, false) => PeerId::Host,
            (false, true) => PeerId::Client,
            (true, true) | (false, false) => PeerId::Host,
        },
        _ => PeerId::Host, // NPCs, objects
    };
    if host == me {
        AuthorityLevel::Local
    } else {
        AuthorityLevel::Remote
    }
}
