//! Concrete production recovery owner over authenticated proofs and held durable descriptors.
//!
//! An opaque owner is constructed only after the real recursive verifier admits a production
//! release, hardware authenticates the complete current checkpoint, and all three native journals
//! revalidate their exact selected prefixes. No decoded snapshot or host journal grants authority.

use std::{path::Path, sync::Arc};

use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedGuardBundleVerifierV1, KagemushaAuthenticatedRecursiveVerifierV1,
    KagemushaHardwareTransactionVerifierV1,
};

/// Concrete machine type; caller-defined accepting verifiers cannot construct this owner.
type Machine = KagemushaStateMachineV1<
    Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    KagemushaAuthenticatedGuardBundleVerifierV1,
    KagemushaDiskAuthenticatedHistoryStoreV1,
>;

/// Authenticated recovered native owner retaining all descriptor locks for its lifetime.
/// This owner grants current recovery evidence; possession and monetary admission are separate.
pub struct KagemushaAuthenticatedCoreOwnerV1 {
    machine: Machine,
    journals: KagemushaPendingRecoveryJournalsV1,
    transactions: KagemushaHardwareTransactionJournalV1,
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Restore existing durable material without creating, truncating or resetting any file.
    /// Every original source is consumed on failure, keeping partial owners from escaping.
    ///
    /// # Errors
    /// Rejects absent monetary release authority, foreign hardware pins, stale checkpoints,
    /// malformed state, contradictory owner/history/journal bindings or unavailable storage.
    #[allow(clippy::too_many_arguments)]
    pub fn restore_existing(
        snapshot: KagemushaStateSnapshotV1,
        anchor: &DurabilityAnchorV1,
        expected_enrollment: &KagemushaRecoveryEnrollmentBindingV1,
        historical_release: &KagemushaAuthenticatedReleaseV1,
        history_credentials: KagemushaHistoryDeviceCredentialsV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        hardware_verifier: KagemushaHardwareTransactionVerifierV1,
        transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
        history_directory: &Path,
        coordinator_directory: &Path,
        response_directory: &Path,
        transaction_directory: &Path,
        maximum_reserved_bytes: u64,
        overlay_capacity_bytes: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let guard =
            KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(&recursive_verifier))
                .and_then(|guard| guard.with_hardware_transactions(hardware_verifier.clone()))
                .map_err(|error| KagemushaStateErrorV1::GuardRejected(error.to_string()))?;
        let release = guard
            .authenticated_release()
            .map_err(|error| KagemushaStateErrorV1::GuardRejected(error.to_string()))?;
        let proof_release = KagemushaStateProofReleaseV1::from_authenticated_release(&release)?;
        let floor_release =
            KagemushaStateProofReleaseV1::from_authenticated_release(historical_release)?;
        let journals = KagemushaPendingRecoveryJournalsV1::open_existing(
            coordinator_directory,
            response_directory,
            &snapshot.state.lane,
            snapshot.state.asset_incarnation,
            maximum_reserved_bytes,
        )?;
        let transactions = KagemushaHardwareTransactionJournalV1::open_existing(
            transaction_directory,
            hardware_verifier,
            transaction_transport,
        )
        .map_err(KagemushaStateErrorV1::RecoveryMaterial)?;
        let machine = Machine::restore_from_disk_history(
            snapshot,
            anchor,
            proof_release,
            floor_release,
            expected_enrollment,
            history_directory,
            history_credentials,
            overlay_capacity_bytes,
            recursive_verifier,
            guard,
        )?;
        journals.validate_pair(&machine)?;
        machine.current_recovery_selection()?;
        journals.validate_pair(&machine)?;
        transactions
            .recovery_prefix()
            .map_err(KagemushaStateErrorV1::RecoveryMaterial)?;
        Ok(Self {
            machine,
            journals,
            transactions,
        })
    }

    /// Reauthenticate the complete live checkpoint with fresh hardware entropy and recheck the
    /// held descriptors before and after that exchange. Retained signatures cannot renew it.
    ///
    /// # Errors
    /// Rejects state rollback, changed journal material, lost authority or unavailable hardware.
    pub fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        self.journals.validate_pair(&self.machine)?;
        self.transactions
            .recovery_prefix()
            .map_err(KagemushaStateErrorV1::RecoveryMaterial)?;
        let selection = self.machine.current_recovery_selection()?;
        self.journals.validate_pair(&self.machine)?;
        self.transactions
            .recovery_prefix()
            .map_err(KagemushaStateErrorV1::RecoveryMaterial)?;
        Ok(selection)
    }
}
