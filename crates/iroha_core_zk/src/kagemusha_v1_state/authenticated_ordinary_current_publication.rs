//! Durable initial ordinary state selected by genuine paired State and ordinary Guard.
//!
//! The actual app approval journal and independent issuer originals remain held. This
//! software publication does not manufacture an OEM hardware checkpoint or monotonicity.

use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryBootstrapGuardV1,
    KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1,
    verify_ordinary_bootstrap_guard_historical_v1, verify_ordinary_bootstrap_guard_v1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-current.norito.wal",
    magic: b"IKGOCW1\0",
    hash_domain: b"iroha:kagemusha:v1:ordinary-current-publication\0",
    maximum_payload_bytes: 512 * 1024,
};

#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCurrentPublicationRecordV1")]
struct Record {
    version: u16,
    published_at_ms: u64,
    approval_captured_at_ms: u64,
    publication_intent_digest: DigestV1,
    initial_state: KagemushaStateV1,
    statement: BootstrapStatementV1,
    retail_certificate_original: Vec<u8>,
    app_credential_original: Vec<u8>,
    approval_original: Vec<u8>,
    authorization_transcript_digest: DigestV1,
    subject_signing_digest: DigestV1,
    normalized_guard_digest: DigestV1,
    state_proof: KagemushaPairedProofV1,
    private_state_checkpoint_original: Vec<u8>,
    paired_ordinary_guard_original: Vec<u8>,
}

impl core::fmt::Debug for Record {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OrdinaryCurrentPublicationRecordV1")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
impl Drop for Record {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.private_state_checkpoint_original.zeroize();
        self.initial_state.balance.zeroize();
        self.initial_state.state_nonce_commitment.zeroize();
    }
}

/// Exclusive initial ordinary publication retaining the actual native journals and proof owner.
/// No decoder, application verifier, raw signature or serialized state constructs this owner.
pub struct KagemushaAuthenticatedOrdinaryCurrentPublicationV1 {
    current: PrivateJournal,
    canonical: Vec<u8>,
    payload_maximum: u64,
    record: Record,
    approvals: KagemushaOrdinaryLogicalApprovalJournalV1,
    financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
    verified_guard: PublishedGuard,
}

impl Drop for KagemushaAuthenticatedOrdinaryCurrentPublicationV1 {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.canonical.zeroize();
    }
}

