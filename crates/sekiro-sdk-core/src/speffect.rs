//! SpEffect ID newtype + reserved-range policy.

/// An SpEffectParam row ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct SpEffectId(pub i32);

impl SpEffectId {
    /// IDs ≥ this are reserved by the mod for custom effects.  Using this
    /// range avoids collisions with vanilla SpEffectParam rows.  SPEC §3.
    pub const CUSTOM_BASE: i32 = 90_000;

    pub fn is_custom(&self) -> bool {
        self.0 >= Self::CUSTOM_BASE
    }
}
