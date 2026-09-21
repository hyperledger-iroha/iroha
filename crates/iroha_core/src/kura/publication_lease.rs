//! Joint nonblocking custody of the physical Kura publication boundary.
//!
//! A lease excludes internal canonical, prune, geometry and sidecar writers.
//! It proves no source, finality or checkpoint identity by itself. The complete
//! publisher must rejoin its exact durable evidence under this boundary before
//! acquiring State writers, and release every guard before any async wait.

use super::{Error, Kura, PublicationGuard, PublicationMutex};
use iroha_data_model::NetworkId;

/// An archive identity refusal remains distinct from an actual storage failure.
#[derive(Debug)]
pub(crate) enum KuraArchiveCaptureAuthenticationError {
    /// Retained capture, original Kura, receipt or durable carrier differ.
    Identity(&'static str),
    /// Exact durable evidence could not be read or authenticated.
    Storage(Error),
}

impl From<Error> for KuraArchiveCaptureAuthenticationError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

/// A local physical refusal, independent of the decided block's validity.
pub(crate) enum KuraPublicationPreparationError {
    /// An actual storage owner must release before another acquisition attempt.
    Busy {
        /// Original Kura mutex which prevented the joint acquisition.
        field: &'static str,
        /// Release observation captured before probing that mutex.
        wait: concread::release::ReleaseWait,
    },
    /// The actual Kura requires storage repair, not a lock-release retry.
    Storage(Error),
}

impl std::fmt::Debug for KuraPublicationPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy { field, wait } => f
                .debug_struct("Busy")
                .field("field", field)
                .field("wait", wait)
                .finish(),
            Self::Storage(error) => f.debug_tuple("Storage").field(error).finish(),
        }
    }
}

impl From<Error> for KuraPublicationPreparationError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

/// All original Kura publication fences, acquired in the established order.
///
/// Fields drop from the innermost fence to the outermost. This is not a receipt
/// or a State publication authorization. Kura methods which acquire these locks
/// must not be called while this owner is retained.
#[must_use = "retain the physical boundary through the authorized operation"]
pub(crate) struct KuraPublicationLease<'kura> {
    kura: &'kura Kura,
    pending_canonical_bytes: u64,
    _sidecar: PublicationGuard<'kura>,
    _geometry: PublicationGuard<'kura>,
    _canonical: PublicationGuard<'kura>,
    _prune: PublicationGuard<'kura>,
}

