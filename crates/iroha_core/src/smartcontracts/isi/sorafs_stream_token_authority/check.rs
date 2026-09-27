//! No-write native role-11 Check predicate over the executing State transaction.
//!
//! The submitted floor is checked against State and durable Kura/QC here. An observer must still
//! independently pin that floor before signing and authenticate the eventual successful Check,
//! original operation execution and private receipt before any token is released.

use super::*;
use crate::query::stream_token_custody::NativeControl;
use iroha_data_model::sorafs::stream_token_authority::{
    STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1, StreamTokenCheckPhaseV1 as Phase,
    StreamTokenCheckV1, StreamTokenFinalityFloorV1, StreamTokenReviewedV1,
    validate_stream_token_check_claim_v1,
};
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
    verify_signer_finality_v1(tx, floor.height, floor.block_hash).map_err(|_| Error::Finality)?;
    let (artifact, receipt) = tx
        .kura()
        .v2_finality_artifact_with_receipt(floor.height)
        .map_err(|_| Error::Finality)?
        .ok_or(Error::Finality)?;
    if artifact.height != floor.height
        || *artifact.block_hash.as_ref() != floor.block_hash
        || artifact.height_context.network_id != *tx.network_id()
        || artifact.context_id() != floor.context_id
        || receipt.height() != floor.height
        || *receipt.block_hash().as_ref() != floor.block_hash
        || receipt.context_id() != floor.context_id
    {
        return Err(Error::Finality);
    }
    Ok(())
}

fn checked_phase(
    tx: &StateTransaction<'_, '_>,
    provider: ProviderId,
    check: &StreamTokenCheckV1,
    head: &OperationHeadV1,
    now: u64,
) -> Result<(StreamTokenReviewedV1, Phase), Error> {
    let id = check.reviewed.request.operation_id;
    match &check.phase {
        Phase::Current(_) => {
            if head.active_operation.is_some()
                || journal::read_history(tx.world(), provider, id)?.is_some()
                || check.reviewed.intent.previous_audit != head.audit
            {
                return Err(Error::Conflict);
            }
            Ok((check.reviewed, Phase::Current(head.audit)))
        }
        Phase::BeforeProvider(_) | Phase::AfterProvider(_) | Phase::BeforeCommit(_) => {
            let history =
                journal::read_history(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let record = &history.current;
            let row = &record.operation;
            if row.operation.outcome != StreamTokenOutcomeV1::Reserved
                || head.active_operation != Some(id)
                || head.revision != record.revision
                || head.digest != journal::record_digest(record)?
                || head.audit != row.operation.reviewed.intent.previous_audit
                || now < row.reserved_execution.recorded_at_unix_ms
                || now >= row.operation.reservation.expires_at_unix_ms
            {
                return Err(Error::Conflict);
            }
            let expected = match &check.phase {
                Phase::BeforeProvider(_) => Phase::BeforeProvider(row.clone()),
                Phase::AfterProvider(_) => Phase::AfterProvider(row.clone()),
                Phase::BeforeCommit(_) => Phase::BeforeCommit(row.clone()),
                _ => return Err(Error::Invalid),
            };
            Ok((history.reserved.operation.operation.reviewed, expected))
        }
        Phase::AfterCommit(_) | Phase::BeforeRelease(_) => {
            let history =
                journal::read_history(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let row = &history.current.operation;
            if !matches!(row.operation.outcome, StreamTokenOutcomeV1::Completed(_))
                || row
                    .terminal_execution
                    .as_ref()
                    .is_none_or(|terminal| now < terminal.recorded_at_unix_ms)
            {
                return Err(Error::Conflict);
            }
            let expected = match &check.phase {
                Phase::AfterCommit(_) => Phase::AfterCommit(row.clone()),
                Phase::BeforeRelease(_) => Phase::BeforeRelease(row.clone()),
                _ => return Err(Error::Invalid),
            };
            Ok((history.reserved.operation.operation.reviewed, expected))
        }
    }
}

fn check_live_custody(
    current: &NativeControl,
    check: &StreamTokenCheckV1,
    authority: &AccountId,
    now: u64,
) -> Result<(), Error> {
    let reviewed = &check.reviewed.request;
    eligible_custody(current, reviewed.original_custody.record_digest, now)?;
    if *authority != check.expected_observer
        || *authority == check.expected_operator
        || *authority == AccountId::new(current.state.policy.binding.public_key.clone())
        || reviewed.original_custody.control_state_digest != current.index.digest
        || reviewed.binding_digest
            != stream_token_binding_digest_v1(&current.state.policy.binding)
                .map_err(|_| Error::Custody)?
        || now < reviewed.issued_at_unix_ms
        || now >= reviewed.expires_at_unix_ms
    {
        return Err(Error::Custody);
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
    // TODO: A separate purpose-owned observer must authenticate the signed Check from its
    // independently pinned floor into one applied cut, historical Reserve/Complete proof,
    // current custody and private receipt before the daemon may sign or release a token.
    Ok(())
}

#[cfg(test)]
mod tests;
