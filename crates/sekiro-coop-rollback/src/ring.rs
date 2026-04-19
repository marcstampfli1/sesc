//! Per-peer input ring buffer.  SPEC §5.3.

use serde::{Deserialize, Serialize};

/// Default rollback window size.  Tuned per SPEC §5.6.
pub const ROLLBACK_MAX_FRAMES: u64 = 8;

/// Size of the input ring per peer (`N = 16` in SPEC §5.3).
pub const INPUT_RING_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub frame: u64,
    pub buttons: u32, // attack, deflect, dodge, jump, etc.
    pub stick_l: [i8; 2],
    pub stick_r: [i8; 2],
    pub digital_flags: u16, // item use, art, etc.
}

impl Input {
    pub const NEUTRAL_FRAME: Input = Input {
        frame: 0,
        buttons: 0,
        stick_l: [0, 0],
        stick_r: [0, 0],
        digital_flags: 0,
    };

    pub fn neutral(frame: u64) -> Self {
        Self {
            frame,
            buttons: 0,
            stick_l: [0, 0],
            stick_r: [0, 0],
            digital_flags: 0,
        }
    }

    pub fn equivalent_inputs(&self, other: &Input) -> bool {
        self.buttons == other.buttons
            && self.stick_l == other.stick_l
            && self.stick_r == other.stick_r
            && self.digital_flags == other.digital_flags
    }
}

/// Ring buffer of inputs.  Indexed by absolute frame modulo capacity.
#[derive(Debug, Clone)]
pub struct InputRing {
    buf: Vec<Option<Input>>,
}

impl InputRing {
    pub fn new() -> Self {
        Self {
            buf: vec![None; INPUT_RING_SIZE],
        }
    }

    pub fn put(&mut self, input: Input) {
        let idx = (input.frame as usize) % self.buf.len();
        self.buf[idx] = Some(input);
    }

    pub fn get(&self, frame: u64) -> Option<Input> {
        let idx = (frame as usize) % self.buf.len();
        match self.buf[idx] {
            Some(i) if i.frame == frame => Some(i),
            _ => None,
        }
    }

    /// The most recently recorded input, if any.  Used as the default
    /// prediction.
    pub fn latest(&self) -> Option<Input> {
        self.buf
            .iter()
            .filter_map(|o| *o)
            .max_by_key(|i| i.frame)
    }
}

impl Default for InputRing {
    fn default() -> Self {
        Self::new()
    }
}
