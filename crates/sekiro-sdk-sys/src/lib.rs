//! Layer 1 — hook substrate, memory, AOB scanning, pointer chains.
//!
//! All Sekiro-version-sensitive addresses live here. No game logic.
//!
//! References: SEKIRO_SEAMLESS_ROLLBACK_SPEC.md §3, OSINT §1.1-§1.2.

#![allow(clippy::missing_safety_doc)]

pub mod aob;
pub mod ce_table;
pub mod chrins;
pub mod chrins_discover;
pub mod live;
pub mod memory;
pub mod natives;
pub mod offsets;
pub mod paramrepo;
pub mod params;
pub mod version;
pub mod worldchrman;
pub mod worldchrman_scan;

pub use aob::{AobPattern, ScanError};
pub use ce_table::{load_chrins_layout_from_path, parse_ce_table, ChrInsFields, CeError};
pub use memory::{Module, PtrChain, RawPtr};
pub use offsets::{BaseAddrs, Symbol};
pub use version::{GameVersion, detect_version};

/// The player's EMEVD entity ID is always 10000 (distinct from c0000 model ID).
/// See OSINT §4.
pub const PLAYER_ENTITY_ID: u32 = 10_000;