// Both variants authenticate one published original. A restored historical result is never
// converted to a live Guard capability or used to approve another operation.
enum PublishedGuard {
    Initial(KagemushaAuthenticatedOrdinaryBootstrapGuardV1),
    Restored(KagemushaAuthenticatedOrdinaryHistoricalBootstrapGuardV1),
}
impl PublishedGuard {
    fn digests(&self) -> [DigestV1; 5] {
        match self {
            Self::Initial(g) => [
                g.normalized_guard_digest(),
                g.credential_digest(),
                g.authorization_transcript_digest(),
                g.subject_signing_digest(),
                g.provider_policy_root(),
            ],
            Self::Restored(g) => [
                g.normalized_guard_digest(),
                g.credential_digest(),
                g.authorization_transcript_digest(),
                g.subject_signing_digest(),
                g.provider_policy_root(),
            ],
        }
    }
    fn original(&self) -> &[u8] {
        match self {
            Self::Initial(g) => g.original(),
            Self::Restored(g) => g.original(),
        }
    }
    fn require_approval_admission_time(&self, expected: u64) -> Result<(), KagemushaStateErrorV1> {
        if let Self::Restored(g) = self {
            if g.approval_admission_time_ms() != expected {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedOrdinaryCurrentPublicationV1 {
    // Only the genuine consuming cash owner can borrow these retained holders. There is no
    // public financial-secret projection, serialized capability or replacement-owner path.
    pub(super) fn cash_financial(&self) -> &KagemushaOrdinaryEnrolledFinancialOwnerV1 {
        &self.financial
    }

    pub(super) fn cash_approvals(&self) -> &KagemushaOrdinaryLogicalApprovalJournalV1 {
        &self.approvals
    }

    /// Private retained proving custody only. This never samples or lends a current clock/FI
    /// grant, and cannot authorize State CAS or exposure. The cash actor first requires its
    /// actual process-marked current-control proof capture; live effects separately recheck all
    /// current financial, policy/revocation, FI, PI and clock originals.
    pub(super) fn recheck_historical_cash_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)?;
        if decode_record(&self.canonical, self.payload_maximum)? != self.record {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.financial
            .recheck_historical_proof_custody()
            .map_err(material)?;
        self.financial
            .recheck_historical_release(self.approvals.retained_release())
            .map_err(material)?;
        if !Arc::ptr_eq(
            self.financial.enrollment(),
            self.approvals.retained_enrollment(),
        ) || self
            .financial
            .historical_financial_authority_commitment()
            .map_err(material)?
            != self
                .approvals
                .retained_enrollment()
                .app_credential()
                .subject()
                .financial_authority_commitment
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match (
            self.financial.retained_integrity_lease(),
            self.approvals.retained_integrity_lease(),
        ) {
            (None, None) => {}
            (Some(a), Some(b)) if Arc::ptr_eq(a, b) => {}
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        }
        let (intent_digest, intent_created_at, captured_at) =
            self.approvals.historical_initial_publication_intent()?;
        if intent_digest != self.record.publication_intent_digest
            || captured_at != self.record.approval_captured_at_ms
            || intent_created_at > self.record.published_at_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.verified_guard
            .require_approval_admission_time(captured_at)?;
        let approval = self.approvals.historical_bootstrap_approval()?;
        if approval.retained_capture_time_ms() != self.record.approval_captured_at_ms {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        approval.recheck_retained_capture_custody()?;
        if self.verified_guard.digests()
            != [
                self.record.normalized_guard_digest,
                self.approvals
                    .retained_enrollment()
                    .app_credential()
                    .digest(),
                self.record.authorization_transcript_digest,
                self.record.subject_signing_digest,
                self.approvals.retained_release().provider_policy_root(),
            ]
            || self.verified_guard.original() != self.record.paired_ordinary_guard_original
            || approval.original() != self.record.approval_original
            || approval.authorization_binding_digest()?
                != self.record.authorization_transcript_digest
            || approval.challenge().subject_signing_digest != self.record.subject_signing_digest
            || self.approvals.bootstrap_preview().state != self.record.initial_state
            || self.approvals.bootstrap_preview().statement != self.record.statement
            || self
                .approvals
                .retained_enrollment()
                .certificate()
                .canonical_bytes()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
                != self.record.retail_certificate_original
            || self
                .approvals
                .retained_enrollment()
                .app_credential()
                .original()
                != self.record.app_credential_original
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)
    }
    pub(super) fn with_retained_initial_state_checkpoint(
        &self,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        self.recheck_historical_cash_custody()?;
        let approval = self.approvals.historical_bootstrap_approval()?;
        let enrollment = self.approvals.retained_enrollment();
        let at = self.record.approval_captured_at_ms;
        let selection = match approval.original_approval_integrity_lease() {
            Some(lease) => KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment_with_current_integrity_lease(enrollment, verifier, self.record.statement.state_nonce_commitment, capacity, lease, at),
            None => KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment(enrollment, verifier, self.record.statement.state_nonce_commitment, capacity, at),
        }?;
        if selection.preview()?.state != self.record.initial_state
            || selection.preview()?.statement != self.record.statement
            || !Arc::ptr_eq(
                &selection.authenticated_release()?,
                self.approvals.retained_release(),
            )
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let restored = selection.restore_private_state_checkpoint(
            &self.record.private_state_checkpoint_original,
            &self.record.state_proof,
        )?;
        consume(&restored)?;
        self.recheck_historical_cash_custody()
    }

    /// Full public zero-State/Guard originals selected from this held immutable publication.
    /// This supplies only data to separate genuine Core Anchor admission and global CAS.
    pub(super) fn lineage_anchor_public_originals(
        &self,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
    ) -> Result<
        (
            iroha_data_model::kagemusha::KagemushaOrdinaryLineageAnchorV1,
            Vec<u8>,
        ),
        KagemushaStateErrorV1,
    > {
        use crate::kagemusha_v1_recursion::{
            KagemushaOrdinaryLineageStateProofBundleV1, KagemushaOrdinaryLineageStatementOriginalV1,
        };
        use iroha_data_model::kagemusha::{
            KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryFinancialLineageV1,
            KagemushaOrdinaryLineageAnchorV1, kagemusha_ordinary_financial_epoch_id_v1,
        };
        self.recheck_historical_cash_custody()?;
        let approval = self.approvals.historical_bootstrap_approval()?;
        let enrollment = self.approvals.retained_enrollment();
        let original_lease = approval.original_approval_integrity_lease();
        let at = self.record.approval_captured_at_ms;
        let selection = match original_lease {
            Some(lease) => KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment_with_current_integrity_lease(
                enrollment, verifier, self.record.statement.state_nonce_commitment, capacity, lease, at),
            None => KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1::from_verified_enrollment(
                enrollment, verifier, self.record.statement.state_nonce_commitment, capacity, at),
        }?;
        if !Arc::ptr_eq(
            &selection.authenticated_release()?,
            self.approvals.retained_release(),
        ) || selection.preview()?.state != self.record.initial_state
            || selection.preview()?.statement != self.record.statement
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let state_original = selection.lineage_public_state_original(&self.record.state_proof)?;
        let lineage = KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: enrollment.certificate().subject.owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(
                enrollment.app_credential().subject(),
            )
            .map_err(material)?,
            financial_authority_commitment: enrollment
                .app_credential()
                .subject()
                .financial_authority_commitment,
        };
        let initial_head = KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: self.record.initial_state.state_commitment,
            logical_sequence: self.record.initial_state.logical_sequence,
            state_original_sha256: Sha256::digest(&state_original).into(),
        };
        let bundle = KagemushaOrdinaryLineageStateProofBundleV1::from_public_parts(
            self.approvals
                .bootstrap_preview()
                .normalized_guard_statement,
            KagemushaOrdinaryLineageStatementOriginalV1::Zero(Box::new(
                self.record.statement.clone(),
            )),
            state_original,
            self.record.app_credential_original.clone(),
            self.record.approval_original.clone(),
            original_lease.map(|lease| lease.original().to_vec()),
            self.record.paired_ordinary_guard_original.clone(),
            None,
            None,
        )
        .map_err(material)?
        .canonical_bytes()
        .map_err(material)?;
        let anchor = KagemushaOrdinaryLineageAnchorV1 {
            lineage,
            initial_head,
            proof_bundle_original_sha256: Sha256::digest(&bundle).into(),
        };
        anchor.validate_shape().map_err(material)?;
        self.recheck_historical_cash_custody()?;
        Ok((anchor, bundle))
    }

    /// Actual original zero-State, available only to the private genuine cash actor for proving.
    pub(super) fn historical_initial_state(
        &self,
    ) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck_historical_cash_custody()?;
        Ok(&self.record.initial_state)
    }
    /// Full original commitments after historical custody checks; these data create no authority.
    pub(super) fn historical_original_commitments(
        &self,
    ) -> Result<[DigestV1; 8], KagemushaStateErrorV1> {
        self.recheck_historical_cash_custody()?;
        let state = norito::encode_canonical(&self.record.initial_state).map_err(material)?;
        let proof = norito::encode_canonical(&self.record.state_proof).map_err(material)?;
        let result = detached_original_commitments(
            self.approvals
                .retained_enrollment()
                .certificate()
                .subject
                .enrollment_id,
            [
                &self.canonical,
                &self.record.retail_certificate_original,
                &self.record.app_credential_original,
                &self.record.approval_original,
                &state,
                &proof,
                &self.record.paired_ordinary_guard_original,
            ],
        )?;
        self.recheck_historical_cash_custody()?;
        Ok(result)
    }

