//! Ordinary enrollment and zero-State proving selection from genuine issuer originals.
//!
//! The app's nonexportable approval key and the native financial witness are separate.
//! This selection creates no journal, current owner, monetary lease or hardware checkpoint.
//! Financial publication still requires the complete ordinary Guard in both recursive parities
//! and a descriptor-held native logical journal; an OEM response cannot substitute for either.

use super::*;
#[cfg(feature = "kagemusha-production-prover")]
pub(crate) use crate::kagemusha_v1_recursion::verify_ordinary_bootstrap_guard_v1;
use iroha_data_model::kagemusha::{
    KagemushaVerifiedOrdinaryAppCredentialV1,
    KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    KagemushaVerifiedPlayIntegrityRefreshLeaseV1, kagemusha_ordinary_financial_epoch_id_v1,
};

#[path = "authenticated_ordinary_current_publication.rs"]
mod current_publication;
pub use current_publication::KagemushaAuthenticatedOrdinaryCurrentPublicationV1;

#[path = "authenticated_ordinary_cash_owner.rs"]
mod cash_owner;
pub use cash_owner::KagemushaNativeOrdinaryCashOwnerV1;
pub(crate) use cash_owner::{
    KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1,
    KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1,
    KagemushaAuthenticatedOrdinaryReceivedCreditOpeningV1,
    KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1,
};

#[path = "authenticated_ordinary_logical_journal.rs"]
mod logical_journal;
pub(crate) use logical_journal::KagemushaAuthenticatedOrdinaryHistoricalApprovalV1;
pub use logical_journal::{
    KagemushaAuthenticatedOrdinaryApprovalV1,
    KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1,
    KagemushaOrdinaryLogicalApprovalJournalV1,
};

/// Borrowed actual ordinary credential floor, never decoded into an authenticated capability.
/// Its epoch is native logical metadata, without a hardware monotonicity claim.
pub struct KagemushaAuthenticatedOrdinaryCredentialFloorV1<'a> {
    enrollment: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    integrity_lease: Option<&'a KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
}

impl<'a> KagemushaAuthenticatedOrdinaryCredentialFloorV1<'a> {
    fn from_verified_enrollment(
        enrollment: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let floor = Self {
            enrollment,
            release,
            integrity_lease: None,
        };
        floor.recheck_at_trusted_time(enrollment.authenticated_at_ms())?;
        Ok(floor)
    }

