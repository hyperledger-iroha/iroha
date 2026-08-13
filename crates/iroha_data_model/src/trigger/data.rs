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
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct DataTriggerStep {
    /// Identifier for this trigger.
    pub id: TriggerId,
    /// Instructions executed in this step.
    pub instructions: ExecutionStep,
}
