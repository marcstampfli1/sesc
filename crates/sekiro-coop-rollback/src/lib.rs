//! Layer 3 — rollback band.
//!
//! Shared-entity set identification, snapshot serialisation, input ring
//! buffer, remote-input prediction, re-simulation driver.  SPEC §5.

pub mod band;
pub mod delta;
pub mod predict;
pub mod resim;
pub mod ring;
pub mod snapshot;
pub mod stepper;

pub use band::{SharedBand, SharedDecision, ROLLBACK_PROXIMITY_RADIUS_M};
pub use delta::{EntityDelta, SnapshotDelta};
pub use predict::Predictor;
pub use resim::{ResimDriver, RollbackPlan, SharedStepper};
pub use ring::{Input, InputRing, ROLLBACK_MAX_FRAMES};
pub use snapshot::{EntitySnapshot, RollbackSnapshot, SnapshotRing};
pub use stepper::ChrInsStepper;