    /// Recheck the same genuine issuer, possession and release bindings under native time.
    /// This argument cannot create a floor or renew an original certificate or policy interval.
    ///
    /// # Errors
    /// Rejects clock regression, expired originals or mixed scope/catalog/credential bindings.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), KagemushaStateErrorV1> {
        if let Some(lease) = self.integrity_lease {
            self.enrollment
                .recheck_with_integrity_lease(lease, now)
                .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        } else {
            self.enrollment
                .recheck_at_trusted_time(now)
                .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        }
        self.recheck_original_bindings()
    }

    fn recheck_original_admission(&self) -> Result<(), KagemushaStateErrorV1> {
        // Completed custody retains the original short possession ceremony. A later genuine
        // refresh lease is checked only at actual current Native time, never backdated here.
        let admitted = self.enrollment.authenticated_at_ms();
        self.enrollment
            .recheck_at_trusted_time(admitted)
            .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        self.enrollment
            .possession()
            .recheck_at_trusted_time(admitted)
            .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        self.recheck_original_bindings()
    }

    fn recheck_original_bindings(&self) -> Result<(), KagemushaStateErrorV1> {
        let retail = &self.enrollment.certificate().subject;
        let credential = self.enrollment.app_credential();
        let subject = credential.subject();
        let enabled = self
            .release
            .enabled_profile(subject.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if retail.issuance.release_id != self.release.release_id()
            || retail.issuance.hardware_policy_digest != self.release.hardware_policy_digest()
            || retail.ordinary_app_credential_digest != credential.digest()
            || retail.challenge_evidence_digest != self.enrollment.possession().evidence_digest()
            || retail.owner.runtime.network_id != self.release.network_id()
            || subject.release_id != self.release.release_id()
            || subject.network_id != *self.release.network_id().as_bytes()
            || subject.lane_id != retail.owner.lane_id
            || subject.suite_id != enabled.suite_id
            || subject.policy_epoch != enabled.policy_epoch
            || subject.platform_class != enabled.hardware_profile.platform_class
            || subject.financial_authority_commitment == [0; 32]
            || subject.app_key_reference == [0; 32]
            || subject.hardware_epoch == 0
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        Ok(())
    }

    pub(crate) fn from_verified_enrollment_with_integrity_lease(
        enrollment: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        integrity_lease: &'a KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        now: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let floor = Self {
            enrollment,
            release,
            integrity_lease: Some(integrity_lease),
        };
        floor.recheck_at_trusted_time(now)?;
        Ok(floor)
    }

    /// Actual independently verified ordinary credential original, without its private app key.
    pub fn credential(&self) -> &KagemushaVerifiedOrdinaryAppCredentialV1 {
        self.enrollment.app_credential()
    }

    /// Signed commitment to the separate native financial witness, never an app-key scalar.
    pub fn financial_authority_commitment(&self) -> DigestV1 {
        self.credential().subject().financial_authority_commitment
    }

    pub(crate) fn approval_valid_until_ms(&self) -> u64 {
        let subject = self.credential().subject();
        let integrity_expiry = self.integrity_lease.map_or_else(
            || {
                subject
                    .play_integrity
                    .map_or(subject.expires_at_ms, |pi| pi.refresh_before_ms)
            },
            |lease| {
                lease
                    .subject()
                    .expires_at_ms
                    .min(lease.subject().binding.refresh_before_ms)
            },
        );
        self.enrollment
            .certificate()
            .subject
            .expires_at_ms
            .min(subject.expires_at_ms)
            .min(integrity_expiry)
    }

    /// Original authenticated Apple enrollment counter floor, separate from financial indexes.
    pub fn app_attest_counter_floor(&self) -> Option<u32> {
        self.enrollment.possession().app_attest_counter()
    }

    /// Pure model-owned epoch identity and generation derived from the signed financial scope.
    /// This is native logical journal metadata, without hardware rollback resistance.
    pub fn financial_epoch(&self) -> Result<HardwareEpochV1, KagemushaStateErrorV1> {
        let subject = self.credential().subject();
        Ok(HardwareEpochV1 {
            generation: u128::from(subject.hardware_epoch),
            epoch_id: kagemusha_ordinary_financial_epoch_id_v1(subject)
                .map_err(|_| KagemushaStateErrorV1::InvalidHardwareProfile)?,
        })
    }

    fn validate_current(&self, state: &KagemushaStateV1) -> Result<(), KagemushaStateErrorV1> {
        let retail = &self.enrollment.certificate().subject;
        let subject = self.credential().subject();
        let epoch = self.financial_epoch()?;
        if state.release_id != self.release.release_id()
            || state.lane.network_id != retail.owner.runtime.network_id
            || state.lane.device_lane_id != retail.owner.lane_id
            || state.lane.asset != retail.owner.runtime.asset
            || state.asset_incarnation != retail.owner.runtime.asset_incarnation
            || state.lane.scale != retail.owner.runtime.scale
            || state.hardware_profile_id != subject.hardware_profile_id
            || state.policy_epoch != subject.policy_epoch
            || state.suite_id != subject.suite_id
            || state.hardware_epoch != epoch
            || state.device_policy_binding.device_key_reference != subject.app_key_reference
            || state.device_policy_binding.hardware_policy_id != self.release.provider_policy_root()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn checkpoint_floor(
        &self,
    ) -> Result<KagemushaAcceptedCredentialFloorV1, KagemushaStateErrorV1> {
        self.recheck_original_admission()?;
        Ok(KagemushaAcceptedCredentialFloorV1::OrdinaryApp {
            credential: iroha_data_model::kagemusha::KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
                self.credential().original(),
            )
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?,
            release_id: self.release.release_id(),
        })
    }
}

