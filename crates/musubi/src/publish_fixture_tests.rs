// Persistence fixtures and journal tests included from the parent module.
use std::{collections::VecDeque, io::Cursor};

#[cfg(unix)]
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use iroha::{
    crypto::{Algorithm, KeyPair, SignatureOf},
    data_model::{
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiArchiveAvailabilityV1,
            MusubiArtifactGovernanceStateV1, MusubiKotodamaEditionV1, MusubiPackageIdV1,
            MusubiPackageScopeV1, MusubiProviderBundleVerificationApprovalV1,
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
            MusubiReasonV1, MusubiReleaseIdV1, MusubiReleaseManifestV1, MusubiReleaseMetadataV1,
            MusubiReleaseRevisionsV1, MusubiReleaseSelectionStateV1, MusubiReleaseYankV1,
            MusubiResolutionProofV1, MusubiSeedIngressReceiptApprovalV1,
            MusubiSeedIngressReceiptPayloadV1, MusubiVerificationLockV1, MusubiVersionV1,
        },
        nexus::DataSpaceId,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionAuthorityV1,
            ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    },
};
use tempfile::tempdir;

use super::*;

struct BytesSource(Vec<u8>);

impl PublicationCarSource for BytesSource {
    fn open_car(&self) -> io::Result<Box<dyn Read + '_>> {
        Ok(Box::new(Cursor::new(self.0.as_slice())))
    }
}

#[test]
fn staged_car_source_reopens_only_the_exact_operation_file() {
    let state = tempdir().expect("state root");
    fs::create_dir(state.path().join(JOURNAL_DIRECTORY)).expect("publication directory");
    let operation_id = "0101010101010101010101010101010101010101010101010101010101010101"
        .parse()
        .expect("operation id");
    let source = PublicationStagedCarSourceV1::new(state.path(), operation_id, 4);
    fs::write(source.path(), b"car!").expect("stage fixture CAR");

    let mut bytes = Vec::new();
    source
        .open_car()
        .expect("open exact CAR")
        .read_to_end(&mut bytes)
        .expect("read exact CAR");
    assert_eq!(bytes, b"car!");

    let wrong_size = PublicationStagedCarSourceV1::new(state.path(), operation_id, 5);
    assert_eq!(
        wrong_size
            .open_car()
            .err()
            .expect("wrong length must fail")
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn staged_car_reader_rejects_hard_links_and_in_place_growth() {
    let state = tempdir().expect("state root");
    fs::create_dir(state.path().join(JOURNAL_DIRECTORY)).expect("publication directory");
    let operation_id = "0404040404040404040404040404040404040404040404040404040404040404"
        .parse()
        .expect("operation id");
    let source = PublicationStagedCarSourceV1::new(state.path(), operation_id, 4);
    fs::write(source.path(), b"car!").expect("stage fixture CAR");
    let linked = state.path().join("linked.car");
    fs::hard_link(source.path(), &linked).expect("create hard link");
    assert_eq!(
        source
            .open_car()
            .err()
            .expect("hard-linked source rejected")
            .kind(),
        io::ErrorKind::InvalidData
    );
    fs::remove_file(linked).expect("remove fixture hard link");

    let mut reader = source.open_car().expect("open exact CAR");
    let mut prefix = [0_u8; 2];
    reader.read_exact(&mut prefix).expect("read prefix");
    OpenOptions::new()
        .append(true)
        .open(source.path())
        .expect("open source for mutation")
        .write_all(b"x")
        .expect("grow source");
    let mut remainder = Vec::new();
    let error = reader
        .read_to_end(&mut remainder)
        .expect_err("in-place growth rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[cfg(unix)]
#[test]
fn staged_car_bytes_are_commitment_checked_and_idempotent() {
    let state = tempdir().expect("state root");
    let _store = PublicationJournalStore::open(state.path()).expect("private journal store");
    let operation_id = "0202020202020202020202020202020202020202020202020202020202020202"
        .parse()
        .expect("operation id");
    let bytes = b"canonical-car";
    let expected_size = u64::try_from(bytes.len()).expect("fixture length fits u64");
    let digest = MusubiContentDigestV1::new(*blake3::hash(bytes).as_bytes());
    let source = PublicationStagedCarSourceV1::stage_bytes(
        state.path(),
        operation_id,
        expected_size,
        digest,
        bytes,
    )
    .expect("stage committed CAR");
    PublicationStagedCarSourceV1::stage_bytes(
        state.path(),
        operation_id,
        expected_size,
        digest,
        bytes,
    )
    .expect("identical retry reuses staged CAR");

    fs::write(source.path(), b"substituted!!").expect("substitute same-length fixture");
    assert!(matches!(
        PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            expected_size,
            digest,
            bytes,
        ),
        Err(PublicationError::InvalidJournal(_))
    ));

    let other_id = "0303030303030303030303030303030303030303030303030303030303030303"
        .parse()
        .expect("other operation id");
    assert!(matches!(
        PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            other_id,
            expected_size,
            MusubiContentDigestV1::new([9; 32]),
            bytes,
        ),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Validation,
            ..
        })
    ));
    assert!(
        !PublicationStagedCarSourceV1::new(state.path(), other_id, expected_size)
            .path()
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn journal_decode_rejects_trailing_bare_and_oversized_frames() {
    let state = tempdir().expect("state root");
    let store = PublicationJournalStore::open(state.path()).expect("journal store");
    let (request, _) = request();
    let operation_id = request.operation_id();
    let journal = store.create(request).expect("create canonical journal");
    let path = state.path().join(journal_relative_path(operation_id));
    let canonical = fs::read(&path).expect("read canonical journal");
    assert_eq!(
        decode_publication_journal(&canonical).expect("decode canonical journal"),
        journal
    );

    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open journal for trailing-byte injection")
        .write_all(&[0])
        .expect("append trailing byte");
    assert!(matches!(
        store.load(operation_id),
        Err(PublicationError::InvalidJournal(_))
    ));

    store
        .root
        .replace(&journal_relative_path(operation_id), &journal.encode())
        .expect("replace with legacy bare encoding");
    assert!(matches!(
        store.load(operation_id),
        Err(PublicationError::InvalidJournal(_))
    ));

    let oversized = vec![0_u8; MAX_JOURNAL_BYTES_USIZE + 1];
    assert!(matches!(
        decode_publication_journal(&oversized),
        Err(PublicationError::InvalidJournal(_))
    ));
}

#[cfg(unix)]
#[test]
fn operation_lock_is_private_exclusive_and_rejects_hard_links() {
    let state = tempdir().expect("state root");
    let store = PublicationJournalStore::open(state.path()).expect("journal store");
    let (request, _) = request();
    let operation_id = request.operation_id();
    store.create(request).expect("create journal");
    let lock_path = state
        .path()
        .join(operation_lock_relative_path(operation_id));
    let metadata = fs::symlink_metadata(&lock_path).expect("operation lock metadata");
    assert_eq!(metadata.len(), 0);
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);

    let held = store
        .lock_operation(operation_id)
        .expect("hold operation lock");
    let second = PublicationJournalStore::open(state.path()).expect("second journal store");
    assert!(matches!(
        second.lock_operation(operation_id),
        Err(PublicationError::ConcurrentJournalUpdate)
    ));
    held.finish(Ok(())).expect("release operation lock");

    let hard_link = state.path().join("operation-lock-hard-link");
    fs::hard_link(&lock_path, &hard_link).expect("link operation lock");
    assert!(matches!(
        store.lock_operation(operation_id),
        Err(PublicationError::InvalidJournal(_))
    ));
    fs::remove_file(hard_link).expect("remove fixture hard link");

    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o4600))
        .expect("add set-user-ID bit to operation lock");
    assert!(matches!(
        store.lock_operation(operation_id),
        Err(PublicationError::InvalidJournal(_))
    ));
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
        .expect("restore operation lock permissions");
}

