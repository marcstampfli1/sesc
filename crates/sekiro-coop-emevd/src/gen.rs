//! EMEVD bytecode emitter — priority instruction subset (SPEC §8.4).
//!
//! One Rust fn per EMEVD instruction we need.  Output is an
//! intermediate representation (`Instruction`) that `patch.rs`
//! serialises to the binary on-disk format.

use std::collections::BTreeMap;

/// EMEVD argument.  The binary format packs these tightly; the IR keeps
/// them typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32Bits(u32), // f32 transmuted to u32 to keep Eq
}

impl Arg {
    pub fn f32(v: f32) -> Self {
        Arg::F32Bits(v.to_bits())
    }

    pub fn byte_width(&self) -> usize {
        match self {
            Arg::U8(_) | Arg::I8(_) => 1,
            Arg::U16(_) | Arg::I16(_) => 2,
            Arg::U32(_) | Arg::I32(_) | Arg::F32Bits(_) => 4,
        }
    }

    pub fn to_le_bytes(&self) -> Vec<u8> {
        match *self {
            Arg::U8(v) => vec![v],
            Arg::I8(v) => vec![v as u8],
            Arg::U16(v) => v.to_le_bytes().to_vec(),
            Arg::I16(v) => v.to_le_bytes().to_vec(),
            Arg::U32(v) => v.to_le_bytes().to_vec(),
            Arg::I32(v) => v.to_le_bytes().to_vec(),
            Arg::F32Bits(v) => v.to_le_bytes().to_vec(),
        }
    }
}

/// (class, instruction) pair uniquely identifies an EMEVD instruction.
/// Class = top-level category; instruction = index within the category
/// (as enumerated in `Function-Definitions.md`).
#[derive(Debug, Clone)]
pub struct Instruction {
    pub class: u32,
    pub instruction: u32,
    pub args: Vec<Arg>,
}

impl Instruction {
    pub fn new(class: u32, instruction: u32, args: Vec<Arg>) -> Self {
        Self { class, instruction, args }
    }
}

/// How an event ends — mirror of `EventEndType` in EMEVD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventEndType {
    End = 0,
    Restart = 1,
}

/// `InitializeEvent`'s "RestartKind" field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartKind {
    Default,
    Restart,
    End,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: u32,
    pub restart: RestartKind,
    pub body: Vec<Instruction>,
}

#[derive(Debug, Default, Clone)]
pub struct EmevdProgram {
    pub events: BTreeMap<u32, Event>,
}

impl EmevdProgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, ev: Event) {
        self.events.insert(ev.id, ev);
    }

    /// Serialise every event's body to a flat byte buffer in the order
    /// they appear.  Real EMEVD format adds per-event headers and
    /// arg-region packing (see `patch.rs`); this is the instruction
    /// blob that gets spliced into it.
    pub fn emit_instructions(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for ev in self.events.values() {
            for ins in &ev.body {
                out.extend_from_slice(&encode_instruction(ins));
            }
        }
        out
    }
}

