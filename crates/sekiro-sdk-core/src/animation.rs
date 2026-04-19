//! Animation ID constants + deflect-animation-set lookup.
//!
//! Per-character deflect-animation IDs are a **P1 gap** (SPEC §11 #8),
//! built up empirically during Phase C.  This module holds the runtime
//! table; entries are added via [`DeflectAnimTable::learn`] when a hit
//! is observed with `AtkStam > 0` and the victim is playing an animation
//! not yet in the table.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Animation ID — opaque u32 value read from `ChrIns.animation_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct AnimationId(pub u32);

/// Per-character-model set of animation IDs that represent a deflect/parry state.
#[derive(Debug, Default, Clone)]
pub struct DeflectAnimTable {
    by_char: HashMap<u32, HashSet<u32>>,
}

impl DeflectAnimTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// True iff `(char_id, anim_id)` is a known deflect animation.
    pub fn contains(&self, char_id: u32, anim_id: AnimationId) -> bool {
        self.by_char
            .get(&char_id)
            .map(|s| s.contains(&anim_id.0))
            .unwrap_or(false)
    }

    /// Record a deflect animation for a character.
    pub fn learn(&mut self, char_id: u32, anim_id: AnimationId) {
        self.by_char
            .entry(char_id)
            .or_default()
            .insert(anim_id.0);
    }

    /// Number of distinct (char, anim) pairs learned.
    pub fn size(&self) -> usize {
        self.by_char.values().map(|s| s.len()).sum()
    }
}