/// Borrowed zero-State proof selection from actual ordinary retail and app-key admission.
/// No decoder, clone or raw-owner constructor can manufacture this selection.
pub struct KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'a> {
    floor: KagemushaAuthenticatedOrdinaryCredentialFloorV1<'a>,
    recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    capacity: KagemushaDurableCapacityV1,
    preview: BootstrapPreviewV1,
    proof_release: KagemushaStateProofReleaseV1,
}

impl<'a> KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'a> {
    /// Derive the exact initial preview from the actual current signed enrollment originals.
    /// Native witness custody must independently open the signed financial commitment and retain
    /// the original approval attempt. Neither this constructor nor a generated State grants money.
    ///
    /// # Errors
    /// Rejects mixed release/owner/credential originals, expired possession, or invalid zero state.
    pub fn from_verified_enrollment(
        enrollment: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        state_nonce_commitment: DigestV1,
        capacity: KagemushaDurableCapacityV1,
        trusted_native_now_ms: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        if trusted_native_now_ms < enrollment.authenticated_at_ms() {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        enrollment
            .possession()
            .recheck_at_trusted_time(enrollment.authenticated_at_ms())
            .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        let release = admitted_release(&recursive_verifier)?;
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            enrollment, release,
        )?;
        let (proof_release, preview) = derive_preview(&floor, state_nonce_commitment, capacity)?;
        let selection = Self {
            floor,
            recursive_verifier,
            capacity,
            preview,
            proof_release,
        };
        selection.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(selection)
    }

    /// Select a genuinely admitted current Integrity refresh without replacing the original
    /// enrollment or zero-State preview. Original possession is checked at its actual admission;
    /// the unchanged credential and selected current lease are checked at current Native time.
    /// # Errors
    /// Rejects foreign scope, expired originals/current lease or a replaced production verifier.
    pub fn from_verified_enrollment_with_current_integrity_lease(
        enrollment: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        state_nonce_commitment: DigestV1,
        capacity: KagemushaDurableCapacityV1,
        integrity_lease: &'a KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        trusted_native_now_ms: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        enrollment
            .possession()
            .recheck_at_trusted_time(enrollment.authenticated_at_ms())
            .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        let release = admitted_release(&recursive_verifier)?;
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment_with_integrity_lease(
            enrollment, release, integrity_lease, trusted_native_now_ms,
        )?;
        floor.recheck_original_admission()?;
        let (proof_release, preview) = derive_preview(&floor, state_nonce_commitment, capacity)?;
        let selection = Self {
            floor,
            recursive_verifier,
            capacity,
            preview,
            proof_release,
        };
        selection.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(selection)
    }

