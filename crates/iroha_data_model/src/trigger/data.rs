//! Data trigger sequence and steps.

use derive_more::Display;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{transaction::ExecutionStep, trigger::TriggerId};

/// Sequence of data trigger execution steps.
pub type DataTriggerSequence = Vec<DataTriggerStep>;

/// Single execution step of the data trigger.
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[display("DataTriggerStep")]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct DataTriggerStep {
    /// Identifier for this trigger.
    pub id: TriggerId,
    /// Instructions executed in this step.
    pub instructions: ExecutionStep,
}
