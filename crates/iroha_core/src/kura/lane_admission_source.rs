//! Exact finalized body observation for first-admission source reads.

use super::*;

/// Finality and optional local executed image read under the same canonical locks.
/// A missing body is authenticated absence, never a swallowed storage error.
#[derive(Debug)]
pub(crate) struct FinalizedAdmissionCarrierReadV1 {
    pub(crate) finality: V2FinalityArtifact,
    pub(crate) body: Option<Arc<SignedBlock>>,
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
        self.read_first_admission_carrier_under_prune_and_canonical_guards(height, expected_hash)
    }

    /// Read through the same exact oracle while the original Kura's prune and
    /// canonical fences remain held. This helper must not acquire either fence.
    pub(super) fn read_first_admission_carrier_under_prune_and_canonical_guards(
        &self,
        height: NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<FinalizedAdmissionCarrierReadV1> {
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let height_u64 = u64::try_from(height.get())?;
        let (header, finality, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(height_u64)?
            .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
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
