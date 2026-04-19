//! EMEVD binary format reader/writer.
//!
//! Based on the DS3/Sekiro layout documented at `soulsmodding.wikidot.com`
//! and implemented by `SoulsFormatsNEXT.EMEVD`.  The on-disk layout:
//!
//! ```text
//! +--- Header (72+ bytes) ---+
//! | magic "EVD\0"            |
//! | version / flags          |
//! | counts & offsets of 6 tables:
//! |   events, instructions, parameters,
//! |   linked events, argument region, string region
//! +--------------------------+
//! | Event table              |  Event {id, inst_count, inst_idx, param_count, param_idx, end_type}
//! | Instruction table        |  Instruction {class, idx, arg_len, arg_offset, layer}
//! | Parameter table          |  Parameter {inst_idx, dst_off, src_off, byte_count}
//! | Linked event table       |  LinkedEvent {file_idx}
//! | Argument region (bytes)  |  Per-instruction raw arg bytes
//! | String region (optional) |
//! +--------------------------+
//! ```
//!
//! DCX compression is **out-of-scope**: run Yabber first to produce the
//! decompressed `.emevd`; run this tool on that; re-pack with Yabber.
//!
//! **Correctness caveat**: the field layout below matches the DS3/Sekiro
//! 64-bit variant per public docs.  I've tested round-trip against my
//! own writer (below); any surprise-field-in-the-wild still requires
//! validation against a known-good file.

use std::convert::TryInto;
use thiserror::Error;

const MAGIC: &[u8; 4] = b"EVD\0";

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("bad magic (expected 'EVD\\0', got {0:?})")]
    BadMagic([u8; 4]),
    #[error("unexpected end of file at offset {0}")]
    Truncated(usize),
    #[error("arg region out of bounds: off={off}, len={len}, total={total}")]
    ArgOutOfBounds { off: u64, len: u64, total: u64 },
    #[error("instruction index out of range: {0}")]
    BadInstructionIndex(u64),
    #[error("expected 4-byte-aligned argument at offset {0}")]
    Misalignment(u64),
}

/// Top-level EMEVD header.  80 bytes on-disk (u64-padded).
#[derive(Debug, Clone, Copy, Default)]
pub struct Header {
    pub unk04: u32,
    pub unk08: u32,
    pub unk0c: u32,
    pub file_size: u32,
    pub event_count: u64,
    pub event_offset: u64,
    pub instruction_count: u64,
    pub instruction_offset: u64,
    pub unk38: u64,
    pub event_layer_offset: u64,
    pub parameter_count: u64,
    pub parameter_offset: u64,
    pub linked_event_count: u64,
    pub linked_event_offset: u64,
    pub arg_length: u64,
    pub arg_offset: u64,
    pub string_length: u64,
    pub string_offset: u64,
}

/// Raw event record — 24 bytes (id, counts, offsets, end-type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawEvent {
    pub id: u64,
    pub instruction_count: u64,
    pub instruction_start_idx: u64,
    pub parameter_count: u64,
    pub parameter_start_idx: u64,
    pub end_type: u8,
}

/// Raw instruction record — 24 bytes on-disk.
///
/// DS3/Sekiro's actual format reserves a u32 for an event-layer table
/// pointer after `arg_offset`; this implementation intentionally
/// drops that field.  Sekiro's `common.emevd` does not use event
/// layers (they're a DS3 holdover), so serialising without them
/// produces a valid file.  If a real file *does* use event layers it
/// will parse successfully (the pointer bytes just get ignored) but
/// the information will not round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInstruction {
    pub class: u32,
    pub instruction: u32,
    pub arg_length: u64,
    pub arg_offset: u64,
}

/// Parameter passing record — forwards a caller's arg bytes into the
/// initialized event's args.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawParameter {
    pub instruction_index: u64,
    pub destination_start_byte: u64,
    pub source_start_byte: u64,
    pub byte_count: u64,
    pub unk18: u32,
}

/// Linked-event reference (cross-file event invocation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawLinkedEvent {
    pub file_index: u64,
}

/// The full parsed document.
#[derive(Debug, Clone)]
pub struct Emevd {
    pub header: Header,
    pub events: Vec<RawEvent>,
    pub instructions: Vec<RawInstruction>,
    pub parameters: Vec<RawParameter>,
    pub linked_events: Vec<RawLinkedEvent>,
    /// The raw argument region.  Each instruction's args live at
    /// `arg_region[ins.arg_offset..ins.arg_offset + ins.arg_length]`.
    pub arg_region: Vec<u8>,
    pub strings: Vec<u8>,
}

