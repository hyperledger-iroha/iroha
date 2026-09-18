//! Bounded proof serving from one WSV-selected, exact finalized block body.

use std::{num::NonZeroU64, num::NonZeroUsize, sync::Arc};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader, SignedBlock,
        proofs::{
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, BlockProofs, BlockReceiptProof,
            ExecutionReceiptProof,
        },
    },
    query::error::QueryExecutionFail,
    transaction::TransactionEntrypoint,
};

use super::BlockProofError;
use crate::{
    kura::Kura,
    smartcontracts::isi::query::{BorrowedSingularStruct, bounded_bare_encoded_len},
};

/// Finite per-request admission for finalized block and Network proof serving.
///
/// Wire bytes are admitted before body I/O. Source, output, and transcript rows
/// are admitted before structural validation and tree construction. The complete
/// response is measured before cloning selected output/transcript values.
/// These ceilings do not reserve the Norito decoder's complete resident graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockProofLimits {
    /// Maximum exact executed block wire bytes, also capped by the proof protocol.
    pub max_block_wire_bytes: u64,
    /// Maximum aggregate source, output, transcript-owner and transcript rows.
    pub max_work_items: u64,
    /// Maximum complete canonical Norito proof frame or exact executed wire response.
    pub max_response_bytes: u64,
}

/// Resource refused by finalized proof serving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockProofResource {
    /// QC-authenticated body bytes, refused before reading the body.
    BlockWireBytes,
    /// Source/output/transcript rows, refused before validating or constructing trees.
    WorkItems,
    /// Complete response bytes, refused before copying its large owned values.
    ResponseBytes,
}

impl BlockProofLimits {
    fn limit(self, resource: BlockProofResource) -> u64 {
        match resource {
            BlockProofResource::BlockWireBytes => self
                .max_block_wire_bytes
                .min(AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 as u64),
            BlockProofResource::WorkItems => self.max_work_items,
            BlockProofResource::ResponseBytes => self.max_response_bytes,
        }
    }

    fn admit(
        self,
        block_height: NonZeroU64,
        resource: BlockProofResource,
        actual: u64,
    ) -> Result<(), BlockProofError> {
        let limit = self.limit(resource);
        if actual > limit {
            return Err(BlockProofError::CapacityExceeded {
                block_height,
                resource,
                actual,
                limit,
            });
        }
        Ok(())
    }
}

fn invalid_outputs(block_height: NonZeroU64, reason: impl ToString) -> BlockProofError {
    BlockProofError::InvalidOutputs {
        block_height,
        reason: reason.to_string(),
    }
}

fn read_finalized_body(
    kura: &Kura,
    block_height: NonZeroU64,
    expected_hash: HashOf<BlockHeader>,
    limits: BlockProofLimits,
    wire_response: bool,
) -> Result<(Arc<SignedBlock>, Vec<u8>), BlockProofError> {
    let height = usize::try_from(block_height.get())
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or(BlockProofError::HeightOutOfRange(block_height))?;
    // A zero resource allowance is a refusal, never an unlimited sentinel.
    for resource in [
        BlockProofResource::BlockWireBytes,
        BlockProofResource::WorkItems,
        BlockProofResource::ResponseBytes,
    ] {
        limits.admit(block_height, resource, 1)?;
    }
    if let Some(actual) = kura.get_block_hash(height)
        && actual != expected_hash
    {
        return Err(BlockProofError::BlockHashMismatch {
            block_height,
            expected: expected_hash,
            actual,
        });
    }
    let storage_error = |error: crate::kura::Error| BlockProofError::Storage {
        block_height,
        reason: error.to_string(),
    };
    let (durable_height, wire_len) = kura
        .durable_block_payload_len_by_hash(expected_hash)
        .map_err(storage_error)?
        .ok_or_else(|| BlockProofError::Storage {
            block_height,
            reason: "committed body has no available exact finalized wire authority".into(),
        })?;
    if durable_height != block_height.get() {
        return Err(BlockProofError::Storage {
            block_height,
            reason: "durable hash locator differs from the committed height".into(),
        });
    }
    limits.admit(block_height, BlockProofResource::BlockWireBytes, wire_len)?;
    if wire_response {
        limits.admit(block_height, BlockProofResource::ResponseBytes, wire_len)?;
    }
    let (block, wire) = kura
        .read_block_body_and_wire_with_wire_bound(height, expected_hash, wire_len)
        .map_err(storage_error)?
        .ok_or_else(|| BlockProofError::Storage {
            block_height,
            reason: "committed finalized body is unavailable".into(),
        })?;
    if block.header().height() != block_height {
        return Err(BlockProofError::BlockHeightMismatch {
            requested: block_height,
            actual: block.header().height(),
        });
    }
    if block.hash() != expected_hash {
        return Err(BlockProofError::BlockHashMismatch {
            block_height,
            expected: expected_hash,
            actual: block.hash(),
        });
    }
    if !block.has_results() {
        return Err(BlockProofError::MissingResults(block_height));
    }
    let mut work = 0_u64;
    for rows in std::iter::once(block.network_entrypoint_count())
        .chain(std::iter::once(block.execution_outputs().len()))
        .chain(std::iter::once(block.fastpq_transcripts().len()))
        .chain(block.fastpq_transcripts().values().map(Vec::len))
    {
        work = work
            .checked_add(rows as u64)
            .ok_or(BlockProofError::CapacityExceeded {
                block_height,
                resource: BlockProofResource::WorkItems,
                actual: u64::MAX,
                limit: limits.max_work_items,
            })?;
        limits.admit(block_height, BlockProofResource::WorkItems, work.max(1))?;
    }
    // TODO: reserve decoded graph, validation scratch and retained response memory
    // from the common query owner; wire/work ceilings alone are not that reservation.
    if block
        .execution_context()
        .is_some_and(|context| !context.has_current_version() || context.merge_entry.is_some())
    {
        return Err(invalid_outputs(
            block_height,
            "retired merge carrier is not a canonical Network source",
        ));
    }
    block
        .validate_output_merkle_cache()
        .map_err(|error| invalid_outputs(block_height, error))?;
    Ok((block, wire))
}

