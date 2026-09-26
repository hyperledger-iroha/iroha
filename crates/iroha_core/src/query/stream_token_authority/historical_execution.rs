//! Bounded role-11 signed operation history joined to one borrowed State view and pinned floor.
//!
//! This proves the original Reserve and any adjacent terminal transition. It does not execute or
//! finalize a challenged Check, establish fresh custody, or authorize a private token release.

use super::{Error, OperationHistoryV1, OperationRecordV1, read_history, request_digest};
use crate::{
    kura::KuraV2CommitReceipt,
    query::{
        signer_check::native_signed_entry_frame_v1, signer_finality::verify_signer_finality_v1,
    },
    state::{StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::v2::VerifiedHeightContext,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
        proofs::TrustedBlockProofAnchor,
    },
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenAuthorityActionV1 as Action, StreamTokenExecutionV1,
            StreamTokenFinalityFloorV1, StreamTokenOutcomeV1,
        },
    },
    transaction::{Executable, ExecutableBatchItem, TransactionEntrypoint},
};
use std::num::NonZeroUsize;

/// Canonical maximum number of consecutive finalized blocks replayed for one role-11 Check.
pub const STREAM_TOKEN_HISTORY_MAX_BLOCKS_V1: u64 = 4_096;
/// Canonical maximum cumulative finality-artifact frame bytes in one role-11 history replay.
pub const STREAM_TOKEN_HISTORY_FINALITY_MAX_BYTES_V1: usize = 64 * 1024 * 1024;

/// Non-serializable proof of exact role-11 Reserve/current rows and their signed execution.
///
/// The borrowed view must remain the same view later used for current custody, permissions,
/// Check application and phase eligibility. This capability alone never authorizes signing.
pub struct VerifiedStreamTokenHistoryV1<'view, 'state> {
    view: &'view StateView<'state>,
    history: OperationHistoryV1,
    floor: StreamTokenFinalityFloorV1,
}
impl<'view, 'state> VerifiedStreamTokenHistoryV1<'view, 'state> {
    /// The exact State view whose operation indexes and block hashes were authenticated.
    #[must_use]
    pub const fn view(&self) -> &'view StateView<'state> {
        self.view
    }

    /// Original immutable Reserve row, including its execution-derived coordinates.
    #[must_use]
    pub const fn reserved(&self) -> &OperationRecordV1 {
        &self.history.reserved
    }

    /// The same Reserve or its immediately adjacent Complete/Expire row.
    #[must_use]
    pub const fn current(&self) -> &OperationRecordV1 {
        &self.history.current
    }

    /// Independent finalized floor matched by the historical proof walk.
    #[must_use]
    pub const fn floor(&self) -> StreamTokenFinalityFloorV1 {
        self.floor
    }
}

fn bound_history_span(start: u64, floor: u64) -> Result<(), Error> {
    let count = floor
        .checked_sub(start)
        .and_then(|distance| distance.checked_add(1))
        .ok_or(Error::Finality)?;
    if count > STREAM_TOKEN_HISTORY_MAX_BLOCKS_V1 {
        return Err(Error::CheckUnavailable);
    }
    Ok(())
}

fn charge_finality_bytes(total: &mut usize, artifact: &V2FinalityArtifact) -> Result<(), Error> {
    let size = norito::canonical_frame_len(artifact).map_err(|_| Error::Finality)?;
    charge_finality_len(total, size)
}

