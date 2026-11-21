//! Standalone G3 receipt data awaiting a distinct selected result-profile owner.
//!
//! This type is deliberately absent from the installed `SignedBlock`/native output graph.
//! A value alone proves neither execution nor certification. TODO(N14): implement an
//! immutable selected result profile whose original producer authenticates and commits it.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::Digest512,
        types::{GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceStageV1},
    },
};

/// The exact resulting attempt of a positioned runtime enactment, outside native outputs.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::RuntimeEnactmentReceiptV1")]
pub struct RuntimeEnactmentReceiptV1 {
    /// The shared pass position and committed governance time of the effect.
    pub anchor: GovernanceAnchorV1,
    /// The original attempt whose production certificate authorized consideration.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Enacted, superseded or deterministically failed, as the original reducer recorded.
    pub status: GovernanceAttemptStatusV1,
    /// The resulting stage retained by that same attempt.
    pub stage: GovernanceStageV1,
    /// The existing domain-separated canonical digest of the resulting attempt State.
    pub attempt_state_digest: Digest512,
}

/// A receipt whose shape cannot describe a positioned runtime enactment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeEnactmentReceiptErrorV1 {
    /// Block zero cannot own a positioned governance effect.
    #[error("runtime enactment receipt requires a nonzero block height")]
    ZeroHeight,
    /// Only the original terminal execution outcomes describe this receipt.
    #[error("runtime enactment receipt requires a terminal execution outcome")]
    NonterminalOutcome,
}

impl RuntimeEnactmentReceiptV1 {
    /// Check data shape only; the future selected result owner must prove all source joins.
    ///
    /// # Errors
    /// Rejects a zero-height anchor or a status other than the three enactment outcomes.
    pub fn validate(&self) -> Result<(), RuntimeEnactmentReceiptErrorV1> {
        if self.anchor.height == 0 {
            return Err(RuntimeEnactmentReceiptErrorV1::ZeroHeight);
        }
        if !matches!(
            self.status,
            GovernanceAttemptStatusV1::Enacted
                | GovernanceAttemptStatusV1::Superseded
                | GovernanceAttemptStatusV1::ExecutionFailed
        ) {
            return Err(RuntimeEnactmentReceiptErrorV1::NonterminalOutcome);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(status: GovernanceAttemptStatusV1) -> RuntimeEnactmentReceiptV1 {
        RuntimeEnactmentReceiptV1 {
            anchor: GovernanceAnchorV1 {
                height: 19,
                position: 2,
                timestamp_ms: 300_000,
            },
            governance_attempt_id: GovernanceAttemptId::new([0x42; 32]),
            status,
            stage: GovernanceStageV1::default(),
            attempt_state_digest: [0x53; 64].into(),
        }
    }

    #[test]
    fn standalone_runtime_receipt_roundtrips_each_original_terminal_outcome() {
        for status in [
            GovernanceAttemptStatusV1::Enacted,
            GovernanceAttemptStatusV1::Superseded,
            GovernanceAttemptStatusV1::ExecutionFailed,
        ] {
            let original = receipt(status);
            original.validate().unwrap();
            let bytes = norito::encode_canonical(&original).unwrap();
            assert_eq!(
                norito::decode_canonical::<RuntimeEnactmentReceiptV1>(&bytes).unwrap(),
                original
            );
            let json = norito::json::to_json(&original).unwrap();
            assert_eq!(
                norito::json::from_str::<RuntimeEnactmentReceiptV1>(&json).unwrap(),
                original
            );
        }
    }

    #[test]
    fn standalone_runtime_receipt_rejects_a_missing_position_or_nonterminal_outcome() {
        let mut invalid = receipt(GovernanceAttemptStatusV1::Certified);
        assert_eq!(
            invalid.validate(),
            Err(RuntimeEnactmentReceiptErrorV1::NonterminalOutcome)
        );
        invalid.anchor.height = 0;
        assert_eq!(
            invalid.validate(),
            Err(RuntimeEnactmentReceiptErrorV1::ZeroHeight)
        );
    }

    #[test]
    fn standalone_runtime_receipt_is_not_reachable_from_the_native_block_graph() {
        let native = crate::block::SignedBlock::schema();
        let own = RuntimeEnactmentReceiptV1::schema();
        let receipt_id = core::any::TypeId::of::<RuntimeEnactmentReceiptV1>();
        assert!(own.iter().any(|(id, _)| *id == receipt_id));
        assert!(native.iter().all(|(id, _)| *id != receipt_id));
        assert!(
            native
                .iter()
                .all(|(_, entry)| !entry.type_id.contains("RuntimeEnactmentReceiptV1"))
        );
        assert!(crate::wire_schema::wire_root_defects(&native).is_empty());
    }
}
