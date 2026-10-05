//! Public DTOs for certificate-only governance proposal draft routes.
//!
//! These request types contain only immutable proposal content. Referendum
//! windows and voting modes belong to the retired proposal-backed referendum
//! flow and are deliberately absent from the first-release API.

use iroha_data_model::{
    account::AccountId,
    governance::types::{AbiVersion, ContractAbiHash, ContractCodeHash, ProposalContentId},
    sccp::governance::SccpGovernanceProposalV1,
    smart_contract::{ContractAddress, ContractAlias, manifest::ManifestProvenance},
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

mod one_instruction {
    use norito::json::{
        self, BoundedJsonError, JsonDeserialize, JsonSerialize, JsonWriteSink, Parser,
    };

    use super::GovernanceProposalInstructionDraftV1;

    pub fn serialize(value: &[GovernanceProposalInstructionDraftV1; 1], out: &mut String) {
        out.push('[');
        value[0].json_serialize(out);
        out.push(']');
    }

    pub fn serialize_bounded(
        value: &[GovernanceProposalInstructionDraftV1; 1],
        out: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        out.begin_container()?;
        let result = (|| -> Result<(), norito::json::BoundedJsonError> {
            out.push('[')?;
            value[0].json_serialize_to(out)?;
            out.push(']')?;
            Ok(())
        })();
        out.end_container();
        result?;
        Ok(())
    }

    pub fn deserialize(
        parser: &mut Parser<'_>,
    ) -> Result<[GovernanceProposalInstructionDraftV1; 1], json::Error> {
        let values = Vec::<GovernanceProposalInstructionDraftV1>::json_deserialize(parser)?;
        values.try_into().map_err(|values: Vec<_>| {
            json::Error::Message(format!(
                "expected exactly one instruction, got {}",
                values.len()
            ))
        })
    }
}

/// Strict request for one deploy-contract proposal instruction draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::governance_proposal_api::DeployContractProposalDraftRequestV1"
)]
pub struct DeployContractProposalDraftRequestV1 {
    /// Canonical transaction authority that will submit the returned instruction.
    pub proposal_operator: AccountId,
    /// Optional canonical contract address targeted by the proposal.
    #[norito(default, skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<ContractAddress>,
    /// Optional on-chain contract alias resolved by Torii to its canonical address.
    #[norito(default, skip_serializing_if = "Option::is_none")]
    pub contract_alias: Option<ContractAlias>,
    /// Exact first-release ABI version; must equal one.
    pub abi_version: AbiVersion,
    /// Blake2b-32 hash of the compiled `.to` bytecode.
    pub code_hash: ContractCodeHash,
    /// Blake2b-32 hash of the ABI surface expected by hosts.
    pub abi_hash: ContractAbiHash,
    /// Optional manifest provenance bound into the immutable proposal content.
    #[norito(default, skip_serializing_if = "Option::is_none")]
    pub manifest_provenance: Option<ManifestProvenance>,
}

/// Strict request for one SCCP Parliament proposal instruction draft (`specs/sccp.md` §4.14.3).
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::governance_proposal_api::SccpRouteGovernanceProposalDraftRequestV1"
)]
pub struct SccpRouteGovernanceProposalDraftRequestV1 {
    /// Complete network-bound proposal: base revisions of its subjects and 1..=16 actions.
    pub proposal: SccpGovernanceProposalV1,
}

/// One canonical proposal instruction returned for local signing.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::governance_proposal_api::GovernanceProposalInstructionDraftV1"
)]
pub struct GovernanceProposalInstructionDraftV1 {
    /// Registered instruction wire identifier.
    pub wire_id: String,
    /// Lowercase hexadecimal canonical framed instruction bytes.
    pub payload_hex: String,
}

/// Bound response for one deploy-contract proposal draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::governance_proposal_api::DeployContractProposalDraftResponseV1"
)]
pub struct DeployContractProposalDraftResponseV1 {
    /// Fingerprint of the complete stored [`iroha_data_model::governance::types::ProposalKind`].
    pub proposal_id: ProposalContentId,
    /// Exactly one typed deploy-contract proposal instruction.
    #[norito(json = "one_instruction")]
    pub tx_instructions: [GovernanceProposalInstructionDraftV1; 1],
}

/// Bound response for one SCCP route-governance proposal draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::governance_proposal_api::SccpRouteGovernanceProposalDraftResponseV1"
)]
pub struct SccpRouteGovernanceProposalDraftResponseV1 {
    /// Fingerprint of the complete stored [`iroha_data_model::governance::types::ProposalKind`].
    pub proposal_id: ProposalContentId,
    /// Exactly one typed `ProposeSccpRouteGovernance` instruction.
    #[norito(json = "one_instruction")]
    pub tx_instructions: [GovernanceProposalInstructionDraftV1; 1],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> GovernanceProposalInstructionDraftV1 {
        GovernanceProposalInstructionDraftV1 {
            wire_id: "example::Instruction".to_owned(),
            payload_hex: "00".to_owned(),
        }
    }

