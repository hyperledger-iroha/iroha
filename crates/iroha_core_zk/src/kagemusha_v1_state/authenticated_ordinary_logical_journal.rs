//! Descriptor-held ordinary app approval attempts and logical replay protection.
//!
//! This WAL is native software custody. It does not claim a hardware counter, non-forking
//! checkpoint or offline rollback resistance. It cannot create a monetary owner. An actual
//! financial selection derives each subject before it is fsynced; paired State/Guard verification
//! and the native financial publication remain separate from this signature admission.

use super::super::super::private_journal::{
    PrivateJournal, PrivateJournalError, PrivateJournalFormat,
};
use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1,
    KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1, KagemushaAppOperationApprovalChallengeV1,
    KagemushaAppOperationApprovalPurposeV1, KagemushaAppOperationApprovalV1,
    KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
    KagemushaVerifiedAppOperationApprovalV1,
};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-approval.norito.wal",
    magic: b"IKGOAJ1\0",
    hash_domain: b"iroha:kagemusha:v1:ordinary-logical-approval-frame\0",
    maximum_payload_bytes: 64 * 1024,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryApprovalJournalRecordV1")]
enum Record {
    Initialize {
        enrollment: KagemushaRecoveryEnrollmentBindingV1,
        original_certificate: Vec<u8>,
        credential: KagemushaAcceptedCredentialFloorV1,
        bootstrap: BootstrapStatementV1,
        normalized_guard_digest: DigestV1,
    },
    Reserve(KagemushaAppOperationApprovalChallengeV1),
    Approval {
        original: Vec<u8>,
        counter_floor_before: Option<u32>,
        accepted_counter: Option<u32>,
    },
}

struct Pending {
    challenge: KagemushaAppOperationApprovalChallengeV1,
    approved: Option<KagemushaVerifiedAppOperationApprovalV1>,
}

/// One descriptor-owned native logical approval journal under genuine ordinary enrollment.
/// It cannot be serialized, cloned, or opened with a response-selected credential or subject.
pub struct KagemushaOrdinaryLogicalApprovalJournalV1 {
    wal: PrivateJournal,
    enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    bootstrap: BootstrapPreviewV1,
    pending: Option<Pending>,
    counter_floor: Option<u32>,
}

/// Borrowed original signature admission under the still-held durable native attempt.
/// This is not a monetary authorization or a hardware checkpoint capability.
#[allow(missing_copy_implementations)]
pub struct KagemushaAuthenticatedOrdinaryApprovalV1<'a> {
    original: &'a KagemushaVerifiedAppOperationApprovalV1,
    journal: &'a KagemushaOrdinaryLogicalApprovalJournalV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl KagemushaAuthenticatedOrdinaryApprovalV1<'_> {
    /// Check original descriptor/generation and the original exclusive approval interval.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), KagemushaStateErrorV1> {
        self.journal.recheck_at_trusted_time(now)?;
        if self.journal.wal.recovery_prefix().map_err(storage)? != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.original
            .recheck_at_trusted_time(now)
            .map_err(material)?;
        self.journal.recheck_at_trusted_time(now)
    }

    /// Actual native reserved challenge; no raw subject overload constructs this admission.
    pub fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        self.original.challenge()
    }
    /// Full canonical original, retaining the actual DER or Apple CBOR byte-for-byte.
    pub fn original(&self) -> &[u8] {
        self.original.original()
    }
    /// Same-original digest that the complete paired Guard must bind as a public input.
    pub fn digest(&self) -> DigestV1 {
        self.original.digest()
    }
    /// Independently advancing Apple assertion counter; never a financial logical index.
    pub fn app_attest_counter(&self) -> Option<u32> {
        self.original.app_attest_counter()
    }
}