impl Emevd {
    pub fn parse(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() < 4 {
            return Err(FormatError::Truncated(0));
        }
        let magic: [u8; 4] = bytes[..4].try_into().unwrap();
        if &magic != MAGIC {
            return Err(FormatError::BadMagic(magic));
        }

        let r = Reader::new(bytes);
        let header = Header {
            unk04: r.u32(0x04)?,
            unk08: r.u32(0x08)?,
            unk0c: r.u32(0x0C)?,
            file_size: r.u32(0x10)?,
            event_count: r.u64(0x18)?,
            event_offset: r.u64(0x20)?,
            instruction_count: r.u64(0x28)?,
            instruction_offset: r.u64(0x30)?,
            unk38: r.u64(0x38)?,
            event_layer_offset: r.u64(0x40)?,
            parameter_count: r.u64(0x48)?,
            parameter_offset: r.u64(0x50)?,
            linked_event_count: r.u64(0x58)?,
            linked_event_offset: r.u64(0x60)?,
            arg_length: r.u64(0x68)?,
            arg_offset: r.u64(0x70)?,
            string_length: r.u64(0x78)?,
            string_offset: if bytes.len() >= 0x88 { r.u64(0x80)? } else { 0 },
        };

        let mut events = Vec::with_capacity(header.event_count as usize);
        for i in 0..header.event_count {
            let base = header.event_offset as usize + (i as usize) * 48;
            events.push(RawEvent {
                id: r.u64(base)?,
                instruction_count: r.u64(base + 8)?,
                instruction_start_idx: r.u64(base + 16)?,
                parameter_count: r.u64(base + 24)?,
                parameter_start_idx: r.u64(base + 32)?,
                end_type: r.u8(base + 40)?,
            });
        }

        let mut instructions = Vec::with_capacity(header.instruction_count as usize);
        for i in 0..header.instruction_count {
            let base = header.instruction_offset as usize + (i as usize) * 24;
            instructions.push(RawInstruction {
                class: r.u32(base)?,
                instruction: r.u32(base + 4)?,
                arg_length: r.u64(base + 8)?,
                arg_offset: r.u64(base + 16)?,
            });
        }

        let mut parameters = Vec::with_capacity(header.parameter_count as usize);
        for i in 0..header.parameter_count {
            let base = header.parameter_offset as usize + (i as usize) * 32;
            parameters.push(RawParameter {
                instruction_index: r.u64(base)?,
                destination_start_byte: r.u64(base + 8)?,
                source_start_byte: r.u64(base + 16)?,
                byte_count: r.u64(base + 24)?,
                unk18: r.u32(base + 28).unwrap_or(0),
            });
        }

        let mut linked_events = Vec::with_capacity(header.linked_event_count as usize);
        for i in 0..header.linked_event_count {
            let base = header.linked_event_offset as usize + (i as usize) * 8;
            linked_events.push(RawLinkedEvent {
                file_index: r.u64(base)?,
            });
        }

        let arg_start = header.arg_offset as usize;
        let arg_end = arg_start + header.arg_length as usize;
        if arg_end > bytes.len() {
            return Err(FormatError::ArgOutOfBounds {
                off: header.arg_offset,
                len: header.arg_length,
                total: bytes.len() as u64,
            });
        }
        let arg_region = bytes[arg_start..arg_end].to_vec();

        let strings = if header.string_length > 0 {
            let s = header.string_offset as usize;
            let e = s + header.string_length as usize;
            if e > bytes.len() {
                Vec::new()
            } else {
                bytes[s..e].to_vec()
            }
        } else {
            Vec::new()
        };

        Ok(Self {
            header,
            events,
            instructions,
            parameters,
            linked_events,
            arg_region,
            strings,
        })
    }

    /// Read this instruction's raw argument bytes (owned copy).
    pub fn instruction_args(&self, idx: usize) -> Result<&[u8], FormatError> {
        let ins = self
            .instructions
            .get(idx)
            .ok_or(FormatError::BadInstructionIndex(idx as u64))?;
        let off = ins.arg_offset as usize;
        let len = ins.arg_length as usize;
        let end = off + len;
        if end > self.arg_region.len() {
            return Err(FormatError::ArgOutOfBounds {
                off: ins.arg_offset,
                len: ins.arg_length,
                total: self.arg_region.len() as u64,
            });
        }
        Ok(&self.arg_region[off..end])
    }

    /// Write a new byte at `arg_region[instruction.arg_offset + byte_index]`.
    pub fn patch_arg_byte(&mut self, idx: usize, byte_index: usize, value: u8) -> Result<(), FormatError> {
        let ins = self
            .instructions
            .get(idx)
            .ok_or(FormatError::BadInstructionIndex(idx as u64))?;
        if byte_index >= ins.arg_length as usize {
            return Err(FormatError::ArgOutOfBounds {
                off: ins.arg_offset,
                len: ins.arg_length,
                total: self.arg_region.len() as u64,
            });
        }
        let pos = ins.arg_offset as usize + byte_index;
        self.arg_region[pos] = value;
        Ok(())
    }