    /// Verify both genuine State parities and the distinct compiled ordinary Guard, then fsync
    /// the complete initial publication before returning its retained owner. Existing paths are
    /// never reset. The proof must open the independent financial commitment; app key custody
    /// alone cannot publish this state. The actual held financial owner supplies fresh
    /// suspend-inclusive Native time before and after proofs and fsync. Its distinct zero-State
    /// approval was already captured while live; slow proof work does not renew that signature.
    pub fn create_new(
        path: &Path,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        approvals: KagemushaOrdinaryLogicalApprovalJournalV1,
        generated_state: crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        paired_ordinary_guard_original: Vec<u8>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let trusted_native_now_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(trusted_native_now_ms)?;
        approvals.recheck_at_trusted_time(trusted_native_now_ms)?;
        require_financial_custody(&financial, &approvals)?;
        if !std::ptr::eq(
            selection.enrollment(),
            approvals.retained_enrollment().as_ref(),
        ) || selection.preview()? != approvals.bootstrap_preview()
            || selection.authenticated_release()?.release_id()
                != approvals.retained_release().release_id()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        validate_guard_bytes(&paired_ordinary_guard_original)?;
        let state_proof = generated_state.proof.clone();
        let private_state_checkpoint_original =
            selection.capture_private_state_checkpoint(&generated_state)?;
        let approval = approvals.captured_bootstrap_at_native_time(trusted_native_now_ms)?;
        let verified_guard = verify_ordinary_bootstrap_guard_v1(
            selection,
            &approval,
            &paired_ordinary_guard_original,
            trusted_native_now_ms,
        )?;
        let preview = selection.preview()?;
        let normalized_guard_digest = preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        if verified_guard.normalized_guard_digest() != normalized_guard_digest
            || verified_guard.credential_digest()
                != selection.enrollment().app_credential().digest()
            || verified_guard.authorization_transcript_digest()
                != approval.authorization_binding_digest()?
            || verified_guard.subject_signing_digest()
                != approval.challenge().subject_signing_digest
            || verified_guard.provider_policy_root()
                != approvals.retained_release().provider_policy_root()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let published_at_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(published_at_ms)?;
        approval.recheck_captured_bootstrap_at_native_time(published_at_ms)?;
        let record = Record {
            version: 1,
            published_at_ms,
            approval_captured_at_ms: approval.captured_at_ms(),
            publication_intent_digest: approvals
                .initial_publication_intent_digest(published_at_ms)?,
            initial_state: preview.state.clone(),
            statement: preview.statement.clone(),
            retail_certificate_original: selection
                .enrollment()
                .certificate()
                .canonical_bytes()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?,
            app_credential_original: selection.enrollment().app_credential().original().to_vec(),
            approval_original: approval.original().to_vec(),
            authorization_transcript_digest: approval.authorization_binding_digest()?,
            subject_signing_digest: approval.challenge().subject_signing_digest,
            normalized_guard_digest,
            state_proof,
            private_state_checkpoint_original,
            paired_ordinary_guard_original,
        };
        let format = publication_format(selection.recursive_verifier())?;
        let canonical = norito::encode_canonical(&record).map_err(material)?;
        if decode_record(&canonical, format.maximum_payload_bytes)? != record {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let before_append_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(before_append_ms)?;
        approval.recheck_captured_bootstrap_at_native_time(before_append_ms)?;
        let mut current = PrivateJournal::create_new(path, format).map_err(storage)?;
        current.append(&canonical).map_err(storage)?;
        let before_exposure_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(before_exposure_ms)?;
        approval.recheck_captured_bootstrap_at_native_time(before_exposure_ms)?;
        let publication = Self {
            current,
            canonical,
            payload_maximum: format.maximum_payload_bytes,
            record,
            approvals,
            financial,
            verified_guard: PublishedGuard::Initial(verified_guard),
        };
        publication.recheck()?;
        Ok(publication)
    }

    /// Recover the already-fsynced initial publication after a restart or lost result. The
    /// genuine financial owner and approval journal must be independently reopened first.
    /// Both State proofs and the complete ordinary Guard are verified again; disk bytes cannot
    /// select an enrollment, verifier, key, lease or original authentication instant.
    pub fn open_existing(
        path: &Path,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        approvals: KagemushaOrdinaryLogicalApprovalJournalV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        require_financial_custody(&financial, &approvals)?;
        let format = publication_format(selection.recursive_verifier())?;
        let mut current = PrivateJournal::open_existing(path, format).map_err(storage)?;
        let (sequence, canonical) = current
            .replay_next()
            .map_err(storage)?
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if sequence != 0 || current.replay_next().map_err(storage)?.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        current.require_single_record(&canonical).map_err(storage)?;
        let record = decode_record(&canonical, format.maximum_payload_bytes)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        if approvals.initial_publication_intent_digest(now)? != record.publication_intent_digest {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if now < record.published_at_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        require_selected_preview(selection, &approvals, &record)?;
        selection.restore_private_state_checkpoint(
            &record.private_state_checkpoint_original,
            &record.state_proof,
        )?;
        let approval =
            approvals.approved_at_original_capture_time(record.approval_captured_at_ms, now)?;
        if approval.original() != record.approval_original
            || approval.authorization_binding_digest()? != record.authorization_transcript_digest
            || approval.challenge().subject_signing_digest != record.subject_signing_digest
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let verified_guard = verify_ordinary_bootstrap_guard_historical_v1(
            selection,
            &approval,
            &record.paired_ordinary_guard_original,
            record.approval_captured_at_ms,
            now,
        )?;
        // Verification can take time: current credential/lease is checked with a new Native
        // sample before any restored owner is exposed. The old approval remains historical.
        let exposure_now = financial.trusted_time_ms().map_err(material)?;
        approval.recheck_originals(record.approval_captured_at_ms, exposure_now)?;
        current.require_single_record(&canonical).map_err(storage)?;
        let publication = Self {
            current,
            canonical,
            payload_maximum: format.maximum_payload_bytes,
            record,
            approvals,
            financial,
            verified_guard: PublishedGuard::Restored(verified_guard),
        };
        publication.recheck()?;
        Ok(publication)
    }

    /// Recheck complete held publication and current genuine credential/lease custody.
    /// An already-published approval is historical: this check does not renew or reuse it.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        let now = self.financial.trusted_time_ms().map_err(material)?;
        if now < self.record.published_at_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.approvals.recheck_at_trusted_time(now)?;
        if self.approvals.initial_publication_intent_digest(now)?
            != self.record.publication_intent_digest
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_financial_custody(&self.financial, &self.approvals)?;
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)?;
        self.verified_guard
            .require_approval_admission_time(self.record.approval_captured_at_ms)?;
        let approval = self
            .approvals
            .approved_at_original_capture_time(self.record.approval_captured_at_ms, now)?;
        if self.verified_guard.digests()
            != [
                self.record.normalized_guard_digest,
                self.approvals
                    .retained_enrollment()
                    .app_credential()
                    .digest(),
                self.record.authorization_transcript_digest,
                self.record.subject_signing_digest,
                self.approvals.retained_release().provider_policy_root(),
            ]
            || self.verified_guard.original() != self.record.paired_ordinary_guard_original
            || approval.original() != self.record.approval_original
            || approval.authorization_binding_digest()?
                != self.record.authorization_transcript_digest
            || approval.challenge().subject_signing_digest != self.record.subject_signing_digest
            || self.approvals.bootstrap_preview().state != self.record.initial_state
            || self.approvals.bootstrap_preview().statement != self.record.statement
            || self
                .approvals
                .retained_enrollment()
                .certificate()
                .canonical_bytes()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
                != self.record.retail_certificate_original
            || self
                .approvals
                .retained_enrollment()
                .app_credential()
                .original()
                != self.record.app_credential_original
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)
    }

    /// Project the retained bootstrap originals from the real published owner. This only
    /// acknowledges its historic capture and current custody; it creates no fresh W or money.
    pub(in crate::kagemusha_v1_state::authenticated_core_owner) fn retained_bootstrap_platform_originals(
        &self,
    ) -> Result<
        (
            iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1,
            Vec<u8>,
            Option<u32>,
        ),
        KagemushaStateErrorV1,
    > {
        self.recheck()?;
        let now = self.financial.trusted_time_ms().map_err(material)?;
        let retained = self
            .approvals
            .approved_at_original_capture_time(self.record.approval_captured_at_ms, now)?;
        if retained.original() != self.record.approval_original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let result = (
            *retained.challenge(),
            retained.original().to_vec(),
            retained.app_attest_counter(),
        );
        self.recheck()?;
        Ok(result)
    }

    /// Pure initial-state projection after the retained actual owner rechecks.
    /// It does not expose a mutable machine or create an outgoing/mint/terminal authorization.
    pub fn initial_state(&self) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.record.initial_state)
    }

    /// Detached commitments to exact retained originals, after current custody rechecks.
    /// These bytes cannot reconstruct this owner or authorize a subsequent financial operation.
    /// Order: enrollment ID, publication, FI certificate, credential, full signed W, State,
    /// paired State proof, paired ordinary Guard. Each digest hashes the full canonical original.
    pub fn original_commitments(&self) -> Result<[DigestV1; 8], KagemushaStateErrorV1> {
        self.recheck()?;
        let state = norito::encode_canonical(&self.record.initial_state).map_err(material)?;
        let proof = norito::encode_canonical(&self.record.state_proof).map_err(material)?;
        let result = detached_original_commitments(
            self.approvals
                .retained_enrollment()
                .certificate()
                .subject
                .enrollment_id,
            [
                &self.canonical,
                &self.record.retail_certificate_original,
                &self.record.app_credential_original,
                &self.record.approval_original,
                &state,
                &proof,
                &self.record.paired_ordinary_guard_original,
            ],
        )?;
        self.recheck()?;
        Ok(result)
    }

    /// Retain one genuine current Integrity lease without replacing the original FI credential,
    /// financial secret, epoch or published proof. This may recover from an expired old lease:
    /// current time and scope are checked against the independently verified new original first.
    /// The same Arc is fsynced in the logical journal before the financial owner selects it.
    pub fn accept_integrity_lease(
        &mut self,
        lease: Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)?;
        if !Arc::ptr_eq(
            self.financial.enrollment(),
            self.approvals.retained_enrollment(),
        ) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let now = self
            .financial
            .trusted_time_for_integrity_refresh(lease.as_ref())
            .map_err(material)?;
        self.approvals
            .retain_integrity_lease(Arc::clone(&lease), now)?;
        // Identical retry may already have retained this original under its prior genuine Arc.
        // Select that actual journal capability, preserving both holders' exact identity.
        let selected = Arc::clone(
            self.approvals
                .retained_integrity_lease()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
        );
        if selected.original() != lease.original() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.financial
            .select_verified_integrity_lease(selected)
            .map_err(material)?;
        self.recheck()
    }
}

