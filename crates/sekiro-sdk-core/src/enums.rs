//! Engine enum values for Sekiro's multiplayer subsystem.
//!
//! Source: `SEKIRO_MULTIPLAYER.md §1.5` (soulsmods EMEDF,
//! DarkScript3 project).  1.06-authoritative.

/// `MultiplayerState` — argument of `IfMultiplayerState` / `GotoIfMultiplayerState`.
/// No explicit "Solo" value; "solo" play is the absence of Host/Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MultiplayerState {
    Host = 0,
    Client = 1,
    TryingToCreateSession = 2,
    TryingToJoinSession = 3,
    LeavingSession = 4,
    FailedToCreateSession = 5,
}

impl MultiplayerState {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `AuthorityLevel` — argument of `SetNetworkUpdateAuthority`.  Only two
/// documented values.  `Normal` = shared per-peer authority; `Forced`
/// = this entity's updates are 100% the local peer's responsibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AuthorityLevel {
    Normal = 0,
    Forced = 4095,
}

impl AuthorityLevel {
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

/// `CharacterUpdateFrequency` — argument of `SetNetworkUpdateRate`.
/// Use `NoUpdate` to freeze an entity on the wire; `AlwaysUpdate` for
/// boss fights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum CharacterUpdateFrequency {
    NoUpdate = -1,
    AlwaysUpdate = 0,
    Every2Frames = 2,
    Every5Frames = 5,
}

impl CharacterUpdateFrequency {
    pub const fn as_i8(self) -> i8 {
        self as i8
    }
}

/// `ClientType` — argument of `SkipIfNumberOfClientsOfType` and family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClientType {
    Coop = 0,
    Invader = 1,
    BetrayalInvader = 2,
}

impl ClientType {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `TeamType` — stored at `ChrIns + 0x74`.  Used to classify who is
/// friendly/hostile to whom in multiplayer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum TeamType {
    Default = -1,
    Disabled = 0,
    Human = 1,
    WhitePhantom = 2,
    BlackPhantom = 3,
    Hollow = 4,
    WanderingPhantom = 5,
    Enemy = 6,
    StrongEnemy = 7,
    Ally = 8,
    HostileAlly = 9,
    DecoyEnemy = 10,
    ChildOfRed = 11,
    FriendlyEnemy = 12,
    Invader = 13,
    Host = 19,
    Coop = 20,
    Hostile = 21,
    Enemy1 = 23,
    Enemy2 = 24,
    FriendlyNpc = 26,
    HostileNpc = 27,
    CoopNpc = 28,
    Other(i8),
}

impl TeamType {
    pub fn from_raw(v: u8) -> Self {
        let v = v as i8;
        match v {
            -1 => TeamType::Default,
            0 => TeamType::Disabled,
            1 => TeamType::Human,
            2 => TeamType::WhitePhantom,
            3 => TeamType::BlackPhantom,
            4 => TeamType::Hollow,
            5 => TeamType::WanderingPhantom,
            6 => TeamType::Enemy,
            7 => TeamType::StrongEnemy,
            8 => TeamType::Ally,
            9 => TeamType::HostileAlly,
            10 => TeamType::DecoyEnemy,
            11 => TeamType::ChildOfRed,
            12 => TeamType::FriendlyEnemy,
            13 => TeamType::Invader,
            19 => TeamType::Host,
            20 => TeamType::Coop,
            21 => TeamType::Hostile,
            23 => TeamType::Enemy1,
            24 => TeamType::Enemy2,
            26 => TeamType::FriendlyNpc,
            27 => TeamType::HostileNpc,
            28 => TeamType::CoopNpc,
            other => TeamType::Other(other),
        }
    }

    /// Numeric value, suitable for `SetCharacterTeamType` (Character #2).
    pub fn as_i8(self) -> i8 {
        match self {
            TeamType::Default => -1,
            TeamType::Disabled => 0,
            TeamType::Human => 1,
            TeamType::WhitePhantom => 2,
            TeamType::BlackPhantom => 3,
            TeamType::Hollow => 4,
            TeamType::WanderingPhantom => 5,
            TeamType::Enemy => 6,
            TeamType::StrongEnemy => 7,
            TeamType::Ally => 8,
            TeamType::HostileAlly => 9,
            TeamType::DecoyEnemy => 10,
            TeamType::ChildOfRed => 11,
            TeamType::FriendlyEnemy => 12,
            TeamType::Invader => 13,
            TeamType::Host => 19,
            TeamType::Coop => 20,
            TeamType::Hostile => 21,
            TeamType::Enemy1 => 23,
            TeamType::Enemy2 => 24,
            TeamType::FriendlyNpc => 26,
            TeamType::HostileNpc => 27,
            TeamType::CoopNpc => 28,
            TeamType::Other(v) => v,
        }
    }

    /// True for any team we'd want to treat as "friendly" during
    /// multiplayer sync (damage dealt by/to these is friendly fire).
    pub fn is_friendly_player(self) -> bool {
        matches!(
            self,
            TeamType::Host
                | TeamType::Coop
                | TeamType::WhitePhantom
                | TeamType::WanderingPhantom
                | TeamType::CoopNpc
                | TeamType::FriendlyNpc
                | TeamType::Ally
        )
    }

    /// True for entities that should receive hostile damage in multiplayer.
    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            TeamType::Enemy
                | TeamType::StrongEnemy
                | TeamType::Hostile
                | TeamType::Invader
                | TeamType::BlackPhantom
                | TeamType::Enemy1
                | TeamType::Enemy2
                | TeamType::HostileAlly
                | TeamType::HostileNpc
        )
    }
}

/// `SummonSignType` — argument of `PlaceNpcSummonSign` etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SummonSignType {
    White = 0,
    Black = 1,
    Red = 2,
    Detection = 3,
    WhiteRelief = 4,
    BlackRelief = 5,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_type_roundtrip() {
        for t in [
            TeamType::Host,
            TeamType::Coop,
            TeamType::Enemy,
            TeamType::Hostile,
            TeamType::Human,
        ] {
            let raw = t.as_i8() as u8;
            assert_eq!(TeamType::from_raw(raw), t);
        }
    }

    #[test]
    fn team_type_friendly_hostile() {
        assert!(TeamType::Host.is_friendly_player());
        assert!(TeamType::Coop.is_friendly_player());
        assert!(!TeamType::Enemy.is_friendly_player());
        assert!(TeamType::Enemy.is_hostile());
        assert!(TeamType::Invader.is_hostile());
        assert!(!TeamType::Host.is_hostile());
    }

    #[test]
    fn multiplayer_state_values() {
        assert_eq!(MultiplayerState::Host.as_u8(), 0);
        assert_eq!(MultiplayerState::Client.as_u8(), 1);
    }

    #[test]
    fn authority_level_values() {
        assert_eq!(AuthorityLevel::Normal.as_u32(), 0);
        assert_eq!(AuthorityLevel::Forced.as_u32(), 4095);
    }
}
