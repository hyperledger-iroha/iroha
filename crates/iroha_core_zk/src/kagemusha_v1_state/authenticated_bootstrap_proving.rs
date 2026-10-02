//! Sealed zero-State proving admission from verified enrollment originals.
//!
//! This selection retains both opaque issuer/possession authentications and the real
//! production recursive verifier. It opens no files, initializes no Core, dispatches no
//! device command and grants no lease. Qualified native witness custody and a fresh
//! trusted-time recheck remain independently mandatory before and after proving.

use super::*;
use iroha_data_model::kagemusha::{
    KagemushaVerifiedRetailEnrollmentCertificateV1, KagemushaVerifiedRetailEnrollmentPossessionV1,
};

/// Borrowed original zero-State proving selection, never decodable or caller assembled.
/// A proof returned against it must still pass the actual bootstrap Guard and INITIAL CAS.
pub struct KagemushaAuthenticatedBootstrapProvingSelectionV1<'a> {
    enrollment: &'a KagemushaVerifiedRetailEnrollmentCertificateV1,
    possession: &'a KagemushaVerifiedRetailEnrollmentPossessionV1,
    recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    capacity: KagemushaDurableCapacityV1,
    preview: BootstrapPreviewV1,
    proof_release: KagemushaStateProofReleaseV1,
}

impl<'a> KagemushaAuthenticatedBootstrapProvingSelectionV1<'a> {
    /// Select the genuine initial preview from the same current verified enrollment pair.
    /// No decoded certificate, generic accepting verifier or raw zero-state overload exists.
    /// The exact native trusted instant must be the instant retained by both originals.
    ///
    /// # Errors
    /// Rejects time/owner/issuance/release/profile/floor mismatches and invalid zero-State input.
    pub fn from_verified_enrollment(
        enrollment: &'a KagemushaVerifiedRetailEnrollmentCertificateV1,
        possession: &'a KagemushaVerifiedRetailEnrollmentPossessionV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        state_nonce_commitment: DigestV1,
        capacity: KagemushaDurableCapacityV1,
        trusted_native_now_ms: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        if trusted_native_now_ms != enrollment.authenticated_at_ms()
            || trusted_native_now_ms != possession.verified_at_ms()
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let guard =
            KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(&recursive_verifier))
                .map_err(|e| KagemushaStateErrorV1::GuardRejected(e.to_string()))?;
        let release = guard
            .authenticated_release()
            .map_err(|e| KagemushaStateErrorV1::GuardRejected(e.to_string()))?;
        let (proof_release, preview) = derive_preview(
            enrollment,
            possession,
            &release,
            state_nonce_commitment,
            capacity,
        )?;
        let selection = Self {
            enrollment,
            possession,
            recursive_verifier,
            release,
            capacity,
            preview,
            proof_release,
        };
        selection.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(selection)
    }

    /// Revalidate the actual immutable originals and recompute the complete initial preview.
    /// This authenticates the originally selected instant; the native witness source must
    /// additionally call `recheck_at_trusted_time` with its current authoritative clock.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        let guard =
            KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(&self.recursive_verifier))
                .map_err(|e| KagemushaStateErrorV1::GuardRejected(e.to_string()))?;
        let release = guard
            .authenticated_release()
            .map_err(|e| KagemushaStateErrorV1::GuardRejected(e.to_string()))?;
        if release.release_id() != self.release.release_id()
            || release.attestation_digest() != self.release.attestation_digest()
            || release.authority_policy_digest() != self.release.authority_policy_digest()
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let (proof_release, preview) = derive_preview(
            self.enrollment,
            self.possession,
            &release,
            self.preview.statement.state_nonce_commitment,
            self.capacity,
        )?;
        if preview != self.preview || proof_release.artifacts != self.proof_release.artifacts {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    /// Reject clock regression and expired originals under the independently supplied native clock.
    /// The clock argument alone is metadata; it cannot manufacture this opaque selection.
    pub fn recheck_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let subject = &self.enrollment.certificate().subject;
        let challenge = self.possession.challenge();
        require_current_time(
            trusted_native_now_ms,
            self.enrollment.authenticated_at_ms(),
            subject.expires_at_ms,
            challenge.expires_at_ms,
        )
    }

    /// Complete initial semantic instance derived from the actual verified originals.
    pub fn preview(&self) -> Result<&BootstrapPreviewV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.preview)
    }
    /// Exact immutable production release retained by the native verifier.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(Arc::clone(&self.release))
    }
    /// Original verified enrollment, retained only by reference for native witness correlation.
    pub fn enrollment(&self) -> &KagemushaVerifiedRetailEnrollmentCertificateV1 {
        self.enrollment
    }
    /// Original verified account/device possession, retained only by reference.
    pub fn possession(&self) -> &KagemushaVerifiedRetailEnrollmentPossessionV1 {
        self.possession
    }
    /// Exact selected capacity; proving does not allocate or initialize its storage.
    pub fn capacity(&self) -> KagemushaDurableCapacityV1 {
        self.capacity
    }

    /// Independently verify the genuine generated paired State against this exact initial preview.
    /// Hardware registration Guard and INITIAL checkpoint authority are still required afterward.
    #[cfg(feature = "kagemusha-production-prover")]
    pub(crate) fn verify_state_proof(
        &self,
        proof: &KagemushaPairedProofV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let inputs =
            bootstrap_state_public_inputs(self.proof_release.artifacts, &self.preview, proof)?;
        verify_kagemusha_state_proof_v1(
            self.recursive_verifier.as_ref(),
            self.proof_release.artifacts,
            &inputs,
            proof,
        )
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
        self.recheck()
    }
}

