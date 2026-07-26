//! Trigger module exports.

/// Trigger input payloads and supporting containers.
pub mod data;
/// Trigger model definitions and execution logic.
pub mod model;
/// Time trigger scheduling utilities.
pub mod time;

pub use data::{DataTriggerSequence, DataTriggerStep};
pub use model::{Trigger, TriggerId, action};
pub use time::TimeTriggerEntrypoint;

pub mod prelude {
    //! Re-exports of commonly used trigger types.
    pub use super::{
        DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint, Trigger, TriggerId,
        action::prelude::*,
    };
}
