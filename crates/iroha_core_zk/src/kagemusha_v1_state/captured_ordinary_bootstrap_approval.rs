//! Native capture of the one immutable zero-State approval before slow proof preparation.
//!
//! Captured authorization is distinct from a live financial approval. Only bootstrap proving
//! and its initial zero publication consume it; its nonce, interval and original never renew.

use super::*;

/// The already-fsynced original zero-State authorization captured by the actual Native clock.
/// No constructor, decoder, clone or conversion into a live monetary approval is available.
#[allow(missing_copy_implementations)]
pub struct KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'a> {
    original: &'a KagemushaVerifiedAppOperationApprovalV1,
    journal: &'a KagemushaOrdinaryLogicalApprovalJournalV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
    captured_at_ms: u64,
}
impl KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_> {
    /// Recheck the same captured original and zero statement, plus current credential/Integrity
    /// and descriptor custody. The caller samples current time from its actual financial owner.
    /// This checks the original only at its durable capture time; it grants no later operation.
    pub fn recheck_captured_bootstrap_at_native_time(
        &self,
        now: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        if now < self.captured_at_ms
            || self.journal.pending.as_ref().and_then(|p| p.captured_at_ms)
                != Some(self.captured_at_ms)
            || self.journal.wal.recovery_prefix().map_err(storage)? != self.prefix
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.journal.recheck_at_trusted_time(now)?;
        self.journal.require_bootstrap_capture(
            self.captured_at_ms,
            self.original.digest(),
            self.authorization_binding_digest()?,
        )?;
        self.journal.wal.check_owned().map_err(storage)
    }
    /// Actual Native capture time retained in the immutable durable authorization record.
    pub fn captured_at_ms(&self) -> u64 {
        self.captured_at_ms
    }
    /// The exact Native zero-State subject and operation nonce selected before OS dispatch.
    pub fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        self.original.challenge()
    }
    /// Complete original platform transcript, including its unchanged DER or CBOR bytes.
    pub fn original(&self) -> &[u8] {
        self.original.original()
    }
    /// Same exact approval original identity; this is not a spending grant.
    pub fn digest(&self) -> DigestV1 {
        self.original.digest()
    }
    /// Identity of the platform wrapper and complete original platform evidence.
    pub fn proof_binding_digest(&self) -> DigestV1 {
        self.original.proof_binding_digest()
    }
    /// Identity of the platform original and exact lease selected before that approval.
    pub fn authorization_binding_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            self.original.proof_binding_digest(),
            self.original_approval_integrity_lease().map(|l| l.digest()),
        )
        .map_err(material)
    }
    pub(crate) fn retained_enrollment(
        &self,
    ) -> &Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1> {
        &self.journal.enrollment
    }
    pub(crate) fn retained_release(&self) -> &Arc<KagemushaAuthenticatedReleaseV1> {
        &self.journal.release
    }
    pub(crate) fn original_approval_integrity_lease(
        &self,
    ) -> Option<&Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>> {
        self.journal
            .pending
            .as_ref()
            .expect("captured pending exists")
            .approval_integrity_lease
            .as_ref()
    }
    /// Original Apple assertion floor retained before this captured approval.
    /// Reading it grants no current approval or financial publication authority.
    pub fn previous_app_attest_counter_floor(&self) -> Option<u32> {
        self.journal
            .pending
            .as_ref()
            .expect("captured pending exists")
            .counter_floor_before
    }
    /// Original independently advancing Apple counter; never a financial sequence number.
    pub fn app_attest_counter(&self) -> Option<u32> {
        self.original.app_attest_counter()
    }
}