impl KagemushaOrdinaryLogicalApprovalJournalV1 {
    /// Initialize only from the actual genuine issuer/recursive-verifier bootstrap selection.
    /// Existing paths are never reset. No Core wallet, state publication, or money is created.
    pub fn create_new(
        path: &Path,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        now: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        selection.recheck_at_trusted_time(now)?;
        let (release, bootstrap) = retain_selected_originals(selection, &enrollment)?;
        let record = initial_record(&enrollment, &release, &bootstrap)?;
        let mut journal = Self {
            wal: PrivateJournal::create_new(path, FORMAT).map_err(storage)?,
            counter_floor: enrollment.possession().app_attest_counter(),
            enrollment,
            release,
            bootstrap,
            pending: None,
        };
        journal.persist(&record)?;
        selection.recheck_at_trusted_time(now)?;
        Ok(journal)
    }

    /// Reopen the original complete prefix under the same genuine native selection.
    /// Original approvals are checked at their original admission interval, without renewal;
    /// current use must separately pass `approved_at_trusted_time` at actual native time.
    pub fn open_existing(
        path: &Path,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        now: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        selection.credential_floor().recheck_at_trusted_time(now)?;
        let (release, bootstrap) = retain_selected_originals(selection, &enrollment)?;
        let expected = initial_record(&enrollment, &release, &bootstrap)?;
        let mut journal = Self {
            wal: PrivateJournal::open_existing(path, FORMAT).map_err(storage)?,
            counter_floor: enrollment.possession().app_attest_counter(),
            enrollment,
            release,
            bootstrap,
            pending: None,
        };
        let mut initialized = false;
        while let Some((sequence, payload)) = journal.wal.replay_next().map_err(storage)? {
            let record: Record = norito::decode_canonical(&payload).map_err(material)?;
            if norito::encode_canonical(&record).map_err(material)? != payload {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            match record {
                Record::Initialize { .. } if sequence == 0 && !initialized => {
                    if record != expected {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                    initialized = true;
                }
                Record::Reserve(challenge) if initialized && journal.pending.is_none() => {
                    journal.require_bootstrap_challenge(&challenge)?;
                    journal.pending = Some(Pending {
                        challenge,
                        approved: None,
                    });
                }
                Record::Approval {
                    original,
                    counter_floor_before,
                    accepted_counter,
                } if initialized => {
                    if counter_floor_before != journal.counter_floor {
                        return Err(KagemushaStateErrorV1::SnapshotRollback);
                    }
                    let verified = journal.authenticate_original(&original, true)?;
                    if verified.app_attest_counter() != accepted_counter {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                    journal.counter_floor = accepted_counter.or(journal.counter_floor);
                    journal
                        .pending
                        .as_mut()
                        .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                        .approved = Some(verified);
                }
                _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            }
        }
        if !initialized {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        journal.recheck_at_trusted_time(now)?;
        Ok(journal)
    }

    /// Reserve one bootstrap approval before exposing any signing bytes. Native entropy chooses
    /// the nonce; an exact retry keeps the original nonce, interval and complete subject.
    /// The actual provisioner supplies time, not a managed request or phone clock claim.
    pub fn reserve_bootstrap(
        &mut self,
        operation_id: DigestV1,
        now: u64,
    ) -> Result<&KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(now)?;
        if operation_id == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if let Some(pending) = &self.pending {
            if pending.challenge.operation_id != operation_id
                || now >= pending.challenge.expires_at_ms
            {
                return Err(KagemushaStateErrorV1::SnapshotRollback);
            }
        } else {
            // A new bootstrap attempt still requires the original possession interval.
            self.enrollment
                .possession()
                .recheck_at_trusted_time(now)
                .map_err(material)?;
            let mut nonce = [0; 32];
            OsRng.try_fill_bytes(&mut nonce).map_err(material)?;
            if nonce == [0; 32] {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let challenge = self.bootstrap_challenge(operation_id, nonce, now)?;
            self.persist(&Record::Reserve(challenge))?;
            self.pending = Some(Pending {
                challenge,
                approved: None,
            });
        }
        self.recheck_at_trusted_time(now)?;
        Ok(&self
            .pending
            .as_ref()
            .expect("reserved pending exists")
            .challenge)
    }

    /// Verify and fsync the actual original platform evidence under the retained native challenge.
    /// A mismatch or expired unsigned attempt consumes neither its nonce nor its retained floor.
    pub fn accept_original(
        &mut self,
        original: &[u8],
        now: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(now)?;
        if original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if let Some(approved) = &pending.approved {
            if approved.original() != original {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            approved.recheck_at_trusted_time(now).map_err(material)?;
            return self.recheck_at_trusted_time(now);
        }
        let verified = self.authenticate_original_at(original, now)?;
        let accepted_counter = verified.app_attest_counter();
        self.persist(&Record::Approval {
            original: original.to_vec(),
            counter_floor_before: self.counter_floor,
            accepted_counter,
        })?;
        self.counter_floor = accepted_counter.or(self.counter_floor);
        self.pending
            .as_mut()
            .expect("authenticated pending exists")
            .approved = Some(verified);
        self.recheck_at_trusted_time(now)
    }

    /// Borrow only an already-fsynced original. Monetary proving must bind the same full original,
    /// genuine financial selection and logical predecessor/successor; this alone cannot spend.
    pub fn approved_at_trusted_time(
        &self,
        now: u64,
    ) -> Result<KagemushaAuthenticatedOrdinaryApprovalV1<'_>, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(now)?;
        let original = self
            .pending
            .as_ref()
            .and_then(|pending| pending.approved.as_ref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        original.recheck_at_trusted_time(now).map_err(material)?;
        Ok(KagemushaAuthenticatedOrdinaryApprovalV1 {
            original,
            journal: self,
            prefix: self.wal.recovery_prefix().map_err(storage)?,
        })
    }

    /// Recheck the retained actual enrollment, release, financial preview and descriptor custody.
    /// Reopening the journal does not refresh a credential, approval interval or Apple floor.
    pub fn recheck_at_trusted_time(&self, now: u64) -> Result<(), KagemushaStateErrorV1> {
        self.wal.check_owned().map_err(storage)?;
        let floor = self.credential_floor()?;
        floor.recheck_at_trusted_time(now)?;
        floor.validate_current(&self.bootstrap.state)?;
        self.wal.check_owned().map_err(storage)
    }

    pub(crate) fn retained_enrollment(
        &self,
    ) -> &Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1> {
        &self.enrollment
    }

    pub(crate) fn retained_release(&self) -> &Arc<KagemushaAuthenticatedReleaseV1> {
        &self.release
    }

    pub(crate) fn bootstrap_preview(&self) -> &BootstrapPreviewV1 {
        &self.bootstrap
    }

    fn credential_floor(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryCredentialFloorV1<'_>, KagemushaStateErrorV1> {
        KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            self.enrollment.as_ref(),
            Arc::clone(&self.release),
        )
    }
    fn persist(&mut self, record: &Record) -> Result<(), KagemushaStateErrorV1> {
        let bytes = norito::encode_canonical(record).map_err(material)?;
        if bytes.len() > FORMAT.maximum_payload_bytes as usize {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.wal.append(&bytes).map_err(storage)
    }
    fn bootstrap_challenge(
        &self,
        operation: DigestV1,
        nonce: DigestV1,
        issued: u64,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        derive_challenge_from_floor(
            &self.credential_floor()?,
            &self.bootstrap,
            operation,
            nonce,
            issued,
        )
    }
    fn require_bootstrap_challenge(
        &self,
        challenge: &KagemushaAppOperationApprovalChallengeV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let expected = self.bootstrap_challenge(
            challenge.operation_id,
            challenge.nonce,
            challenge.issued_at_ms,
        )?;
        if expected != *challenge {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
    fn authenticate_original(
        &self,
        original: &[u8],
        replay: bool,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !replay || pending.approved.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.authenticate_original_at(original, pending.challenge.issued_at_ms)
    }
    fn authenticate_original_at(
        &self,
        original: &[u8],
        now: u64,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
        if original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let decoded: KagemushaAppOperationApprovalV1 =
            norito::decode_canonical(original).map_err(material)?;
        if norito::encode_canonical(&decoded).map_err(material)? != original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.require_bootstrap_challenge(&pending.challenge)?;
        decoded
            .authenticate(
                &pending.challenge,
                self.enrollment.app_credential(),
                self.counter_floor,
                now,
            )
            .map_err(material)
    }
}

fn retain_selected_originals(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    enrollment: &Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
) -> Result<(Arc<KagemushaAuthenticatedReleaseV1>, BootstrapPreviewV1), KagemushaStateErrorV1> {
    // Equality of decoded fields is insufficient: retain the same actual admitted capability.
    require_selected_enrollment_identity(enrollment, selection.enrollment())?;
    let release = selection.authenticated_release()?;
    let bootstrap = selection.preview()?.clone();
    let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
        enrollment.as_ref(),
        Arc::clone(&release),
    )?;
    floor.validate_current(&bootstrap.state)?;
    Ok((release, bootstrap))
}

fn require_selected_enrollment_identity(
    retained: &Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    selected: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
) -> Result<(), KagemushaStateErrorV1> {
    if std::ptr::eq(retained.as_ref(), selected) {
        Ok(())
    } else {
        Err(KagemushaStateErrorV1::SnapshotIntegrity)
    }
}

fn initial_record(
    enrollment: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    release: &Arc<KagemushaAuthenticatedReleaseV1>,
    preview: &BootstrapPreviewV1,
) -> Result<Record, KagemushaStateErrorV1> {
    let retail = &enrollment.certificate().subject;
    let binding = KagemushaRecoveryEnrollmentBindingV1 {
        enrollment_id: retail.enrollment_id,
        owner: retail.owner.clone(),
        core_authorization_key_reference: retail.issuance.core_authorization_key_reference,
    };
    binding.validate_for_state(&preview.state)?;
    let original_certificate =
        norito::encode_canonical(enrollment.certificate()).map_err(material)?;
    if original_certificate.len() > KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
        enrollment,
        Arc::clone(release),
    )?;
    Ok(Record::Initialize {
        enrollment: binding,
        original_certificate,
        credential: floor.checkpoint_floor()?,
        bootstrap: preview.statement.clone(),
        normalized_guard_digest: preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(material)?,
    })
}

fn derive_challenge_from_floor(
    floor: &KagemushaAuthenticatedOrdinaryCredentialFloorV1<'_>,
    preview: &BootstrapPreviewV1,
    operation: DigestV1,
    nonce: DigestV1,
    issued: u64,
) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
    floor.validate_current(&preview.state)?;
    let c = floor.credential();
    let epoch = floor.financial_epoch()?;
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: preview.state.release_id,
        provider_policy_root: preview.state.device_policy_binding.hardware_policy_id,
        app_policy_digest: c.static_binding_digest(),
        credential_id: c.digest(),
        network_id: preview.state.lane.network_id,
        lane_commitment: preview.state.lane.device_lane_id,
        hardware_profile_id: preview.state.hardware_profile_id,
        policy_epoch: preview.state.policy_epoch,
        hardware_epoch_id: epoch.epoch_id,
        hardware_epoch_generation: u64::try_from(epoch.generation).map_err(material)?,
        operation_kind: KagemushaOperationKindV1::Bootstrap,
        transition_statement_digest: preview.statement.proof_statement_digest()?,
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: 0,
        secure_index_after: 0,
    };
    let expires = issued
        .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
        .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
        .min(floor.enrollment.certificate().subject.expires_at_ms)
        .min(c.subject().expires_at_ms);
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: operation,
        nonce,
        account_binding: c.subject().account_binding,
        authority_policy_digest: c.subject().app_authority_policy_digest,
        attested_key_id: c.subject().attested_key_id,
        enrollment_digest: c.digest(),
        subject_signing_digest: Sha256::digest(
            subject.canonical_signing_bytes().map_err(material)?,
        )
        .into(),
        normalized_guard_digest: preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(material)?,
        issued_at_ms: issued,
        expires_at_ms: expires,
        subject,
    };
    challenge.canonical_signing_bytes().map_err(material)?;
    Ok(challenge)
}
fn storage(error: PrivateJournalError) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}
fn material(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        kagemusha::KagemushaAppOperationApprovalEvidenceV1,
        testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn capacity() -> KagemushaDurableCapacityV1 {
        KagemushaDurableCapacityV1 {
            inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        }
    }
    fn sign(
        challenge: KagemushaAppOperationApprovalChallengeV1,
        apple: bool,
        counter: u32,
    ) -> KagemushaAppOperationApprovalV1 {
        // Known-public synthetic key matches the actual model fixture, never native provision.
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let message = challenge.canonical_signing_bytes().unwrap();
        let evidence = if apple {
            let mut auth = [0; 37];
            auth[..32].copy_from_slice(&[2; 32]);
            auth[32] = 0x40;
            auth[33..].copy_from_slice(&counter.to_be_bytes());
            let mut nonce = Sha256::new();
            nonce.update(auth);
            nonce.update(Sha256::digest(message));
            let signature: Signature = key.sign(&nonce.finalize());
            let der = signature.to_der();
            let mut raw = vec![0xa2, 0x69];
            raw.extend_from_slice(b"signature");
            raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
            raw.extend_from_slice(der.as_bytes());
            raw.push(0x71);
            raw.extend_from_slice(b"authenticatorData");
            raw.extend_from_slice(&[0x58, 37]);
            raw.extend(auth);
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
        } else {
            let signature: Signature = key.sign(&message);
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            }
        };
        KagemushaAppOperationApprovalV1 {
            challenge,
            evidence,
        }
    }
    fn check(
        apple: bool,
        run: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryCredentialFloorV1<'_>,
            &BootstrapPreviewV1,
            KagemushaAppOperationApprovalChallengeV1,
        ),
    ) {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let enrollment = fixture.verify(300).unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            &enrollment,
            Arc::clone(&fixture.release),
        )
        .unwrap();
        let (_, preview) = derive_preview(&floor, [43; 32], capacity()).unwrap();
        let expected =
            derive_challenge_from_floor(&floor, &preview, [44; 32], [45; 32], 300).unwrap();
        run(&floor, &preview, expected);
    }
    #[test]
    fn retained_enrollment_requires_the_same_actual_admitted_capability() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let selected = Arc::new(fixture.verify(300).unwrap());
        let independently_reopened = Arc::new(fixture.verify(300).unwrap());
        assert_eq!(selected.certificate(), independently_reopened.certificate());
        assert!(require_selected_enrollment_identity(&selected, selected.as_ref()).is_ok());
        assert!(
            require_selected_enrollment_identity(&independently_reopened, selected.as_ref())
                .is_err()
        );
    }

    #[test]
    fn initialized_prefix_pins_the_complete_original_fi_certificate() {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let enrollment = fixture.verify(300).unwrap();
        let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
            &enrollment,
            Arc::clone(&fixture.release),
        )
        .unwrap();
        let (_, preview) = derive_preview(&floor, [43; 32], capacity()).unwrap();
        let original = initial_record(&enrollment, &fixture.release, &preview).unwrap();
        let expected = norito::encode_canonical(enrollment.certificate()).unwrap();
        let mut changed = original.clone();
        match &mut changed {
            Record::Initialize {
                original_certificate,
                ..
            } => {
                assert_eq!(original_certificate, &expected);
                *original_certificate.last_mut().unwrap() ^= 1;
            }
            _ => unreachable!(),
        }
        assert_ne!(changed, original);
        assert!(
            norito::encode_canonical(&original).unwrap().len()
                < FORMAT.maximum_payload_bytes as usize
        );
    }

    #[test]
    fn genuine_key_approves_exact_native_bootstrap_and_independent_apple_counter_gap() {
        for apple in [false, true] {
            check(apple, |floor, preview, expected| {
                let original = sign(expected, apple, 17);
                let verified = original
                    .authenticate(
                        &expected,
                        floor.credential(),
                        floor.app_attest_counter_floor(),
                        301,
                    )
                    .unwrap();
                assert_eq!(verified.challenge().subject.secure_index_before, 0);
                assert_eq!(verified.challenge().subject.secure_index_after, 0);
                assert_eq!(
                    verified.challenge().normalized_guard_digest,
                    preview
                        .normalized_guard_statement
                        .canonical_digest()
                        .unwrap()
                );
                assert_eq!(
                    verified.app_attest_counter(),
                    if apple { Some(17) } else { None }
                );
                assert_eq!(
                    verified.original(),
                    norito::encode_canonical(&original).unwrap()
                );
            });
        }
    }
    #[test]
    fn substituted_nonce_operation_subject_or_normalized_guard_cannot_consume_original_attempt() {
        check(false, |floor, _, expected| {
            for field in 0..5 {
                let mut changed = expected;
                match field {
                    0 => changed.nonce[0] ^= 1,
                    1 => changed.operation_id[0] ^= 1,
                    2 => changed.normalized_guard_digest[0] ^= 1,
                    3 => changed.subject.transition_statement_digest[0] ^= 1,
                    _ => changed.enrollment_digest[0] ^= 1,
                }
                changed.subject_signing_digest =
                    Sha256::digest(changed.subject.canonical_signing_bytes().unwrap()).into();
                let foreign = sign(changed, false, 0);
                assert!(
                    foreign
                        .authenticate(&expected, floor.credential(), None, 301)
                        .is_err()
                );
            }
            assert!(
                sign(expected, false, 0)
                    .authenticate(&expected, floor.credential(), None, 301)
                    .is_ok()
            );
        });
    }
    #[test]
    fn expired_original_never_renews_nonce_or_interval() {
        check(false, |floor, _, expected| {
            let original = sign(expected, false, 0);
            assert!(
                original
                    .authenticate(&expected, floor.credential(), None, expected.expires_at_ms)
                    .is_err()
            );
            let mut renewed = expected;
            renewed.issued_at_ms += 1;
            renewed.expires_at_ms += 1;
            assert!(
                sign(renewed, false, 0)
                    .authenticate(&expected, floor.credential(), None, 301)
                    .is_err()
            );
            let verified = original
                .authenticate(&expected, floor.credential(), None, 301)
                .unwrap();
            assert!(
                verified
                    .recheck_at_trusted_time(expected.expires_at_ms)
                    .is_err()
            );
            assert_eq!(verified.challenge(), &expected);
        });
    }
    #[test]
    fn exact_descriptor_prefix_retains_full_original_and_refuses_atomic_replacement() {
        check(false, |floor, _, expected| {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("ordinary");
            let original = sign(expected, false, 0);
            let verified = original
                .authenticate(&expected, floor.credential(), None, 301)
                .unwrap();
            let records = [
                Record::Reserve(expected),
                Record::Approval {
                    original: verified.original().to_vec(),
                    counter_floor_before: None,
                    accepted_counter: None,
                },
            ];
            let mut wal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            for record in &records {
                wal.append(&norito::encode_canonical(record).unwrap())
                    .unwrap();
            }
            let prefix = wal.recovery_prefix().unwrap();
            drop(wal);
            let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
            for expected_record in &records {
                let (_, raw) = reopened.replay_next().unwrap().unwrap();
                let actual: Record = norito::decode_canonical(&raw).unwrap();
                assert_eq!(&actual, expected_record);
            }
            assert!(reopened.replay_next().unwrap().is_none());
            assert_eq!(reopened.recovery_prefix().unwrap(), prefix);
            let file = path.join(FORMAT.filename);
            let replacement = path.join("replacement");
            std::fs::copy(&file, &replacement).unwrap();
            std::fs::rename(&replacement, &file).unwrap();
            assert!(reopened.check_owned().is_err());
        });
    }
    #[test]
    fn owned_original_tamper_cannot_preserve_a_valid_prefix() {
        use std::io::{Seek as _, Write as _};
        check(false, |floor, _, expected| {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("ordinary");
            let original = sign(expected, false, 0)
                .authenticate(&expected, floor.credential(), None, 301)
                .unwrap();
            let mut wal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            wal.append(
                &norito::encode_canonical(&Record::Approval {
                    original: original.original().to_vec(),
                    counter_floor_before: None,
                    accepted_counter: None,
                })
                .unwrap(),
            )
            .unwrap();
            drop(wal);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(path.join(FORMAT.filename))
                .unwrap();
            file.seek(std::io::SeekFrom::Start(100)).unwrap();
            file.write_all(&[0xff]).unwrap();
            file.sync_all().unwrap();
            let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
            assert!(reopened.replay_next().is_err());
        });
    }
}
