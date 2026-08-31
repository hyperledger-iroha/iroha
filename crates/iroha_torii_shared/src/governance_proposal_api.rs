//! Public DTOs for certificate-only governance proposal draft routes.
//!
//! These request types contain only immutable proposal content. Referendum
//! windows and voting modes belong to the retired proposal-backed referendum
//! flow and are deliberately absent from the first-release API.

use iroha_data_model::{
    account::AccountId,
    governance::types::{AbiVersion, ContractAbiHash, ContractCodeHash, ProposalContentId},
    isi::bridge::SccpRouteGovernanceActionV1,
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
        out.push('[')?;
        value[0].json_serialize_to(out)?;
        out.push(']')?;
        out.end_container();
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

/// Strict request for one SCCP route-governance proposal instruction draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct SccpRouteGovernanceProposalDraftRequestV1 {
    /// Atomic closed registry action proposed for enactment.
    pub action: SccpRouteGovernanceActionV1,
}

/// One canonical proposal instruction returned for local signing.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
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
pub struct SccpRouteGovernanceProposalDraftResponseV1 {
    /// Fingerprint of the complete stored [`iroha_data_model::governance::types::ProposalKind`].
    pub proposal_id: ProposalContentId,
    /// Exactly one typed SCCP route-governance proposal instruction.
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
}