#[cfg(unix)]
#[test]
fn concurrent_transition_cas_has_exactly_one_winner() {
    use std::sync::{Arc, Barrier};

    let state = tempdir().expect("state root");
    let store = PublicationJournalStore::open(state.path()).expect("journal store");
    let (request, _) = request();
    let operation_id = request.operation_id();
    let previous = store.create(request).expect("create journal");
    let barrier = Arc::new(Barrier::new(2));
    let root = state.path().to_path_buf();
    let workers = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let root = root.clone();
            let previous = previous.clone();
            std::thread::spawn(move || {
                let store = PublicationJournalStore::open(&root).expect("worker journal store");
                barrier.wait();
                store.transition(&previous, previous.clone())
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("transition worker"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(PublicationError::ConcurrentJournalUpdate)))
            .count(),
        1
    );
    assert_eq!(
        store.load(operation_id).expect("winning journal").revision,
        2
    );
}

struct EarlyBackend {
    broker: KeyPair,
    fail_validation_once: bool,
    substitute_receipt: bool,
}

struct CompleteBackend {
    broker: KeyPair,
    replication_pending_once: bool,
    finality_pending_once: bool,
    substitute_readback: bool,
    submissions: usize,
}

struct ArchiveRecoveryBackend {
    broker: KeyPair,
    now_ms: u64,
    staged_receipts: Vec<MusubiSeedIngressReceiptV1>,
    prepare_calls: usize,
    registration_calls: usize,
    pin_calls: usize,
    archive_committed: bool,
    drop_commit_response_once: bool,
    return_conflicting_archive: bool,
    registration_mode: ArchiveRecoveryMode,
}

