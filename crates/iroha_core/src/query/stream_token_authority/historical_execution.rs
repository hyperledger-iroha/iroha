//! Bounded role-11 signed operation history joined to one borrowed State view and pinned floor.
//!
//! This proves the original Reserve and any adjacent terminal transition. It does not execute or
//! finalize a challenged Check, establish fresh custody, or authorize a private token release.

use super::{Error, OperationHistoryV1, OperationRecordV1, read_history, request_digest};
use crate::{
    query::signer_check::native_signed_entry_frame_v1,
    state::{StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::certified_chain::{CertifiedBlock, CertifiedChain},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
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

/// Canonical maximum number of consecutive finalized blocks replayed for one role-11 Check.
pub const STREAM_TOKEN_HISTORY_MAX_BLOCKS_V1: u64 = 4_096;
/// Canonical maximum cumulative commit-certificate frame bytes in one role-11 history replay.
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
    target: &CertifiedBlock,
) -> Result<(), Error> {
    let execution = execution_for(record, kind)?;
    let entry_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
        execution.transaction_hash,
    ));
    if target.height() != execution.height
        || view
            .transactions
            .get(&entry_hash)
            .map(|height| height.get())
            != usize::try_from(execution.height).ok()
    {
        return Err(Error::Execution);
    }
    let anchor = target
        .entry_anchor(&entry_hash)
        .map_err(|_| Error::Execution)?;
    let block = target.block();
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
    if !output.result.is_ok() {
        return Err(Error::Execution);
    }
    signed_source_matches(
        record,
        kind,
        entry,
        *view.network_id().as_bytes(),
        target.block_time_ms(),
    )
}

/// Authenticate one exact role-11 history pair from a single applied State view through an
/// independently retained finalized Check floor.
///
/// The caller must pin `floor` before looking at the candidate operation and later use the same
/// borrowed view for custody, permission, phase and finalized Check eligibility. A decoded row,
/// self-selected floor, or this history capability alone cannot authorize private key use.
/// The protocol caps one replay at 4,096 heights and 64 MiB of commit-certificate frames; exhaustion returns
/// `CheckUnavailable` before any State side effect. Kura's own per-record bounds apply as well.
///
/// # Errors
/// Rejects missing or incoherent State rows, an unavailable bounded proof window, discontinuous
/// certified chain, missing target membership, rejected output, or changed signed source.
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
    let chain = CertifiedChain::new(view).map_err(|_| Error::Finality)?;
    let mut finality_bytes = 0_usize;
    let mut reserved = None;
    let mut terminal = None;
    for block in chain.walk(start, floor.height) {
        let block = block.map_err(|_| Error::Finality)?;
        charge_finality_len(&mut finality_bytes, block.certificate_len())?;
        let height = block.height();
        if height == floor.height
            && (*block.block_hash().as_ref() != floor.block_hash || block.id() != floor.context_id)
        {
            return Err(Error::Finality);
        }
        if height == start {
            reserved = Some(block);
        } else if terminal_height == Some(height) {
            terminal = Some(block);
        }
    }
    authenticate_target(
        view,
        &history.reserved,
        TargetKind::Reserved,
        reserved.as_ref().ok_or(Error::Finality)?,
    )?;
    if terminal_height.is_some() {
        authenticate_target(
            view,
            &history.current,
            TargetKind::Terminal,
            terminal.as_ref().ok_or(Error::Finality)?,
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
