//! `common.emevd` patching — structured over the real EMEVD format.
//!
//! See [`crate::format`] for the on-disk schema.  This module owns the
//! high-level transforms:
//!
//! 1. Parse a real EMEVD file.
//! 2. Promote SOLO-branches so multiplayer paths fire.
//! 3. Inject custom events 99000-99003.
//! 4. Write back.

use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::format::{Emevd as EmevdFile, FormatError, RawInstruction};
use crate::gen::{Event, EmevdProgram, Instruction, RestartKind};

#[derive(Debug, Error)]
pub enum EmevdError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("format: {0}")]
    Format(#[from] FormatError),
}

/// Top-level wrapper so callers only depend on `patch::`.
#[derive(Debug, Clone)]
pub struct Emevd {
    pub file: EmevdFile,
}

impl Emevd {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EmevdError> {
        let bytes = std::fs::read(path)?;
        Ok(Self {
            file: EmevdFile::parse(&bytes)?,
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), EmevdError> {
        std::fs::write(path, self.file.serialise())?;
        Ok(())
    }

    /// Rewrite every `IfMultiplayerState(SOLO)` / `SkipIfMultiplayerState(SOLO)`
    /// / `GotoIfMultiplayerState(SOLO)` to use `new_state` (Host / Client).
    pub fn promote_solo_branches(&mut self, new_state: u8) -> usize {
        self.file.promote_solo_branches(new_state)
    }

    /// Inject a pre-built program (events 99000-99003) at the end of
    /// the table.  Instruction offsets inside the program are renumbered
    /// to target the appended arg region.
    pub fn inject_program(&mut self, prog: &EmevdProgram) {
        for (id, event) in prog.events.iter() {
            let instrs = event_to_raw_instructions(event);
            let end_type = match event.restart {
                RestartKind::Restart => 1,
                RestartKind::End | RestartKind::Default => 0,
            };
            self.file.append_event(*id as u64, end_type, instrs);
        }
    }
}

fn event_to_raw_instructions(event: &Event) -> Vec<(RawInstruction, Vec<u8>)> {
    event
        .body
        .iter()
        .map(|ins| {
            let raw = RawInstruction {
                class: ins.class,
                instruction: ins.instruction,
                arg_length: 0, // filled in by append_event
                arg_offset: 0, // filled in by append_event
            };
            let args = flatten_args(ins);
            (raw, args)
        })
        .collect()
}

fn flatten_args(ins: &Instruction) -> Vec<u8> {
    let mut buf = Vec::new();
    for a in &ins.args {
        buf.extend_from_slice(&a.to_le_bytes());
    }
    // 4-byte-align per engine convention.
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
    buf
}

/// Engine `MultiplayerState` enum values.
///
/// **Was P2 gap #13 — now resolved via SEKIRO_MULTIPLAYER.md (soulsmods
/// emedf, DarkScript3 project):**
///
/// ```text
/// Host = 0
/// Client = 1
/// TryingToCreateSession = 2
/// TryingToJoinSession = 3
/// LeavingSession = 4
/// FailedToCreateSession = 5
/// ```
///
/// There is **no explicit `Solo` value** — Sekiro's EMEVD branches gate
/// on "not Host and not Client" for solo play (the condition fails for
/// both `MultiplayerState == 0` and `== 1`).  Callers asking for
/// `SOLO` should branch on "!= Host && != Client" instead.
#[allow(non_snake_case)]
pub mod MultiplayerState {
    pub const HOST: u8 = 0;
    pub const CLIENT: u8 = 1;
    pub const TRYING_TO_CREATE_SESSION: u8 = 2;
    pub const TRYING_TO_JOIN_SESSION: u8 = 3;
    pub const LEAVING_SESSION: u8 = 4;
    pub const FAILED_TO_CREATE_SESSION: u8 = 5;

    /// Sentinel: the value an EMEVD `IfMultiplayerState(SOLO)` check
    /// effectively looks at.  Our `promote_solo_branches` historically
    /// wrote `0` to turn solo paths into multiplayer paths — that now
    /// matches `HOST`, which is the right default for a host-promoted
    /// profile.  Deprecated; kept for back-compat.
    pub const SOLO: u8 = 0;
}

/// A full patch plan: input path, output path, transformations.
#[derive(Debug, Clone)]
pub struct PatchPlan {
    pub input: PathBuf,
    pub output: PathBuf,
    pub promote_to: u8,
    pub inject_events: Option<EmevdProgram>,
}

impl PatchPlan {
    pub fn run(&self) -> Result<PatchReport, EmevdError> {
        let mut emevd = Emevd::load(&self.input)?;
        let promoted = emevd.promote_solo_branches(self.promote_to);
        let injected = match &self.inject_events {
            Some(p) => {
                emevd.inject_program(p);
                p.events.len()
            }
            None => 0,
        };
        emevd.save(&self.output)?;
        Ok(PatchReport { promoted, injected })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PatchReport {
    pub promoted: usize,
    pub injected: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Emevd as EmevdFile, Header, RawEvent};
    use crate::gen::{build_custom_events, class};

    fn with_solo_branch() -> EmevdFile {
        let arg_bytes = vec![0, MultiplayerState::SOLO, 0, 0];
        let mut e = EmevdFile {
            header: Header::default(),
            events: vec![RawEvent {
                id: 1,
                instruction_count: 1,
                instruction_start_idx: 0,
                parameter_count: 0,
                parameter_start_idx: 0,
                end_type: 0,
            }],
            instructions: vec![RawInstruction {
                class: class::CONDITION_EVENT,
                instruction: 7,
                arg_length: 2,
                arg_offset: 0,
            }],
            parameters: Vec::new(),
            linked_events: Vec::new(),
            arg_region: arg_bytes,
            strings: Vec::new(),
        };
        e.header.unk08 = 0xCC;
        e
    }

    #[test]
    fn promote_then_inject_then_roundtrip() {
        let mut e = Emevd {
            file: with_solo_branch(),
        };
        let promoted = e.promote_solo_branches(MultiplayerState::HOST);
        assert_eq!(promoted, 1);

        let prog = build_custom_events((30_000, 30_063), &[5080, 5100]);
        e.inject_program(&prog);

        let bytes = e.file.serialise();
        let reparsed = EmevdFile::parse(&bytes).expect("reparse");

        // Original event + 4 injected = 5 total.
        assert_eq!(reparsed.events.len(), 5);
        assert_eq!(reparsed.events[0].id, 1);
        assert_eq!(reparsed.events[1].id, 99_000);
        assert_eq!(reparsed.events[4].id, 99_003);

        // The SOLO byte became HOST (=1).
        let args = reparsed
            .instruction_args(0)
            .expect("original instruction args");
        assert_eq!(args[1], MultiplayerState::HOST);
    }
}