struct LocationRecoveryBackend {
    broker: KeyPair,
    replication_script: VecDeque<LocationPollV1>,
    prepared_generations: Vec<(u8, Vec<MusubiArchiveLocationIdV1>)>,
    applied_generations: Vec<u8>,
    drop_location_response_once: bool,
    reject_release: bool,
    release_submissions: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveRecoveryMode {
    Commit,
    Pending,
    ExpiredAbsent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocationPollV1 {
    Healthy,
    HealthyRevisionOffset(u64),
    HealthyDirectoryAdvance,
    Retired,
    RetiredRevisionOffset(u64),
}

impl EarlyBackend {
    fn unsupported() -> PublicationBackendError {
        PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
    }
}

impl PublicationBackend for EarlyBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(1_500)
    }

    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        if self.fail_validation_once {
            self.fail_validation_once = false;
            return Err(PublicationBackendError::retryable(
                "COMPILER_TEMPORARILY_UNAVAILABLE",
            ));
        }
        let mut consumed = Vec::new();
        car.read_to_end(&mut consumed)
            .map_err(|_| PublicationBackendError::permanent("CAR_READ_FAILED"))?;
        if consumed.is_empty() {
            return Err(PublicationBackendError::permanent("EMPTY_TEST_CAR"));
        }
        Ok(validation_evidence(request))
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        let mut receipt = signed_receipt(expected, &self.broker);
        if self.substitute_receipt {
            receipt.payload.binding.archive_id = ArchiveId::new([0xEE; 32]);
        }
        Ok(receipt)
    }

    fn prepare_archive_registration_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn prepare_archive_location_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _location: &MusubiArchiveLocationV1,
        _provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn submit_release_native_amx(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Err(Self::unsupported())
    }
}

impl PublicationBackend for CompleteBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(1_500)
    }

    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        Ok(signed_receipt(expected, &self.broker))
    }

    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }

    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            registered_archive(request, &self.broker, intent),
        ))
    }

    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        let mut result = registration(request, &self.broker).intent;
        result.operation_id = operation_id;
        result.generation = generation;
        Ok(result)
    }

    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        let mut result = registration(request, &self.broker);
        result.intent = intent.clone();
        Ok(PublicationArchiveLocationAdvanceV1::Registered(result))
    }

    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        if self.replication_pending_once {
            self.replication_pending_once = false;
            return Ok(PublicationReplicationAdvanceV1::Pending);
        }
        Ok(PublicationReplicationAdvanceV1::Healthy(
            replication_checkpoint(request, registration, 3),
        ))
    }

    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        let mut evidence = PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        };
        if self.substitute_readback && provider == location.providers[0] {
            evidence.commitment.car_digest = MusubiContentDigestV1::new([0xEE; 32]);
        }
        Ok(evidence)
    }

    fn submit_release_native_amx(
        &mut self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
        self.submissions += 1;
        Ok(PublicationAmxSubmissionV1::new(
            operation_id,
            instruction,
            [0x71; 32],
            80,
        ))
    }

    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        if self.finality_pending_once {
            self.finality_pending_once = false;
            return Ok(None);
        }
        Ok(Some(final_evidence(request)))
    }
}

impl ArchiveRecoveryBackend {
    fn unsupported() -> PublicationBackendError {
        PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
    }
}

impl PublicationBackend for ArchiveRecoveryBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(self.now_ms)
    }

    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        let receipt = signed_receipt_at(expected, &self.broker, self.now_ms, self.now_ms + 100);
        self.staged_receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        self.prepare_calls += 1;
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }

    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        self.registration_calls += 1;
        match self.registration_mode {
            ArchiveRecoveryMode::Pending => {
                return Ok(PublicationArchiveRegistrationAdvanceV1::Pending);
            }
            ArchiveRecoveryMode::ExpiredAbsent => {
                self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
                return Ok(PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(
                    PublicationArchiveRegistrationTerminalV1::registry_expired(
                        intent,
                        Some(60),
                        archive_absence_evidence(request, 60),
                    ),
                ));
            }
            ArchiveRecoveryMode::Commit => {}
        }
        if !self.archive_committed {
            self.archive_committed = true;
            self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
            if self.drop_commit_response_once {
                self.drop_commit_response_once = false;
                return Err(PublicationBackendError::retryable(
                    "ARCHIVE_COMMIT_RESPONSE_DROPPED",
                ));
            }
        }
        let mut recovered = registered_archive(request, &self.broker, intent);
        if self.return_conflicting_archive {
            recovered.archive.registered_by = account(99).0;
        }
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            recovered,
        ))
    }

    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        self.pin_calls += 1;
        let mut result = registration(request, &self.broker).intent;
        result.operation_id = operation_id;
        result.generation = generation;
        Ok(result)
    }

    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        let mut result = registration(request, &self.broker);
        result.intent = intent.clone();
        Ok(PublicationArchiveLocationAdvanceV1::Registered(result))
    }

    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _location: &MusubiArchiveLocationV1,
        _provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn submit_release_native_amx(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
        Err(Self::unsupported())
    }

    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Err(Self::unsupported())
    }
}

