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

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCurrentPublicationRecordV1")]
struct Record {
    version: u16,
    published_at_ms: u64,
    initial_state: KagemushaStateV1,
    statement: BootstrapStatementV1,
    retail_certificate_original: Vec<u8>,
    app_credential_original: Vec<u8>,
    approval_original: Vec<u8>,
    authorization_transcript_digest: DigestV1,
    subject_signing_digest: DigestV1,
    normalized_guard_digest: DigestV1,
    state_proof: KagemushaPairedProofV1,
    paired_ordinary_guard_original: Vec<u8>,
}

/// Exclusive initial ordinary publication retaining the actual native journals and proof owner.
/// No decoder, application verifier, raw signature or serialized state constructs this owner.
pub struct KagemushaAuthenticatedOrdinaryCurrentPublicationV1 {
    current: PrivateJournal,
    canonical: Vec<u8>,
    record: Record,
    approvals: KagemushaOrdinaryLogicalApprovalJournalV1,
    financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
    verified_guard: PublishedGuard,
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
    fn require_publication_time(&self, expected: u64) -> Result<(), KagemushaStateErrorV1> {
        if let Self::Restored(g) = self {
            if g.publication_time_ms() != expected {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedOrdinaryCurrentPublicationV1 {
    /// Verify both genuine State parities and the distinct compiled ordinary Guard, then fsync
    /// the complete initial publication before returning its retained owner. Existing paths are
    /// never reset. The proof must open the independent financial commitment; app key custody
    /// alone cannot publish this state. The actual held financial owner supplies fresh
    /// suspend-inclusive Native time before and after proofs and fsync.
    pub fn create_new(
        path: &Path,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
        approvals: KagemushaOrdinaryLogicalApprovalJournalV1,
        state_proof: KagemushaPairedProofV1,
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
        selection.verify_state_proof(&state_proof)?;
        let approval = approvals.approved_at_trusted_time(trusted_native_now_ms)?;
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
        approval.recheck_at_trusted_time(published_at_ms)?;
        let record = Record {
            version: 1,
            published_at_ms,
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
            paired_ordinary_guard_original,
        };
        let canonical = norito::encode_canonical(&record).map_err(material)?;
        if decode_record(&canonical)? != record {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let before_append_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(before_append_ms)?;
        approval.recheck_at_trusted_time(before_append_ms)?;
        let mut current = PrivateJournal::create_new(path, FORMAT).map_err(storage)?;
        current.append(&canonical).map_err(storage)?;
        let before_exposure_ms = financial.trusted_time_ms().map_err(material)?;
        selection.recheck_at_trusted_time(before_exposure_ms)?;
        approval.recheck_at_trusted_time(before_exposure_ms)?;
        let publication = Self {
            current,
            canonical,
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
        let mut current = PrivateJournal::open_existing(path, FORMAT).map_err(storage)?;
        let (sequence, canonical) = current
            .replay_next()
            .map_err(storage)?
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if sequence != 0 || current.replay_next().map_err(storage)?.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        current.require_single_record(&canonical).map_err(storage)?;
        let record = decode_record(&canonical)?;
        let now = financial.trusted_time_ms().map_err(material)?;
        if now < record.published_at_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        require_selected_preview(selection, &approvals, &record)?;
        selection.verify_state_proof(&record.state_proof)?;
        let approval =
            approvals.approved_at_original_publication_time(record.published_at_ms, now)?;
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
            record.published_at_ms,
            now,
        )?;
        // Verification can take time: current credential/lease is checked with a new Native
        // sample before any restored owner is exposed. The old approval remains historical.
        let exposure_now = financial.trusted_time_ms().map_err(material)?;
        approval.recheck_originals(record.published_at_ms, exposure_now)?;
        current.require_single_record(&canonical).map_err(storage)?;
        let publication = Self {
            current,
            canonical,
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
        require_financial_custody(&self.financial, &self.approvals)?;
        self.current
            .require_single_record(&self.canonical)
            .map_err(storage)?;
        self.verified_guard
            .require_publication_time(self.record.published_at_ms)?;
        let approval = self
            .approvals
            .approved_at_original_publication_time(self.record.published_at_ms, now)?;
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

    /// Pure initial-state projection after the retained actual owner rechecks.
    /// It does not expose a mutable machine or create an outgoing/mint/terminal authorization.
    pub fn initial_state(&self) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.record.initial_state)
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

fn decode_record(bytes: &[u8]) -> Result<Record, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record: Record =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(material)?;
    if norito::encode_canonical(&record).map_err(material)? != bytes
        || record.version != 1
        || record.published_at_ms == 0
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