pub(super) fn executed_block_wire_from_kura(
    kura: &Kura,
    block_height: NonZeroU64,
    expected_hash: HashOf<BlockHeader>,
    limits: BlockProofLimits,
) -> Result<Vec<u8>, BlockProofError> {
    read_finalized_body(kura, block_height, expected_hash, limits, true).map(|(_, wire)| wire)
}

pub(super) fn block_proofs_for_entry_from_kura(
    kura: &Kura,
    block_height: NonZeroU64,
    expected_hash: HashOf<BlockHeader>,
    entry_hash: HashOf<TransactionEntrypoint>,
    limits: BlockProofLimits,
) -> Result<BlockProofs, BlockProofError> {
    let (block, wire) = read_finalized_body(kura, block_height, expected_hash, limits, false)?;
    let executed_block_wire_hash = Hash::new(&wire);
    // The original wire served its authentication purpose. Do not retain this
    // second representation while materializing the proof response.
    drop(wire);
    let input_index = block
        .network_input_hashes()
        .position(|hash| hash == entry_hash)
        .ok_or(BlockProofError::EntrypointNotFound {
            entry_hash,
            block_height,
        })?;
    let input_index = u32::try_from(input_index)
        .map_err(|_| invalid_outputs(block_height, "Network input index exceeds proof width"))?;
    let (output_index, _) = block
        .network_output_at(input_index)
        .ok_or_else(|| invalid_outputs(block_height, "Network input has no exact output join"))?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or_else(|| invalid_outputs(block_height, "joined output is absent"))?;
    let inputs = block.network_input_merkle_tree();
    let entry_commitment = inputs
        .commitment()
        .ok_or_else(|| invalid_outputs(block_height, "Network input commitment is absent"))?;
    let input_path = inputs
        .get_proof(input_index)
        .ok_or_else(|| invalid_outputs(block_height, "Network input proof is absent"))?;
    let entry_proof = BlockReceiptProof::new(entry_hash, input_path);
    let output_commitment = block
        .output_merkle_commitment()
        .ok_or_else(|| invalid_outputs(block_height, "output commitment is absent"))?;
    let output_path = block
        .output_proof(output_index)
        .ok_or_else(|| invalid_outputs(block_height, "output proof is absent"))?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let output_fields: [&dyn norito::core::SerializePayload; 2] = [output, &output_path];
    let borrowed_output = BorrowedSingularStruct::new(output_fields);
    let fields: [&dyn norito::core::SerializePayload; 9] = [
        &block_height,
        &expected_hash,
        &executed_block_wire_hash,
        &entry_hash,
        &entry_commitment,
        &entry_proof,
        &output_commitment,
        &borrowed_output,
        block.fastpq_transcripts(),
    ];
    let borrowed = BorrowedSingularStruct::new(fields);
    let align = norito::core::archived_payload_align::<BlockProofs>().max(1);
    let padding = (align - norito::core::Header::SIZE % align) % align;
    let overhead = (norito::core::Header::SIZE + padding) as u64;
    limits.admit(block_height, BlockProofResource::ResponseBytes, overhead)?;
    let payload_limit = limits.max_response_bytes - overhead;
    let payload_len = bounded_bare_encoded_len(&borrowed, payload_limit).map_err(|error| {
        if matches!(error, QueryExecutionFail::GasBudgetExceeded) {
            BlockProofError::CapacityExceeded {
                block_height,
                resource: BlockProofResource::ResponseBytes,
                actual: limits.max_response_bytes.saturating_add(1),
                limit: limits.max_response_bytes,
            }
        } else {
            invalid_outputs(block_height, error)
        }
    })?;
    let proofs = BlockProofs {
        block_height,
        block_hash: expected_hash,
        executed_block_wire_hash,
        entry_hash,
        entry_commitment,
        entry_proof,
        output_commitment,
        output_proof: ExecutionReceiptProof::new(output.clone(), output_path),
        fastpq_transcripts: block.fastpq_transcripts().clone(),
    };
    let actual = norito::canonical_frame_len(&proofs)
        .map_err(|error| invalid_outputs(block_height, error))? as u64;
    if actual != overhead + payload_len {
        return Err(invalid_outputs(
            block_height,
            "borrowed and owned proof lengths differ",
        ));
    }
    Ok(proofs)
}