impl LocationRecoveryBackend {
    fn new(broker: KeyPair, replication_script: impl IntoIterator<Item = LocationPollV1>) -> Self {
        Self {
            broker,
            replication_script: replication_script.into_iter().collect(),
            prepared_generations: Vec::new(),
            applied_generations: Vec::new(),
            drop_location_response_once: false,
            reject_release: false,
            release_submissions: 0,
        }
    }
}

impl PublicationBackend for LocationRecoveryBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(1_500)
    }

    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        Ok(signed_receipt(expected, &self.broker))
    }

    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }

    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            registered_archive(request, &self.broker, intent),
        ))
    }

    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        self.prepared_generations
            .push((generation, prior_location_ids.to_vec()));
        Ok(location_registration_generation(operation_id, request, registered, generation).intent)
    }

    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        if !self.applied_generations.contains(&intent.generation) {
            self.applied_generations.push(intent.generation);
            if self.drop_location_response_once {
                self.drop_location_response_once = false;
                return Err(PublicationBackendError::retryable(
                    "ARCHIVE_LOCATION_COMMIT_RESPONSE_DROPPED",
                ));
            }
        }
        Ok(PublicationArchiveLocationAdvanceV1::Registered(
            finalized_location_registration(request, intent),
        ))
    }

    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        match self
            .replication_script
            .pop_front()
            .unwrap_or(LocationPollV1::Healthy)
        {
            LocationPollV1::Healthy => Ok(PublicationReplicationAdvanceV1::Healthy(
                replication_checkpoint(request, registration, 3),
            )),
            LocationPollV1::HealthyRevisionOffset(offset) => {
                Ok(PublicationReplicationAdvanceV1::Healthy(
                    replication_checkpoint_with_revision_offset(request, registration, offset),
                ))
            }
            LocationPollV1::HealthyDirectoryAdvance => {
                Ok(PublicationReplicationAdvanceV1::Healthy(
                    replication_checkpoint_with_directory_advance(request, registration),
                ))
            }
            LocationPollV1::Retired => Ok(PublicationReplicationAdvanceV1::Retired(
                retired_location_terminal(registration),
            )),
            LocationPollV1::RetiredRevisionOffset(offset) => {
                Ok(PublicationReplicationAdvanceV1::Retired(
                    retired_location_terminal_with_revision_offset(registration, offset),
                ))
            }
        }
    }

    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Ok(PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        })
    }

    fn submit_release_native_amx(
        &mut self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
        self.release_submissions += 1;
        if self.reject_release {
            return Err(PublicationBackendError::permanent(
                "RELEASE_SUBMISSION_TRANSACTION_REJECTED",
            ));
        }
        Ok(PublicationAmxSubmissionV1::new(
            operation_id,
            instruction,
            [0x71; 32],
            80,
        ))
    }

    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Ok(Some(final_evidence(request)))
    }
}

fn account(seed: u8) -> (AccountId, KeyPair) {
    let keypair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture keypair");
    (AccountId::new(keypair.public_key().clone()), keypair)
}

fn snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 42,
        finalized_block_hash: [0x42; 32],
        index_revision: 3,
    }
}

fn archive_commitment() -> MusubiArchiveCommitmentV1 {
    MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
        por_root: MusubiContentDigestV1::new([3; 32]),
        content_length: 1_024,
        car_digest: MusubiContentDigestV1::new([4; 32]),
        car_size: 2_048,
        bundle_digest: MusubiContentDigestV1::new([5; 32]),
        source_tree_digest: MusubiContentDigestV1::new([6; 32]),
        descriptor_digest: MusubiContentDigestV1::new([7; 32]),
        file_count: 2,
        chunk_count: 4,
    }
}

