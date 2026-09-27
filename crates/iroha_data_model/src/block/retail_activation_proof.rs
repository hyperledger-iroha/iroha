//! Finalized first-release retail activation evidence.
//!
//! This authenticates an executed activation transaction at its block height.
//! It does not prove a later state-map value: the Sumeragi-v2 execution root
//! commits to that block's witness, not to all accumulated contract state.

use super::{
    SignedBlock,
    consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    proofs::{TrustedBlockProofAnchor, TrustedBlockProofAnchorError},
};
use crate::{
    Identifiable, Registrable,
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetDefinitionId, RetailDailyActivationV1, RetailDailyLimitPolicyV1,
    },
    isi::retail_daily_limit::ActivateRetailDailyLimitV1,
    transaction::{Executable, signed::TransactionEntrypoint},
};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};

/// Exact immutable values established by a verified successful activation.
///
/// The caller must independently pin the height context, owner, complete
/// policy, definition, domain, dataspace and entrypoint hash. This result is historical evidence,
/// not an authenticated read of the current state map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedRetailActivationV1 {
    /// Committed activation block height.
    pub height: u64,
    /// Finalized activation block header hash.
    pub block_hash: HashOf<super::BlockHeader>,
    /// Exact signed activation entrypoint hash.
    pub entry_hash: HashOf<TransactionEntrypoint>,
    /// Exact owner-installed first-release policy.
    pub policy: RetailDailyLimitPolicyV1,
    /// Activation marker derived from the finalized block timestamp.
    pub activation: RetailDailyActivationV1,
}

/// Why finalized activation evidence was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RetailActivationProofError {
    /// Finality, block wire, context, or output material failed verification.
    #[error("activation block finality or executed wire is not authenticated: {0}")]
    Finality(#[from] TrustedBlockProofAnchorError),
    /// Exact transaction and successful result are absent.
    #[error("activation is not one successful direct signed network input")]
    MissingSuccessfulDirectInput,
    /// Expected owner did not sign the activation.
    #[error("activation authority differs from the independently approved owner")]
    WrongOwner,
    /// The owner's payload signature is invalid.
    #[error("activation owner's signed transaction failed signature verification")]
    InvalidOwnerSignature,
    /// The direct transaction does not contain exactly one activation instruction.
    #[error("activation input is not one direct first-release instruction")]
    WrongInstruction,
    /// The policy and definition differ from independently approved coordinates.
    #[error("activation policy or definition differs from approved first-release coordinates")]
    WrongPolicy,
    /// The finalized block timestamp cannot yield the following UTC day.
    #[error("activation timestamp cannot represent the following UTC day")]
    InvalidTimestamp,
    /// Canonical policy bytes could not be encoded.
    #[error("activation policy cannot be encoded canonically")]
    InvalidPolicyEncoding,
}

/// Verify one direct activation from a finalized, result-bearing block.
///
/// `expected_context` must be established from an independently trusted chain
/// checkpoint. It must never be copied from `artifact` before verification.
/// The owner, complete policy (including cap, reserve, issuer accounts and
/// keys), and exact BPNG coordinates must come from signed allocation and
/// policy authority, not from the candidate transaction or this response.
///
/// # Errors
/// Refuses incomplete finality, failed execution, hidden batch/VM effects, or
/// any mismatch to the independently supplied release coordinates.
pub fn verify_finalized_retail_activation_v1(
    block: &SignedBlock,
    artifact: &V2FinalityArtifact,
    expected_context: HeightContextId,
    expected_entry_hash: HashOf<TransactionEntrypoint>,
    expected_owner: &AccountId,
    expected_policy: &RetailDailyLimitPolicyV1,
    expected_definition: &AssetDefinitionId,
    expected_domain: &DomainId,
    expected_dataspace: DataSpaceId,
) -> Result<FinalizedRetailActivationV1, RetailActivationProofError> {
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        block,
        artifact,
        expected_context,
        &expected_entry_hash,
    )?;
    let index = usize::try_from(anchor.entry_index())
        .map_err(|_| RetailActivationProofError::MissingSuccessfulDirectInput)?;
    let Some(TransactionEntrypoint::External(transaction)) = block.network_entrypoint_at(index)
    else {
        return Err(RetailActivationProofError::MissingSuccessfulDirectInput);
    };
    let Some((output_index, output)) = block.network_output_at(anchor.entry_index()) else {
        return Err(RetailActivationProofError::MissingSuccessfulDirectInput);
    };
    if output_index != anchor.entry_index() || output.result.as_ref().is_err() {
        return Err(RetailActivationProofError::MissingSuccessfulDirectInput);
    }
    if transaction.authority() != expected_owner {
        return Err(RetailActivationProofError::WrongOwner);
    }
    transaction
        .verify_signature()
        .map_err(|_| RetailActivationProofError::InvalidOwnerSignature)?;
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(RetailActivationProofError::WrongInstruction);
    };
    let [instruction] = instructions.as_ref() else {
        return Err(RetailActivationProofError::WrongInstruction);
    };
    let activation = instruction
        .as_any()
        .downcast_ref::<ActivateRetailDailyLimitV1>()
        .ok_or(RetailActivationProofError::WrongInstruction)?;
    let definition = activation.definition.clone().build(expected_owner);
    let policy = &activation.policy;
    if policy != expected_policy
        || definition.id() != expected_definition
        || &policy.asset_definition_id != expected_definition
        || policy.physical_dataspace != expected_dataspace
        || expected_dataspace == DataSpaceId::UNIVERSAL
        || definition.owning_domain().as_ref() != Some(expected_domain)
        || definition.balance_scope_policy() != AssetBalancePolicy::DataspaceRestricted
        || definition.spec().scale() != Some(2)
        || policy.revision != 1
        || !policy.institutional_exceptions.is_empty()
        || policy.validate_shape().is_err()
    {
        return Err(RetailActivationProofError::WrongPolicy);
    }
    let activated_at_ms = block.header().creation_time().as_millis();
    let activated_at_ms =
        u64::try_from(activated_at_ms).map_err(|_| RetailActivationProofError::InvalidTimestamp)?;
    let enforce_from_day_start_ms = activated_at_ms
        .checked_div(86_400_000)
        .and_then(|day| day.checked_add(1))
        .and_then(|day| day.checked_mul(86_400_000))
        .ok_or(RetailActivationProofError::InvalidTimestamp)?;
    let policy_bytes = norito::encode_canonical(policy)
        .map_err(|_| RetailActivationProofError::InvalidPolicyEncoding)?;
    let marker = RetailDailyActivationV1 {
        asset_definition_id: policy.asset_definition_id.clone(),
        physical_dataspace: policy.physical_dataspace,
        policy_digest: *Hash::new(policy_bytes).as_ref(),
        activated_at_ms,
        enforce_from_day_start_ms,
    };
    Ok(FinalizedRetailActivationV1 {
        height: anchor.block_height().get(),
        block_hash: anchor.block_hash(),
        entry_hash: anchor.entry_hash(),
        policy: policy.clone(),
        activation: marker,
    })
}
