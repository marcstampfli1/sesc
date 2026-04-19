//! Layer 1 — typed entity API, hook primitives, AtkParam/SpEffect wrappers.
//!
//! No game logic: this layer exposes typed accessors over Layer 0
//! (`sekiro-sdk-sys`) memory.  SPEC §3.

pub mod animation;
pub mod atkparam;
pub mod characters;
pub mod debug;
pub mod debug_patch;
pub mod entity;
pub mod enums;
pub mod hook;
pub mod items;
pub mod speffect;
pub mod tae;

pub use animation::AnimationId;
pub use atkparam::{AtkParam, AtkParamField};
pub use characters::{bosses, by_id as character_by_id, name_of as character_name, CharacterInfo, CHARACTERS};
pub use debug::DebugFlags;
pub use entity::{Boss, Enemy, Entity, EntityKind, EntityId, Player, World};
pub use hook::{Hook, HookError, HookRegistry};
pub use speffect::SpEffectId;
