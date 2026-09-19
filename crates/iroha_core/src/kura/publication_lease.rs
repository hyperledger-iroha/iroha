//! Joint nonblocking custody of the physical Kura publication boundary.
//!
//! A lease excludes internal canonical, prune, geometry and sidecar writers.
//! It proves no source, finality or checkpoint identity by itself. The complete
//! publisher must rejoin its exact durable evidence under this boundary before
//! acquiring State writers, and release every guard before any async wait.

use super::{Error, Kura, PublicationGuard, PublicationMutex};

/// A local physical refusal, independent of the decided block's validity.
#[derive(Debug)]
pub(crate) enum KuraPublicationPreparationError {
    /// An actual storage owner must release before another acquisition attempt.
    Busy {
        /// Original Kura mutex which prevented the joint acquisition.
        field: &'static str,
        /// Release observation captured before probing that mutex.
        wait: mv::ReleaseWait,
    },
    /// The actual Kura requires storage repair, not a lock-release retry.
    Storage(Error),
}

/// All original Kura publication fences, acquired in the established order.
///
/// Fields drop from the innermost fence to the outermost. This is not a receipt
/// or a State publication authorization. Kura methods which acquire these locks
/// must not be called while this owner is retained.
#[must_use = "retain the physical boundary through the authorized operation"]
pub(crate) struct KuraPublicationLease<'kura> {
    kura: &'kura Kura,
    _sidecar: PublicationGuard<'kura>,
    _geometry: PublicationGuard<'kura>,
    _canonical: PublicationGuard<'kura>,
    _prune: PublicationGuard<'kura>,
}

impl Kura {
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
        let geometry = acquire("lane_geometry_lock", &self.lane_geometry_lock)?;
        let sidecar = acquire("sidecar_lock", &self.sidecar_lock)?;
        self.ensure_prune_recovery_not_required()
            .map_err(KuraPublicationPreparationError::Storage)?;
        self.ensure_canonical_storage_not_poisoned()
            .map_err(KuraPublicationPreparationError::Storage)?;
        Ok(KuraPublicationLease {
            kura: self,
            _sidecar: sidecar,
            _geometry: geometry,
            _canonical: canonical,
            _prune: prune,
        })
    }
}

impl KuraPublicationLease<'_> {
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
}

#[cfg(test)]
#[path = "publication_lease_tests.rs"]
mod tests;
