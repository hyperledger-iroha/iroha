/// Module providing tools to create a circuit using our gates
pub mod circuit;
/// Module implementing our simple custom gate and common functions using it
pub mod flex_gate;
/// Module using a single lookup table for range checks
pub mod range;

/// Tests
#[cfg(test)]
pub mod tests;

pub use flex_gate::{GateChip, GateInstructions};
pub use range::{RangeChip, RangeInstructions};