    /// Recompute the complete original preview and production release identity under native time.
    /// No original interval, capacity, nonce or financial commitment is replaced on retry.
    ///
    /// # Errors
    /// Rejects original expiry or any release/preview substitution.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), KagemushaStateErrorV1> {
        self.floor.recheck_at_trusted_time(now)?;
        self.floor
            .enrollment
            .possession()
            .recheck_at_trusted_time(self.floor.enrollment.authenticated_at_ms())
            .map_err(|_| KagemushaStateErrorV1::SnapshotRollback)?;
        let release = admitted_release(&self.recursive_verifier)?;
        if release.release_id() != self.floor.release.release_id()
            || release.attestation_digest() != self.floor.release.attestation_digest()
            || release.authority_policy_digest() != self.floor.release.authority_policy_digest()
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let (proof_release, preview) = derive_preview(
            &self.floor,
            self.preview.statement.state_nonce_commitment,
            self.capacity,
        )?;
        if preview != self.preview || proof_release.artifacts != self.proof_release.artifacts {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn recheck_original_selection(&self) -> Result<(), KagemushaStateErrorV1> {
        self.floor.recheck_original_admission()?;
        let release = admitted_release(&self.recursive_verifier)?;
        if release.release_id() != self.floor.release.release_id()
            || release.attestation_digest() != self.floor.release.attestation_digest()
            || release.authority_policy_digest() != self.floor.release.authority_policy_digest()
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let (proof_release, preview) = derive_preview(
            &self.floor,
            self.preview.statement.state_nonce_commitment,
            self.capacity,
        )?;
        if preview != self.preview || proof_release.artifacts != self.proof_release.artifacts {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    /// Complete genuine initial semantic instance; proving does not initialize its storage.
    pub fn preview(&self) -> Result<&BootstrapPreviewV1, KagemushaStateErrorV1> {
        self.recheck_original_selection()?;
        Ok(&self.preview)
    }
    /// Exact verified issuer/app credential original retained throughout proving.
    pub fn enrollment(&self) -> &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
        self.floor.enrollment
    }
    /// Actual ordinary credential floor, without any app-key private scalar.
    pub fn credential_floor(&self) -> &KagemushaAuthenticatedOrdinaryCredentialFloorV1<'a> {
        &self.floor
    }
    /// Exact selected native capacity; this projection grants no allocation or spending lease.
    pub fn capacity(&self) -> KagemushaDurableCapacityV1 {
        self.capacity
    }
    /// Actual threshold-authenticated production release retained by the native verifier.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_original_selection()?;
        Ok(Arc::clone(&self.floor.release))
    }

    pub(crate) fn recursive_verifier(&self) -> &KagemushaAuthenticatedRecursiveVerifierV1 {
        &self.recursive_verifier
    }

    /// Independently verify the real paired zero-State against the complete selected preview.
    /// The ordinary platform Guard and actual logical checkpoint remain mandatory afterward.
    /// Copy the sole complete public original from this exact zero selection after both real
    /// State proofs and their whole histories verify. This data grants no Anchor or money.
    pub(crate) fn lineage_public_state_original(
        &self,
        proof: &KagemushaPairedProofV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.verify_state_proof(proof)?;
        let inputs =
            bootstrap_state_public_inputs(self.proof_release.artifacts, &self.preview, proof)?;
        let original = crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageStateOriginalV1::from_bootstrap_public_inputs(&inputs, proof)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
            .canonical_bytes()
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.recheck_original_selection()?;
        Ok(original)
    }

    pub(crate) fn verify_state_proof(
        &self,
        proof: &KagemushaPairedProofV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_original_selection()?;
        let inputs =
            bootstrap_state_public_inputs(self.proof_release.artifacts, &self.preview, proof)?;
        verify_kagemusha_state_proof_v1(
            self.recursive_verifier.as_ref(),
            self.proof_release.artifacts,
            &inputs,
            proof,
        )
        .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        self.recheck_original_selection()
    }
}

fn admitted_release(
    verifier: &Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
    KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(verifier))
        .and_then(|guard| guard.authenticated_release())
        .map_err(|error| KagemushaStateErrorV1::GuardRejected(error.to_string()))
}

