//! Production native assembly of complete fixed operation stage relations.
//!
//! Installation owns artifact authentication and canonical G1 conversion. The
//! relation owners consume typed inputs and retain every proof obligation.

pub mod bootstrap;

pub mod load;

pub mod receive;
pub mod send;
pub mod unload;

/// Genuine fixed Retiring A/W producer and source-bound checkpoints.
pub mod retiring;

/// Complete original-key native ArchiveSent lineage stages.
pub mod archive;
/// Complete original-key native signed-policy/credential Refresh lineage stages.
pub mod refresh;
