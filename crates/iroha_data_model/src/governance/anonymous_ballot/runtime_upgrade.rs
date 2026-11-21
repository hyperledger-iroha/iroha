//! Source-bound runtime proposal requests for the anonymous Parliament family.
//!
//! The original signed creator supplies an action and its release endorsements, never
//! attribution, notice, time, purpose or a compare-and-set head. A retry names the exact
//! previous attempt, so it cannot replace the admitted proposal's frozen metadata.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::types::GovernanceAttemptId,
    runtime::lifecycle::{ChangeSetEndorsementV1, RuntimeUpgradeActionV1},
};

/// The two permitted sources of an anonymous runtime proposal's immutable content.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "kind", content = "content", deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousRuntimeUpgradeRequestV1"
)]
pub enum AnonymousRuntimeUpgradeRequestV1 {
    /// Admit a new action against the root's committed notice and release policies.
    #[codec(index = 0)]
    Initial {
        /// Exact schedule or cancellation requested.
        action: RuntimeUpgradeActionV1,
        /// Distinct trusted release endorsements required by committed policy.
        provenance: Vec<ChangeSetEndorsementV1>,
    },
    /// Retry the exact proposal of a terminal, non-enacted predecessor.
    #[codec(index = 1)]
    Retry {
        /// Previous attempt, whose content and operator remain unchanged.
        predecessor: GovernanceAttemptId,
    },
}

/// Create an anonymous runtime-upgrade attempt from its sole original signed instruction.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::CreateAnonymousRuntimeUpgradeAttemptV1"
)]
pub struct CreateAnonymousRuntimeUpgradeAttemptV1 {
    /// New signed action or exact predecessor reference.
    pub request: AnonymousRuntimeUpgradeRequestV1,
}

impl crate::seal::Instruction for CreateAnonymousRuntimeUpgradeAttemptV1 {}

impl PartialOrd for CreateAnonymousRuntimeUpgradeAttemptV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.encode().cmp(&other.encode()))
    }
}

impl CreateAnonymousRuntimeUpgradeAttemptV1 {
    /// Distinct tolerant interface of the anonymous family; grants no ballot qualification.
    pub const WIRE_ID: &'static str =
        "iroha.governance.parliament.anonymous.runtime_upgrade.create.v1";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::lifecycle::ChangeSetId;

    #[test]
    fn runtime_request_roundtrips_and_cannot_carry_frozen_metadata() {
        let request = CreateAnonymousRuntimeUpgradeAttemptV1 {
            request: AnonymousRuntimeUpgradeRequestV1::Initial {
                action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([7; 32])),
                provenance: Vec::new(),
            },
        };
        let encoded = norito::encode_canonical(&request).unwrap();
        assert_eq!(
            norito::decode_canonical::<CreateAnonymousRuntimeUpgradeAttemptV1>(&encoded).unwrap(),
            request
        );
        let json = norito::json::to_json(&request).unwrap();
        assert_eq!(
            norito::json::from_str::<CreateAnonymousRuntimeUpgradeAttemptV1>(&json).unwrap(),
            request
        );
        let forged = format!("{{\"proposed_ms\":0,{}", &json[1..]);
        assert!(norito::json::from_str::<CreateAnonymousRuntimeUpgradeAttemptV1>(&forged).is_err());
    }
}
