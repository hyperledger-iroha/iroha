//! Exact original Reserve source joined to the same finalized Check State cut.

use std::num::NonZeroUsize;

use iroha_data_model::{
    block::{consensus_v2::finality::V2FinalityArtifact, proofs::TrustedBlockProofAnchor},
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{
        FinalPromotionAuthorityActionV1, FinalPromotionOperationOutcomeV1,
        FinalPromotionOperationRecordV1,
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

use super::{Error, FinalPromotionCheckFloorV1};
use crate::{
    kura::KuraV2CommitReceipt,
    query::{
        final_promotion_authority::{
            final_promotion_authority_request_digest_v1,
            operation::read_original_reserved_operation,
        },
        signer_check::{NativeCheckFloorV1, NativeCheckRoundV1, native_signed_entry_frame_v1},
        signer_finality::verify_signer_finality_v1,
    },
    state::{StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::v2::VerifiedHeightContext,
};

/// Maximum finalized lineage from the independently pinned floor through an admitted Reserve.
const MAX_RESERVE_HISTORY_BLOCKS_V1: u64 = 4_096;
/// Maximum cumulative canonical finality frames in one Reserve source replay.
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
    // Replaying floor-to-Reserve retains the target's exact Kura/QC context for its entry proof.
    let mut cumulative_bytes = 0_usize;
    let mut parent: Option<(V2FinalityArtifact, KuraV2CommitReceipt)> = None;
    for cursor in floor.height..=height {
        round.ensure_live()?;
        let index = usize::try_from(cursor)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(Error::Finality)?;
        let hash = *view
            .block_hashes()
            .get(index.get() - 1)
            .ok_or(Error::Finality)?;
        verify_signer_finality_v1(view, cursor, *hash.as_ref()).map_err(|_| Error::Finality)?;
        let (artifact, receipt) = view
            .kura()
            .v2_finality_artifact_with_receipt(cursor)
            .map_err(|_| Error::Finality)?
            .ok_or(Error::Finality)?;
        cumulative_bytes = cumulative_bytes
            .checked_add(norito::canonical_frame_len(&artifact).map_err(|_| Error::Finality)?)
            .ok_or(Error::Finality)?;
        if cumulative_bytes > MAX_RESERVE_HISTORY_FINALITY_BYTES_V1 {
            return Err(Error::Finality);
        }
        let block = view
            .canonical_block_by_height(index)
            .map_err(|_| Error::Finality)?;
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
        if artifact.height != cursor
            || artifact.block_hash != hash
            || artifact.subject.parent_block_hash != expected_parent
            || artifact.height_context.network_id != *view.network_id()
            || receipt.height() != cursor
            || receipt.block_hash() != hash
            || receipt.context_id() != artifact.context_id()
            || block.header().height().get() != cursor
            || block.header().prev_block_hash() != expected_parent
            || block.hash() != hash
        {
            return Err(Error::Finality);
        }
        if cursor == floor.height {
            if *hash.as_ref() != floor.block_hash || artifact.context_id() != floor.context_id {
                return Err(Error::Finality);
            }
        } else {
            let (previous, previous_receipt) = parent.as_ref().ok_or(Error::Finality)?;
            VerifiedHeightContext::successor(
                artifact.height_context.clone(),
                artifact.validator_set_pops.clone(),
                previous,
                previous_receipt,
                &previous.validator_set_pops,
            )
            .map_err(|_| Error::Finality)?;
        }
        parent = Some((artifact, receipt));
    }
    round.ensure_live()?;
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or(Error::Execution)?;
    let block = view
        .canonical_block_by_height(index)
        .map_err(|_| Error::Execution)?;
    let (artifact, _) = parent.as_ref().ok_or(Error::Finality)?;
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        artifact,
        artifact.context_id(),
        &entry_hash,
    )
    .map_err(|_| Error::Execution)?;
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
    let block_time =
        u64::try_from(block.header().creation_time().as_millis()).map_err(|_| Error::Execution)?;
    if !output.result.is_ok()
        || block_time != row.reserved.recorded_at_unix_ms
        || native_signed_entry_frame_v1(actual).map_err(|_| Error::Execution)? != source_frame
    {
        return Err(Error::Execution);
    }
    round.ensure_live()?;
    Ok(())
}
