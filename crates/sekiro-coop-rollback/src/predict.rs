//! Remote-input prediction.  SPEC §5.4.
//!
//! Default: repeat the remote peer's last known input.
//! Decay: after K frames without update, predict "neutral".

use crate::ring::{Input, InputRing};

/// Predictor config.
#[derive(Debug, Clone, Copy)]
pub struct Predictor {
    pub decay_after_frames: u64,
}

impl Predictor {
    pub const DEFAULT_DECAY: u64 = 6;

    pub fn new() -> Self {
        Self {
            decay_after_frames: Self::DEFAULT_DECAY,
        }
    }

    /// Produce the predicted input for `frame`, given the remote peer's
    /// input history so far.
    pub fn predict(&self, ring: &InputRing, frame: u64) -> Input {
        if let Some(real) = ring.get(frame) {
            return real;
        }
        let latest = ring.latest();
        match latest {
            Some(i) if frame.saturating_sub(i.frame) <= self.decay_after_frames => Input {
                frame,
                ..i
            },
            _ => Input::neutral(frame),
        }
    }
}

impl Default for Predictor {
    fn default() -> Self {
        Self::new()
    }
}
