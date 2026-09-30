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

/// Concrete verified bootstrap stage. Its constructor requires real production proof authority.
/// No accepting application-defined verifier or decoded enrollment can create this owner.
pub type KagemushaAuthenticatedBootstrapStageV1 = KagemushaBootstrapJournalStageV1<
    Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    KagemushaAuthenticatedGuardBundleVerifierV1,
    KagemushaDiskAuthenticatedHistoryStoreV1,
>;

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Stage a fresh zero-balance lane from current independently verified enrollment and
    /// account/device possession, then authenticate both actual paired proof relations.
    ///
    /// Lane, asset incarnation, hardware epoch, profile, credential and owner come from those
    /// opaque originals. The proof binds the same trusted enrollment instant and provider root.
    /// This stage cannot spend or expose a machine: no-replace journal initialization and the
    /// original hardware INITIAL checkpoint CAS plus fresh selection must still finish.
    ///
    /// The service owner must independently enforce current ledger asset/release authority and
    /// consume its retained enrollment challenge before invoking this native stage. This method
    /// grants neither a ledger state witness nor a substitute for that service-owned CAS.
    ///
    /// # Errors
    /// Rejects mixed enrollment/proof originals, stale times, unqualified releases, invalid
    /// paired proofs or hardware guards, already existing storage and unavailable journals.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_enrolled_bootstrap(
        enrollment: iroha_data_model::kagemusha::KagemushaVerifiedRetailEnrollmentCertificateV1,
        possession: iroha_data_model::kagemusha::KagemushaVerifiedRetailEnrollmentPossessionV1,
        state_nonce_commitment: DigestV1,
        durable_capacity: KagemushaDurableCapacityV1,
        authorization: BootstrapAuthorizationV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        hardware_verifier: KagemushaHardwareTransactionVerifierV1,
        history_directory: &Path,
        overlay_capacity_bytes: u64,
    ) -> Result<KagemushaAuthenticatedBootstrapStageV1, KagemushaStateErrorV1> {
        let guard =
            KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(&recursive_verifier))
                .and_then(|guard| guard.with_hardware_transactions(hardware_verifier))
                .map_err(|error| KagemushaStateErrorV1::GuardRejected(error.to_string()))?;
        let release = guard
            .authenticated_release()
            .map_err(|error| KagemushaStateErrorV1::GuardRejected(error.to_string()))?;
        let subject = &enrollment.certificate().subject;
        require_exact_enrollment_possession(
            subject,
            possession.challenge(),
            possession.evidence_digest(),
            enrollment.authenticated_at_ms(),
            possession.verified_at_ms(),
        )?;
        if subject.issuance.release_id != release.release_id()
            || subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let credential = subject.issuance.credential;
        let enabled = release
            .enabled_profile(credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        let proof_release = KagemushaStateProofReleaseV1::from_authenticated_release(&release)?;
        let context = KagemushaStateContextV1 {
            protocol_version: KAGEMUSHA_STATE_VERSION_V1,
            suite_id: enabled.suite_id,
            vk_digest: enabled.vk_digest,
            release_id: release.release_id(),
            asset_incarnation: subject.owner.runtime.asset_incarnation,
            hardware_profile_id: enabled.hardware_profile_id,
            policy_epoch: enabled.policy_epoch,
        };
        let lane = KagemushaLaneIdV1 {
            network_id: subject.owner.runtime.network_id,
            device_lane_id: subject.owner.lane_id,
            asset: subject.owner.runtime.asset.clone(),
            scale: subject.owner.runtime.scale,
        };
        let epoch = HardwareEpochV1 {
            generation: u128::from(credential.hardware_epoch_generation),
            epoch_id: credential.hardware_epoch_id,
        };
        let binding = DevicePolicyBindingV1 {
            device_key_reference: credential.device_key_reference,
            hardware_policy_id: release.provider_policy_root(),
        };
        durable_capacity.validate()?;
        let preview = Machine::preview_bootstrap(
            proof_release.clone(),
            context,
            lane.clone(),
            epoch,
            binding,
            // Qualified counter/epoch devices. A nonzero KeyMint ratchet needs its own
            // attestation relation recursively constrained in both Pasta folds.
            [0; 32],
            state_nonce_commitment,
            enrollment.authenticated_at_ms(),
        )?;
        KagemushaAcceptedCredentialFloorV1 {
            credential,
            release_id: release.release_id(),
        }
        .validate_current(&preview.state, &proof_release)?;
        // Authenticate before creating a history directory: invalid proofs cannot leave an
        // initialized but unusable path that prevents an exact authorized retry.
        authenticate_bootstrap_authorization(
            &proof_release,
            &preview,
            &authorization,
            &recursive_verifier,
            &guard,
        )?;
        let history_credentials = KagemushaHistoryDeviceCredentialsV1::authenticate(
            &release,
            &lane,
            enabled.hardware_profile_id,
            [credential],
        )
        .map_err(map_authenticated_history_error)?;
        let history = KagemushaDiskAuthenticatedHistoryStoreV1::create_new(
            history_directory,
            disk_history_lane_binding(context, &lane)?,
            history_credentials,
            overlay_capacity_bytes,
        )
        .map_err(map_authenticated_history_error)?;
        let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(history)
            .map_err(map_authenticated_history_error)?;
        if authenticated_history.committed_roots() != KagemushaHistoryRootsV1::empty() {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        KagemushaBootstrapJournalStageV1::new(
            preview.state,
            proof_release,
            credential,
            KagemushaRecoveryEnrollmentBindingV1 {
                enrollment_id: subject.enrollment_id,
                owner: subject.owner.clone(),
            },
            durable_capacity,
            authenticated_history,
            recursive_verifier,
            guard,
        )
    }

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

// Only the constructor above calls this comparison after both opaque model authentications.
// Kept separate so substitution failures can be tested without manufacturing capabilities.
fn require_exact_enrollment_possession(
    subject: &iroha_data_model::kagemusha::KagemushaRetailEnrollmentSubjectV1,
    challenge: &iroha_data_model::kagemusha::KagemushaRetailEnrollmentChallengeV1,
    evidence_digest: DigestV1,
    certificate_time_ms: u64,
    possession_time_ms: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if certificate_time_ms == 0
        || certificate_time_ms != possession_time_ms
        || certificate_time_ms < subject.issued_at_ms
        || certificate_time_ms >= subject.expires_at_ms
        || certificate_time_ms < challenge.issued_at_ms
        || certificate_time_ms >= challenge.expires_at_ms
        || subject.owner != challenge.owner
        || subject.issuance != challenge.issuance
        || subject.issuer_policy_id != challenge.issuer_policy_id
        || subject.issuer_audience != challenge.issuer_audience
        || subject.app_attestation_digest != challenge.app_attestation_digest
        || subject.challenge_evidence_digest != evidence_digest
        || evidence_digest == [0; 32]
        || subject.enrollment_id
            != subject
                .owner
                .enrollment_id()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
    {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KagemushaRetailEnrollmentChallengeV1, KagemushaRetailEnrollmentIssuanceV1,
        KagemushaRetailEnrollmentSubjectV1,
    };

    // Structural comparison fixture only: no opaque enrollment, proof authority or production
    // owner is manufactured. The public constructor separately requires both real authentications.
    fn fixture() -> (
        KagemushaRetailEnrollmentSubjectV1,
        KagemushaRetailEnrollmentChallengeV1,
    ) {
        let machine = super::super::tests::coordinator_operation_store_tests::machine().0;
        let owner = machine.enrollment_binding().owner.clone();
        let issuance = KagemushaRetailEnrollmentIssuanceV1 {
            release_id: machine.proof_release.release_id(),
            hardware_policy_digest: [0x91; 32],
            core_authorization_key_reference: [0x92; 32],
            credential: machine.accepted_credential_floor().credential,
        };
        let subject = KagemushaRetailEnrollmentSubjectV1 {
            version: 1,
            enrollment_id: owner.enrollment_id().unwrap(),
            issuer_policy_id: [0x93; 32],
            issuer_audience: "native-bootstrap".parse().unwrap(),
            owner: owner.clone(),
            issuance: issuance.clone(),
            challenge_evidence_digest: [0x94; 32],
            app_attestation_digest: [0x95; 32],
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };
        let challenge = KagemushaRetailEnrollmentChallengeV1 {
            version: 1,
            client_nonce: [0x96; 32],
            server_nonce: [0x97; 32],
            issuer_policy_id: subject.issuer_policy_id,
            issuer_audience: subject.issuer_audience.clone(),
            owner,
            issuance,
            app_attestation_digest: subject.app_attestation_digest,
            issued_at_ms: 900,
            expires_at_ms: 1500,
        };
        (subject, challenge)
    }

    #[test]
    fn bootstrap_compares_complete_enrollment_and_original_possession() {
        let (subject, challenge) = fixture();
        assert!(
            require_exact_enrollment_possession(&subject, &challenge, [0x94; 32], 1100, 1100)
                .is_ok()
        );
        let mut different = challenge.clone();
        different.owner.runtime.fi_id = "other-fi".parse().unwrap();
        assert!(
            require_exact_enrollment_possession(&subject, &different, [0x94; 32], 1100, 1100)
                .is_err()
        );
        different = challenge.clone();
        different.issuance.credential.hardware_epoch_generation += 1;
        assert!(
            require_exact_enrollment_possession(&subject, &different, [0x94; 32], 1100, 1100)
                .is_err()
        );
        different = challenge;
        different.issuance.core_authorization_key_reference[0] ^= 1;
        assert!(
            require_exact_enrollment_possession(&subject, &different, [0x94; 32], 1100, 1100)
                .is_err()
        );
    }

    #[test]
    fn bootstrap_refuses_substituted_issuer_app_and_proof_commitments() {
        let (subject, challenge) = fixture();
        for mutation in 0..3 {
            let mut changed = challenge.clone();
            match mutation {
                0 => changed.issuer_policy_id[0] ^= 1,
                1 => changed.issuer_audience = "other-issuer".parse().unwrap(),
                _ => changed.app_attestation_digest[0] ^= 1,
            }
            assert!(
                require_exact_enrollment_possession(&subject, &changed, [0x94; 32], 1100, 1100)
                    .is_err()
            );
        }
        assert!(
            require_exact_enrollment_possession(&subject, &challenge, [0xA4; 32], 1100, 1100)
                .is_err()
        );
        assert!(
            require_exact_enrollment_possession(&subject, &challenge, [0; 32], 1100, 1100).is_err()
        );
    }

    #[test]
    fn bootstrap_requires_same_current_verification_instant() {
        let (subject, challenge) = fixture();
        for (certificate_time, possession_time) in
            [(0, 0), (1100, 1101), (999, 999), (1500, 1500), (2000, 2000)]
        {
            assert!(
                require_exact_enrollment_possession(
                    &subject,
                    &challenge,
                    [0x94; 32],
                    certificate_time,
                    possession_time
                )
                .is_err()
            );
        }
    }

    #[test]
    fn bootstrap_rejects_relabelled_stable_owner_identity() {
        let (mut subject, challenge) = fixture();
        subject.enrollment_id[0] ^= 1;
        assert!(
            require_exact_enrollment_possession(&subject, &challenge, [0x94; 32], 1100, 1100)
                .is_err()
        );
    }
}
