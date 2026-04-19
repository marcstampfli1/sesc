//! Re-simulation driver.  SPEC §5.5.

use crate::ring::{Input, InputRing, ROLLBACK_MAX_FRAMES};
use crate::snapshot::{RollbackSnapshot, SnapshotRing};

/// The plan produced when a misprediction is detected.
#[derive(Debug, Clone)]
pub struct RollbackPlan {
    pub from_frame: u64,
    pub to_frame: u64,
    pub restore: RollbackSnapshot,
    pub corrected_remote: Input,
}

/// Trait the rollback driver uses to step the shared entities.  The DLL
/// implements this by calling the bridge hooks with authority-driven
/// decisions re-applied.
pub trait SharedStepper {
    /// Restore shared-entity state from the snapshot.
    fn restore(&mut self, snap: &RollbackSnapshot);

    /// Step shared entities one tick using the given input pair.
    fn step(&mut self, frame: u64, local: Input, remote: Input);
}

pub struct ResimDriver {
    pub max_frames: u64,
}

impl ResimDriver {
    pub fn new() -> Self {
        Self { max_frames: ROLLBACK_MAX_FRAMES }
    }

    /// Examine a newly-arrived remote input against the prediction used
    /// for the corresponding frame.  If the prediction was wrong and
    /// the snapshot is still in the ring, return a [`RollbackPlan`].
    pub fn detect_mispredict(
        &self,
        real: Input,
        predicted_for_same_frame: Input,
        snapshots: &SnapshotRing,
        current_frame: u64,
    ) -> Option<RollbackPlan> {
        if real.equivalent_inputs(&predicted_for_same_frame) {
            return None;
        }
        if current_frame.saturating_sub(real.frame) > self.max_frames {
            // Past the rollback window — nothing to do; signal desync
            // upstream.
            return None;
        }
        let snap = snapshots.at(real.frame)?;
        Some(RollbackPlan {
            from_frame: real.frame,
            to_frame: current_frame,
            restore: snap.clone(),
            corrected_remote: real,
        })
    }

    /// Execute the rollback.  The stepper hooks into Layer 2 and
    /// re-runs the tick range with corrected remote input applied only
    /// at `from_frame`; subsequent frames use whatever was already in
    /// the ring (predicted or real).
    pub fn execute<S: SharedStepper>(
        &self,
        stepper: &mut S,
        plan: &RollbackPlan,
        local_inputs: &InputRing,
        remote_inputs: &InputRing,
    ) {
        stepper.restore(&plan.restore);
        for k in plan.from_frame..=plan.to_frame {
            let local = local_inputs
                .get(k)
                .unwrap_or_else(|| Input::neutral(k));
            let remote = if k == plan.from_frame {
                plan.corrected_remote
            } else {
                remote_inputs
                    .get(k)
                    .unwrap_or_else(|| Input::neutral(k))
            };
            stepper.step(k, local, remote);
        }
    }
}

impl Default for ResimDriver {
    fn default() -> Self {
        Self::new()
    }
}
