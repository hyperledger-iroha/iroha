//! Collect original native carriers, archived complete lane_evidence and actual query proofs.
//!
//! No network status or separate finality sidecar supplies authority. The exact stopped
//! Kura interval and immutable original projection records are independently bounded,
//! authenticated by the native chain owner, and retained through consuming publication.

use super::*;
use crate::kura::scaling_evidence::lane_proof::{
    LaneFrameV1, LaneMergeEvidenceV1, LaneProofState, signed_genesis_policy,
};
use iroha_core::{
    query::native_context_archive::NativeContextArchive,
    state::{AllocationBudget, NativeLaneStateProjectionV1},
};
use iroha_model_base::chain::ChainId;
use std::num::NonZeroUsize;

/// Independent finite admissions for complete native vector collection.
#[derive(Clone, Copy)]
pub(crate) struct CollectionLimits {
    /// Complete carrier-vector encoding, including every SignedBlockWire.
    pub(crate) carrier_bytes: u64,
    /// Complete committed-query vector encoding.
    pub(crate) query_bytes: u64,
    /// Combined original input bytes and returned canonical vector allocations.
    pub(crate) total_bytes: u64,
    /// One original complete context projection and its charged read buffer.
    pub(crate) context_bytes: usize,
    /// All Network inputs retained from actual global executed carriers.
    pub(crate) queries: usize,
    /// Complete typed Network outputs admitted per native carrier.
    pub(crate) leaves_per_carrier: usize,
}

/// Complete authenticated vectors retaining the original Kura completion and archive role.
/// This is a collector completion, not a useful-work or original schedule verdict.
pub(crate) struct CollectedNativeInputs {
    pub(super) disk: CanonicalKuraEvidenceComplete,
    _archive: NativeContextArchive,
    lane_sources: filesystem::LaneFrameReader,
    carrier: Vec<u8>,
    queries: Vec<u8>,
    height_count: u64,
    query_count: usize,
}
impl CollectedNativeInputs {
    /// Exact complete carrier/context vector; returned only after native verification.
    pub(crate) fn carrier_bytes(&self) -> &[u8] {
        &self.carrier
    }
    /// Exact complete committed Network query vector, including failed outputs.
    pub(crate) fn query_bytes(&self) -> &[u8] {
        &self.queries
    }
    /// Number of contiguous original carriers, including authenticated genesis.
    pub(crate) const fn height_count(&self) -> u64 {
        self.height_count
    }
    /// Number of all retained Network outputs; none are filtered by success.
    pub(crate) const fn query_count(&self) -> usize {
        self.query_count
    }
    /// Recheck original stopped-store ownership immediately before and after publication.
    pub(crate) fn recheck_sources(&self) -> Result<()> {
        self.disk.recheck_sources()?;
        self._archive.recheck_namespace()?;
        self.lane_sources.recheck_sources()?;
        Ok(())
    }
}

