//! Adapter lifecycle tests using real account/device signatures and private structural owners.
//! These fixtures do not qualify hardware, authenticate an issuer release or manufacture a Core
//! machine. The production sealed-owner implementation uses the actual Core selection instead.

use super::*;
use crate::kagemusha_core_coordinator_v1::{
    enrolled_open::tests as open_fixture, startup_qualification::tests as fixture,
};
use crate::kagemusha_device_bridge_v1::QualificationProjectionV1;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::kagemusha::KagemushaHardwareCredentialV1;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

struct FixtureOwner {
    metadata_reads: Arc<AtomicUsize>,
    current: QualificationProjectionV1,
    original_floor: KagemushaHardwareCredentialV1,
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    source: EnrolledOpenAuthoritySourceV1,
    observer: NativeStartupQualificationOwnerV1,
    evidence: Option<VerifiedEnrolledOpenEvidenceV1>,
    begin_pause: Option<(mpsc::Sender<()>, Mutex<mpsc::Receiver<()>>)>,
    prepare_pause: Option<(mpsc::Sender<()>, Mutex<mpsc::Receiver<()>>)>,
}

impl FixtureOwner {
    fn new(current: QualificationProjectionV1) -> Self {
        Self {
            metadata_reads: Arc::new(AtomicUsize::new(0)),
            original_floor: current.credential,
            enrollment: fixture::enrollment_binding(&current),
            source: open_fixture::recovery_source(&current),
            observer: fixture::restored_owner(&current, current.credential),
            current,
            evidence: None,
            begin_pause: None,
            prepare_pause: None,
        }
    }
}

impl sealed::RecoveredOwner for FixtureOwner {}

impl RecoveredOwnerAccessV1 for FixtureOwner {
    fn begin_possession(&self, deadline: NativeDeadlineV1) -> Result<PendingEnrolledOpenV1> {
        self.metadata_reads.fetch_add(1, Ordering::SeqCst);
        if let Some((entered, resume)) = &self.begin_pause {
            entered.send(()).unwrap();
            resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        Ok(open_fixture::recovery_pending_for_source(
            &self.current,
            self.original_floor,
            self.source.clone(),
            deadline,
        ))
    }

    fn prepare_possession(
        &self,
        verified: VerifiedEnrolledOpenV1,
    ) -> Result<PreparedRecoveredOpenV1> {
        if let Some((entered, resume)) = &self.prepare_pause {
            entered.send(()).unwrap();
            resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        prepare_recovered_observer(
            &self.observer,
            &self.enrollment,
            &self.source,
            self.current.release_id,
            self.current.hardware_policy_digest,
            self.current.core_authorization_key_reference,
            verified,
        )
    }

    fn install_possession(&mut self, prepared: PreparedRecoveredOpenV1) {
        self.observer = prepared.observer;
        self.evidence = Some(prepared.evidence);
    }

    fn observer_mut(&mut self) -> &mut NativeStartupQualificationOwnerV1 {
        &mut self.observer
    }
}

fn proof(
    begun: &BegunRecoveredSessionV1,
    qualification: &QualificationProjectionV1,
) -> (Vec<u8>, Vec<u8>) {
    let account = KeyPair::from_seed(vec![211; 32], Algorithm::Ed25519);
    let account_signature =
        Signature::try_new(account.private_key(), &begun.account_signing_message)
            .unwrap()
            .payload()
            .to_vec();
    let response = open_fixture::frame_for_projection(
        begun.device_request_id,
        &begun.device_command,
        qualification,
    );
    (account_signature, response)
}

fn read_command() -> Vec<u8> {
    crate::kagemusha_device_bridge_v1::canonical_stock_command_for_tests(
        crate::KagemushaDeviceLifecycleOperationV1::from_code(21).unwrap(),
    )
    .unwrap()[80..]
        .to_vec()
}

#[test]
fn actual_dual_signature_completion_installs_one_owner_and_close_revokes_observation_access() {
    let current = fixture::qualification(1);
    let registry = RecoveredEnrolledSessionRegistryV1::new();
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    assert!(!begun.canonical_challenge.is_empty());
    let (signature, response) = proof(&begun, &current);
    let handle = registry
        .complete(begun.attempt_id, &signature, &response)
        .unwrap();
    let result = registry
        .dispatch_observation(handle, |observer| observer.begin(21, &read_command()))
        .unwrap();
    assert!(result.value.is_ok());
    assert!(result.session_is_current);
    assert_eq!(
        owner
            .lock()
            .unwrap()
            .evidence
            .as_ref()
            .unwrap()
            .observation()
            .qualification,
        current
    );
    registry.close(handle).unwrap();
    assert!(
        registry
            .dispatch_observation(handle, |_| panic!("closed session"))
            .is_err()
    );
    // A same-owner reconnect uses the original Arc and consumes a new proof.
    let next = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&next, &current);
    let next_handle = registry
        .complete(next.attempt_id, &signature, &response)
        .unwrap();
    assert_ne!(next_handle, handle);
    assert!(
        registry
            .begin(Arc::new(Mutex::new(FixtureOwner::new(current))))
            .is_err()
    );
}

#[test]
fn invalid_signature_consumes_the_native_attempt_and_never_installs_an_observer() {
    let current = fixture::qualification(1);
    let registry = RecoveredEnrolledSessionRegistryV1::new();
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&begun, &current);
    let mut changed = signature.clone();
    changed[0] ^= 1;
    assert_eq!(
        registry.complete(begun.attempt_id, &changed, &response),
        Err(RegistryError::Rejected)
    );
    assert_eq!(
        registry.complete(begun.attempt_id, &signature, &response),
        Err(RegistryError::Rejected)
    );
    assert!(owner.lock().unwrap().evidence.is_none());
}

