//! Shared certified predecessor and epoch-transition rules for native and portable consumers.

use super::{
    HeightContext, V2FinalityArtifact, V2FinalityValidationError,
    V2QuorumCertificateVerificationError, ValidationError,
    verify_quorum_certificate_with_validator_pops,
};

/// Invalid adjacency or authority in a revision-4 finalized context chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum V2FinalityLineageError {
    /// A height context is structurally invalid.
    #[error("invalid successor context: {0}")]
    Context(#[from] ValidationError),
    /// A predecessor artifact is structurally invalid.
    #[error("invalid predecessor artifact: {0}")]
    Artifact(#[from] V2FinalityValidationError),
    /// The successor does not extend the exact preceding decision and network policy.
    #[error("finality predecessor mismatch")]
    Parent,
    /// Election or authority fields changed without the exact certified epoch transition.
    #[error("finality epoch transition mismatch")]
    Epoch,
    /// A certificate, complete roster, or proof of possession is unauthenticated.
    #[error("finality lineage cryptography rejected: {0}")]
    Cryptography(#[from] V2QuorumCertificateVerificationError),
}

/// Validate the canonical successor relation, excluding cryptography and local durability.
///
/// Core supplements this shared relation with its Kura receipt. Portable consumers must call
/// [`verify_finality_successor`] to authenticate the certificates and every roster proof.
///
/// # Errors
/// Rejects a skipped/forked parent, snapshot reset, network/policy drift or unauthorized epoch.
pub fn validate_successor_height_context(
    parent: &V2FinalityArtifact,
    context: &HeightContext,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), V2FinalityLineageError> {
    context.validate()?;
    parent.validate()?;
    let predecessor = &parent.height_context;
    let parent_qc = context
        .parent_commit_qc
        .as_ref()
        .ok_or(V2FinalityLineageError::Parent)?;
    if context.snapshot_bootstrap.is_some()
        || parent.height.checked_add(1) != Some(context.height)
        || context.network_id != predecessor.network_id
        || context.mode != predecessor.mode
        || context.da_layout != predecessor.da_layout
        || context.execution_policy_hash != predecessor.execution_policy_hash
        || !parent_qc
            .as_ref()
            .same_commit_decision(parent.commit_qc.as_ref())
    {
        return Err(V2FinalityLineageError::Parent);
    }
    if let Some(snapshot) = &predecessor.next_epoch_snapshot {
        if context.epoch != snapshot.epoch
            || context.epoch_end_height != snapshot.epoch_end_height
            || context.mode != snapshot.mode
            || context.roster != snapshot.roster
            || context.quorum != snapshot.quorum
            || context.leader_seed != snapshot.leader_seed
            || context.kagemusha_mint_finality_authorization
                != snapshot.kagemusha_mint_finality_authorization
            || context.kagemusha_mint_finality_authority
                != snapshot.kagemusha_mint_finality_authority
            || proofs_of_possession != snapshot.validator_set_pops.as_slice()
        {
            return Err(V2FinalityLineageError::Epoch);
        }
    } else if context.epoch != predecessor.epoch
        || context.epoch_end_height != predecessor.epoch_end_height
        || context.roster != predecessor.roster
        || context.quorum != predecessor.quorum
        || context.leader_seed != predecessor.leader_seed
        || context.kagemusha_mint_finality_authorization
            != predecessor.kagemusha_mint_finality_authorization
        || context.kagemusha_mint_finality_authority
            != predecessor.kagemusha_mint_finality_authority
        || proofs_of_possession != parent.validator_set_pops.as_slice()
    {
        return Err(V2FinalityLineageError::Epoch);
    }
    Ok(())
}

/// Authenticate one adjacent finalized artifact after an independently trusted predecessor.
///
/// This verifies both artifacts, every current/future roster proof of possession, the carried
/// parent certificate, and the same successor relation used by Core. It never chooses a trust
/// anchor: the caller must pin the initial predecessor's exact hash/context and network from
/// independent configuration, then retain each verified successor. Decode/count/byte/time limits
/// belong to the enclosing proof consumer. A successful call does not prove transaction execution;
/// the exact signed block and execution-result inclusion still need their own verification.
///
/// # Errors
/// Rejects invalid cryptography, parent substitution, skipped heights and unauthorized contexts.
pub fn verify_finality_successor(
    parent: &V2FinalityArtifact,
    next: &V2FinalityArtifact,
) -> Result<(), V2FinalityLineageError> {
    validate_successor_height_context(parent, &next.height_context, &next.validator_set_pops)?;
    parent.verify()?;
    next.verify()?;
    let parent_qc = next
        .height_context
        .parent_commit_qc
        .as_ref()
        .ok_or(V2FinalityLineageError::Parent)?;
    verify_quorum_certificate_with_validator_pops(
        &parent.height_context,
        parent_qc,
        &parent.validator_set_pops,
    )?;
    Ok(())
}