fn request() -> (PublicationRequestV1, KeyPair) {
    let commitment = archive_commitment();
    let package = MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::DataspaceRoot,
        "demo".parse().expect("package"),
    );
    let release = MusubiReleaseIdV1::new(
        package,
        "1.0.0".parse::<MusubiVersionV1>().expect("release version"),
    );
    let lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release.clone(),
        root_dependencies: Vec::new(),
        nodes: Vec::new(),
    };
    let manifest = MusubiReleaseManifestV1 {
        release,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
        dependencies: Vec::new(),
        exports: Vec::new(),
        interface_digest: MusubiContentDigestV1::new([9; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id: commitment.archive_id(),
        verification_lock_digest: lock.digest(),
    };
    let (publisher, _) = account(20);
    let (broker, broker_keypair) = account(21);
    (
        PublicationRequestV1 {
            chain_id: ChainId::from("musubi-publish-test"),
            genesis_block_hash: [0x15; 32],
            publisher,
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x16; 32]),
            namespace: MusubiNamespaceV1::new("dex").expect("namespace"),
            publication: MusubiPublicationV1 {
                manifest,
                resolution: MusubiResolutionProofV1 {
                    snapshot: snapshot(),
                    lock,
                },
            },
            archive_commitment: commitment,
            namespace_delegation: None,
            expected_policy_revision: 1,
            expected_governance_revision: None,
            nonce: [0x18; 32],
        },
        broker_keypair,
    )
}

fn validation_evidence(request: &PublicationRequestV1) -> PublicationValidationEvidenceV1 {
    PublicationValidationEvidenceV1 {
        archive_id: request.archive_commitment.archive_id(),
        semantic_release_digest: request.publication.manifest.semantic_digest(),
        release_digest: request.publication.manifest.release_digest(),
        source_tree_digest: request.archive_commitment.source_tree_digest,
        descriptor_digest: request.archive_commitment.descriptor_digest,
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
        car_digest: request.archive_commitment.car_digest,
        car_size: request.archive_commitment.car_size,
        compiler_output_digest: MusubiContentDigestV1::new([0x63; 32]),
        resolution_snapshot: request.publication.resolution.snapshot,
    }
}

fn signed_receipt(
    binding: &MusubiSeedIngressReceiptBindingV1,
    broker: &KeyPair,
) -> MusubiSeedIngressReceiptV1 {
    signed_receipt_at(binding, broker, 1_000, 2_000)
}

fn signed_receipt_at(
    binding: &MusubiSeedIngressReceiptBindingV1,
    broker: &KeyPair,
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> MusubiSeedIngressReceiptV1 {
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
        issued_at_ms,
        expires_at_ms,
    };
    MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker.public_key().clone(),
            signature: SignatureOf::try_from_hash(broker.private_key(), payload.signing_hash())
                .expect("sign fixture ingress receipt"),
        }],
        payload,
    }
}

fn archive_absence_evidence(
    request: &PublicationRequestV1,
    finalized_height: u64,
) -> PublicationArchiveAbsenceEvidenceV1 {
    PublicationArchiveAbsenceEvidenceV1 {
        chain_id: request.chain_id.clone(),
        genesis_block_hash: request.genesis_block_hash,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height,
            finalized_block_hash: [0xA5; 32],
            index_revision: finalized_height,
        },
        finalized_time_ms: 1_700_000_000_000,
        decision: MusubiArchiveRetentionDecisionV1 {
            archive_id: request.archive_commitment.archive_id(),
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        },
    }
}

fn registration_intent(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    receipt: MusubiSeedIngressReceiptV1,
) -> PublicationArchiveRegistrationIntentV1 {
    let (publisher, publisher_keypair) = account(20);
    assert_eq!(publisher, request.publisher);
    let mut builder = TransactionBuilder::new(
        request.chain_id.clone(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.archive_registration_instruction(&receipt)]);
    builder.set_creation_time(std::time::Duration::from_millis(
        receipt.payload.issued_at_ms,
    ));
    let signed_transaction = builder.sign(publisher_keypair.private_key());
    PublicationArchiveRegistrationIntentV1::new(operation_id, request, receipt, signed_transaction)
}

fn registered_archive(
    request: &PublicationRequestV1,
    broker: &KeyPair,
    intent: &PublicationArchiveRegistrationIntentV1,
) -> PublicationRegisteredArchiveV1 {
    let mut archive = archive_record(request, broker);
    archive.staging_receipt = intent.staging_receipt.clone();
    PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: intent.transaction_hash,
        chain_id: request.chain_id.clone(),
        genesis_block_hash: request.genesis_block_hash,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
        archive,
    }
}

fn archive_record(request: &PublicationRequestV1, broker: &KeyPair) -> MusubiArchiveRecordV1 {
    MusubiArchiveRecordV1 {
        archive_id: request.archive_commitment.archive_id(),
        commitment: request.archive_commitment.clone(),
        staging_receipt: signed_receipt(&request.receipt_binding(), broker),
        registered_by: request.publisher.clone(),
        registered_at_height: 50,
        location_revision: 1,
        location_ids: Vec::new(),
    }
}