fn charge_finality_len(total: &mut usize, size: usize) -> Result<(), Error> {
    *total = total.checked_add(size).ok_or(Error::CheckUnavailable)?;
    if *total > STREAM_TOKEN_HISTORY_FINALITY_MAX_BYTES_V1 {
        return Err(Error::CheckUnavailable);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetKind {
    Reserved,
    Terminal,
}

fn execution_for(
    record: &OperationRecordV1,
    kind: TargetKind,
) -> Result<&StreamTokenExecutionV1, Error> {
    match kind {
        TargetKind::Reserved
            if record.operation.operation.outcome == StreamTokenOutcomeV1::Reserved =>
        {
            Ok(&record.operation.reserved_execution)
        }
        TargetKind::Terminal
            if record.operation.operation.outcome != StreamTokenOutcomeV1::Reserved =>
        {
            record
                .operation
                .terminal_execution
                .as_ref()
                .ok_or(Error::CorruptHistory)
        }
        _ => Err(Error::CorruptHistory),
    }
}

fn signed_source_matches(
    record: &OperationRecordV1,
    kind: TargetKind,
    entry: &TransactionEntrypoint,
    network_id: [u8; 32],
    block_time: u64,
) -> Result<(), Error> {
    let execution = execution_for(record, kind)?;
    let TransactionEntrypoint::External(signed) = entry else {
        return Err(Error::Execution);
    };
    native_signed_entry_frame_v1(entry).map_err(|_| Error::Execution)?;
    if signed.network_id().map(|id| *id.as_bytes()) != Some(network_id)
        || signed.authority() != &execution.authority
        || *entry.hash().as_ref() != execution.transaction_hash
        || signed.hash_as_entrypoint() != entry.hash()
        || block_time != execution.recorded_at_unix_ms
    {
        return Err(Error::Execution);
    }
    let index = usize::try_from(execution.instruction_index).map_err(|_| Error::Execution)?;
    let item = match signed.instructions() {
        Executable::Instructions(instructions) => instructions.get(index),
        Executable::Batch(items) => items.get(index).and_then(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => Some(instruction),
            ExecutableBatchItem::ContractCall(_) => None,
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => None,
    }
    .and_then(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<MutateSorafsStreamTokenAuthority>()
    })
    .ok_or(Error::Execution)?;
    let request = &item.request;
    let row = &record.operation;
    if request.network_id != network_id
        || request.provider_id != row.provider_id
        || request_digest(item, signed.authority())? != record.request_digest
    {
        return Err(Error::Execution);
    }
    let exact_action = match (kind, &request.action, &row.operation.outcome) {
        (TargetKind::Reserved, Action::Reserve(reviewed), StreamTokenOutcomeV1::Reserved) => {
            request.expected_control_revision == row.custody_control_revision
                && request.expected_control_digest == row.custody_control_digest
                && reviewed == &row.operation.reviewed
        }
        (
            TargetKind::Terminal,
            Action::Complete(candidate),
            StreamTokenOutcomeV1::Completed(done),
        ) => {
            request.expected_control_revision == row.custody_control_revision
                && request.expected_control_digest == row.custody_control_digest
                && candidate.reviewed == done.reviewed
                && candidate.reservation == done.reservation
                && candidate.commitment == done.commitment
                && candidate.signatures_digest == done.signatures_digest
        }
        (TargetKind::Terminal, Action::Expire(candidate), StreamTokenOutcomeV1::Expired) => {
            // Expire may follow revocation under a newer current custody generation. The
            // original row intentionally retains Reserve's custody; the exact signed request
            // digest and successful native output prove the current-custody CAS at execution.
            candidate.operation_id == row.operation.reviewed.request.operation_id
                && candidate.reservation == row.operation.reservation
        }
        _ => false,
    };
    if !exact_action {
        return Err(Error::Execution);
    }
    Ok(())
}

fn authenticate_target(
    view: &StateView<'_>,
    record: &OperationRecordV1,
    kind: TargetKind,
    expected_context: HeightContextId,
) -> Result<(), Error> {
    let execution = execution_for(record, kind)?;
    let index = usize::try_from(execution.height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or(Error::Execution)?;
    let entry_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
        execution.transaction_hash,
    ));
    if view
        .transactions
        .get(&entry_hash)
        .map(|height| height.get())
        != Some(index.get())
    {
        return Err(Error::Execution);
    }
    let hash = *view
        .block_hashes()
        .get(index.get() - 1)
        .ok_or(Error::Finality)?;
    verify_signer_finality_v1(view, execution.height, *hash.as_ref())
        .map_err(|_| Error::Finality)?;
    let (artifact, receipt) = view
        .kura()
        .v2_finality_artifact_with_receipt(execution.height)
        .map_err(|_| Error::Finality)?
        .ok_or(Error::Finality)?;
    let block = view
        .canonical_block_by_height(index)
        .map_err(|_| Error::Finality)?;
    if artifact.context_id() != expected_context
        || artifact.block_hash != hash
        || receipt.height() != execution.height
        || receipt.block_hash() != hash
        || receipt.context_id() != expected_context
        || block.hash() != hash
    {
        return Err(Error::Finality);
    }
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        &artifact,
        expected_context,
        &entry_hash,
    )
    .map_err(|_| Error::Execution)?;
    let proof = block
        .network_execution_proof(&entry_hash)
        .ok_or(Error::Execution)?;
    if !proof.verify(&anchor) || anchor.entry_index() != execution.entry_index {
        return Err(Error::Execution);
    }
    let entry = block
        .network_entrypoint_at(
            usize::try_from(execution.entry_index).map_err(|_| Error::Execution)?,
        )
        .ok_or(Error::Execution)?;
    let (_, output) = block
        .network_output_at(execution.entry_index)
        .ok_or(Error::Execution)?;
    let block_time =
        u64::try_from(block.header().creation_time().as_millis()).map_err(|_| Error::Execution)?;
    if !output.result.is_ok() || block.header().height().get() != execution.height {
        return Err(Error::Execution);
    }
    signed_source_matches(
        record,
        kind,
        entry,
        *view.network_id().as_bytes(),
        block_time,
    )
}