fn derive_preview(
    enrollment: &KagemushaVerifiedRetailEnrollmentCertificateV1,
    possession: &KagemushaVerifiedRetailEnrollmentPossessionV1,
    release: &KagemushaAuthenticatedReleaseV1,
    state_nonce_commitment: DigestV1,
    capacity: KagemushaDurableCapacityV1,
) -> Result<(KagemushaStateProofReleaseV1, BootstrapPreviewV1), KagemushaStateErrorV1> {
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
    let proof_release = KagemushaStateProofReleaseV1::from_authenticated_release(release)?;
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
    capacity.validate()?;
    // Retain the current genuine stage algorithm: a nonzero KeyMint ratchet requires a
    // separate complete paired attestation relation and is not synthesized here.
    let preview = Machine::preview_bootstrap(
        proof_release.clone(),
        context,
        lane,
        epoch,
        binding,
        [0; 32],
        state_nonce_commitment,
        enrollment.authenticated_at_ms(),
    )?;
    KagemushaAcceptedCredentialFloorV1::Oem {
        credential,
        release_id: release.release_id(),
    }
    .validate_current(&preview.state, &proof_release)?;
    Ok((proof_release, preview))
}

fn require_current_time(
    now: u64,
    admitted: u64,
    certificate_expiry: u64,
    challenge_expiry: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if admitted == 0 || now < admitted || now >= certificate_expiry || now >= challenge_expiry {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_time_requires_live_both_originals_and_no_regression() {
        for now in [0, 999, 1200, 1300, u64::MAX] {
            assert!(require_current_time(now, 1000, 1200, 1300).is_err());
        }
        for now in [1000, 1001, 1199] {
            require_current_time(now, 1000, 1200, 1300).unwrap();
        }
        assert!(require_current_time(1000, 1000, 1300, 1000).is_err());
        assert!(require_current_time(1, 0, 1300, 1400).is_err());
    }
    #[test]
    fn each_original_expiry_is_strict_independently() {
        require_current_time(1999, 1000, 2000, 3000).unwrap();
        assert!(require_current_time(2000, 1000, 2000, 3000).is_err());
        require_current_time(1999, 1000, 3000, 2000).unwrap();
        assert!(require_current_time(2000, 1000, 3000, 2000).is_err());
    }
}
