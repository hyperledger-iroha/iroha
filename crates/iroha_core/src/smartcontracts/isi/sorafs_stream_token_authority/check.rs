//! No-write native role-11 Check predicate over the executing State transaction.
//!
//! The submitted floor is checked against State and its committed Kura frame here (the certified
//! block id of its header and result, never the node-local `CommitQC`). An observer must still
//! independently pin that floor before signing and authenticate the eventual successful Check,
//! original operation execution and private receipt before any token is released.

use super::*;
use crate::query::stream_token_authority::eligibility::{check_live_custody, checked_phase};
use crate::query::stream_token_custody::NativeControl;
use crate::sumeragi::certified_chain::committed_block;
use iroha_data_model::sorafs::stream_token_authority::{
    STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1, StreamTokenCheckV1, StreamTokenFinalityFloorV1,
    validate_stream_token_check_claim_v1,
};
#[cfg(test)]
use iroha_data_model::sorafs::stream_token_authority::{
    StreamTokenCheckPhaseV1 as Phase, StreamTokenReviewedV1,
};
#[cfg(test)]
use sorafs_manifest::signer::stream_token::stream_token_binding_digest_v1;

fn check_floor(
    tx: &StateTransaction<'_, '_>,
    floor: StreamTokenFinalityFloorV1,
    challenge: [u8; 32],
    execution_height: u64,
) -> Result<(), Error> {
    if challenge == [0; 32]
        || floor.height == 0
        || floor.height >= execution_height
        || floor.block_hash == [0; 32]
        || *floor.context_id.0.as_ref() == [0; 32]
    {
        return Err(Error::BindingMismatch);
    }
    let offset = floor
        .height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or(Error::Finality)?;
    if tx.block_hashes().get(offset).map(|hash| *hash.as_ref()) != Some(floor.block_hash) {
        return Err(Error::Finality);
    }
    // Consensus-visible data only: the floor's committed header and result preimage, never the
    // node-local `CommitQC` (certificates are per node).
    let committed = committed_block(tx, floor.height).map_err(|_| Error::Finality)?;
    if *committed.block_hash().as_ref() != floor.block_hash || committed.id() != floor.context_id {
        return Err(Error::Finality);
    }
    Ok(())
}

fn check_retained_phase(
    instruction: &MutateSorafsStreamTokenAuthority,
    authority: &AccountId,
    tx: &StateTransaction<'_, '_>,
    check: &StreamTokenCheckV1,
    current: &NativeControl,
    now: u64,
) -> Result<(), Error> {
    let request = &instruction.request;
    let provider = request.provider_id;
    let head = journal::read_head(tx.world(), provider)?;
    let (state_reviewed, state_phase) = checked_phase(tx, provider, check, &head, now)?;
    validate_stream_token_check_claim_v1(
        request,
        *tx.network_id().as_bytes(),
        provider,
        current.index.revision,
        current.index.digest,
        &check.expected_operator,
        authority,
        check.challenge,
        check.floor,
        &state_reviewed,
        &state_phase,
    )
    .map_err(|_| Error::Conflict)
}

pub(super) fn evaluate_check(
    instruction: &MutateSorafsStreamTokenAuthority,
    authority: &AccountId,
    tx: &StateTransaction<'_, '_>,
    execution: &StreamTokenExecutionV1,
    check: &StreamTokenCheckV1,
) -> Result<(), Error> {
    // This bound is consensus-canonical; Check never copies an unbounded submitted row into
    // State or begins finality I/O for an oversized request.
    if norito::canonical_frame_len(&instruction.request).map_err(|_| Error::Invalid)?
        > STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1
    {
        return Err(Error::Invalid);
    }
    let request = &instruction.request;
    let provider = request.provider_id;
    check_floor(tx, check.floor, check.challenge, execution.height)?;
    let current = current_custody(
        tx,
        provider,
        request.expected_control_revision,
        request.expected_control_digest,
    )?;
    check_live_custody(&current, check, authority, execution.recorded_at_unix_ms)?;
    // Native authorization already established the current registered provider owner and the
    // observer's separately scoped permission; repeat it here so direct predicate callers cannot
    // bypass that precondition.
    if !authorized(tx, authority, provider, &request.action) {
        return Err(Error::BindingMismatch);
    }
    check_retained_phase(
        instruction,
        authority,
        tx,
        check,
        &current,
        execution.recorded_at_unix_ms,
    )?;
    // The purpose-owned query::stream_token_authority::observation consumer authenticates
    // the signed Check and original operation history. Private receipt validation belongs to
    // the signer/issuer and remains required before release.
    Ok(())
}

#[cfg(test)]
mod tests;