    /// Walk every instruction; if it's a SOLO-branch test, patch the
    /// `DesiredMultiplayerState` argument byte to `new_state`.  Works
    /// at the IR level — honours the argument region properly.
    pub fn promote_solo_branches(&mut self, new_state: u8) -> usize {
        use crate::gen::class;
        let mut patched = 0;
        // Collect indices first so we don't hold an instruction borrow
        // across patch_arg_byte.
        let targets: Vec<(usize, usize)> = self
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(idx, ins)| match (ins.class, ins.instruction) {
                (c, 7) if c == class::CONDITION_EVENT => Some((idx, 1)), // IfMultiplayerState
                (c, 6) if c == class::CONTROL_FLOW_EVENT => Some((idx, 1)), // SkipIf…
                (c, 20) if c == class::CONTROL_FLOW_EVENT => Some((idx, 1)), // GotoIf…
                _ => None,
            })
            .collect();
        for (idx, byte_off) in targets {
            let args = match self.instruction_args(idx) {
                Ok(a) => a,
                Err(_) => continue,
            };
            if args.get(byte_off) == Some(&crate::patch::MultiplayerState::SOLO) {
                if self.patch_arg_byte(idx, byte_off, new_state).is_ok() {
                    patched += 1;
                }
            }
        }
        patched
    }

    /// Serialise back to bytes.  Rebuilds offsets from current contents.
    pub fn serialise(&self) -> Vec<u8> {
        // Layout: [header][events][instructions][parameters][linked_events][arg_region][strings]
        // All tables are 16-byte-aligned.
        const HEADER_SIZE: u64 = 0x88;
        let event_off = align_up(HEADER_SIZE, 16);
        let event_bytes: u64 = (self.events.len() as u64) * 48;
        let instr_off = align_up(event_off + event_bytes, 16);
        let instr_bytes: u64 = (self.instructions.len() as u64) * 24;
        let param_off = align_up(instr_off + instr_bytes, 16);
        let param_bytes: u64 = (self.parameters.len() as u64) * 32;
        let linked_off = align_up(param_off + param_bytes, 16);
        let linked_bytes: u64 = (self.linked_events.len() as u64) * 8;
        let arg_off = align_up(linked_off + linked_bytes, 16);
        let arg_bytes: u64 = self.arg_region.len() as u64;
        let string_off = align_up(arg_off + arg_bytes, 16);
        let string_bytes: u64 = self.strings.len() as u64;
        let total_size = align_up(string_off + string_bytes, 16);

        let mut out = vec![0u8; total_size as usize];

        out[0..4].copy_from_slice(MAGIC);
        write_u32(&mut out, 0x04, self.header.unk04);
        write_u32(&mut out, 0x08, self.header.unk08);
        write_u32(&mut out, 0x0C, self.header.unk0c);
        write_u32(&mut out, 0x10, total_size as u32);
        write_u64(&mut out, 0x18, self.events.len() as u64);
        write_u64(&mut out, 0x20, event_off);
        write_u64(&mut out, 0x28, self.instructions.len() as u64);
        write_u64(&mut out, 0x30, instr_off);
        write_u64(&mut out, 0x38, self.header.unk38);
        write_u64(&mut out, 0x40, self.header.event_layer_offset);
        write_u64(&mut out, 0x48, self.parameters.len() as u64);
        write_u64(&mut out, 0x50, param_off);
        write_u64(&mut out, 0x58, self.linked_events.len() as u64);
        write_u64(&mut out, 0x60, linked_off);
        write_u64(&mut out, 0x68, arg_bytes);
        write_u64(&mut out, 0x70, arg_off);
        write_u64(&mut out, 0x78, string_bytes);
        write_u64(&mut out, 0x80, string_off);

        for (i, ev) in self.events.iter().enumerate() {
            let b = event_off as usize + i * 48;
            write_u64(&mut out, b, ev.id);
            write_u64(&mut out, b + 8, ev.instruction_count);
            write_u64(&mut out, b + 16, ev.instruction_start_idx);
            write_u64(&mut out, b + 24, ev.parameter_count);
            write_u64(&mut out, b + 32, ev.parameter_start_idx);
            out[b + 40] = ev.end_type;
        }

        for (i, ins) in self.instructions.iter().enumerate() {
            let b = instr_off as usize + i * 24;
            write_u32(&mut out, b, ins.class);
            write_u32(&mut out, b + 4, ins.instruction);
            write_u64(&mut out, b + 8, ins.arg_length);
            write_u64(&mut out, b + 16, ins.arg_offset);
        }

        for (i, p) in self.parameters.iter().enumerate() {
            let b = param_off as usize + i * 32;
            write_u64(&mut out, b, p.instruction_index);
            write_u64(&mut out, b + 8, p.destination_start_byte);
            write_u64(&mut out, b + 16, p.source_start_byte);
            write_u64(&mut out, b + 24, p.byte_count);
            write_u32(&mut out, b + 28, p.unk18);
        }

        for (i, l) in self.linked_events.iter().enumerate() {
            let b = linked_off as usize + i * 8;
            write_u64(&mut out, b, l.file_index);
        }

        let a = arg_off as usize;
        out[a..a + self.arg_region.len()].copy_from_slice(&self.arg_region);

        if !self.strings.is_empty() {
            let s = string_off as usize;
            out[s..s + self.strings.len()].copy_from_slice(&self.strings);
        }

        out
    }

    /// Append new events — with their instructions + args — to the end
    /// of each table.  Offsets are rebuilt on `serialise`.
    pub fn append_event(
        &mut self,
        id: u64,
        end_type: u8,
        instructions: Vec<(RawInstruction, Vec<u8>)>,
    ) {
        let first_instr_idx = self.instructions.len() as u64;
        for (mut ins, args) in instructions {
            ins.arg_offset = self.arg_region.len() as u64;
            ins.arg_length = args.len() as u64;
            self.instructions.push(ins);
            self.arg_region.extend_from_slice(&args);
            // Pad the arg region to 4-byte alignment.
            while self.arg_region.len() % 4 != 0 {
                self.arg_region.push(0);
            }
        }
        let inst_count = self.instructions.len() as u64 - first_instr_idx;
        self.events.push(RawEvent {
            id,
            instruction_count: inst_count,
            instruction_start_idx: first_instr_idx,
            parameter_count: 0,
            parameter_start_idx: 0,
            end_type,
        });
    }
}