fn registration(
    request: &PublicationRequestV1,
    broker: &KeyPair,
) -> PublicationArchiveRegistrationV1 {
    let archive = archive_record(request, broker);
    let prepared_page = MusubiArchiveLocationPageV1 {
        chain_id: request.chain_id.clone(),
        genesis_hash: request.genesis_block_hash,
        archive: archive.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
    };
    let replication_order = ReplicationOrderId::new([0x33; 32]);
    let provider_attestations = (1..=3)
        .map(|provider| provider_attestation(request, replication_order, provider))
        .collect::<Vec<_>>();
    let instruction = AddMusubiArchiveLocationV1 {
        archive_id: request.archive_commitment.archive_id(),
        location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
        pin_manifest: ManifestDigest::new([0x32; 32]),
        replication_order,
        provider_attestations: provider_attestations.clone(),
        renew_after_epoch: 10,
        expires_at_epoch: 20,
        expected_location_revision: archive.location_revision,
    };
    let (_, publisher_keypair) = account(20);
    let mut builder = TransactionBuilder::new(
        request.chain_id.clone(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction.clone()]);
    builder.set_creation_time(std::time::Duration::from_millis(1_000));
    let intent = PublicationArchiveLocationIntentV1::new(
        request.operation_id(),
        1,
        prepared_page,
        instruction,
        builder.sign(publisher_keypair.private_key()),
    );
    let location = MusubiArchiveLocationV1 {
        location_id: intent.location_id,
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: intent.pin_manifest,
        replication_order: intent.replication_order,
        providers: provider_attestations
            .iter()
            .map(|attestation| attestation.payload.binding.provider_id)
            .collect(),
        provider_attestations,
        renew_after_epoch: intent.renew_after_epoch,
        expires_at_epoch: intent.expires_at_epoch,
        finalized_height: 70,
        revision: 2,
        state: MusubiArchiveLocationStateV1::Healthy,
    };
    let mut finalized_archive = archive;
    finalized_archive.location_revision = 2;
    finalized_archive.location_ids = vec![intent.location_id];
    PublicationArchiveRegistrationV1 {
        intent,
        applied_height: 70,
        finalized_page: MusubiArchiveLocationPageV1 {
            chain_id: request.chain_id.clone(),
            genesis_hash: request.genesis_block_hash,
            archive: finalized_archive,
            items: vec![location],
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 70,
                finalized_block_hash: [0x46; 32],
                index_revision: 3,
            },
        },
    }
}

fn location_registration_generation(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    registered: &PublicationRegisteredArchiveV1,
    generation: u8,
) -> PublicationArchiveRegistrationV1 {
    assert!(generation > 0);
    let generation_u64 = u64::from(generation);
    let completed_generations = generation_u64 - 1;
    let prepared_height = registered.snapshot.finalized_height + completed_generations * 2;
    let prepared_revision = registered.archive.location_revision + completed_generations * 2;
    let prepared_snapshot = if generation == 1 {
        registered.snapshot
    } else {
        MusubiRegistrySnapshotV1 {
            finalized_height: prepared_height,
            finalized_block_hash: [0x6F_u8.saturating_add(generation); 32],
            index_revision: registered.snapshot.index_revision + completed_generations * 2,
        }
    };
    let mut prepared_archive = registered.archive.clone();
    prepared_archive.location_revision = prepared_revision;
    prepared_archive.location_ids.clear();
    let prepared_page = MusubiArchiveLocationPageV1 {
        chain_id: request.chain_id.clone(),
        genesis_hash: request.genesis_block_hash,
        archive: prepared_archive,
        items: Vec::new(),
        next_cursor: None,
        snapshot: prepared_snapshot,
    };
    let replication_order = ReplicationOrderId::new([0x40_u8.saturating_add(generation); 32]);
    let provider_attestations = (1..=3)
        .map(|provider| provider_attestation(request, replication_order, provider))
        .collect::<Vec<_>>();
    let instruction = AddMusubiArchiveLocationV1 {
        archive_id: request.archive_commitment.archive_id(),
        location_id: MusubiArchiveLocationIdV1::new([0x30_u8.saturating_add(generation); 32]),
        pin_manifest: ManifestDigest::new([0x50_u8.saturating_add(generation); 32]),
        replication_order,
        provider_attestations,
        renew_after_epoch: 20 + generation_u64,
        expires_at_epoch: 40 + generation_u64,
        expected_location_revision: prepared_revision,
    };
    let (_, publisher_keypair) = account(20);
    let mut builder = TransactionBuilder::new(
        request.chain_id.clone(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction.clone()]);
    builder.set_creation_time(std::time::Duration::from_millis(1_000 + generation_u64));
    let intent = PublicationArchiveLocationIntentV1::new(
        operation_id,
        generation,
        prepared_page,
        instruction,
        builder.sign(publisher_keypair.private_key()),
    );
    finalized_location_registration(request, &intent)
}

