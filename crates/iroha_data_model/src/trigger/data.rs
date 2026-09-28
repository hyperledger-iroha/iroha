//! Data trigger sequence and steps.
use crate::{transaction::ExecutionStep, trigger::TriggerId};
use derive_more::Display;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Sequence of data trigger execution steps.
pub type DataTriggerSequence = Vec<DataTriggerStep>;
/// Single execution step of the data trigger.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[display("DataTriggerStep")]
pub struct DataTriggerStep {
    /// Identifier for this trigger.
    pub id: TriggerId,
    /// Instructions executed in this step.
    pub instructions: ExecutionStep,
}