/// Read and authenticate the complete original stopped interval and archived context values.
///
/// Missing original archive records fail before any returned artifact. The caller's
/// chain, network and genesis epoch identity never come from the collected artifact.
/// Publication must retain this completion through both files and its final reply.
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_native_inputs(
    chain_id: ChainId,
    network: NetworkId,
    genesis_epoch_context_id: [u8; 32],
    original_genesis: &[u8],
    block_store: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    limits: CollectionLimits,
) -> Result<CollectedNativeInputs> {
    ensure!(
        reader_limits.first_height == 1 && reader_limits.last_height >= 2,
        "native collection requires original genesis and its authenticated H2 successor"
    );
    ensure!(
        limits.carrier_bytes > 0
            && limits.query_bytes > 0
            && limits.total_bytes > 0
            && limits.total_bytes <= MAX_PROOF_BYTES
            && limits.carrier_bytes <= limits.total_bytes
            && limits.query_bytes <= limits.total_bytes
            && limits.context_bytes > 0
            && limits.context_bytes <= MAX_CONTEXT_BYTES
            && limits.queries > 0
            && limits.queries <= MAX_REQUESTS
            && limits.leaves_per_carrier > 0
            && limits.leaves_per_carrier <= limits.queries
            && reader_limits.max_output_bytes <= limits.total_bytes,
        "invalid native collection byte or work admission"
    );
    let heights = usize::try_from(reader_limits.last_height)?;
    let slots = heights
        .checked_mul(std::mem::size_of::<NativeHeightEvidenceV1>())
        .and_then(|n| {
            limits
                .queries
                .checked_mul(
                    std::mem::size_of::<CommittedTransaction>()
                        + 64 * std::mem::size_of::<Option<Hash>>(),
                )
                .and_then(|q| n.checked_add(q))
        })
        .ok_or_else(|| eyre!("native collection slot overflow"))?;
    let mut retained_bytes = charged(0, slots, limits.total_bytes)?;
    retained_bytes = charged(retained_bytes, original_genesis.len(), limits.total_bytes)?;
    let budget = AllocationBudget::new(limits.context_bytes);
    let archive = NativeContextArchive::open_read_only(
        block_store,
        budget,
        NonZeroUsize::new(limits.context_bytes).ok_or_else(|| eyre!("zero context admission"))?,
    )?;
    let mut reader = CanonicalKuraEvidenceReader::open(block_store, reader_limits)?;
    let mut native = NativeExecutionEvidenceVerifier::new(
        chain_id.clone(),
        network,
        NativeExecutionEvidenceLimits {
            max_carriers: reader_limits.last_height,
            max_carrier_bytes: reader_limits.max_carrier_bytes as u64,
            max_context_bytes: limits.context_bytes as u64,
            max_retained_bytes: limits.total_bytes,
        },
    )
    .map_err(|error| eyre!(error))?;
    let original = decode_versioned_signed_block(original_genesis)?;
    let policy = signed_genesis_policy(&original)?;
    let mut lane_sources = filesystem::LaneFrameReader::new(block_store, limits.queries)?;
    let mut lane_proofs = LaneProofState::default();
    let mut pending_genesis = None;
    let mut carriers = Vec::new();
    carriers.try_reserve_exact(heights)?;
    let mut queries = Vec::new();
    queries.try_reserve_exact(limits.queries)?;
    for height in 1..=reader_limits.last_height {
        let carrier = reader.read_carrier(height)?;
        retained_bytes = charged(retained_bytes, carrier.len(), limits.total_bytes)?;
        let block = norito::with_decode_limits_scope(decode_limits(carrier.len()), || {
            decode_versioned_signed_block(&carrier)
        })?;
        ensure!(
            block.encode_wire()? == carrier && block.header().height().get() == height,
            "collected carrier is not exact canonical SignedBlockWire"
        );
        validate_carrier(&block, limits.leaves_per_carrier)?;
        if height == 1 {
            ensure!(
                block
                    .canonical_resultless_proposal()
                    .encode_wire()?
                    .as_slice()
                    == original_genesis,
                "collected genesis proposal differs from original signed genesis"
            );
            let epoch = iroha_data_model::sumeragi_finality::genesis_epoch(&block)
                .map_err(|error| eyre!(error))?;
            ensure!(
                epoch.context_id().map_err(|error| eyre!(error))? == genesis_epoch_context_id,
                "original signed genesis epoch differs from collection authority"
            );
        }
        let projection = archive.read_exact(height, block.hash())?;
        retained_bytes = charged(
            retained_bytes,
            projection.as_slice().len(),
            limits.total_bytes,
        )?;
        let state: NativeLaneStateProjectionV1 = canonical(projection.as_slice())?;
        let mut frames = Vec::new();
        if let Some(section) = block.lane_merge() {
            let count = section.merges.iter().try_fold(0usize, |count, merge| {
                ensure!(
                    !merge.is_empty() && merge.len() <= u64::from(policy.max_merge_blocks),
                    "collected lane range exceeds signed policy"
                );
                count
                    .checked_add(usize::try_from(merge.len())?)
                    .ok_or_else(|| eyre!("lane frame count overflow"))
            })?;
            ensure!(
                count <= limits.leaves_per_carrier,
                "collected lane frame count exceeds admission"
            );
            retained_bytes = charged(
                retained_bytes,
                count
                    .checked_mul(std::mem::size_of::<LaneFrameV1>() + 256)
                    .ok_or_else(|| eyre!("lane frame retention overflow"))?,
                limits.total_bytes,
            )?;
            frames.try_reserve_exact(count)?;
            let crypto = iroha_core::sumeragi::crypto::BlsCrypto::new();
            for merge in &section.merges {
                let instance = iroha_core::sumeragi::lanes::incarnation_instance(
                    &crypto,
                    &network,
                    &chain_id.to_string(),
                    merge.lane,
                    &merge.incarnation,
                );
                for lane_height in merge.from..=merge.to {
                    let maximum = MAX_CARRIER_BYTES
                        .min(usize::try_from(limits.total_bytes - retained_bytes)?);
                    let frame = lane_sources.read(instance.0, lane_height, maximum)?;
                    retained_bytes = charged(retained_bytes, frame.len(), limits.total_bytes)?;
                    frames.push(LaneFrameV1 {
                        lane: merge.lane,
                        frame,
                    });
                }
            }
        }
        if height == 1 {
            ensure!(
                frames.is_empty(),
                "genesis cannot contain original lane frames"
            );
            pending_genesis = Some((block.clone(), state.lanes.clone()));
        }
        let verified = native
            .push_height(block, projection.as_slice())
            .map_err(|error| eyre!(error))?;
        drop(projection);
        if let Some(verified) = verified {
            if let Some((genesis, lanes)) = pending_genesis.take() {
                lane_proofs.anchor_genesis(&genesis, lanes)?;
            }
            let block = verified.block();
            lane_proofs.verify(
                block,
                verified.lanes(),
                &frames,
                &policy,
                network,
                &chain_id,
            )?;
            ensure!(
                queries
                    .len()
                    .checked_add(block.network_entrypoint_count())
                    .is_some_and(|count| count <= limits.queries),
                "native collection query count exceeds admission"
            );
            for (index, entrypoint) in block.network_entrypoints().enumerate() {
                let index = u32::try_from(index)?;
                let (output_index, _) = block
                    .network_output_at(index)
                    .ok_or_else(|| eyre!("native input has no complete typed output"))?;
                let output = &block.execution_outputs()[output_index as usize];
                // Bound both original bodies before their owned query copies.
                let body_bytes = norito::canonical_frame_len(entrypoint)?
                    .checked_add(norito::canonical_frame_len(output)?)
                    .ok_or_else(|| eyre!("native query body length overflow"))?;
                ensure!(
                    body_bytes <= MAX_TRANSACTION_BYTES,
                    "native query body exceeds bound"
                );
                retained_bytes = charged(retained_bytes, body_bytes, limits.total_bytes)?;
                let query = CommittedTransaction {
                    block_hash: block.hash(),
                    entrypoint_hash: entrypoint.hash(),
                    entrypoint_proof: block
                        .network_input_proof(index)
                        .ok_or_else(|| eyre!("native input proof missing"))?,
                    entrypoint: entrypoint.clone(),
                    output_hash: HashOf::new(output),
                    output_proof: block
                        .output_proof(output_index)
                        .ok_or_else(|| eyre!("native output proof missing"))?,
                    output: output.clone(),
                };
                ensure!(
                    query.verify_inclusion_in_block(block),
                    "native collected query inclusion mismatch"
                );
                ensure!(
                    norito::canonical_frame_len(&query)? <= MAX_TRANSACTION_BYTES,
                    "native complete query exceeds bound"
                );
                queries.push(query);
            }
        } else {
            ensure!(
                height == 1,
                "native collector left a non-genesis carrier unauthenticated"
            );
        }
        let lane_evidence = LaneMergeEvidenceV1 { state, frames };
        ensure!(
            norito::canonical_frame_len(&lane_evidence)? <= MAX_CONTEXT_BYTES,
            "lane evidence exceeds per-carrier admission"
        );
        carriers.push(NativeHeightEvidenceV1 {
            carrier,
            lane_evidence,
        });
    }
    let disk = reader.finish()?;
    ensure!(
        disk.committed_height() == reader_limits.last_height
            && disk.carrier_count() == reader_limits.last_height,
        "native collection must cover the exact original stopped tip"
    );
    let carrier_length = norito::canonical_frame_len(&carriers)?;
    let query_length = norito::canonical_frame_len(&queries)?;
    ensure!(
        carrier_length as u64 <= limits.carrier_bytes && query_length as u64 <= limits.query_bytes,
        "native vector output exceeds admission"
    );
    retained_bytes = charged(retained_bytes, carrier_length, limits.total_bytes)?;
    charged(retained_bytes, query_length, limits.total_bytes)?;
    let query_count = queries.len();
    let carrier = norito::encode_canonical(&carriers)?;
    let queries = norito::encode_canonical(&queries)?;
    ensure!(
        carrier.len() == carrier_length && queries.len() == query_length,
        "native vector canonical length changed"
    );
    disk.recheck_sources()?;
    lane_sources.recheck_sources()?;
    Ok(CollectedNativeInputs {
        disk,
        _archive: archive,
        lane_sources,
        carrier,
        queries,
        height_count: reader_limits.last_height,
        query_count,
    })
}