// --- IO helpers --------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn u8(&self, off: usize) -> Result<u8, FormatError> {
        self.bytes.get(off).copied().ok_or(FormatError::Truncated(off))
    }

    fn u32(&self, off: usize) -> Result<u32, FormatError> {
        let end = off + 4;
        if end > self.bytes.len() {
            return Err(FormatError::Truncated(off));
        }
        Ok(u32::from_le_bytes(self.bytes[off..end].try_into().unwrap()))
    }

    #[allow(dead_code)]
    fn i32(&self, off: usize) -> Result<i32, FormatError> {
        self.u32(off).map(|v| v as i32)
    }

    fn u64(&self, off: usize) -> Result<u64, FormatError> {
        let end = off + 8;
        if end > self.bytes.len() {
            return Err(FormatError::Truncated(off));
        }
        Ok(u64::from_le_bytes(self.bytes[off..end].try_into().unwrap()))
    }
}

fn write_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn align_up(n: u64, to: u64) -> u64 {
    (n + to - 1) & !(to - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::class;
    use crate::patch::MultiplayerState;

    fn minimal_emevd() -> Emevd {
        // One event with one IfMultiplayerState(SOLO) instruction.
        let arg_bytes = vec![0, MultiplayerState::SOLO, 0, 0]; // group=0, state=0, pad, pad
        let mut e = Emevd {
            header: Header::default(),
            events: vec![RawEvent {
                id: 42,
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
    fn round_trip() {
        let e = minimal_emevd();
        let bytes = e.serialise();
        let parsed = Emevd::parse(&bytes).expect("parse");
        assert_eq!(parsed.events, e.events);
        assert_eq!(parsed.instructions, e.instructions);
        assert_eq!(&parsed.arg_region[..2], &e.arg_region[..2]);
    }

    #[test]
    fn promote_patches_solo_byte() {
        let mut e = minimal_emevd();
        let patched = e.promote_solo_branches(MultiplayerState::HOST);
        assert_eq!(patched, 1);
        let args = e.instruction_args(0).unwrap();
        assert_eq!(args[1], MultiplayerState::HOST);
    }

    #[test]
    fn bad_magic_rejected() {
        let bytes = b"BAD\0".to_vec();
        assert!(matches!(Emevd::parse(&bytes), Err(FormatError::BadMagic(_))));
    }

    #[test]
    fn append_event_extends_tables() {
        let mut e = minimal_emevd();
        let prior_events = e.events.len();
        let prior_instructions = e.instructions.len();
        let prior_args = e.arg_region.len();

        let new_ins = RawInstruction {
            class: class::EVENT,
            instruction: 2,
            arg_length: 0,
            arg_offset: 0,
        };
        e.append_event(99_000, 0, vec![(new_ins, vec![0x05, 0x01, 0, 0])]);

        assert_eq!(e.events.len(), prior_events + 1);
        assert_eq!(e.instructions.len(), prior_instructions + 1);
        assert!(e.arg_region.len() > prior_args);

        // Round-trip after append.
        let bytes = e.serialise();
        let parsed = Emevd::parse(&bytes).expect("reparse");
        assert_eq!(parsed.events.last().unwrap().id, 99_000);
    }
}
