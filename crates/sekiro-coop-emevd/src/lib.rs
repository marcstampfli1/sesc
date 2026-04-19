//! Offline EMEVD patcher.  SPEC §8.
//!
//! Loads `common.emevd.dcx`, rewrites `IfMultiplayerState(SOLO)` branches
//! to take the multiplayer path, injects custom events 99000-99003.

pub mod catalog;
pub mod format;
pub mod gen;
pub mod patch;

pub use catalog::{by_class_index, by_name, emit_by_name, ArgType, Spec, CATALOG};
pub use format::{
    Emevd as EmevdFile, FormatError, Header, RawEvent, RawInstruction, RawLinkedEvent,
    RawParameter,
};
pub use gen::{Arg, EmevdProgram, Event, EventEndType, Instruction, InstructionBuilder, RestartKind};
pub use patch::{Emevd, EmevdError, PatchPlan};