    fn with_instruction_count(mut value: norito::json::Value, count: usize) -> norito::json::Value {
        let instructions = value
            .as_object_mut()
            .and_then(|object| object.get_mut("tx_instructions"))
            .and_then(norito::json::Value::as_array_mut)
            .expect("response instruction array");
        instructions.clear();
        instructions.extend(core::iter::repeat_n(
            norito::json::to_value(&draft()).expect("encode draft"),
            count,
        ));
        value
    }

    #[test]
    fn proposal_draft_responses_require_exactly_one_instruction() {
        let deploy = DeployContractProposalDraftResponseV1 {
            proposal_id: ProposalContentId::new([0x11; 32]),
            tx_instructions: [draft()],
        };
        let sccp = SccpRouteGovernanceProposalDraftResponseV1 {
            proposal_id: ProposalContentId::new([0x22; 32]),
            tx_instructions: [draft()],
        };
        for count in [0, 2] {
            let hostile = with_instruction_count(
                norito::json::to_value(&deploy).expect("encode deploy response"),
                count,
            );
            assert!(
                norito::json::from_value::<DeployContractProposalDraftResponseV1>(hostile).is_err(),
                "deploy response accepted {count} instructions"
            );
            let hostile = with_instruction_count(
                norito::json::to_value(&sccp).expect("encode SCCP response"),
                count,
            );
            assert!(
                norito::json::from_value::<SccpRouteGovernanceProposalDraftResponseV1>(hostile)
                    .is_err(),
                "SCCP response accepted {count} instructions"
            );
        }
    }

    fn sccp_draft_request() -> SccpRouteGovernanceProposalDraftRequestV1 {
        use iroha_data_model::{
            NetworkId,
            block::BlockHeader,
            sccp::{
                governance::{
                    SccpGovernanceActionV1, SccpGovernanceBaseRevisionV1, SccpGovernanceSubjectV1,
                    SccpSetParametersActionV1,
                },
                params::SccpParametersV1,
            },
        };
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::new(
                [0x5a; iroha_crypto::Hash::LENGTH],
            )),
        );
        SccpRouteGovernanceProposalDraftRequestV1 {
            proposal: SccpGovernanceProposalV1 {
                network_id,
                base_revisions: vec![SccpGovernanceBaseRevisionV1 {
                    subject: SccpGovernanceSubjectV1::Parameters,
                    revision: 0,
                }],
                actions: vec![SccpGovernanceActionV1::SetParameters(
                    SccpSetParametersActionV1 {
                        next: SccpParametersV1::taira_default(),
                    },
                )],
            },
        }
    }

    #[test]
    fn sccp_route_governance_draft_request_carries_exactly_one_v1_proposal() {
        let request = sccp_draft_request();
        let value = norito::json::to_value(&request).expect("encode SCCP draft request");
        let fields = value.as_object().expect("request object");
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            ["proposal"]
        );
        let decoded: SccpRouteGovernanceProposalDraftRequestV1 =
            norito::json::from_value(value.clone()).expect("decode SCCP draft request");
        assert_eq!(decoded, request);
        let bytes = norito::to_bytes(&request).expect("encode Norito request");
        assert_eq!(
            norito::decode_from_bytes::<SccpRouteGovernanceProposalDraftRequestV1>(&bytes)
                .expect("decode Norito request"),
            request
        );

        let mut extra = value;
        extra
            .as_object_mut()
            .expect("request object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(
            norito::json::from_value::<SccpRouteGovernanceProposalDraftRequestV1>(extra).is_err(),
            "unknown request fields must reject"
        );
    }
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<
            super::DeployContractProposalDraftRequestV1,
        >(
            "iroha_torii_shared::governance_proposal_api::DeployContractProposalDraftRequestV1"
        );
        crate::captured_identity_tests::assert_bidirectional::<
            super::DeployContractProposalDraftResponseV1,
        >(
            "iroha_torii_shared::governance_proposal_api::DeployContractProposalDraftResponseV1"
        );
        crate::captured_identity_tests::assert_bidirectional::<
            super::GovernanceProposalInstructionDraftV1,
        >(
            "iroha_torii_shared::governance_proposal_api::GovernanceProposalInstructionDraftV1"
        );
        crate::captured_identity_tests::assert_bidirectional::<
            super::SccpRouteGovernanceProposalDraftRequestV1,
        >(
            "iroha_torii_shared::governance_proposal_api::SccpRouteGovernanceProposalDraftRequestV1"
        );
        crate::captured_identity_tests::assert_bidirectional::<
            super::SccpRouteGovernanceProposalDraftResponseV1,
        >(
            "iroha_torii_shared::governance_proposal_api::SccpRouteGovernanceProposalDraftResponseV1",
        );
    }
}

#[cfg(test)]
mod service_depth_tests {
    //! Owning checked service writers keep the caller depth on exact refusals.
    use super::*;
    use crate::service_checked_writer_test_support::audit;

    #[test]
    fn original_one_instruction_adapter_keeps_exact_fixed_array_and_refusal_depth() {
        let values = [GovernanceProposalInstructionDraftV1 {
            wire_id: "example::Instruction".into(),
            payload_hex: "00".into(),
        }];
        let mut expected = String::new();
        one_instruction::serialize(&values, &mut expected);
        audit(&expected, |sink| {
            one_instruction::serialize_bounded(&values, sink)
        });
        assert_eq!(values.len(), 1);
    }
}