#[test]
fn checkpoint_change_after_challenge_rejects_the_old_source_before_installation() {
    let current = fixture::qualification(1);
    let registry = RecoveredEnrolledSessionRegistryV1::new();
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&begun, &current);
    {
        let mut owner = owner.lock().unwrap();
        let EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint { statement, .. } = &mut owner.source
        else {
            panic!("recovery fixture")
        };
        statement.metadata_revision += 1;
    }
    assert_eq!(
        registry.complete(begun.attempt_id, &signature, &response),
        Err(RegistryError::Rejected)
    );
    assert!(owner.lock().unwrap().evidence.is_none());
    let next = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&next, &current);
    registry
        .complete(next.attempt_id, &signature, &response)
        .unwrap();
}

#[test]
fn reconnect_cannot_lower_a_stronger_process_observation_to_the_old_core_checkpoint_floor() {
    let current = fixture::qualification(1);
    let mut renewed = current.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = fixture::reseal(renewed);
    let registry = RecoveredEnrolledSessionRegistryV1::new();
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    fixture::qualify(&mut owner.lock().unwrap().observer, &renewed);

    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&begun, &current);
    assert_eq!(
        registry.complete(begun.attempt_id, &signature, &response),
        Err(RegistryError::Rejected)
    );
    assert!(owner.lock().unwrap().evidence.is_none());
    let next = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&next, &renewed);
    registry
        .complete(next.attempt_id, &signature, &response)
        .unwrap();
    assert_eq!(
        owner
            .lock()
            .unwrap()
            .evidence
            .as_ref()
            .unwrap()
            .observation()
            .qualification,
        renewed
    );
}

#[test]
fn initial_certificate_possession_cannot_enter_the_recovery_adapter() {
    let current = fixture::qualification(1);
    let owner = FixtureOwner::new(current);
    let (pending, signature, response) = open_fixture::signed_fixture(1);
    let verified = pending.complete(&signature, &response).unwrap();
    assert!(owner.prepare_possession(verified).is_err());
    assert!(owner.evidence.is_none());
}

#[test]
fn cancellation_during_actual_verified_proof_preparation_prevents_publication() {
    let current = fixture::qualification(1);
    let registry = Arc::new(RecoveredEnrolledSessionRegistryV1::new());
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&begun, &current);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    owner.lock().unwrap().prepare_pause = Some((entered_tx, Mutex::new(resume_rx)));
    let worker_registry = Arc::clone(&registry);
    let worker =
        thread::spawn(move || worker_registry.complete(begun.attempt_id, &signature, &response));
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    registry.cancel(begun.attempt_id).unwrap();
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert!(owner.lock().unwrap().evidence.is_none());
}