fn detached_original_commitments(
    enrollment_id: DigestV1,
    originals: [&[u8]; 7],
) -> Result<[DigestV1; 8], KagemushaStateErrorV1> {
    if enrollment_id == [0; 32] || originals.iter().any(|original| original.is_empty()) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut result = [[0; 32]; 8];
    result[0] = enrollment_id;
    for (destination, original) in result[1..].iter_mut().zip(originals) {
        *destination = Sha256::digest(original).into();
        if *destination == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod commitment_tests {
    use super::*;

    #[test]
    fn detached_commitments_hash_complete_originals_in_order_without_creating_authority() {
        let originals = [
            b"publication".as_slice(),
            b"FI",
            b"credential",
            b"W and its signature",
            b"State",
            b"paired State",
            b"paired ordinary Guard",
        ];
        let result = detached_original_commitments([7; 32], originals).unwrap();
        assert_eq!(result[0], [7; 32]);
        for (digest, original) in result[1..].iter().zip(originals) {
            assert_eq!(*digest, <DigestV1>::from(Sha256::digest(original)));
        }
        assert_ne!(result[4], <DigestV1>::from(Sha256::digest(b"W")));
        assert!(detached_original_commitments([0; 32], originals).is_err());
        for index in 0..originals.len() {
            let mut missing = originals;
            missing[index] = b"";
            assert!(detached_original_commitments([7; 32], missing).is_err());
        }
    }
}

fn storage(_: PrivateJournalError) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

fn material(_: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

fn require_financial_custody(
    financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    approvals: &KagemushaOrdinaryLogicalApprovalJournalV1,
) -> Result<(), KagemushaStateErrorV1> {
    financial.recheck().map_err(material)?;
    if !Arc::ptr_eq(financial.enrollment(), approvals.retained_enrollment())
        || crate::kagemusha_v1_recursion::device_authority_commitment_v1(
            *financial.financial_secret().map_err(material)?,
        ) != financial
            .enrollment()
            .app_credential()
            .subject()
            .financial_authority_commitment
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    match (
        financial.retained_integrity_lease(),
        approvals.retained_integrity_lease(),
    ) {
        (None, None) => Ok(()),
        (Some(a), Some(b)) if Arc::ptr_eq(a, b) => Ok(()),
        _ => Err(KagemushaStateErrorV1::SnapshotIntegrity),
    }
}

fn require_selected_preview(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    approvals: &KagemushaOrdinaryLogicalApprovalJournalV1,
    record: &Record,
) -> Result<(), KagemushaStateErrorV1> {
    let preview = selection.preview()?;
    if !std::ptr::eq(
        selection.enrollment(),
        approvals.retained_enrollment().as_ref(),
    ) || preview != approvals.bootstrap_preview()
        || preview.state != record.initial_state
        || preview.statement != record.statement
        || preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(material)?
            != record.normalized_guard_digest
        || selection
            .enrollment()
            .certificate()
            .canonical_bytes()
            .map_err(material)?
            != record.retail_certificate_original
        || selection.enrollment().app_credential().original() != record.app_credential_original
        || selection.authenticated_release()?.release_id()
            != approvals.retained_release().release_id()
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn publication_format(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<PrivateJournalFormat, KagemushaStateErrorV1> {
    let maximum =
        crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(
            verifier,
        )
        .map_err(material)?;
    let maximum_payload_bytes = u64::try_from(maximum)
        .map_err(material)?
        .checked_add(FORMAT.maximum_payload_bytes)
        .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
    Ok(PrivateJournalFormat {
        maximum_payload_bytes,
        ..FORMAT
    })
}

fn decode_record(bytes: &[u8], payload_maximum: u64) -> Result<Record, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > payload_maximum {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record: Record =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(material)?;
    if norito::encode_canonical(&record).map_err(material)? != bytes
        || record.version != 1
        || record.published_at_ms == 0
        || record.approval_captured_at_ms == 0
        || record.approval_captured_at_ms > record.published_at_ms
        || record.publication_intent_digest == [0; 32]
        || record.private_state_checkpoint_original.is_empty()
        || record.retail_certificate_original.is_empty()
        || record.retail_certificate_original.len()
            > KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1
        || record.app_credential_original.is_empty()
        || record.app_credential_original.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
        || record.approval_original.is_empty()
        || record.approval_original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    validate_guard_bytes(&record.paired_ordinary_guard_original)?;
    Ok(record)
}