/// Low-level: encode one instruction as `[class:u32][instruction:u32][args_len:u32][args…]`.
fn encode_instruction(ins: &Instruction) -> Vec<u8> {
    let mut body: Vec<u8> = ins
        .args
        .iter()
        .flat_map(|a| a.to_le_bytes())
        .collect();
    // Pad args to 4-byte alignment.
    while body.len() % 4 != 0 {
        body.push(0);
    }
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(&ins.class.to_le_bytes());
    out.extend_from_slice(&ins.instruction.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// Category codes — the top-level EMEVD class ID.  Derived from the
/// `Function-Definitions.md` ordering.  Used here for the priority
/// subset (SPEC §8.4).
pub mod class {
    pub const CONDITION_SYSTEM: u32 = 0;
    pub const CONDITION_TIMER: u32 = 1;
    pub const CONDITION_EVENT: u32 = 2;
    pub const CONDITION_CHARACTER: u32 = 3;
    pub const CONDITION_OBJECT: u32 = 4;
    pub const CONDITION_HIT: u32 = 5;
    pub const CONDITION_MAP: u32 = 6;
    pub const CONDITION_ADVANCED: u32 = 7;
    pub const CONTROL_FLOW_SYSTEM: u32 = 1000;
    pub const CONTROL_FLOW_TIMER: u32 = 1001;
    pub const CONTROL_FLOW_EVENT: u32 = 1003;
    pub const CONTROL_FLOW_OBJECT: u32 = 1005;
    pub const SYSTEM: u32 = 2000;
    pub const CUTSCENE: u32 = 2001;
    pub const EVENT: u32 = 2002;
    pub const CHARACTER: u32 = 2004;
    pub const OBJECT: u32 = 2005;
    pub const SFX: u32 = 2006;
    pub const MESSAGE: u32 = 2007;
    pub const CAMERA: u32 = 2008;
    pub const SCRIPT: u32 = 2009;
    pub const SOUND: u32 = 2010;
    pub const HIT: u32 = 2011;
    pub const MAP: u32 = 2012;
    pub const PLAYLOG: u32 = 2013;
    pub const PROJECT: u32 = 2014;
}

/// Ergonomic wrappers for the priority subset.
pub struct InstructionBuilder;

impl InstructionBuilder {
    pub fn set_event_flag(flag_id: i32, on: bool) -> Instruction {
        Instruction::new(
            class::EVENT,
            2,
            vec![Arg::I32(flag_id), Arg::U8(on as u8)],
        )
    }

    pub fn if_event_flag(group: u8, on: bool, flag_type: u8, flag_id: i32) -> Instruction {
        Instruction::new(
            class::CONDITION_EVENT,
            1,
            vec![
                Arg::U8(group),
                Arg::U8(on as u8),
                Arg::U8(flag_type),
                Arg::I32(flag_id),
            ],
        )
    }

    pub fn batch_set_event_flags(start: i32, end: i32, on: bool) -> Instruction {
        Instruction::new(
            class::EVENT,
            22,
            vec![Arg::I32(start), Arg::I32(end), Arg::U8(on as u8)],
        )
    }

    pub fn if_multiplayer_state(group: u8, desired: u8) -> Instruction {
        Instruction::new(class::CONDITION_EVENT, 7, vec![Arg::U8(group), Arg::U8(desired)])
    }

    pub fn skip_if_multiplayer_state(skip_lines: u8, desired: u8) -> Instruction {
        Instruction::new(
            class::CONTROL_FLOW_EVENT,
            6,
            vec![Arg::U8(skip_lines), Arg::U8(desired)],
        )
    }

    pub fn goto_if_multiplayer_state(label: u8, desired: u8) -> Instruction {
        Instruction::new(
            class::CONTROL_FLOW_EVENT,
            20,
            vec![Arg::U8(label), Arg::U8(desired)],
        )
    }

    pub fn set_network_update_authority(entity_id: i32, level: u8) -> Instruction {
        Instruction::new(
            class::CHARACTER,
            28,
            vec![Arg::I32(entity_id), Arg::U8(level)],
        )
    }

    pub fn set_network_update_rate(entity_id: i32, fixed: bool, freq: u8) -> Instruction {
        Instruction::new(
            class::CHARACTER,
            34,
            vec![
                Arg::I32(entity_id),
                Arg::U8(fixed as u8),
                Arg::U8(freq),
            ],
        )
    }

    pub fn trigger_multiplayer_event(id: u32) -> Instruction {
        Instruction::new(class::EVENT, 16, vec![Arg::U32(id)])
    }

    pub fn if_multiplayer_event(group: u8, id: u32) -> Instruction {
        Instruction::new(
            class::CONDITION_EVENT,
            10,
            vec![Arg::U8(group), Arg::U32(id)],
        )
    }

    pub fn wait_for_network_approval(timeout_s: f32) -> Instruction {
        Instruction::new(
            class::CONTROL_FLOW_SYSTEM,
            10,
            vec![Arg::f32(timeout_s)],
        )
    }

    pub fn set_network_sync_state(on: bool) -> Instruction {
        Instruction::new(class::SYSTEM, 3, vec![Arg::U8(on as u8)])
    }

    pub fn set_network_connected_event_flag(flag_id: i32, on: bool) -> Instruction {
        Instruction::new(
            class::EVENT,
            58,
            vec![Arg::I32(flag_id), Arg::U8(on as u8)],
        )
    }

    pub fn batch_set_network_connected_event_flags(start: i32, end: i32, on: bool) -> Instruction {
        Instruction::new(
            class::EVENT,
            59,
            vec![Arg::I32(start), Arg::I32(end), Arg::U8(on as u8)],
        )
    }

    pub fn randomly_set_event_flag_in_range(start: u32, end: u32, on: bool) -> Instruction {
        Instruction::new(
            class::EVENT,
            17,
            vec![Arg::U32(start), Arg::U32(end), Arg::U8(on as u8)],
        )
    }

    pub fn clear_event_value(base: i32, bits: u32) -> Instruction {
        Instruction::new(
            class::EVENT,
            32,
            vec![Arg::I32(base), Arg::U32(bits)],
        )
    }

    pub fn initialize_common_event(id: u32, params: u32) -> Instruction {
        Instruction::new(
            class::SYSTEM,
            7,
            vec![Arg::U32(id), Arg::U32(params)],
        )
    }

    pub fn initialize_event(slot: i32, id: u32, params: u32) -> Instruction {
        Instruction::new(
            class::SYSTEM,
            1,
            vec![Arg::I32(slot), Arg::U32(id), Arg::U32(params)],
        )
    }

    pub fn if_event_value(
        group: u8,
        base: i32,
        bits: u8,
        comparison: u8,
        threshold: u32,
    ) -> Instruction {
        Instruction::new(
            class::CONDITION_EVENT,
            13,
            vec![
                Arg::U8(group),
                Arg::I32(base),
                Arg::U8(bits),
                Arg::U8(comparison),
                Arg::U32(threshold),
            ],
        )
    }

    pub fn if_compare_event_values(
        group: u8,
        left_base: i32,
        left_bits: u8,
        comparison: u8,
        right_base: i32,
        right_bits: u8,
    ) -> Instruction {
        Instruction::new(
            class::CONDITION_EVENT,
            21,
            vec![
                Arg::U8(group),
                Arg::I32(left_base),
                Arg::U8(left_bits),
                Arg::U8(comparison),
                Arg::I32(right_base),
                Arg::U8(right_bits),
            ],
        )
    }

    pub fn end_unconditionally(end_type: EventEndType) -> Instruction {
        Instruction::new(
            class::CONTROL_FLOW_SYSTEM,
            5,
            vec![Arg::U8(end_type as u8)],
        )
    }

    pub fn end_if_event_flag(
        end_type: EventEndType,
        on: bool,
        flag_type: u8,
        flag_id: i32,
    ) -> Instruction {
        Instruction::new(
            class::CONTROL_FLOW_EVENT,
            3,
            vec![
                Arg::U8(end_type as u8),
                Arg::U8(on as u8),
                Arg::U8(flag_type),
                Arg::I32(flag_id),
            ],
        )
    }
}

/// Build the custom events 99000-99003 (SPEC §8.5).
pub fn build_custom_events(
    seeded_flag_range: (u32, u32),
    boss_entity_ids: &[i32],
) -> EmevdProgram {
    let mut prog = EmevdProgram::new();

    // Event 99000 — seeded RNG initializer.
    prog.add(Event {
        id: 99_000,
        restart: RestartKind::Default,
        body: vec![
            InstructionBuilder::batch_set_event_flags(
                seeded_flag_range.0 as i32,
                seeded_flag_range.1 as i32,
                false,
            ),
            InstructionBuilder::randomly_set_event_flag_in_range(
                seeded_flag_range.0,
                seeded_flag_range.1,
                true,
            ),
            InstructionBuilder::end_unconditionally(EventEndType::End),
        ],
    });

    // Event 99001 — authority designator for bosses.  Runs once the boss
    // fog wall is crossed; we approximate that trigger here by gating
    // on a per-boss event flag.  In practice the patcher wires this up
    // per-boss file.
    let mut auth_body: Vec<Instruction> = Vec::new();
    for eid in boss_entity_ids {
        auth_body.push(InstructionBuilder::set_network_update_authority(*eid, 0 /* Host */));
        auth_body.push(InstructionBuilder::set_network_update_rate(*eid, true, 1 /* Fast */));
    }
    auth_body.push(InstructionBuilder::end_unconditionally(EventEndType::End));
    prog.add(Event {
        id: 99_001,
        restart: RestartKind::Default,
        body: auth_body,
    });

    // Event 99002 — proximity authority refresh ticker.  Does a
    // broadcast TriggerMultiplayerEvent; listener in-mod decodes.
    prog.add(Event {
        id: 99_002,
        restart: RestartKind::Restart,
        body: vec![
            InstructionBuilder::trigger_multiplayer_event(99_200),
            InstructionBuilder::end_unconditionally(EventEndType::Restart),
        ],
    });

    // Event 99003 — boss fog lockstep.  Wraps native fog-wall handling
    // with WaitForNetworkApproval(5.0).
    prog.add(Event {
        id: 99_003,
        restart: RestartKind::Default,
        body: vec![
            InstructionBuilder::wait_for_network_approval(5.0),
            InstructionBuilder::end_unconditionally(EventEndType::End),
        ],
    });

    prog
}
