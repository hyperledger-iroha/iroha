//! Exact original Reserve source joined to the same finalized Check State cut.

use iroha_data_model::{
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{
        FinalPromotionAuthorityActionV1, FinalPromotionOperationOutcomeV1,
        FinalPromotionOperationRecordV1,
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

use super::{Error, FinalPromotionCheckFloorV1};
use crate::{
    query::signer_check::SignerCertifiedWalkV1,
    query::{
        final_promotion_authority::{
            final_promotion_authority_request_digest_v1,
            operation::read_original_reserved_operation,
        },
        signer_check::{NativeCheckFloorV1, NativeCheckRoundV1, native_signed_entry_frame_v1},
    },
    state::{StateReadOnly, StateView, TransactionsReadOnly},
};

/// Maximum finalized lineage from the independently pinned floor through an admitted Reserve.
const MAX_RESERVE_HISTORY_BLOCKS_V1: u64 = 4_096;
/// Maximum cumulative canonical commit-certificate frames in one Reserve source replay.
const MAX_RESERVE_HISTORY_FINALITY_BYTES_V1: usize = 64 * 1024 * 1024;

pub(super) fn authenticate_reserved_source(
    view: &StateView<'_>,
    floor: FinalPromotionCheckFloorV1,
    applied: NativeCheckFloorV1,
    expected: &FinalPromotionOperationRecordV1,
    source: &SignedTransaction,
    round: &NativeCheckRoundV1,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<Error>> {
    let reserved = read_original_reserved_operation(
        &view.world,
        &expected.deployment_id,
        expected.intent.operation_id,
    )
    .map_err(|_| Error::Execution)?
    .ok_or(Error::Execution)?;
    let row = &reserved.record;
    let height = row.reserved.height;
    if row != expected
        || row.outcome != FinalPromotionOperationOutcomeV1::Reserved
        || row.execution != row.reserved
        || row.execution_origin != Some(row.reserved_origin)
        || floor.height >= height
        || applied.height < height
        || applied
            .height
            .checked_sub(floor.height)
            .and_then(|distance| distance.checked_add(1))
            .is_none_or(|span| span > MAX_RESERVE_HISTORY_BLOCKS_V1)
    {
        return Err(Error::Execution.into());
    }
    let entry_hash = source.hash_as_entrypoint();
    if row.reserved_origin.entry_hash != *entry_hash.as_ref()
        || source.network_id().map(|id| *id.as_bytes()) != Some(*view.network_id().as_bytes())
        || source.authority() != &row.reserved.authority
        || view.transactions.get(&entry_hash).map(|index| index.get())
            != usize::try_from(height).ok()
    {
        return Err(Error::Execution.into());
    }
    let Executable::Instructions(instructions) = source.instructions() else {
        return Err(Error::Execution.into());
    };
    let instruction = instructions
        .first()
        .filter(|_| instructions.len() == 1)
        .and_then(|item| {
            item.as_any()
                .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
        })
        .ok_or(Error::Execution)?;
    let FinalPromotionAuthorityActionV1::Reserve(request) = &instruction.action else {
        return Err(Error::Execution.into());
    };
    if instruction.deployment_id != row.deployment_id
        || instruction.expected_control_digest != row.custody.control_state_digest
        || request.intent != row.intent
        || request.custody != row.custody
        || final_promotion_authority_request_digest_v1(instruction, source.authority())
            .map_err(|_| Error::Execution)?
            != row.request_digest
    {
        return Err(Error::Execution.into());
    }
    // TODO: admit historical source clones and nested codec scratch from the original State pool.
    let source_frame =
        native_signed_entry_frame_v1(&TransactionEntrypoint::External(source.clone()))
            .map_err(|error| error.map_rejection(|_| Error::Execution))?;

    // The challenged Check already authenticated floor-to-applied continuity using this view.
    // Replaying floor-to-Reserve retains the target's certified block for its entry proof.
    let chain =
        SignerCertifiedWalkV1::new(view).map_err(|error| error.map_rejection(Error::from))?;
    let mut cumulative_bytes = 0_usize;
    let mut target = None;
    // Keep the one original iterator in place. Moving only this reference into
    // the loop avoids duplicate parent/successor storage while decoding another
    // certificate; the original source allowance and view remain unchanged.
    let mut walk = chain.walk(floor.height, height);
    for block in &mut walk {
        round.ensure_live().map_err(Error::from)?;
        // Convert only an actual error, without another full receipt-bearing
        // Result temporary live alongside the next original decoder frame.
        let receipt = match block {
            Ok(receipt) => receipt,
            Err(error) => return Err(error.map_rejection(Error::from)),
        };
        let block = receipt.in_view(view).map_err(Error::from)?;
        cumulative_bytes = cumulative_bytes
            .checked_add(block.certificate_len())
            .ok_or_else(|| {
                crate::execution_attempt::ExecutionAttemptError::Deferred(
                    ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
                )
            })?;
        if cumulative_bytes > MAX_RESERVE_HISTORY_FINALITY_BYTES_V1 {
            return Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
            ));
        }
        if block.height() == floor.height
            && (*block.block_hash().as_ref() != floor.block_hash || block.id() != floor.context_id)
        {
            return Err(Error::Finality.into());
        }
        target = Some(receipt);
    }
    drop(walk);
    round.ensure_live().map_err(Error::from)?;
    // Borrow the retained final receipt. Consuming Option::filter copies its
    // full decoded commitment into another debug-frame temporary.
    let target = target
        .as_ref()
        .filter(|block| block.height() == height)
        .ok_or(Error::Finality)?;
    let target = target.in_view(view).map_err(Error::from)?;
    authenticate_reserved_entry(target, row, &entry_hash, &source_frame, round)
}

// Entry proof work starts only after the complete original walk. Keep its fixed
// temporaries outside the frame that performs certificate decoding; this borrows
// the same final receipt and never re-reads, clones or re-verifies its source.
#[inline(never)]
fn authenticate_reserved_entry(
    target: &crate::sumeragi::certified_chain::CertifiedBlock,
    row: &FinalPromotionOperationRecordV1,
    entry_hash: &iroha_crypto::HashOf<TransactionEntrypoint>,
    source_frame: &[u8],
    round: &NativeCheckRoundV1,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<Error>> {
    let anchor = target
        .entry_anchor(entry_hash)
        .map_err(|_| Error::Execution)?;
    let block = target.block();
    let proof = block
        .network_execution_proof(entry_hash)
        .ok_or(Error::Execution)?;
    if !proof.verify(&anchor) || anchor.entry_index() != row.reserved_origin.entry_index {
        return Err(Error::Execution.into());
    }
    let actual = block
        .network_entrypoint_at(
            usize::try_from(row.reserved_origin.entry_index).map_err(|_| Error::Execution)?,
        )
        .ok_or(Error::Execution)?;
    let (_, output) = block
        .network_output_at(row.reserved_origin.entry_index)
        .ok_or(Error::Execution)?;
    if !output.result.is_ok()
        || target.block_time_ms() != row.reserved.recorded_at_unix_ms
        || native_signed_entry_frame_v1(actual)
            .map_err(|error| error.map_rejection(|_| Error::Execution))?
            .as_slice()
            != source_frame
    {
        return Err(Error::Execution.into());
    }
    round.ensure_live().map_err(Error::from)?;
    Ok(())
}