fn finalized_location_registration(
    request: &PublicationRequestV1,
    intent: &PublicationArchiveLocationIntentV1,
) -> PublicationArchiveRegistrationV1 {
    let finalized_height = intent.prepared_page.snapshot.finalized_height + 1;
    let finalized_revision = intent.expected_location_revision + 1;
    let providers = intent
        .provider_attestations
        .iter()
        .map(|attestation| attestation.payload.binding.provider_id)
        .collect::<Vec<_>>();
    let location = MusubiArchiveLocationV1 {
        location_id: intent.location_id,
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: intent.pin_manifest,
        replication_order: intent.replication_order,
        providers,
        provider_attestations: intent.provider_attestations.clone(),
        renew_after_epoch: intent.renew_after_epoch,
        expires_at_epoch: intent.expires_at_epoch,
        finalized_height,
        revision: finalized_revision,
        state: MusubiArchiveLocationStateV1::Healthy,
    };
    let mut finalized_archive = intent.prepared_page.archive.clone();
    finalized_archive.location_revision = finalized_revision;
    finalized_archive.location_ids = vec![intent.location_id];
    PublicationArchiveRegistrationV1 {
        intent: intent.clone(),
        applied_height: finalized_height,
        finalized_page: MusubiArchiveLocationPageV1 {
            chain_id: request.chain_id.clone(),
            genesis_hash: request.genesis_block_hash,
            archive: finalized_archive,
            items: vec![location],
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height,
                finalized_block_hash: [0x60_u8.saturating_add(intent.generation); 32],
                index_revision: intent.prepared_page.snapshot.index_revision + 1,
            },
        },
    }
}

fn retired_location_terminal(
    registration: &PublicationArchiveRegistrationV1,
) -> PublicationArchiveLocationTerminalV1 {
    retired_location_terminal_with_revision_offset(registration, 0)
}

fn retired_location_terminal_with_revision_offset(
    registration: &PublicationArchiveRegistrationV1,
    offset: u64,
) -> PublicationArchiveLocationTerminalV1 {
    let mut finalized_page = registration.finalized_page.clone();
    finalized_page.archive.location_revision += 1 + offset;
    finalized_page.archive.location_ids.clear();
    finalized_page.items.clear();
    finalized_page.snapshot.finalized_height += 1 + offset;
    finalized_page.snapshot.finalized_block_hash = [0x70_u8
        .saturating_add(registration.intent.generation)
        .saturating_add(u8::try_from(offset).unwrap_or(u8::MAX));
        32];
    finalized_page.snapshot.index_revision += 1 + offset;
    PublicationArchiveLocationTerminalV1 {
        transaction_hash: registration.intent.transaction_hash,
        reason: PublicationArchiveLocationTerminalReasonV1::Retired,
        finalized_page,
    }
}

fn provider_attestation(
    request: &PublicationRequestV1,
    replication_order: ReplicationOrderId,
    provider_byte: u8,
) -> MusubiProviderBundleVerificationAttestationV1 {
    let (owner, keypair) = account(provider_byte.saturating_add(60));
    let binding = MusubiProviderBundleVerificationBindingV1 {
        chain_id: request.chain_id.clone(),
        genesis_block_hash: request.genesis_block_hash,
        provider_id: ProviderId::new([provider_byte; 32]),
        completed_by: owner.clone(),
        completion_authority: ProviderIngestCompletionAuthorityV1::new(
            owner,
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [provider_byte.saturating_add(20); 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [provider_byte.saturating_add(30); 32],
            },
        ),
        replication_order,
        assignment_revision: 1,
        completion_epoch: 12,
        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
            height: 60,
            block_hash: [provider_byte.saturating_add(40); 32],
        },
        archive_id: request.archive_commitment.archive_id(),
        bundle_digest: request.archive_commitment.bundle_digest,
        descriptor_digest: request.archive_commitment.descriptor_digest,
        semantic_release_manifest_digest: request.publication.manifest.semantic_digest(),
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
        source_tree_digest: request.archive_commitment.source_tree_digest,
    };
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding,
    };
    MusubiProviderBundleVerificationAttestationV1 {
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(keypair.private_key(), payload.signing_hash())
                .expect("sign provider fixture attestation"),
        }],
        payload,
    }
}

fn location(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    provider_count: u8,
) -> MusubiArchiveLocationV1 {
    let registered_location = registration
        .location()
        .expect("registered fixture location");
    let attestations = (1..=provider_count)
        .map(|provider| {
            provider_attestation(request, registration.intent.replication_order, provider)
        })
        .collect::<Vec<_>>();
    MusubiArchiveLocationV1 {
        location_id: registration.location_id(),
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: registration.intent.pin_manifest,
        replication_order: registration.intent.replication_order,
        providers: attestations
            .iter()
            .map(|attestation| attestation.payload.binding.provider_id)
            .collect(),
        provider_attestations: attestations,
        renew_after_epoch: registration.intent.renew_after_epoch,
        expires_at_epoch: registration.intent.expires_at_epoch,
        finalized_height: registered_location.finalized_height,
        revision: registered_location.revision,
        state: MusubiArchiveLocationStateV1::Healthy,
    }
}