/// Authenticate one exact role-11 history pair from a single applied State view through an
/// independently retained finalized Check floor.
///
/// The caller must pin `floor` before looking at the candidate operation and later use the same
/// borrowed view for custody, permission, phase and finalized Check eligibility. A decoded row,
/// self-selected floor, or this history capability alone cannot authorize private key use.
/// The protocol caps one replay at 4,096 heights and 64 MiB of finality frames; exhaustion returns
/// `CheckUnavailable` before any State side effect. Kura's own per-record bounds apply as well.
///
/// # Errors
/// Rejects missing or incoherent State rows, an unavailable bounded proof window, discontinuous
/// signed-RS16 finality, missing target membership, rejected output, or changed signed source.
pub fn authenticate_stream_token_history_to_floor_v1<'view, 'state>(
    view: &'view StateView<'state>,
    provider: ProviderId,
    operation_id: [u8; 32],
    floor: StreamTokenFinalityFloorV1,
) -> Result<VerifiedStreamTokenHistoryV1<'view, 'state>, Error> {
    if floor.height == 0 || floor.block_hash == [0; 32] || *floor.context_id.0.as_ref() == [0; 32] {
        return Err(Error::Finality);
    }
    let history = read_history(&view.world, provider, operation_id)?.ok_or(Error::Conflict)?;
    let start = history.reserved.operation.reserved_execution.height;
    let terminal_height = if history.current == history.reserved {
        None
    } else {
        Some(
            history
                .current
                .operation
                .terminal_execution
                .as_ref()
                .ok_or(Error::CorruptHistory)?
                .height,
        )
    };
    if start == 0 || terminal_height.is_some_and(|height| height <= start || height > floor.height)
    {
        return Err(Error::CorruptHistory);
    }
    bound_history_span(start, floor.height)?;
    if floor.height > u64::try_from(view.block_hashes().len()).map_err(|_| Error::Finality)? {
        return Err(Error::Finality);
    }
    let mut finality_bytes = 0_usize;
    let mut parent: Option<(V2FinalityArtifact, KuraV2CommitReceipt)> = None;
    let mut reserved_context = None;
    let mut terminal_context = None;
    for height in start..=floor.height {
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(Error::Finality)?;
        let hash = *view
            .block_hashes()
            .get(index.get() - 1)
            .ok_or(Error::Finality)?;
        verify_signer_finality_v1(view, height, *hash.as_ref()).map_err(|_| Error::Finality)?;
        let (artifact, receipt) = view
            .kura()
            .v2_finality_artifact_with_receipt(height)
            .map_err(|_| Error::Finality)?
            .ok_or(Error::Finality)?;
        charge_finality_bytes(&mut finality_bytes, &artifact)?;
        let expected_parent = if index.get() == 1 {
            None
        } else {
            Some(
                *view
                    .block_hashes()
                    .get(index.get() - 2)
                    .ok_or(Error::Finality)?,
            )
        };
        if artifact.height != height
            || artifact.block_hash != hash
            || artifact.subject.parent_block_hash != expected_parent
            || artifact.height_context.network_id != *view.network_id()
            || receipt.height() != height
            || receipt.block_hash() != hash
            || receipt.context_id() != artifact.context_id()
        {
            return Err(Error::Finality);
        }
        if let Some((previous, previous_receipt)) = parent.as_ref() {
            VerifiedHeightContext::successor(
                artifact.height_context.clone(),
                artifact.validator_set_pops.clone(),
                previous,
                previous_receipt,
                &previous.validator_set_pops,
            )
            .map_err(|_| Error::Finality)?;
        }
        if height == start {
            reserved_context = Some(artifact.context_id());
        }
        if terminal_height == Some(height) {
            terminal_context = Some(artifact.context_id());
        }
        if height == floor.height
            && (*hash.as_ref() != floor.block_hash || artifact.context_id() != floor.context_id)
        {
            return Err(Error::Finality);
        }
        parent = Some((artifact, receipt));
    }
    authenticate_target(
        view,
        &history.reserved,
        TargetKind::Reserved,
        reserved_context.ok_or(Error::Finality)?,
    )?;
    if terminal_height.is_some() {
        authenticate_target(
            view,
            &history.current,
            TargetKind::Terminal,
            terminal_context.ok_or(Error::Finality)?,
        )?;
    }
    Ok(VerifiedStreamTokenHistoryV1 {
        view,
        history,
        floor,
    })
}

#[cfg(test)]
#[path = "historical_execution/tests.rs"]
mod tests;