impl Kura {
    /// Capture immutable pending-byte accounting before acquiring geometry/sidecar.
    /// The caller owns prune and canonical fences. Cold merge lookups must return
    /// the actual sidecar release observation instead of blocking behind its owner.
    fn try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards(
        &self,
    ) -> Result<u64, KuraPublicationPreparationError> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
        self.pending_block_bytes_with_merge_resolver(persisted_count, unindexed_bytes, |hash| {
            let sidecar = self.sidecar_lock.try_lock_or_wait().map_err(|wait| {
                KuraPublicationPreparationError::Busy {
                    field: "sidecar_lock",
                    wait,
                }
            })?;
            self.merge_entry_by_hash_with_sidecar_guard(hash, sidecar)
                .map_err(KuraPublicationPreparationError::Storage)
        })
    }

    /// Reauthenticate the original participant owner before live Apply stages State.
    ///
    /// Live Apply still owns a borrowed StateBlock and cannot retain decided work
    /// across a local lock refusal. Preserve its blocking publication lock order.
    /// The same check is available on a try-acquired lease for a retained consumer;
    /// that consumer still requires its original staged-frontier/source custody.
    /// Every Kura fence is released before this returns, so this is a durable join
    /// before staging, not custody through State visibility.
    pub(crate) fn reauthenticate_native_amx_prepublication(
        &self,
        token: &super::NativeAmxParticipantApplicationPrepublicationToken,
        block: &super::SignedBlock,
        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
        finality: &super::V2FinalityArtifact,
        frontiers: &[crate::state::AppliedNativeAmxParticipantFrontierMarker],
    ) -> super::Result<()> {
        self.ensure_canonical_storage_not_poisoned()?;
        let prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let canonical = self.canonical_chain_lock.lock();
        let geometry = self.lane_geometry_lock.lock();
        let sidecar = self.sidecar_lock.lock();
        let result = self.reauthenticate_native_amx_prepublication_under_publication_guards(
            token, block, manifest, finality, frontiers,
        );
        drop(sidecar);
        drop(geometry);
        drop(canonical);
        drop(prune);
        result
    }

    /// Read-only participant reauthentication under this original Kura's four
    /// publication fences. The live wrapper and retained lease share this oracle.
    fn reauthenticate_native_amx_prepublication_under_publication_guards(
        &self,
        token: &super::NativeAmxParticipantApplicationPrepublicationToken,
        block: &super::SignedBlock,
        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
        finality: &super::V2FinalityArtifact,
        frontiers: &[crate::state::AppliedNativeAmxParticipantFrontierMarker],
    ) -> super::Result<()> {
        let invalid = |message| Kura::invalid_lane_artifact_error(self.store_root.clone(), message);
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        if !token.original_kura.matches(self) {
            return Err(invalid(
                "Native AMX prepublication token belongs to another Kura instance",
            ));
        }
        if !token.authenticates_state_frontiers(block, manifest, finality, frontiers) {
            return Err(invalid(
                "Native AMX prepublication token differs from its exact State frontier projection",
            ));
        }
        // Authenticate durable canonical/finality even for an empty manifest;
        // an empty participant list is not authority for a foreign carrier.
        let Some((header, durable_finality, _)) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(
                token.application_block_height,
            )?
        else {
            return Err(invalid(
                "Native AMX prepublication finality read-back is unavailable",
            ));
        };
        if header != block.header()
            || super::HashOf::new(&durable_finality) != token.finality_artifact_hash
        {
            return Err(invalid(
                "Native AMX prepublication finality read-back differs from its original carrier",
            ));
        }
        let artifacts = super::native_amx_participant_application_artifacts(
            manifest,
            token.finality_artifact_hash,
        )
        .ok_or_else(|| invalid("Native AMX prepublication artifact projection failed"))?;
        if artifacts.len() != token.identities.len() {
            return Err(invalid(
                "Native AMX prepublication artifacts do not cover every original frontier",
            ));
        }
        for ((expected_manifest, expected_receipt), expected_identity) in
            artifacts.iter().zip(&token.identities)
        {
            let actual = self
                .authenticate_native_amx_participant_application_prepublication_under_publication_guards(
                    expected_manifest, expected_receipt, false,
                )?;
            if actual != *expected_identity {
                return Err(invalid(
                    "Native AMX durable participant differs from its original read-back identity",
                ));
            }
        }
        Ok(())
    }

    /// Authenticate a retained archive capture without an enclosing Kura lease.
    ///
    /// Standalone and aggregate publication use the same guarded oracle. The
    /// caller must admit exact body/finality decoding before entering either.
    pub(crate) fn authenticate_archive_capture(
        &self,
        network_id: NetworkId,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
        receipt: &super::KuraV2CommitReceipt,
    ) -> Result<(), KuraArchiveCaptureAuthenticationError> {
        let _prune = self.prune_lock.lock();
        let _canonical = self.canonical_chain_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        self.authenticate_archive_capture_under_publication_guards(
            network_id,
            height,
            block_hash,
            finalized_at_unix_ms,
            receipt,
        )
    }

    /// Caller retains prune, canonical and sidecar guards from this exact Kura.
    fn authenticate_archive_capture_under_publication_guards(
        &self,
        network_id: NetworkId,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
        receipt: &super::KuraV2CommitReceipt,
    ) -> Result<(), KuraArchiveCaptureAuthenticationError> {
        use KuraArchiveCaptureAuthenticationError::Identity;

        if receipt.height() != height || *receipt.block_hash().as_ref() != block_hash {
            return Err(Identity(
                "retained capture anchor differs from the durable Kura receipt",
            ));
        }
        let height_index = usize::try_from(height)
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .ok_or(Identity("durable Kura receipt height is not representable"))?;
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        if self.exact_durable_blocks_count()? < height_index.get()
            || self
                .get_durable_block_hash(height_index)
                .map(|hash| *hash.as_ref())
                != Some(block_hash)
        {
            return Err(Identity(
                "Kura canonical block log differs from the durable receipt",
            ));
        }
        let (header, artifact, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(height)?
            .ok_or(Identity(
                "Kura has no v2 finality artifact for the capture height",
            ))?;
        let recovered = super::v2_commit_receipt(&artifact);
        if receipt.height() != recovered.height()
            || receipt.block_hash() != recovered.block_hash()
            || receipt.context_id() != recovered.context_id()
            || receipt.subject() != recovered.subject()
            || receipt.certificate() != recovered.certificate()
            || receipt.artifact_hash() != recovered.artifact_hash()
            || artifact.height_context.network_id != network_id
            || artifact.height != height
            || *artifact.block_hash.as_ref() != block_hash
        {
            return Err(Identity(
                "Kura artifact, receipt, and capture identify different blocks",
            ));
        }
        // Archive publication requires the actual result-bearing body as well
        // as the retained header/wire association. This exact signed-wire reader
        // does not reacquire the lease's prune, canonical or sidecar fences.
        let block = self
            .read_block_body_under_prune_and_canonical_guards(height_index)?
            .ok_or(Identity(
                "exact result-bearing Kura block is unavailable to the retained capture",
            ))?;
        if block.header() != header
            || block.header().height().get() != height
            || *block.hash().as_ref() != block_hash
            || finalized_at_unix_ms == 0
            || finalized_at_unix_ms == u64::MAX
            || block.header().creation_time_ms != finalized_at_unix_ms
        {
            return Err(Identity(
                "result-bearing Kura block has a mismatched identity or timestamp",
            ));
        }
        Ok(())
    }

    /// Acquire prune, canonical, geometry and sidecar ownership without waiting.
    ///
    /// Every refusal releases all earlier guards before returning. The release
    /// observation belongs only to the failed lock; unwinding earlier successful
    /// probes therefore cannot wake this attempt itself. Retry must perform all
    /// exact storage checks again. The lease alone does not admit publication.
    pub(crate) fn try_publication_lease(
        &self,
    ) -> Result<KuraPublicationLease<'_>, KuraPublicationPreparationError> {
        fn acquire<'kura>(
            field: &'static str,
            lock: &'kura PublicationMutex,
        ) -> Result<PublicationGuard<'kura>, KuraPublicationPreparationError> {
            lock.try_lock_or_wait()
                .map_err(|wait| KuraPublicationPreparationError::Busy { field, wait })
        }
        // Canonical poisoning is permanent for this Kura. It cannot become a
        // lock-release dependency, even when another physical owner is busy.
        self.ensure_canonical_storage_not_poisoned()
            .map_err(KuraPublicationPreparationError::Storage)?;
        let prune = acquire("prune_lock", &self.prune_lock)?;
        // Active pruning also sets this flag while it owns prune_lock. Only
        // classify it as restart-required after acquiring that actual owner.
        self.ensure_prune_recovery_not_required()
            .map_err(KuraPublicationPreparationError::Storage)?;
        let canonical = acquire("canonical_chain_lock", &self.canonical_chain_lock)?;
        let pending_canonical_bytes =
            self.try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let geometry = acquire("lane_geometry_lock", &self.lane_geometry_lock)?;
        let sidecar = acquire("sidecar_lock", &self.sidecar_lock)?;
        self.ensure_prune_recovery_not_required()
            .map_err(KuraPublicationPreparationError::Storage)?;
        self.ensure_canonical_storage_not_poisoned()
            .map_err(KuraPublicationPreparationError::Storage)?;
        Ok(KuraPublicationLease {
            kura: self,
            pending_canonical_bytes,
            _sidecar: sidecar,
            _geometry: geometry,
            _canonical: canonical,
            _prune: prune,
        })
    }
}