fn replication_checkpoint(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    provider_count: u8,
) -> PublicationReplicationCheckpointV1 {
    let mut finalized_page = registration.finalized_page.clone();
    let index = finalized_page
        .items
        .binary_search_by_key(&registration.location_id(), |location| location.location_id)
        .expect("registered fixture location is present");
    finalized_page.items[index] = location(request, registration, provider_count);
    PublicationReplicationCheckpointV1 { finalized_page }
}

fn replication_checkpoint_with_revision_offset(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    offset: u64,
) -> PublicationReplicationCheckpointV1 {
    let mut checkpoint = replication_checkpoint(request, registration, 3);
    if offset == 0 {
        return checkpoint;
    }
    let location = checkpoint
        .finalized_page
        .items
        .iter_mut()
        .find(|location| location.location_id == registration.location_id())
        .expect("registered fixture location is present");
    location.revision += offset;
    location.finalized_height += offset;
    checkpoint.finalized_page.archive.location_revision += offset;
    checkpoint.finalized_page.snapshot.finalized_height += offset;
    checkpoint.finalized_page.snapshot.finalized_block_hash =
        [0x80_u8.saturating_add(u8::try_from(offset).unwrap_or(u8::MAX)); 32];
    checkpoint.finalized_page.snapshot.index_revision += offset;
    checkpoint
}

fn replication_checkpoint_with_directory_advance(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
) -> PublicationReplicationCheckpointV1 {
    let mut checkpoint = replication_checkpoint(request, registration, 3);
    checkpoint.finalized_page.archive.location_revision += 1;
    checkpoint.finalized_page.snapshot.finalized_height += 1;
    checkpoint.finalized_page.snapshot.finalized_block_hash = [0x91; 32];

    let mut unrelated = checkpoint
        .location(registration)
        .expect("registered fixture location")
        .clone();
    unrelated.location_id = MusubiArchiveLocationIdV1::new([0xF0; 32]);
    unrelated.revision = checkpoint.finalized_page.archive.location_revision;
    unrelated.finalized_height = checkpoint.finalized_page.snapshot.finalized_height;
    checkpoint
        .finalized_page
        .archive
        .location_ids
        .push(unrelated.location_id);
    checkpoint.finalized_page.items.push(unrelated);
    checkpoint
}

fn final_evidence(request: &PublicationRequestV1) -> PublicationFinalEvidenceV1 {
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 100,
        finalized_block_hash: [0x64; 32],
        index_revision: 4,
    };
    let yank = MusubiReleaseYankV1 {
        release: request.publication.manifest.release.clone(),
        yanked: false,
        reason: MusubiReasonV1::new("initial publication").expect("reason"),
        changed_by: request.publisher.clone(),
        changed_at_height: 80,
        revision: 1,
    };
    let governance = MusubiArtifactGovernanceStateV1::Available;
    let home_release = MusubiReleaseRecordV1 {
        manifest: request.publication.manifest.clone(),
        release_digest: request.publication.manifest.release_digest(),
        published_by: request.publisher.clone(),
        published_at_height: 80,
        yank: yank.clone(),
        artifact_governance: governance.clone(),
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 1,
        },
    };
    let universal_release = MusubiResolverReleaseRowV1 {
        release: request.publication.manifest.release.clone(),
        release_digest: request.publication.manifest.release_digest(),
        archive_id: request.archive_commitment.archive_id(),
        source_digest: request.archive_commitment.source_tree_digest,
        interface_digest: request.publication.manifest.interface_digest,
        abi: request.publication.manifest.abi,
        dependencies: request.publication.manifest.dependencies.clone(),
        selection: MusubiReleaseSelectionStateV1 {
            yank,
            storage: MusubiArchiveAvailabilityV1 {
                archive_id: request.archive_commitment.archive_id(),
                availability: MusubiStorageAvailabilityV1::Selectable,
                healthy_replicas: 3,
                active_locations: 1,
                finalized_height: 70,
                finalized_block_hash: [0x46; 32],
                index_revision: snapshot.index_revision,
            },
            governance,
        },
        index_revision: snapshot.index_revision,
    };
    PublicationFinalEvidenceV1 {
        chain_id: request.chain_id.clone(),
        genesis_block_hash: request.genesis_block_hash,
        snapshot,
        home_release,
        universal_release,
    }
}
