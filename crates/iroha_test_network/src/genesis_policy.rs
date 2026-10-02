//! Bind builder-owned genesis policy commitments before publishing strictly validated output.

use super::{GenesisBlock, KeyPair, RawGenesisTransaction, Report, StagedGenesisPolicyHashes};
use color_eyre::eyre::eyre;
use iroha_core::block::BlockValidationError;
use iroha_data_model::block::SignedBlock;

/// Discover policy commitments only while constructing a new signed genesis.
///
/// Core has already executed the original inputs when it reports this exact typed
/// mismatch. Its rejected overlay and incomplete output are discarded. The caller
/// must bind these commitments, sign a new proposal, and strictly validate it before
/// publishing any genesis or network identity. Other failures retain their cause.
/// Authoritative signed-block validation must use the strict executor directly.
pub(crate) fn discover_generated_policy_hashes(
    validation: Result<StagedGenesisPolicyHashes, Report>,
) -> Result<StagedGenesisPolicyHashes, Report> {
    validation.or_else(|error| policy_mismatch_hashes(&error).ok_or(error))
}

fn policy_mismatch_hashes(error: &Report) -> Option<StagedGenesisPolicyHashes> {
    // preexecute_genesis_on_current_thread retains Core's boxed error beneath
    // contextual eyre reports. Never infer policy authority from diagnostic text.
    match error.downcast_ref::<Box<BlockValidationError>>()?.as_ref() {
        BlockValidationError::GenesisPolicyMismatch {
            actual_execution,
            actual_nexus,
            ..
        } => Some(StagedGenesisPolicyHashes {
            execution_policy: *actual_execution,
            nexus_amx: *actual_nexus,
        }),
        _ => None,
    }
}

/// Execute a locally generated manifest, binding provisional policy at most once.
/// Explicitly supplied commitments always take the strict single-execution path.
pub(crate) fn execute_generated_genesis(
    proposal: GenesisBlock,
    manifest: RawGenesisTransaction,
    key: &KeyPair,
    bind_generated_policy: bool,
    mut execute: impl FnMut(&GenesisBlock) -> Result<(SignedBlock, StagedGenesisPolicyHashes), Report>,
) -> Result<
    (
        GenesisBlock,
        StagedGenesisPolicyHashes,
        RawGenesisTransaction,
    ),
    Report,
> {
    let derived = match execute(&proposal) {
        Ok((executed, hashes)) => return Ok((GenesisBlock(executed), hashes, manifest)),
        Err(error) if bind_generated_policy => discover_generated_policy_hashes(Err(error))?,
        Err(error) => return Err(error),
    };
    let (rebound, manifest) = bind_generated_manifest(&proposal, manifest, key, derived)?;
    // Exactly one rebinding is allowed. A second mismatch, rejected instruction,
    // invalid signature, or any other validation error must remain an error.
    let (executed, actual) = execute(&rebound)?;
    if actual != derived {
        return Err(eyre!(
            "generated genesis policy binding is not an execution fixed point"
        ));
    }
    Ok((GenesisBlock(executed), actual, manifest))
}

fn bind_generated_manifest(
    proposal: &GenesisBlock,
    manifest: RawGenesisTransaction,
    key: &KeyPair,
    hashes: StagedGenesisPolicyHashes,
) -> Result<(GenesisBlock, RawGenesisTransaction), Report> {
    let creation_time_base_ms = proposal
        .0
        .external_transactions()
        .next()
        .ok_or_else(|| eyre!("generated genesis has no transaction creation-time base"))?
        .creation_time()
        .as_millis()
        .try_into()?;
    let mut context = manifest.sumeragi_context_parameters();
    context.execution_policy_hash = hashes.execution_policy.into();
    context.nexus_amx_context_hash = hashes.nexus_amx.into();
    let manifest = manifest
        .with_sumeragi_context_parameters(context)
        .with_consensus_meta()?;
    // Re-sign the original manifest, retaining its topology/PoPs, authority,
    // transaction order, timestamps, DA policies, and confidential policy.
    let rebound = manifest
        .clone()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            key,
            proposal.0.da_proof_policies().cloned(),
            proposal
                .0
                .header()
                .confidential_features()
                .and_then(|digest| digest.zk_policy_hash),
            creation_time_base_ms,
        )?;
    Ok((rebound, manifest))
}

#[cfg(test)]
#[path = "genesis_policy_tests.rs"]
mod tests;
