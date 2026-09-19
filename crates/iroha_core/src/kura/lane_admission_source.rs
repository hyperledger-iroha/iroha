//! Exact finalized body observation for first-admission source reads.

use super::*;

/// Finality and optional local executed image read under the same canonical locks.
/// A missing body is authenticated absence, never a swallowed storage error.
#[derive(Debug)]
pub(crate) struct FinalizedAdmissionCarrierReadV1 {
    pub(crate) finality: V2FinalityArtifact,
    pub(crate) body: Option<Arc<SignedBlock>>,
}

/// One cumulative decoder budget for the complete canonical input observation.
/// Nested metadata/body/input decoders cannot reset this allocation account.
pub(crate) fn canonical_admission_read_decode_limits() -> Option<norito::DecodeLimits> {
    Some(norito::canonical_decode_limits(
        usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).ok()?,
    ))
}

/// Checked peak for the named owners in a cold canonical-input read.
///
/// This fixed single-slot envelope is deliberately independent of the retried
/// transaction's size: its first carrier can be a maximum executed block.
/// Canonical decoders count before materializing authentication buffers.
pub(crate) fn canonical_admission_read_working_set_bytes() -> Option<usize> {
    let wire = usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).ok()?;
    let decoded = canonical_admission_read_decode_limits()?.max_total_allocated_bytes();
    let metadata_wire =
        MAX_KURA_V2_FINALITY_RECORD_BYTES.checked_add(MAX_RETAINED_BLOCK_RECORD_BYTES)?;
    // ByteSink grows by doubling from 1 KiB. Account its capacity, not just
    // serialized length, for each independently bounded metadata owner.
    let metadata_encoding = MAX_KURA_V2_FINALITY_RECORD_BYTES
        .checked_next_power_of_two()?
        .checked_add(MAX_RETAINED_BLOCK_RECORD_BYTES.checked_next_power_of_two()?)?;
    let input = iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES;
    let input_graph = norito::canonical_decode_limits(input).max_total_allocated_bytes();
    [
        wire,                           // Original complete executed carrier bytes.
        decoded,           // Cumulative metadata, block and selected-input decode graph.
        decoded, // Full SignedBlock clone before stripping outputs for proposal authentication.
        wire,    // Canonical payload scratch, counted before allocation.
        wire,    // Canonical version-prefixed payload.
        wire,    // Canonical framed bytes.
        metadata_wire, // Original immutable finality + retained metadata snapshots.
        metadata_wire, // Headerless DecodeAll source copies.
        metadata_encoding, // Canonical metadata comparison buffer capacities.
        MAX_V2_FINALITY_ARTIFACT_BYTES, // Cryptographic finality serialization scratch.
        input_graph, // Owned certificate clone retained by complete-input validation.
        input,   // Canonical selected-input/binding authentication scratch.
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
}

impl Kura {
    /// Represent genuine finalized-body eviction in recovery fixtures.
    ///
    /// Unlike a snapshot's hash-only prefix, an evicted slot retains the exact
    /// executed wire length authenticated by its published finality. This helper
    /// does not exercise quota selection or reclaim the now-unreachable bytes.
    #[cfg(test)]
    pub(crate) fn evict_first_admission_body_for_testing(
        &self,
        height: NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let height_u64 = u64::try_from(height.get())?;
        let (_, finality, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(height_u64)?
            .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
        let body = self
            .read_block_body_under_prune_and_canonical_guards(height)?
            .ok_or(Error::CanonicalBlockWireMismatch { height: height_u64 })?;
        if body.hash() != expected_hash || finality.block_hash != expected_hash {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let _write_guard = self.block_store_write_lock.lock();
        let mut store = self.block_store.lock();
        if store.read_optional_da_cache(height_u64)?.is_some() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "evicted-body fixture requires an absent DA cache replica",
            ));
        }
        let count = store.read_exact_durable_index_count()?;
        store.write_block_index(
            height_u64 - 1,
            EVICTED_BLOCK_START,
            finality
                .commit_qc
                .execution_commitment
                .executed_block_wire_len,
        )?;
        store.publish_commit_marker(count)?;
        drop(store);
        let mut data = self.block_data.lock();
        if let Some((_, cached_body)) = data.get_mut(height.get() - 1) {
            *cached_body = None;
        }
        Ok(())
    }

    /// Read exact historical finality and a local executed body without cache effects.
    ///
    /// Missing required finality is an error. Only the shared fallible body
    /// kernel's authenticated evicted/imported-prefix absence can yield no body.
    /// The caller must assign an existing certified-body recovery owner before
    /// waiting; this read performs no network request or persistence.
    pub(crate) fn read_first_admission_carrier(
        &self,
        height: NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<FinalizedAdmissionCarrierReadV1> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let height_u64 = u64::try_from(height.get())?;
        // Authenticate immutable metadata without get_block/cache materialization.
        // The explicit fallible body kernel below is the sole full-body read.
        let blocks_dir = self.active_blocks_dir.lock().clone();
        let directory = Self::v2_finality_artifact_dir_for(&blocks_dir);
        let path = Self::v2_finality_artifact_path_for(&blocks_dir, height_u64);
        let (record, read_identity) = self
            .decode_v2_finality_record_at(&path, &directory)?
            .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
        Self::validate_v2_finality_record_at(&path, height_u64, expected_hash, &record)?;
        let (header, proposal_hash, wire_len, wire_hash, _, _) = self
            .retained_block_record_at_without_live_body(&blocks_dir, height_u64, expected_hash)?
            .ok_or(Error::MissingRetainedBlockRecord { height: height_u64 })?;
        if header != record.block_header {
            return Err(Error::ConflictingRetainedBlockRecord { height: height_u64 });
        }
        Self::validate_v2_finality_wire_bindings(
            height_u64,
            &record.artifact,
            proposal_hash,
            wire_len,
            wire_hash,
        )?;
        self.verify_v2_finality_artifact_at(&path, &directory, &record.artifact, &read_identity)?;
        let finality = record.artifact;
        drop(read_identity);
        if header.hash() != expected_hash || finality.block_hash != expected_hash {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let body = self.read_block_body_under_prune_and_canonical_guards(height)?;
        if let Some(body) = &body
            && (body.header() != header
                || body.canonical_proposal_wire_hash()? != finality.subject.payload_hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        Ok(FinalizedAdmissionCarrierReadV1 { finality, body })
    }
}
