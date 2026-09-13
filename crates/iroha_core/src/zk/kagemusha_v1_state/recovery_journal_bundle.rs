//! Consuming recovery of the two descriptor-owned native journals as one untrusted bundle.

use super::{
    KagemushaAuthenticatedHistoryStoreV1, KagemushaCoordinatorOperationStoreV1,
    KagemushaCurrentRecoveryOwnerV1, KagemushaGuardBundleVerifierV1, KagemushaLaneIdV1,
    KagemushaRecursiveVerifierV1, KagemushaResponseEvidenceArchiveV1, KagemushaStateErrorV1,
    KagemushaStateMachineV1,
};
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use std::path::Path;

/// Held coordinator and response journals that confer no monetary or device authority.
///
/// Neither journal is exposed until both have been bound to the complete current Core
/// checkpoint. Opening and binding append no records and never return capacity by retiring
/// sender operations. The existing qualified guard remains responsible for hardware freshness
/// and the authentication of selected journal material and any speculative suffix.
pub struct KagemushaPendingRecoveryJournalsV1 {
    coordinator: KagemushaCoordinatorOperationStoreV1,
    responses: KagemushaResponseEvidenceArchiveV1,
}

impl KagemushaPendingRecoveryJournalsV1 {
    /// Reopen and fully replay both retained journals while keeping their locks private.
    ///
    /// Complete surviving frames are synced by the existing journal opener. No missing file
    /// is initialized and no torn suffix is truncated. The supplied owner is only a structural
    /// expectation; `bind` must compare it with an authenticated Core machine before exposure.
    ///
    /// # Errors
    /// Rejects unavailable, already owned, corrupt, torn or wrongly scoped journal material.
    pub fn open_existing(
        coordinator_path: &Path,
        response_path: &Path,
        lane: &KagemushaLaneIdV1,
        asset_incarnation: AxtAssetIncarnationV1,
        maximum_reserved_bytes: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        // Do not call the machine's public coordinator opener here: it may append sender
        // retirement before the response archive and the pair have been validated.
        let coordinator = KagemushaCoordinatorOperationStoreV1::open_existing(
            coordinator_path,
            lane.clone(),
            asset_incarnation,
            maximum_reserved_bytes,
        )
        .map_err(material_error)?;
        let responses = KagemushaResponseEvidenceArchiveV1::open_existing(
            response_path,
            lane,
            asset_incarnation,
        )
        .map_err(material_error)?;
        Ok(Self {
            coordinator,
            responses,
        })
    }

    /// Consume the pending pair and an already restored machine before exposing any owner.
    ///
    /// Both journals must contain their exact hardware-selected prefixes. A replayed suffix
    /// is permitted but remains untrusted evidence. The complete Core operation index must
    /// reconcile, and the existing current-recovery owner must match the complete snapshot
    /// and authenticate a fresh hardware selection. Both live descriptors are checked again
    /// after that exchange so cached prefix matches cannot hide an intervening file mutation.
    ///
    /// # Errors
    /// Rejects stale/uncheckpointed Core state, journal rollback, mixed histories, mismatched
    /// operations, unavailable authority or storage changes. All three owners are consumed
    /// on failure; neither journal is appended, reset, truncated or exposed.
    pub fn bind<R, G, H>(
        self,
        machine: KagemushaStateMachineV1<R, G, H>,
    ) -> Result<
        (
            KagemushaStateMachineV1<R, G, H>,
            KagemushaCoordinatorOperationStoreV1,
            KagemushaResponseEvidenceArchiveV1,
        ),
        KagemushaStateErrorV1,
    >
    where
        R: KagemushaRecursiveVerifierV1,
        G: KagemushaGuardBundleVerifierV1,
        H: KagemushaAuthenticatedHistoryStoreV1,
    {
        self.validate_pair(&machine)?;
        machine.current_recovery_selection()?;
        self.validate_pair(&machine)?;
        Ok((machine, self.coordinator, self.responses))
    }

    fn validate_pair<R, G, H>(
        &self,
        machine: &KagemushaStateMachineV1<R, G, H>,
    ) -> Result<(), KagemushaStateErrorV1>
    where
        R: KagemushaRecursiveVerifierV1,
        G: KagemushaGuardBundleVerifierV1,
        H: KagemushaAuthenticatedHistoryStoreV1,
    {
        machine
            .reconcile_coordinator_operations(&self.coordinator)
            .map_err(material_error)?;
        self.responses
            .validate_recovery_prefix(
                &machine.state.lane,
                machine.state.asset_incarnation,
                machine.recovery_metadata.journals.responses,
            )
            .map_err(material_error)
    }
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}
