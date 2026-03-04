//! Time trigger entrypoint definitions.

use derive_more::Display;
use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    transaction::{ExecutionStep, TransactionEntrypoint},
    trigger::TriggerId,
};

/// A time-triggered entrypoint, forming the second half of the transaction entrypoints.
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[display("TimeTriggerEntrypoint")]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct TimeTriggerEntrypoint {
    /// Identifier for this trigger.
    pub id: TriggerId,
    /// Instructions executed in this step.
    pub instructions: ExecutionStep,
    /// Account authorized to initiate this time-triggered transaction.
    pub authority: AccountId,
}

impl TimeTriggerEntrypoint {
    /// Hash for this time-triggered entrypoint as `TransactionEntrypoint`.
    #[inline]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        HashOf::new(&TransactionEntrypoint::Time(self.clone()))
    }
}
