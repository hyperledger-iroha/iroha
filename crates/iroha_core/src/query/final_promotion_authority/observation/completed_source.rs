//! Exact original Reserve and Complete sources joined to one finalized Check State cut.

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
            operation::{read_operation_slot, read_original_reserved_operation},
        },
        signer_check::{NativeCheckFloorV1, NativeCheckRoundV1, native_signed_entry_frame_v1},
        signer_finality::verify_signer_finality_v1,
    },
    state::{StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::v2::VerifiedHeightContext,
};

/// A completed Check may replay at most this many finalized blocks from its original floor.
const MAX_COMPLETED_HISTORY_BLOCKS_V1: u64 = 4_096;
/// The complete Reserve-to-Complete lineage has one cumulative canonical finality-frame cap.
const MAX_COMPLETED_HISTORY_FINALITY_BYTES_V1: usize = 64 * 1024 * 1024;

pub(super) fn authenticate_completed_source(
    view: &StateView<'_>,
    floor: FinalPromotionCheckFloorV1,
    applied: NativeCheckFloorV1,
    expected: &FinalPromotionOperationRecordV1,
    reserve: &SignedTransaction,
    complete: &SignedTransaction,
    round: &NativeCheckRoundV1,
) -> Result<(), Error> {
    let terminal = read_operation_slot(
        &view.world,
        &expected.deployment_id,
        expected.intent.operation_id,
    )
    .map_err(|_| Error::Execution)?
    .ok_or(Error::Execution)?;
    let admitted = read_original_reserved_operation(
        &view.world,
        &expected.deployment_id,
        expected.intent.operation_id,
    )
    .map_err(|_| Error::Execution)?
    .ok_or(Error::Execution)?;
    let original = &admitted.record;
    let complete_origin = expected.execution_origin.ok_or(Error::Execution)?;
    if &terminal.record != expected
        || original.outcome != FinalPromotionOperationOutcomeV1::Reserved
        || original.execution != original.reserved
        || original.execution_origin != Some(original.reserved_origin)
        || expected.revision != original.revision.checked_add(1).ok_or(Error::Execution)?
        || expected.predecessor_digest != admitted.index.head.digest
        || expected.reserved != original.reserved
        || expected.reserved_origin != original.reserved_origin
        || expected.intent != original.intent
        || expected.custody != original.custody
        || expected.reservation != original.reservation
        || !matches!(
            expected.outcome,
            FinalPromotionOperationOutcomeV1::Completed(_)
        )
        || floor.height >= original.reserved.height
        || original.reserved.height >= expected.execution.height
        || applied.height < expected.execution.height
        || applied
            .height
            .checked_sub(floor.height)
            .and_then(|distance| distance.checked_add(1))
            .is_none_or(|span| span > MAX_COMPLETED_HISTORY_BLOCKS_V1)
        || expected.execution.recorded_at_unix_ms >= expected.reservation.expires_at_unix_ms
    {
        return Err(Error::Execution);
    }

    let reserve_hash = reserve.hash_as_entrypoint();
    let complete_hash = complete.hash_as_entrypoint();
    if original.reserved_origin.entry_hash != *reserve_hash.as_ref()
        || complete_origin.entry_hash != *complete_hash.as_ref()
        || reserve.network_id().map(|id| *id.as_bytes()) != Some(*view.network_id().as_bytes())
        || complete.network_id().map(|id| *id.as_bytes()) != Some(*view.network_id().as_bytes())
        || reserve.authority() != &original.reserved.authority
        || complete.authority() != &expected.execution.authority
        || view
            .transactions
            .get(&reserve_hash)
            .map(|index| index.get())
            != usize::try_from(original.reserved.height).ok()
        || view
            .transactions
            .get(&complete_hash)
            .map(|index| index.get())
            != usize::try_from(expected.execution.height).ok()
    {
        return Err(Error::Execution);
    }

    let Executable::Instructions(reserve_instructions) = reserve.instructions() else {
        return Err(Error::Execution);
    };
    let reserve_instruction = reserve_instructions
        .first()
        .filter(|_| reserve_instructions.len() == 1)
        .and_then(|item| {
            item.as_any()
                .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
        })
        .ok_or(Error::Execution)?;
    let FinalPromotionAuthorityActionV1::Reserve(reserve_request) = &reserve_instruction.action
    else {
        return Err(Error::Execution);
    };
    if reserve_instruction.deployment_id != original.deployment_id
        || reserve_instruction.expected_control_digest != original.custody.control_state_digest
        || reserve_request.intent != original.intent
        || reserve_request.custody != original.custody
        || final_promotion_authority_request_digest_v1(reserve_instruction, reserve.authority())
            .map_err(|_| Error::Execution)?
            != original.request_digest
    {
        return Err(Error::Execution);
    }

    let Executable::Instructions(complete_instructions) = complete.instructions() else {
        return Err(Error::Execution);
    };
    let complete_instruction = complete_instructions
        .first()
        .filter(|_| complete_instructions.len() == 1)
        .and_then(|item| {
            item.as_any()
                .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
        })
        .ok_or(Error::Execution)?;
    let FinalPromotionAuthorityActionV1::Complete(complete_request) = &complete_instruction.action
    else {
        return Err(Error::Execution);
    };
    let FinalPromotionOperationOutcomeV1::Completed(completed) = expected.outcome else {
        return Err(Error::Execution);
    };
    if complete_instruction.deployment_id != expected.deployment_id
        || complete_instruction.expected_control_digest != expected.custody.control_state_digest
        || complete_request.intent != expected.intent
        || complete_request.custody != expected.custody
        || complete_request.reservation != expected.reservation
        || complete_request.commitment != completed.commitment
        || complete_request.signatures_digest != completed.signatures_digest
        || final_promotion_authority_request_digest_v1(complete_instruction, complete.authority())
            .map_err(|_| Error::Execution)?
            != expected.request_digest
    {
        return Err(Error::Execution);
    }

    let reserve_frame =
        native_signed_entry_frame_v1(&TransactionEntrypoint::External(reserve.clone()))
            .map_err(|_| Error::Execution)?;
    let complete_frame =
        native_signed_entry_frame_v1(&TransactionEntrypoint::External(complete.clone()))
            .map_err(|_| Error::Execution)?;

    // The challenged Check already authenticated floor-to-applied continuity. Replaying the
    // original floor through Complete retains both target Kura/QC contexts and exact entries.
    let mut cumulative_bytes = 0_usize;
    let mut parent: Option<(V2FinalityArtifact, KuraV2CommitReceipt)> = None;
    for cursor in floor.height..=expected.execution.height {
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
        if cumulative_bytes > MAX_COMPLETED_HISTORY_FINALITY_BYTES_V1 {
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

        for (execution, origin, entry_hash, source_frame) in [
            (
                &original.reserved,
                original.reserved_origin,
                &reserve_hash,
                reserve_frame.as_slice(),
            ),
            (
                &expected.execution,
                complete_origin,
                &complete_hash,
                complete_frame.as_slice(),
            ),
        ] {
            if execution.height != cursor {
                continue;
            }
            round.ensure_live()?;
            let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &block,
                &artifact,
                artifact.context_id(),
                entry_hash,
            )
            .map_err(|_| Error::Execution)?;
            let proof = block
                .network_execution_proof(entry_hash)
                .ok_or(Error::Execution)?;
            if !proof.verify(&anchor) || anchor.entry_index() != origin.entry_index {
                return Err(Error::Execution);
            }
            let actual = block
                .network_entrypoint_at(
                    usize::try_from(origin.entry_index).map_err(|_| Error::Execution)?,
                )
                .ok_or(Error::Execution)?;
            let (_, output) = block
                .network_output_at(origin.entry_index)
                .ok_or(Error::Execution)?;
            let block_time = u64::try_from(block.header().creation_time().as_millis())
                .map_err(|_| Error::Execution)?;
            if !output.result.is_ok()
                || block_time != execution.recorded_at_unix_ms
                || native_signed_entry_frame_v1(actual)
                    .map_err(|_| Error::Execution)?
                    .as_slice()
                    != source_frame
            {
                return Err(Error::Execution);
            }
        }
        parent = Some((artifact, receipt));
    }
    round.ensure_live()?;
    Ok(())
}