#[cfg(test)]
impl<'kura> KuraPublicationLease<'kura> {
    /// Transfer structural catalog fixture guards into the common boundary.
    ///
    /// The fixture has resolved canonical recovery and completed authorized GC
    /// before acquiring sidecar. No fence is released or reacquired. Production
    /// geometry publication retains its original raw attempt under a lease.
    pub(super) fn from_geometry_guards(
        kura: &'kura Kura,
        sidecar: PublicationGuard<'kura>,
        geometry: PublicationGuard<'kura>,
        canonical: PublicationGuard<'kura>,
        prune: PublicationGuard<'kura>,
        pending_canonical_bytes: u64,
    ) -> Self {
        Self {
            kura,
            pending_canonical_bytes,
            _sidecar: sidecar,
            _geometry: geometry,
            _canonical: canonical,
            _prune: prune,
        }
    }
}

impl KuraPublicationLease<'_> {
    /// Release every physical Kura fence without invoking retry callbacks.
    /// The caller retains these original notifications through its outer fences.
    pub(crate) fn release_deferred(self) -> [concread::release::DeferredRelease; 4] {
        let Self {
            _sidecar,
            _geometry,
            _canonical,
            _prune,
            ..
        } = self;
        [
            _sidecar.release_deferred(),
            _geometry.release_deferred(),
            _canonical.release_deferred(),
            _prune.release_deferred(),
        ]
    }

    /// Pending canonical bytes captured before the inner publication fences.
    /// Prune/canonical custody keeps this snapshot valid for the lease lifetime.
    pub(super) fn pending_canonical_bytes(&self) -> u64 {
        self.pending_canonical_bytes
    }

    /// Private guarded implementations may borrow only this original physical owner.
    pub(super) fn original_kura(&self) -> &Kura {
        self.kura
    }

    /// Check original physical ownership before capturing another retained plan.
    /// This grants no source, finality or mutation authorization.
    pub(crate) fn belongs_to(&self, kura: &Kura) -> bool {
        std::ptr::eq(self.kura, kura)
    }

    /// Authenticate the exact archive owner and durable carrier under this lease.
    ///
    /// Success authorizes only the retained archive insertion, never State or
    /// source publication. No physical fence is reacquired and no token escapes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn authenticate_archive_capture(
        &self,
        original_kura: &Kura,
        network_id: NetworkId,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
        receipt: &super::KuraV2CommitReceipt,
    ) -> Result<(), KuraArchiveCaptureAuthenticationError> {
        if !std::ptr::eq(self.kura, original_kura) {
            return Err(KuraArchiveCaptureAuthenticationError::Identity(
                "retained archive capture belongs to another Kura instance",
            ));
        }
        self.kura
            .authenticate_archive_capture_under_publication_guards(
                network_id,
                height,
                block_hash,
                finalized_at_unix_ms,
                receipt,
            )
    }

    /// Rejoin one move-only participant token to its original Kura and exact State projection.
    ///
    /// Every manifest, receipt and latest index is read again under this lease's
    /// original prune/canonical/geometry/sidecar fences. This neither writes nor
    /// reacquires a fence, and it grants no State or source authorization. The
    /// caller must admit canonical decoding and proof work before acquisition.
    // TODO: enable this production entry only when the retained publisher owns
    // its original staged participant frontiers and complete source authority.
    #[cfg(test)]
    pub(crate) fn reauthenticate_native_amx_prepublication(
        &self,
        token: &super::NativeAmxParticipantApplicationPrepublicationToken,
        block: &super::SignedBlock,
        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
        finality: &super::V2FinalityArtifact,
        frontiers: &[crate::state::AppliedNativeAmxParticipantFrontierMarker],
    ) -> super::Result<()> {
        self.kura
            .reauthenticate_native_amx_prepublication_under_publication_guards(
                token, block, manifest, finality, frontiers,
            )
    }

    /// Rejoin exact durable finality/checkpoint under this original held boundary.
    ///
    /// This uses only already-guarded Kura readers and never reacquires the four
    /// fences. It must precede State writer acquisition; the caller admits the
    /// bounded decoding, body/finality cache and verification work in advance.
    /// Success proves this storage join only, never source or State permission.
    pub(crate) fn reauthenticate_checkpoint(
        &self,
        receipt: &super::KuraWsvCheckpointReceipt,
        finality: &super::V2FinalityArtifact,
        state_hash: iroha_crypto::Hash,
    ) -> super::Result<()> {
        self.kura
            .reauthenticate_checkpoint_under_publication_guards(receipt, finality, state_hash)
    }

    /// Observe exact published finality and its local result-bearing body under
    /// this original held boundary without reacquiring any publication fence.
    ///
    /// Uses the standalone first-admission reader's full validation. Missing or
    /// corrupt proof and occupied body corruption remain errors; authenticated
    /// evicted/imported-prefix body absence remains `None`. The read has no body
    /// cache effects and grants no source or State publication authorization.
    pub(crate) fn read_first_admission_carrier(
        &self,
        height: std::num::NonZeroUsize,
        expected_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    ) -> super::Result<super::lane_admission_source::FinalizedAdmissionCarrierReadV1> {
        self.kura
            .read_first_admission_carrier_under_prune_and_canonical_guards(height, expected_hash)
    }

    /// Require the final witness projection under the original publication fences.
    ///
    /// The caller has already joined exact durable body/finality/checkpoint on
    /// this lease. Missing or staged-only material grants no permission. This
    /// bounded reader acquires no publication lock, verifies every retained
    /// witness root and the exact finality artifact, then rejoins read identity.
    pub(crate) fn reauthenticate_execution_witness(
        &self,
        finality: &super::V2FinalityArtifact,
    ) -> super::Result<()> {
        let path = self.kura.kagemusha_finality_sidecar_path(finality.height);
        let Some((sidecar, read)) = self.kura.decode_kagemusha_finality_sidecar(&path)? else {
            return Err(Error::KagemushaFinalitySidecar(
                "State publication requires its final execution witness sidecar".to_owned(),
            ));
        };
        Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?;
        let directory = self.kura.kagemusha_finality_sidecar_dir();
        let current = self.kura.regular_sidecar_metadata(&path, &directory)?;
        if !current
            .as_ref()
            .is_some_and(|current| Kura::stable_sidecar_metadata_unchanged(&read.metadata, current))
        {
            return Err(Error::KagemushaFinalitySidecar(
                "final execution witness sidecar changed during publication authentication"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "publication_lease_tests.rs"]
mod tests;
