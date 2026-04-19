//! Typed entity wrappers over `ChrIns`.

use sekiro_sdk_sys::chrins::{ChrInsLayout, ChrInsSnapshot, read_snapshot};
use sekiro_sdk_sys::memory::RawPtr;
use sekiro_sdk_sys::worldchrman::{ChrInsIter, WorldChrManLayout};
use serde::{Deserialize, Serialize};

/// EMEVD entity ID.  Distinct from the c-number model ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct EntityId(pub u32);

impl EntityId {
    /// Player's fixed EMEVD entity ID.  OSINT §4.
    pub const PLAYER: EntityId = EntityId(10_000);
}

/// Character-model classification based on c-number ranges (OSINT §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Player,       // c0000
    Invisible,    // c1000–c1001
    Enemy,        // c1xxx (regular enemies)
    Boss,         // c5xxx
    Npc,          // c7xxx
    Object,       // o-prefix
    Unknown,
}

impl EntityKind {
    pub fn from_char_id(char_id: u32) -> Self {
        match char_id {
            0 => EntityKind::Player,
            1000 | 1001 => EntityKind::Invisible,
            1002..=1999 => EntityKind::Enemy,
            5000..=5999 => EntityKind::Boss,
            7000..=7999 => EntityKind::Npc,
            _ if char_id >= 100_000 => EntityKind::Object,
            _ => EntityKind::Unknown,
        }
    }
}

/// Typed wrapper over a `ChrIns*` + layout pair.
///
/// Entities are cheap copies — they're just a pointer plus a layout
/// reference; the actual data is read on demand.
#[derive(Debug, Clone, Copy)]
pub struct Entity<'a> {
    pub ptr: RawPtr,
    pub layout: &'a ChrInsLayout,
}

impl<'a> Entity<'a> {
    pub fn new(ptr: RawPtr, layout: &'a ChrInsLayout) -> Self {
        Self { ptr, layout }
    }

    /// Read an immutable snapshot.
    ///
    /// # Safety
    /// `ptr` + `layout` must be live for this tick.
    pub unsafe fn snapshot(&self) -> ChrInsSnapshot {
        read_snapshot(self.ptr, self.layout)
    }

    /// # Safety
    /// See [`Self::snapshot`].
    pub unsafe fn kind(&self) -> EntityKind {
        EntityKind::from_char_id(self.snapshot().char_id)
    }

    /// # Safety
    /// See [`Self::snapshot`].
    pub unsafe fn id(&self) -> EntityId {
        EntityId(self.snapshot().entity_id)
    }

    /// # Safety
    /// See [`Self::snapshot`].
    pub unsafe fn position(&self) -> [f32; 3] {
        self.snapshot().position
    }

    /// # Safety
    /// See [`Self::snapshot`].
    pub unsafe fn distance_to(&self, other: &Entity<'_>) -> f32 {
        let a = self.position();
        let b = other.position();
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Newtype for player (always c0000).
pub type Player<'a> = Entity<'a>;
/// Newtype for enemies (c1xxx).
pub type Enemy<'a> = Entity<'a>;
/// Newtype for bosses (c5xxx).
pub type Boss<'a> = Entity<'a>;

/// World view — iterates loaded entities and classifies them.
pub struct World<'a> {
    pub chrins: &'a ChrInsLayout,
    pub wcm: &'a WorldChrManLayout,
    pub world_ptr: RawPtr,
}

impl<'a> World<'a> {
    pub fn new(world_ptr: RawPtr, wcm: &'a WorldChrManLayout, chrins: &'a ChrInsLayout) -> Self {
        Self { chrins, wcm, world_ptr }
    }

    /// # Safety
    /// `world_ptr` must be live; iterate from the tick hook.
    pub unsafe fn entities(&self) -> impl Iterator<Item = Entity<'a>> + '_ {
        let chrins = self.chrins;
        ChrInsIter::from_world(self.world_ptr, self.wcm)
            .map(move |p| Entity::new(p, chrins))
    }

    /// # Safety
    /// See [`Self::entities`].
    pub unsafe fn players(&self) -> Vec<Player<'a>> {
        self.entities()
            .filter(|e| e.kind() == EntityKind::Player)
            .collect()
    }

    /// # Safety
    /// See [`Self::entities`].
    pub unsafe fn enemies(&self) -> Vec<Enemy<'a>> {
        self.entities()
            .filter(|e| matches!(e.kind(), EntityKind::Enemy))
            .collect()
    }

    /// # Safety
    /// See [`Self::entities`].
    pub unsafe fn bosses(&self) -> Vec<Boss<'a>> {
        self.entities()
            .filter(|e| e.kind() == EntityKind::Boss)
            .collect()
    }

    /// Find the entity with a specific entity ID.
    ///
    /// # Safety
    /// See [`Self::entities`].
    pub unsafe fn find(&self, id: EntityId) -> Option<Entity<'a>> {
        self.entities().find(|e| e.id() == id)
    }
}
