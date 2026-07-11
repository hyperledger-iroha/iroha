//! Module for managing the virtual region corresponding to [super::FlexGateConfig]
//!
//! In the virtual region we have virtual columns. Each virtual column is referred to as a "thread"
//! because it can be generated in a separate CPU thread. The virtual region manager will collect all
//! threads together, virtually concatenate them all together back into a single virtual column, and
//! then assign this virtual column to multiple physical Halo2 columns according to the provided configuration parameters.
//!
//! Supports multiple phases.

/// Thread builder for multiple phases
mod multi_phase;
mod parallelize;
/// Thread builder for a single phase
pub mod single_phase;

pub use multi_phase::{GateStatistics, MultiPhaseCoreManager};
pub use parallelize::parallelize_core;
pub use single_phase::SinglePhaseCoreManager;