#[test]
fn logout_revokes_actual_recovery_handle_without_dropping_original_possession_history() {
    let current = fixture::qualification(1);
    let registry = RecoveredEnrolledSessionRegistryV1::new();
    let owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(Arc::clone(&owner)).unwrap();
    let (signature, response) = proof(&begun, &current);
    let handle = registry
        .complete(begun.attempt_id, &signature, &response)
        .unwrap();
    registry.revoke_selection().unwrap();
    assert!(
        registry
            .dispatch_observation(handle, |_| panic!("revoked session"))
            .is_err()
    );
    assert!(owner.lock().unwrap().evidence.is_some());
}

#[test]
fn account_revocation_during_hardware_metadata_read_never_returns_an_attempt() {
    let registry = Arc::new(RecoveredEnrolledSessionRegistryV1::new());
    let current = fixture::qualification(1);
    let mut fixture_owner = FixtureOwner::new(current);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    fixture_owner.begin_pause = Some((entered_tx, Mutex::new(resume_rx)));
    let owner = Arc::new(Mutex::new(fixture_owner));
    let worker_registry = registry.clone();
    let worker = thread::spawn(move || worker_registry.begin(owner));
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    registry.revoke_selection().unwrap();
    resume_tx.send(()).unwrap();
    assert!(matches!(
        worker.join().unwrap(),
        Err(RegistryError::Rejected)
    ));
}

#[test]
fn a_later_owner_begin_survives_an_older_hardware_metadata_read() {
    let registry = Arc::new(RecoveredEnrolledSessionRegistryV1::new());
    let current = fixture::qualification(1);
    let mut fixture_owner = FixtureOwner::new(current.clone());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    fixture_owner.begin_pause = Some((entered_tx, Mutex::new(resume_rx)));
    let owner = Arc::new(Mutex::new(fixture_owner));
    let worker_registry = registry.clone();
    let worker = thread::spawn(move || worker_registry.begin(owner));
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let newer_owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
    let begun = registry.begin(newer_owner).unwrap();
    let (account, device) = proof(&begun, &current);
    let handle = registry
        .complete(begun.attempt_id, &account, &device)
        .unwrap();
    resume_tx.send(()).unwrap();
    assert!(matches!(
        worker.join().unwrap(),
        Err(RegistryError::Rejected)
    ));
    assert!(
        registry
            .dispatch_observation(handle, |_| ())
            .unwrap()
            .session_is_current
    );
}

fn queued_adapter_begin_never_reads_metadata(replacement: bool) {
    let registry = Arc::new(RecoveredEnrolledSessionRegistryV1::new());
    let current = fixture::qualification(1);
    let fixture_owner = FixtureOwner::new(current.clone());
    let reads = fixture_owner.metadata_reads.clone();
    let owner = Arc::new(Mutex::new(fixture_owner));
    let held = owner.lock().unwrap();
    let worker_registry = registry.clone();
    let worker_owner = owner.clone();
    let worker = thread::spawn(move || worker_registry.begin(worker_owner));
    // Observe the actual reservation while the owner mutex excludes all metadata
    // callbacks. This is a causal barrier, not an assumed scheduling delay.
    let limit = std::time::Instant::now() + Duration::from_secs(5);
    while registry.registry.preparing_for_test().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < limit,
            "begin did not reserve its preparation"
        );
        thread::yield_now();
    }
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    let newer = if replacement {
        let new_owner = Arc::new(Mutex::new(FixtureOwner::new(current.clone())));
        let begun = registry.begin(new_owner).unwrap();
        let (account, device) = proof(&begun, &current);
        Some(
            registry
                .complete(begun.attempt_id, &account, &device)
                .unwrap(),
        )
    } else {
        registry.revoke_selection().unwrap();
        None
    };
    drop(held);
    assert!(matches!(
        worker.join().unwrap(),
        Err(RegistryError::Rejected)
    ));
    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "revoked queued begin read hardware metadata"
    );
    assert!(owner.lock().unwrap().evidence.is_none());
    if let Some(handle) = newer {
        assert!(
            registry
                .dispatch_observation(handle, |_| ())
                .unwrap()
                .session_is_current
        );
    }
}

#[test]
fn revoked_begin_waiting_for_owner_never_reads_hardware_metadata() {
    queued_adapter_begin_never_reads_metadata(false);
}

#[test]
fn replaced_begin_waiting_for_owner_never_reads_hardware_metadata() {
    queued_adapter_begin_never_reads_metadata(true);
}
