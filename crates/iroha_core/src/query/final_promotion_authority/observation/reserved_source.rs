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
    query::{
        final_promotion_authority::{
            final_promotion_authority_request_digest_v1,
            operation::read_original_reserved_operation,
        },
        signer_check::{NativeCheckFloorV1, NativeCheckRoundV1, native_signed_entry_frame_v1},
    },
    state::{StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::certified_chain::CertifiedChain,
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
) -> Result<(), Error> {
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
        return Err(Error::Execution);
    }
    let entry_hash = source.hash_as_entrypoint();
    if row.reserved_origin.entry_hash != *entry_hash.as_ref()
        || source.network_id().map(|id| *id.as_bytes()) != Some(*view.network_id().as_bytes())
        || source.authority() != &row.reserved.authority
        || view.transactions.get(&entry_hash).map(|index| index.get())
            != usize::try_from(height).ok()
    {
        return Err(Error::Execution);
    }
    let Executable::Instructions(instructions) = source.instructions() else {
        return Err(Error::Execution);
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
        return Err(Error::Execution);
    };
    if instruction.deployment_id != row.deployment_id
        || instruction.expected_control_digest != row.custody.control_state_digest
        || request.intent != row.intent
        || request.custody != row.custody
        || final_promotion_authority_request_digest_v1(instruction, source.authority())
            .map_err(|_| Error::Execution)?
            != row.request_digest
    {
        return Err(Error::Execution);
    }
    let source_frame =
        native_signed_entry_frame_v1(&TransactionEntrypoint::External(source.clone()))
            .map_err(|_| Error::Execution)?;

    // The challenged Check already authenticated floor-to-applied continuity using this view.
    // Replaying floor-to-Reserve retains the target's certified block for its entry proof.
    let chain = CertifiedChain::new(view).map_err(|_| Error::Finality)?;
    let mut cumulative_bytes = 0_usize;
    let mut target = None;
    for block in chain.walk(floor.height, height) {
        round.ensure_live()?;
        let block = block.map_err(|_| Error::Finality)?;
        cumulative_bytes = cumulative_bytes
            .checked_add(block.certificate_len())
            .ok_or(Error::Finality)?;
        if cumulative_bytes > MAX_RESERVE_HISTORY_FINALITY_BYTES_V1 {
            return Err(Error::Finality);
        }
        if block.height() == floor.height
            && (*block.block_hash().as_ref() != floor.block_hash || block.id() != floor.context_id)
        {
            return Err(Error::Finality);
        }
        target = Some(block);
    }
    round.ensure_live()?;
    let target = target
        .filter(|block| block.height() == height)
        .ok_or(Error::Finality)?;
    let anchor = target
        .entry_anchor(&entry_hash)
        .map_err(|_| Error::Execution)?;
    let block = target.block();
    let proof = block
        .network_execution_proof(&entry_hash)
        .ok_or(Error::Execution)?;
    if !proof.verify(&anchor) || anchor.entry_index() != row.reserved_origin.entry_index {
        return Err(Error::Execution);
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
        || native_signed_entry_frame_v1(actual).map_err(|_| Error::Execution)? != source_frame
    {
        return Err(Error::Execution);
    }
    round.ensure_live()?;
    Ok(())
}
