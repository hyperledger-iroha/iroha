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
#[path = "ordinary_bootstrap_platform_attempt.rs"]
mod platform_attempt;
use platform_attempt::BootstrapPlatformAttempt;

#[derive(Clone, Copy)]
enum BootstrapStoragePurpose {
    LogicalApproval,
    PlatformAttempt,
    CurrentPublication,
}

fn bootstrap_storage_path(root: &Path, purpose: BootstrapStoragePurpose) -> PathBuf {
    root.join(match purpose {
        BootstrapStoragePurpose::LogicalApproval => "logical-approvals",
        BootstrapStoragePurpose::PlatformAttempt => "bootstrap-platform",
        BootstrapStoragePurpose::CurrentPublication => "current-publication",
    })
}

/// Actual Native staging owner before the first paired State publication.
/// It exposes no financial secret, key constructor, caller clock or accepting verifier callback.
/// Publishing consumes both exclusive holders into the actual durable current publication.
pub struct KagemushaNativeOrdinaryBootstrapOwnerV1 {
    path: PathBuf,
    enrollment:
        Arc<iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    financial: Option<KagemushaOrdinaryEnrolledFinancialOwnerV1>,
    approvals: Option<KagemushaOrdinaryLogicalApprovalJournalV1>,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    nonce_commitment: DigestV1,
    capacity: KagemushaDurableCapacityV1,
    publication: Option<KagemushaAuthenticatedOrdinaryCurrentPublicationV1>,
    publication_attempted: bool,
    platform_attempt: Option<BootstrapPlatformAttempt>,
    recover_platform_attempt: bool,
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
        mut financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
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
        let current_integrity_lease = financial.retained_integrity_lease().cloned();
        let mut approvals = if recover {
            KagemushaOrdinaryLogicalApprovalJournalV1::open_existing_with_native_current_integrity_lease(
                &bootstrap_storage_path(&path, BootstrapStoragePurpose::LogicalApproval),
                &selection,
                financial.enrollment().clone(),
                integrity_leases,
                current_integrity_lease.as_ref(),
                now,
            )?
        } else {
            KagemushaOrdinaryLogicalApprovalJournalV1::create_new(
                &bootstrap_storage_path(&path, BootstrapStoragePurpose::LogicalApproval),
                &selection,
                financial.enrollment().clone(),
                now,
            )?
        };
        if let Some(lease) = current_integrity_lease {
            // Creation and exact reopen both fsync before selecting the journal's actual Arc.
            // An identical independently re-admitted original retains the old journal Arc.
            approvals.retain_integrity_lease(lease, now)?;
            let retained = Arc::clone(
                approvals
                    .retained_integrity_lease()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            );
            financial
                .select_verified_integrity_lease(retained)
                .map_err(material)?;
        } else if approvals.retained_integrity_lease().is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let exposed_at = financial.trusted_time_ms().map_err(material)?;
        approvals.recheck_at_trusted_time(exposed_at)?;
        financial.recheck().map_err(material)?;
        Ok(Self {
            path,
            enrollment: financial.enrollment().clone(),
            financial: Some(financial),
            approvals: Some(approvals),
            verifier,
            nonce_commitment,
            capacity,
            publication: None,
            publication_attempted: false,
            platform_attempt: None,
            recover_platform_attempt: recover,
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
    /// Retain a genuinely admitted current Integrity original before initial publication.
    /// This can recover an expired prior lease without renewing the captured bootstrap W.
    /// The actual financial owner supplies time; the journal fsyncs the exact typed lease
    /// before both holders select its same admitted capability. Uncertain WAL writes fail closed.
    pub fn accept_integrity_lease(
        &mut self,
        lease: Arc<iroha_data_model::kagemusha::KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.publication_attempted || self.publication.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let financial = self
            .financial
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approvals = self
            .approvals
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !Arc::ptr_eq(financial.enrollment(), approvals.retained_enrollment()) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let now = financial
            .trusted_time_for_integrity_refresh(lease.as_ref())
            .map_err(material)?;
        approvals.retain_integrity_lease(Arc::clone(&lease), now)?;
        // An identical retry keeps the journal's prior actual Arc rather than substituting
        // a separately re-admitted token with matching decoded fields.
        let selected = Arc::clone(
            approvals
                .retained_integrity_lease()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
        );
        if selected.original() != lease.original() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        financial
            .select_verified_integrity_lease(selected)
            .map_err(material)?;
        approvals.recheck_at_trusted_time(financial.trusted_time_ms().map_err(material)?)?;
        self.selected()?;
        Ok(())
    }
    /// Borrow the same immutable verified FI original after the actual retained owner rechecks.
    /// It supplies no financial secret, State proof, spending grant or caller-selected policy.
    pub fn enrollment(
        &self,
    ) -> Result<
        &Arc<iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        KagemushaStateErrorV1,
    > {
        if let Some(publication) = &self.publication {
            publication.recheck()?;
        } else {
            self.financial
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
                .recheck()
                .map_err(material)?;
        }
        Ok(&self.enrollment)
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
        if self.publication.is_some() {
            let (challenge, _, _) = self.captured_bootstrap_platform_originals()?;
            if challenge.operation_id != operation_id {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return Ok(challenge);
        }
        self.selected()?;
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        let approvals = self
            .approvals
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let challenge = match approvals.captured_bootstrap_at_native_time(now) {
            Ok(captured) if captured.challenge().operation_id == operation_id => {
                *captured.challenge()
            }
            Ok(_) => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            Err(KagemushaStateErrorV1::InvalidCandidateStage) => {
                *approvals.reserve_bootstrap(operation_id, now)?
            }
            Err(error) => return Err(error),
        };
        financial.recheck().map_err(material)?;
        Ok(challenge)
    }
    /// Create or reopen the exact Native OS attempt before exposing W or its signing scope.
    /// Returned fields are ticket, W, Native scope and same complete S; they grant no money.
    pub fn prepare_bootstrap_platform(
        &mut self,
        operation_id: DigestV1,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        use sha2::{Digest as _, Sha256};
        let challenge = self.reserve_bootstrap(operation_id)?;
        let enrollment = Arc::clone(self.enrollment()?);
        let credential = enrollment.app_credential().subject();
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-bootstrap-platform-scope\0");
        hash.update(Sha256::digest(
            enrollment
                .certificate()
                .canonical_bytes()
                .map_err(material)?,
        ));
        hash.update(self.nonce_commitment);
        hash.update(Sha256::digest(
            challenge.canonical_signing_bytes().map_err(material)?,
        ));
        let scope = hash.finalize().into();
        if self.platform_attempt.is_none() {
            self.platform_attempt = Some(
                if self.recover_platform_attempt || self.publication.is_some() {
                    BootstrapPlatformAttempt::open_existing(
                        &bootstrap_storage_path(
                            &self.path,
                            BootstrapStoragePurpose::PlatformAttempt,
                        ),
                        challenge,
                        scope,
                        credential.platform_class,
                        credential.app_release_digest,
                    )?
                } else {
                    BootstrapPlatformAttempt::create(
                        &bootstrap_storage_path(
                            &self.path,
                            BootstrapStoragePurpose::PlatformAttempt,
                        ),
                        challenge,
                        scope,
                        credential.platform_class,
                        credential.app_release_digest,
                    )?
                },
            );
        }
        let attempt = self
            .platform_attempt
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if attempt.challenge() != &challenge || attempt.scope() != scope {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        attempt.recheck()?;
        let original = if self.publication.is_some() {
            self.captured_bootstrap_platform_originals()?
                .0
                .subject
                .canonical_signing_bytes()
                .map_err(material)?
        } else {
            self.approvals
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
                .bootstrap_selection_original()?
        };
        self.enrollment()?;
        if self.publication.is_some() {
            self.require_platform_ticket(attempt.ticket())?;
        }
        Ok(vec![
            attempt.ticket().to_le_bytes().to_vec(),
            challenge.canonical_signing_bytes().map_err(material)?,
            scope.to_vec(),
            original,
        ])
    }
    fn captured_bootstrap_platform_originals(
        &self,
    ) -> Result<
        (
            iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1,
            Vec<u8>,
            Option<u32>,
        ),
        KagemushaStateErrorV1,
    > {
        if let Some(publication) = &self.publication {
            return publication.retained_bootstrap_platform_originals();
        }
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let captured = self
            .approvals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .captured_bootstrap_at_native_time(financial.trusted_time_ms().map_err(material)?)?;
        Ok((
            *captured.challenge(),
            captured.original().to_vec(),
            captured.app_attest_counter(),
        ))
    }
    fn require_platform_ticket(&self, ticket: u64) -> Result<(), KagemushaStateErrorV1> {
        if let Some(publication) = &self.publication {
            publication.recheck()?;
        } else {
            self.selected()?;
        }
        let attempt = self
            .platform_attempt
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if attempt.ticket() != ticket {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        attempt.recheck()?;
        if self.publication.is_some() {
            let (_, original, counter) = self.captured_bootstrap_platform_originals()?;
            attempt.retained_consumed_receipt_for_original(&original, counter)?;
        }
        Ok(())
    }
    /// Fsync the single OS invocation fence. Unknown invocation outcomes never re-sign.
    pub fn fence_bootstrap_platform(
        &mut self,
        ticket: u64,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_live_or_captured_platform(ticket)?;
        let response = self
            .platform_attempt
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .fence()?;
        self.require_platform_ticket(ticket)?;
        Ok(response)
    }
    /// Retain unchanged bounded DER/CBOR before Native signature admission or bootstrap capture.
    pub fn retain_bootstrap_platform_original(
        &mut self,
        ticket: u64,
        raw: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_platform_ticket(ticket)?;
        let digest = self
            .platform_attempt
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .retain_raw(raw)?;
        self.require_platform_ticket(ticket)?;
        Ok(digest)
    }
    /// Authenticate the exact retained platform original and fsync bootstrap capture before receipt.
    /// A recovered capture is matched to the same raw original; it is never renewed or resigned.
    pub fn consume_bootstrap_platform(
        &mut self,
        ticket: u64,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_platform_ticket(ticket)?;
        let original = self
            .platform_attempt
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .approval_original()?;
        if self.publication.is_some() {
            let (_, captured_original, counter) = self.captured_bootstrap_platform_originals()?;
            let receipt = self
                .platform_attempt
                .as_ref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                .retained_consumed_receipt_for_original(&captured_original, counter)?;
            self.require_platform_ticket(ticket)?;
            return Ok(receipt);
        }
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        let captured = self
            .approvals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .captured_bootstrap_at_native_time(now);
        match captured {
            Ok(captured) if captured.original() == original => (),
            Ok(_) => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            Err(KagemushaStateErrorV1::InvalidCandidateStage) => {
                self.accept_bootstrap_approval(&original)?
            }
            Err(error) => return Err(error),
        }
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let captured = self
            .approvals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .captured_bootstrap_at_native_time(financial.trusted_time_ms().map_err(material)?)?;
        if captured.original() != original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let counter = captured.app_attest_counter();
        let response = self
            .platform_attempt
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .complete(counter)?;
        self.require_platform_ticket(ticket)?;
        Ok(response)
    }
    /// Recover the exact original fence/evidence/receipt without an OS or issuer call.
    pub fn recover_bootstrap_platform(
        &self,
        ticket: u64,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_platform_ticket(ticket)?;
        let attempt = self
            .platform_attempt
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let response = attempt.recover()?;
        if response[0] == [2] {
            let original = attempt.approval_original()?;
            let (_, captured_original, counter) = self.captured_bootstrap_platform_originals()?;
            if captured_original != original
                || response[2] != attempt.receipt_for_counter(counter)?
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(response)
    }
    /// Recheck exact W and Native scope; this data projection grants no financial authority.
    pub fn recheck_bootstrap_platform(
        &self,
        ticket: u64,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        use sha2::{Digest as _, Sha256};
        self.require_live_or_captured_platform(ticket)?;
        let attempt = self
            .platform_attempt
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        Ok(vec![
            attempt.scope().to_vec(),
            Sha256::digest(
                attempt
                    .challenge()
                    .canonical_signing_bytes()
                    .map_err(material)?,
            )
            .to_vec(),
        ])
    }
    /// Cancel only an uninvoked original. An uncertain OS effect is never reset.
    pub fn cancel_bootstrap_platform(&mut self, ticket: u64) -> Result<(), KagemushaStateErrorV1> {
        self.require_platform_ticket(ticket)?;
        self.platform_attempt
            .as_mut()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .cancel()
    }
    fn require_live_or_captured_platform(&self, ticket: u64) -> Result<(), KagemushaStateErrorV1> {
        self.require_platform_ticket(ticket)?;
        if self.publication.is_some() {
            return Ok(());
        }
        let financial = self
            .financial
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        let approvals = self
            .approvals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        match approvals.captured_bootstrap_at_native_time(now) {
            Ok(captured) => {
                let attempt = self
                    .platform_attempt
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
                if attempt.approval_original()? != captured.original() {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                Ok(())
            }
            Err(KagemushaStateErrorV1::InvalidCandidateStage) => {
                let challenge = self
                    .platform_attempt
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
                    .challenge();
                if now < challenge.issued_at_ms || now >= challenge.expires_at_ms {
                    return Err(KagemushaStateErrorV1::SnapshotRollback);
                }
                Ok(())
            }
            Err(error) => Err(error),
        }
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
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.publication_attempted || self.publication.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let proof = self.with_selected_bootstrap(|selection, approval, financial| {
            let auxiliaries = prover
                .prepare_ordinary_bootstrap_auxiliaries(
                    selection,
                    approval,
                    financial,
                    &paired_guard_original,
                )
                .map_err(material)?;
            let claim = prover
                .prove_ordinary_bootstrap_state_hash_claim(
                    selection,
                    approval,
                    financial,
                    &paired_guard_original,
                    &auxiliaries,
                )
                .map_err(material)?;
            let generated = prover
                .prove_ordinary_bootstrap_state(
                    selection,
                    approval,
                    financial,
                    &paired_guard_original,
                    &claim,
                    &auxiliaries,
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
                &bootstrap_storage_path(&self.path, BootstrapStoragePurpose::CurrentPublication),
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
                &bootstrap_storage_path(&self.path, BootstrapStoragePurpose::CurrentPublication),
                &selection,
                financial,
                approvals,
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

/// Run the native platform receipt regression with already authenticated maintained fixture data.
#[cfg(test)]
pub(super) fn assert_bootstrap_platform_receipt_binding_and_replay(
    approval: &iroha_data_model::kagemusha::KagemushaAppOperationApprovalV1,
    app_release: DigestV1,
    counter: Option<u32>,
) {
    platform_attempt::assert_receipt_binding_and_replay(approval, app_release, counter);
}

#[cfg(test)]
#[path = "authenticated_ordinary_bootstrap_owner_tests.rs"]
mod tests;