fn derive_preview(
    floor: &KagemushaAuthenticatedOrdinaryCredentialFloorV1<'_>,
    nonce: DigestV1,
    capacity: KagemushaDurableCapacityV1,
) -> Result<(KagemushaStateProofReleaseV1, BootstrapPreviewV1), KagemushaStateErrorV1> {
    capacity.validate()?;
    let retail = &floor.enrollment.certificate().subject;
    let subject = floor.credential().subject();
    let enabled = floor
        .release
        .enabled_profile(subject.hardware_profile_id)
        .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
    let proof_release =
        KagemushaStateProofReleaseV1::from_authenticated_ordinary_release(&floor.release)?;
    let context = KagemushaStateContextV1 {
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id: enabled.suite_id,
        vk_digest: enabled.vk_digest,
        release_id: floor.release.release_id(),
        asset_incarnation: retail.owner.runtime.asset_incarnation,
        hardware_profile_id: enabled.hardware_profile_id,
        policy_epoch: enabled.policy_epoch,
    };
    let lane = KagemushaLaneIdV1 {
        network_id: retail.owner.runtime.network_id,
        device_lane_id: retail.owner.lane_id,
        asset: retail.owner.runtime.asset.clone(),
        scale: retail.owner.runtime.scale,
    };
    let binding = DevicePolicyBindingV1 {
        device_key_reference: subject.app_key_reference,
        hardware_policy_id: floor.release.provider_policy_root(),
    };
    let preview = Machine::preview_bootstrap(
        proof_release.clone(),
        context,
        lane,
        floor.financial_epoch()?,
        binding,
        [0; 32],
        nonce,
        floor.enrollment.authenticated_at_ms(),
    )?;
    floor.validate_current(&preview.state)?;
    Ok((proof_release, preview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

    fn capacity() -> KagemushaDurableCapacityV1 {
        KagemushaDurableCapacityV1 {
            inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        }
    }

    #[test]
    fn genuine_ordinary_floor_retains_separate_financial_and_app_key_roles() {
        for apple in [false, true] {
            let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
            let enrollment = fixture.verify(300).unwrap();
            let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
                &enrollment,
                Arc::clone(&fixture.release),
            )
            .unwrap();
            let credential = floor.credential();
            assert_eq!(floor.financial_authority_commitment(), [19; 32]);
            assert_ne!(
                floor.financial_authority_commitment(),
                credential.subject().attested_key_id
            );
            assert_ne!(
                floor.financial_authority_commitment(),
                credential.subject().app_key_reference
            );
            assert_eq!(
                floor.app_attest_counter_floor(),
                if apple { Some(11) } else { None }
            );
            assert_eq!(floor.financial_epoch().unwrap().generation, 1);
            assert_eq!(
                credential.original(),
                enrollment.app_credential().original()
            );
        }
    }

    #[test]
    fn ordinary_preview_has_zero_financial_indexes_without_one_use_hardware_claim() {
        for apple in [false, true] {
            let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
            let enrollment = fixture.verify(300).unwrap();
            let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
                &enrollment,
                Arc::clone(&fixture.release),
            )
            .unwrap();
            // Pure semantic preview only: no verifier, journal, CAS or software wallet is created.
            let (_, preview) = derive_preview(&floor, [41; 32], capacity()).unwrap();
            floor.validate_current(&preview.state).unwrap();
            assert_eq!(preview.state.balance, 0);
            assert_eq!(preview.state.logical_sequence, 0);
            assert_eq!(preview.state.secure_index, 0);
            assert_eq!(preview.state.next_one_use_key_reference, [0; 32]);
            assert_eq!(
                preview.state.device_policy_binding.hardware_policy_id,
                fixture.release.provider_policy_root()
            );
            assert_ne!(
                fixture.release.provider_policy_root(),
                fixture.release.hardware_policy_digest()
            );
        }
    }

    #[test]
    fn ordinary_floor_rejects_foreign_current_owner_key_release_and_logical_epoch() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(true);
        let enrollment = fixture.verify(300).unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            &enrollment,
            Arc::clone(&fixture.release),
        )
        .unwrap();
        let (_, preview) = derive_preview(&floor, [41; 32], capacity()).unwrap();
        for field in 0..9 {
            let mut state = preview.state.clone();
            match field {
                0 => state.release_id[0] ^= 1,
                1 => state.lane.device_lane_id[0] ^= 1,
                2 => state.lane.scale += 1,
                3 => state.hardware_profile_id[0] ^= 1,
                4 => state.policy_epoch += 1,
                5 => state.hardware_epoch.generation += 1,
                6 => state.hardware_epoch.epoch_id[0] ^= 1,
                7 => state.device_policy_binding.device_key_reference[0] ^= 1,
                _ => {
                    state.device_policy_binding.hardware_policy_id =
                        fixture.release.hardware_policy_digest()
                }
            }
            assert!(
                floor.validate_current(&state).is_err(),
                "foreign state field {field} admitted"
            );
        }
    }

    #[test]
    fn ordinary_floor_rejects_time_regression_and_original_certificate_expiry() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(true);
        let enrollment = fixture.verify(300).unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            &enrollment,
            Arc::clone(&fixture.release),
        )
        .unwrap();
        floor.recheck_at_trusted_time(301).unwrap();
        assert!(floor.recheck_at_trusted_time(299).is_err());
        assert!(
            floor
                .recheck_at_trusted_time(enrollment.certificate().subject.expires_at_ms)
                .is_err()
        );
        // A reusable enrollment floor does not renew the original bootstrap possession attempt.
        let deadline = enrollment.possession().challenge().expires_at_ms;
        floor.recheck_at_trusted_time(deadline).unwrap();
        assert!(
            enrollment
                .possession()
                .recheck_at_trusted_time(deadline)
                .is_err()
        );
    }

    #[test]
    fn ordinary_preview_rejects_missing_financial_nonce_and_unfunded_capacity() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let enrollment = fixture.verify(300).unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            &enrollment,
            Arc::clone(&fixture.release),
        )
        .unwrap();
        assert!(derive_preview(&floor, [0; 32], capacity()).is_err());
        let mut absent = capacity();
        absent.inbox_bytes = 0;
        assert!(derive_preview(&floor, [41; 32], absent).is_err());
        absent = capacity();
        absent.outbox_bytes = 0;
        assert!(derive_preview(&floor, [41; 32], absent).is_err());
    }

    #[test]
    fn completed_original_floor_does_not_backdate_a_current_integrity_lease() {
        use iroha_crypto::{Algorithm, KeyPair};
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let enrollment = fixture.verify(300).unwrap();
        let (challenge, raw_lease) = fixture.integrity_refresh_originals();
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let lease = raw_lease
            .authenticate(
                enrollment.app_credential(),
                &fixture.release,
                &fixture.trust,
                &fixture.app_authority,
                &challenge,
                issuer.public_key(),
                1500,
            )
            .unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment_with_integrity_lease(
            &enrollment, Arc::clone(&fixture.release), &lease, 2100).unwrap();
        // The original two-minute possession is over. This projection retains its actual
        // original admission and performs separate genuine current lease admission at 2100.
        assert!(
            enrollment
                .possession()
                .recheck_at_trusted_time(2100)
                .is_err()
        );
        assert!(floor.recheck_at_trusted_time(300).is_err());
        let checkpoint = floor.checkpoint_floor().unwrap();
        assert_eq!(
            checkpoint
                .ordinary_original()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
            enrollment.app_credential().original()
        );
        let (_, preview) = derive_preview(&floor, [41; 32], capacity()).unwrap();
        floor.validate_current(&preview.state).unwrap();
        assert!(floor.recheck_at_trusted_time(2400).is_err());
    }

    #[test]
    fn closed_checkpoint_floor_retains_exact_ordinary_original_without_oem_conversion() {
        for apple in [false, true] {
            let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
            let enrollment = fixture.verify(300).unwrap();
            let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
                &enrollment,
                Arc::clone(&fixture.release),
            )
            .unwrap();
            let original = floor.checkpoint_floor().unwrap();
            assert!(original.oem_original().is_err());
            assert_eq!(
                original
                    .ordinary_original()
                    .map(|credential| credential.canonical_bytes().unwrap()),
                Some(enrollment.app_credential().original().to_vec())
            );
            assert_eq!(
                original.original_digest().unwrap(),
                enrollment.app_credential().digest()
            );
            let bytes = norito::encode_canonical(&original).unwrap();
            let restored: KagemushaAcceptedCredentialFloorV1 =
                norito::decode_canonical(&bytes).unwrap();
            assert_eq!(restored, original);
            assert!(restored.oem_original().is_err());
            let (release, preview) = derive_preview(&floor, [41; 32], capacity()).unwrap();
            restored.validate_current(&preview.state, &release).unwrap();
            let mut substituted = preview.state;
            substituted.next_one_use_key_reference = [99; 32];
            assert!(restored.validate_current(&substituted, &release).is_err());
        }
    }
}