impl KagemushaOrdinaryLogicalApprovalJournalV1 {
    /// Capture the exact bootstrap original while its signature interval is still live, using
    /// the same actual Native financial owner. Fsync precedes capability exposure. Exact retries
    /// retain the original capture and never recapture an expired or replaced approval.
    pub fn capture_bootstrap_approval(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>, KagemushaStateErrorV1>
    {
        if !Arc::ptr_eq(financial.enrollment(), &self.enrollment) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let now = financial.trusted_time_ms().map_err(material)?;
        self.persist_bootstrap_capture_at_checked_native_time(now)?;
        let exposed_at = financial.trusted_time_ms().map_err(material)?;
        self.captured_bootstrap_at_native_time(exposed_at)
    }
    fn persist_bootstrap_capture_at_checked_native_time(
        &mut self,
        now: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(now)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.captured_at_ms.is_some() {
            return self.captured_bootstrap_at_native_time(now).map(|_| ());
        }
        let original = pending
            .approved
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approval_digest = original.digest();
        let authorization_binding_digest =
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                original.proof_binding_digest(),
                pending
                    .approval_integrity_lease
                    .as_ref()
                    .map(|l| l.digest()),
            )
            .map_err(material)?;
        self.require_bootstrap_capture(now, approval_digest, authorization_binding_digest)?;
        self.persist(&Record::CaptureBootstrap {
            captured_at_ms: now,
            approval_digest,
            authorization_binding_digest,
        })?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .captured_at_ms = Some(now);
        Ok(())
    }
    pub(crate) fn captured_bootstrap_at_native_time(
        &self,
        now: u64,
    ) -> Result<KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>, KagemushaStateErrorV1>
    {
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let cap = KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1 {
            original: pending
                .approved
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
            journal: self,
            prefix: self.wal.recovery_prefix().map_err(storage)?,
            captured_at_ms: pending
                .captured_at_ms
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
        };
        cap.recheck_captured_bootstrap_at_native_time(now)?;
        Ok(cap)
    }
    pub(super) fn require_bootstrap_capture(
        &self,
        captured_at: u64,
        approval_digest: DigestV1,
        authorization_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let p = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let original = p
            .approved
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if p.accepted_at_ms
            .is_none_or(|accepted| captured_at < accepted)
            || original.digest() != approval_digest
            || p.challenge.subject.operation_kind != KagemushaOperationKindV1::Bootstrap
            || p.challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
            || p.challenge.subject.secure_index_before != 0
            || p.challenge.subject.secure_index_after != 0
            || p.challenge.subject.candidate_envelope_digest != [0; 32]
            || p.challenge.subject.terminal_body_commitment != [0; 32]
            || original.challenge() != &p.challenge
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match (&p.reserve_integrity_lease, &p.approval_integrity_lease) {
            (None, None) => {}
            (Some(a), Some(b)) if Arc::ptr_eq(a, b) => {}
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        }
        let expected = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            original.proof_binding_digest(),
            p.approval_integrity_lease.as_ref().map(|l| l.digest()),
        )
        .map_err(material)?;
        if expected != authorization_digest {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_bootstrap_challenge(&p.challenge, p.reserve_integrity_lease.as_deref())?;
        original
            .recheck_at_trusted_time(captured_at)
            .map_err(material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        kagemusha::KagemushaAppOperationApprovalEvidenceV1,
        testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn journal(
        fixture: &KagemushaOrdinaryRetailEnrollmentFixtureV1,
        path: &Path,
    ) -> KagemushaOrdinaryLogicalApprovalJournalV1 {
        let enrollment = Arc::new(fixture.verify(300).unwrap());
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            enrollment.as_ref(),
            Arc::clone(&fixture.release),
        )
        .unwrap();
        let (_, bootstrap) = derive_preview(
            &floor,
            [43; 32],
            KagemushaDurableCapacityV1 {
                inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
                outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
            },
        )
        .unwrap();
        let challenge =
            derive_challenge_from_floor(&floor, &bootstrap, [44; 32], [45; 32], 300).unwrap();
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let signature: Signature = key.sign(&challenge.canonical_signing_bytes().unwrap());
        let approval = KagemushaAppOperationApprovalV1 {
            challenge,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        };
        let verified = approval
            .authenticate(&challenge, floor.credential(), None, 301)
            .unwrap();
        let mut wal = PrivateJournal::create_new(path, FORMAT).unwrap();
        for record in [
            initial_record(enrollment.as_ref(), &fixture.release, &bootstrap).unwrap(),
            Record::Reserve {
                challenge,
                integrity_lease_digest: None,
            },
            Record::Approval {
                accepted_at_ms: 301,
                integrity_lease_digest: None,
                original: verified.original().to_vec(),
                counter_floor_before: None,
                accepted_counter: None,
            },
        ] {
            wal.append(&norito::encode_canonical(&record).unwrap())
                .unwrap();
        }
        drop(floor);
        // Genuine model originals and owned real WAL only; no production recursive verifier,
        // financial owner, actual device qualification, or monetary grant is manufactured.
        KagemushaOrdinaryLogicalApprovalJournalV1 {
            wal,
            enrollment,
            release: Arc::clone(&fixture.release),
            bootstrap,
            pending: Some(Pending {
                accepted_at_ms: Some(301),
                captured_at_ms: None,
                counter_floor_before: None,
                challenge,
                approved: Some(verified),
                approval_integrity_lease: None,
                reserve_integrity_lease: None,
            }),
            counter_floor: None,
            integrity_lease: None,
        }
    }
    fn refresh(
        fixture: &KagemushaOrdinaryRetailEnrollmentFixtureV1,
        journal: &mut KagemushaOrdinaryLogicalApprovalJournalV1,
    ) {
        let (challenge, lease) = fixture.integrity_refresh_originals();
        let issuer =
            iroha_crypto::KeyPair::from_seed(vec![61; 32], iroha_crypto::Algorithm::Ed25519);
        let lease = Arc::new(
            lease
                .authenticate(
                    journal.enrollment.app_credential(),
                    &fixture.release,
                    &fixture.trust,
                    &fixture.app_authority,
                    &challenge,
                    issuer.public_key(),
                    1500,
                )
                .unwrap(),
        );
        journal.retain_integrity_lease(lease, 1500).unwrap();
    }
    #[test]
    fn captured_bootstrap_survives_slow_proving_without_renewing_live_approval() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let tmp = tempfile::tempdir().unwrap();
        let mut journal = journal(&fixture, &tmp.path().join("capture"));
        assert_eq!(
            journal
                .approved_at_trusted_time(302)
                .unwrap()
                .previous_app_attest_counter_floor(),
            None,
        );
        journal
            .persist_bootstrap_capture_at_checked_native_time(302)
            .unwrap();
        let original = journal
            .captured_bootstrap_at_native_time(302)
            .unwrap()
            .original()
            .to_vec();
        let binding = journal
            .captured_bootstrap_at_native_time(302)
            .unwrap()
            .authorization_binding_digest()
            .unwrap();
        refresh(&fixture, &mut journal);
        assert!(journal.approved_at_trusted_time(1500).is_err());
        let cap = journal.captured_bootstrap_at_native_time(1500).unwrap();
        assert_eq!(cap.captured_at_ms(), 302);
        assert_eq!(cap.previous_app_attest_counter_floor(), None);
        assert_eq!(cap.original(), original);
        assert_eq!(cap.authorization_binding_digest().unwrap(), binding);
        assert!(cap.original_approval_integrity_lease().is_none());
        assert!(cap.recheck_captured_bootstrap_at_native_time(301).is_err());
        assert!(cap.recheck_captured_bootstrap_at_native_time(2400).is_err());
        drop(cap);
        let before = journal.wal.recovery_prefix().unwrap();
        journal
            .persist_bootstrap_capture_at_checked_native_time(1500)
            .unwrap();
        assert_eq!(
            journal.wal.recovery_prefix().unwrap(),
            before,
            "retry does not recapture or append"
        );
    }
    #[test]
    fn first_capture_requires_original_live_interval_and_exact_native_admission() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let tmp = tempfile::tempdir().unwrap();
        let mut journal = journal(&fixture, &tmp.path().join("expired"));
        let prefix = journal.wal.recovery_prefix().unwrap();
        assert!(
            journal
                .persist_bootstrap_capture_at_checked_native_time(300)
                .is_err()
        );
        assert!(
            journal
                .persist_bootstrap_capture_at_checked_native_time(1200)
                .is_err()
        );
        refresh(&fixture, &mut journal);
        assert!(
            journal
                .persist_bootstrap_capture_at_checked_native_time(1500)
                .is_err()
        );
        assert!(journal.pending.as_ref().unwrap().captured_at_ms.is_none());
        assert_ne!(
            journal.wal.recovery_prefix().unwrap(),
            prefix,
            "only genuine refresh changed prefix"
        );
        assert!(journal.captured_bootstrap_at_native_time(1500).is_err());
    }
    #[test]
    fn capture_fsync_retains_the_complete_original_and_descriptor_prefix() {
        use std::io::Write as _;
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("durable");
        let mut journal = journal(&fixture, &path);
        assert!(journal.captured_bootstrap_at_native_time(302).is_err());
        journal
            .persist_bootstrap_capture_at_checked_native_time(302)
            .unwrap();
        let cap = journal.captured_bootstrap_at_native_time(303).unwrap();
        let digest = cap.digest();
        let binding = cap.authorization_binding_digest().unwrap();
        let original = cap.original().to_vec();
        drop(cap);
        let prefix = journal.wal.recovery_prefix().unwrap();
        let mut reader = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        let mut originals = Vec::new();
        while let Some((_, bytes)) = reader.replay_next().unwrap() {
            originals.push(norito::decode_canonical::<Record>(&bytes).unwrap());
        }
        assert_eq!(reader.recovery_prefix().unwrap(), prefix);
        assert_eq!(originals.len(), 4);
        assert!(matches!(&originals[2],Record::Approval{original:raw,..} if raw==&original));
        assert_eq!(
            originals[3],
            Record::CaptureBootstrap {
                captured_at_ms: 302,
                approval_digest: digest,
                authorization_binding_digest: binding
            }
        );
        let cap = journal.captured_bootstrap_at_native_time(303).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(path.join(FORMAT.filename))
            .unwrap()
            .write_all(&[1])
            .unwrap();
        assert!(cap.recheck_captured_bootstrap_at_native_time(304).is_err());
    }
}
