//! Exclusive ordinary bootstrap orchestration over actual Native financial custody.
//!
//! The owner holds its journal and financial witness by value. Platform evidence is untrusted
//! intake; the genuine issuer, Guard and State relations remain separate mandatory checks.

use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaArtifactByteResolverV1, KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
    KagemushaProductionProverV1,
};
use std::path::PathBuf;

/// Actual Native staging owner before the first paired State publication.
/// It exposes no financial secret, key constructor, caller clock or accepting verifier callback.
/// Publishing consumes both exclusive holders into the actual durable current publication.
pub struct KagemushaNativeOrdinaryBootstrapOwnerV1 {
    path: PathBuf,
    financial: Option<KagemushaOrdinaryEnrolledFinancialOwnerV1>,
    approvals: Option<KagemushaOrdinaryLogicalApprovalJournalV1>,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    nonce_commitment: DigestV1,
    capacity: KagemushaDurableCapacityV1,
    publication: Option<KagemushaAuthenticatedOrdinaryCurrentPublicationV1>,
    publication_attempted: bool,
}
impl KagemushaNativeOrdinaryBootstrapOwnerV1 {
    /// Create the sole original bootstrap approval journal under an independently admitted
    /// financial owner and actual production verifier. The nonce derives from original held
    /// Native secret/nonce custody, and therefore survives exact recovery without app inputs.
    /// # Errors
    /// Rejects existing journals, stale originals, another release or invalid durable capacity.
    pub fn create_new(
        path: PathBuf,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Self::from_financial(path, financial, verifier, capacity, false, &[])
    }
    /// Reopen the actual original approval prefix with independently verified original leases.
    /// Disk bytes do not authenticate enrollment, release, financial custody or current lease.
    /// # Errors
    /// Refuses missing/mixed journals, changed original preview or stale current custody.
    pub fn open_existing(
        path: PathBuf,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: &[Arc<
            iroha_data_model::kagemusha::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        >],
    ) -> Result<Self, KagemushaStateErrorV1> {
        Self::from_financial(path, financial, verifier, capacity, true, integrity_leases)
    }
    fn from_financial(
        path: PathBuf,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
        recover: bool,
        integrity_leases: &[Arc<
            iroha_data_model::kagemusha::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        >],
    ) -> Result<Self, KagemushaStateErrorV1> {
        capacity.validate()?;
        let now = financial.trusted_time_ms().map_err(material)?;
        let nonce_commitment = financial
            .bootstrap_state_nonce_commitment()
            .map_err(material)?;
        let selection = selected_enrollment(
            financial.enrollment(),
            verifier.clone(),
            nonce_commitment,
            capacity,
            financial.retained_integrity_lease().map(Arc::as_ref),
            now,
        )?;
        selection.recheck_at_trusted_time(now)?;
        let approvals = if recover {
            KagemushaOrdinaryLogicalApprovalJournalV1::open_existing_with_integrity_leases(
                &path,
                &selection,
                financial.enrollment().clone(),
                integrity_leases,
                now,
            )?
        } else {
            KagemushaOrdinaryLogicalApprovalJournalV1::create_new(
                &path,
                &selection,
                financial.enrollment().clone(),
                now,
            )?
        };
        financial.recheck().map_err(material)?;
        Ok(Self {
            path,
            financial: Some(financial),
            approvals: Some(approvals),
            verifier,
            nonce_commitment,
            capacity,
            publication: None,
            publication_attempted: false,
        })
    }
    fn selected(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>, KagemushaStateErrorV1>
    {
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let selected = selected_enrollment(
            financial.enrollment(),
            self.verifier.clone(),
            self.nonce_commitment,
            self.capacity,
            financial.retained_integrity_lease().map(Arc::as_ref),
            financial.trusted_time_ms().map_err(material)?,
        )?;
        selected.recheck_at_trusted_time(financial.trusted_time_ms().map_err(material)?)?;
        Ok(selected)
    }
    /// Reserve the Native-derived exact W/S before any platform invocation.
    /// The operation identifier selects an existing attempt; it is neither a subject nor a clock.
    pub fn reserve_bootstrap(
        &mut self,
        operation_id: DigestV1,
    ) -> Result<
        iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1,
        KagemushaStateErrorV1,
    > {
        self.selected()?;
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        let challenge = *self
            .approvals
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .reserve_bootstrap(operation_id, now)?;
        financial.recheck().map_err(material)?;
        Ok(challenge)
    }
    /// Verify and fsync the same exact original W platform evidence under Native time.
    /// Passing this stage grants no monetary or current-state capability.
    pub fn accept_bootstrap_approval(
        &mut self,
        original: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.selected()?;
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        self.approvals
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .accept_original(original, now)?;
        self.approvals
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .capture_bootstrap_approval(financial)?;
        financial.recheck().map_err(material)
    }
    /// Lend only genuine opaque Native holders to Rust proving orchestration.
    /// The callback is work under held originals, never an accepting verifier or C/JNI intake.
    pub fn with_selected_bootstrap<R>(
        &self,
        consume: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
            &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
            &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        ) -> Result<R, KagemushaStateErrorV1>,
    ) -> Result<R, KagemushaStateErrorV1> {
        let selection = self.selected()?;
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approval = self
            .approvals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .captured_bootstrap_at_native_time(financial.trusted_time_ms().map_err(material)?)?;
        let result = consume(&selection, &approval, financial)?;
        selection.recheck_at_trusted_time(financial.trusted_time_ms().map_err(material)?)?;
        Ok(result)
    }
    /// Prove the real SHA claim and both initial State parities, then fsync the actual publication.
    /// The distinct ordinary Guard original is genuinely checked by the fixed Native prover.
    /// Any uncertain final publication requires reopening the surviving original WAL.
    pub fn prove_and_publish<R: KagemushaArtifactByteResolverV1>(
        &mut self,
        prover: &KagemushaProductionProverV1<R>,
        paired_guard_original: Vec<u8>,
        auxiliaries: &dyn KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.publication_attempted || self.publication.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let proof = self.with_selected_bootstrap(|selection, approval, financial| {
            let claim = prover
                .prove_ordinary_bootstrap_state_hash_claim(
                    selection,
                    approval,
                    financial,
                    &paired_guard_original,
                    auxiliaries,
                )
                .map_err(material)?;
            let generated = prover
                .prove_ordinary_bootstrap_state(
                    selection,
                    approval,
                    financial,
                    &paired_guard_original,
                    &claim,
                    auxiliaries,
                )
                .map_err(material)?;
            Ok(generated.proof)
        })?;
        self.publication_attempted = true;
        let enrollment = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .enrollment()
            .clone();
        let held_financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let lease = held_financial.retained_integrity_lease().cloned();
        let selection = selected_enrollment(
            &enrollment,
            self.verifier.clone(),
            self.nonce_commitment,
            self.capacity,
            lease.as_deref(),
            held_financial.trusted_time_ms().map_err(material)?,
        )?;
        let financial = self
            .financial
            .take()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approvals = self
            .approvals
            .take()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.publication = Some(
            KagemushaAuthenticatedOrdinaryCurrentPublicationV1::create_new(
                &self.path,
                &selection,
                financial,
                approvals,
                proof,
                paired_guard_original,
            )?,
        );
        self.publication
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .recheck()
    }
    /// Recover a publication using genuine independently reopened financial/journal holders.
    /// Historical Guard admission is never converted to a fresh operation approval.
    pub fn recover_publication(&mut self) -> Result<(), KagemushaStateErrorV1> {
        if self.publication.is_some() || self.publication_attempted {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.publication_attempted = true;
        let enrollment = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .enrollment()
            .clone();
        let held_financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let lease = held_financial.retained_integrity_lease().cloned();
        let selection = selected_enrollment(
            &enrollment,
            self.verifier.clone(),
            self.nonce_commitment,
            self.capacity,
            lease.as_deref(),
            held_financial.trusted_time_ms().map_err(material)?,
        )?;
        let financial = self
            .financial
            .take()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approvals = self
            .approvals
            .take()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.publication = Some(
            KagemushaAuthenticatedOrdinaryCurrentPublicationV1::open_existing(
                &self.path, &selection, financial, approvals,
            )?,
        );
        self.publication
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .recheck()
    }
    /// Read the genuinely published current owner after its actual Native custody rechecks.
    /// Before publication no owner is returned and no UI can fabricate readiness.
    pub fn publication(
        &self,
    ) -> Result<&KagemushaAuthenticatedOrdinaryCurrentPublicationV1, KagemushaStateErrorV1> {
        let published = self
            .publication
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        published.recheck()?;
        Ok(published)
    }
}

fn selected_enrollment<'a>(
    enrollment: &'a iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    nonce: DigestV1,
    capacity: KagemushaDurableCapacityV1,
    lease: Option<&'a iroha_data_model::kagemusha::KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    now: u64,
) -> Result<KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'a>, KagemushaStateErrorV1> {
    let selection = if let Some(lease) = lease {
        KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment_with_current_integrity_lease(
            enrollment, verifier, nonce, capacity, lease, now,
        )?
    } else {
        KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment(
            enrollment,
            verifier,
            nonce,
            capacity,
            enrollment.authenticated_at_ms(),
        )?
    };
    selection.recheck_at_trusted_time(now)?;
    Ok(selection)
}

fn material(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}
