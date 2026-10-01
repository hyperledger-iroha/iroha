//! Synthetic signed enrollment originals and explicit memory stores; no hardware qualification.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use iroha_crypto::Signature;
use iroha_data_model::kagemusha::{
    KagemushaDeviceReadCredentialCommandV1, KagemushaRetailEnrollmentPossessionProofV1,
    kagemusha_decode_device_success_response_v1,
};

use super::*;
use crate::kagemusha_core_coordinator_v1::initial_enrollment::tests as fixture;
use crate::kagemusha_core_coordinator_v1::{
    KagemushaEnrollmentJournalErrorV1, KagemushaEnrollmentJournalResultV1,
    KagemushaEnrollmentJournalSelectionV1, kagemusha_core_coordinator_decode_response_v1,
    kagemusha_core_coordinator_encode_request_v1,
};

#[derive(Default)]
struct MemoryStore(Mutex<(Option<Vec<u8>>, Option<u64>)>);
impl KagemushaEnrollmentJournalStoreV1 for MemoryStore {
    fn load_checked(&self) -> KagemushaEnrollmentJournalResultV1<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().0.clone())
    }
    fn compare_and_swap(
        &self,
        old: Option<u64>,
        next: &[u8],
    ) -> KagemushaEnrollmentJournalResultV1<()> {
        let mut state = self.0.lock().unwrap();
        if state.1 != old {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.0 = Some(next.to_vec());
        state.1 = Some(old.unwrap_or(0) + 1);
        Ok(())
    }
}

struct Inner;
impl KagemushaCoreCoordinatorBackendV1 for Inner {
    fn open(&self, _: &str) -> Result<u64, Error> {
        Ok(7)
    }
    fn invoke(
        &self,
        _: u64,
        _: KagemushaCoreCoordinatorMethodV1,
        _: &[u8],
    ) -> Result<Vec<u8>, Error> {
        Err(Error::Unavailable)
    }
    fn close(&self, _: u64) -> Result<(), Error> {
        Ok(())
    }
}

struct Context {
    substitute_policy: AtomicBool,
}
impl KagemushaEnrollmentContextProviderV1 for Context {
    fn context_for_selection(
        &self,
        _: u64,
        live: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<KagemushaEnrollmentProvisionedContextV1, Error> {
        let mut context =
            fixture::journal_context(live.require_live().map_err(|_| Error::Rejected)?);
        if self.substitute_policy.load(Ordering::SeqCst) {
            Arc::make_mut(&mut context.policy).issuer_audience =
                "substituted-authority".parse().unwrap();
        }
        Ok(context)
    }
}

fn template() -> KagemushaEnrollmentProvisionedContextV1 {
    let pins = fixture::journal_pins();
    fixture::journal_context(&KagemushaEnrollmentJournalSelectionV1 {
        account_i105: fixture::journal_account(),
        ticket: 1,
        client_nonce: [2; 32],
        release_id: pins.release_id,
        hardware_profile_id: pins.hardware_profile_id,
        lane_id: [3; 32],
    })
}

struct Provisioner {
    calls: AtomicUsize,
    original_valid: AtomicBool,
    fail_provision: AtomicBool,
    fail_handoff: AtomicBool,
    handoffs: AtomicUsize,
    trusted_native_time: AtomicU64,
    context: Arc<Context>,
}
impl Default for Provisioner {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            original_valid: AtomicBool::new(true),
            fail_provision: AtomicBool::new(false),
            fail_handoff: AtomicBool::new(false),
            handoffs: AtomicUsize::new(0),
            trusted_native_time: AtomicU64::new(1_500),
            context: Arc::new(Context {
                substitute_policy: AtomicBool::new(false),
            }),
        }
    }
}
impl KagemushaNativeEnrollmentProvisionerV1 for Provisioner {
    fn recheck_originals(&self) -> Result<(), Error> {
        if self.original_valid.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
    fn provision(&self, _: &str) -> Result<KagemushaNativeEnrollmentProvisioningV1, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_provision.load(Ordering::SeqCst) {
            return Err(Error::Unavailable);
        }
        let selected = template();
        KagemushaNativeEnrollmentProvisioningV1::from_trusted_platform(
            Arc::new(MemoryStore::default()),
            self.context.clone(),
            selected.policy,
            selected.app_policy,
            selected.release,
            selected.native_authorization_public_key,
            fixture::journal_pins().hardware_profile_id,
        )
    }
    fn bootstrap_source(
        &self,
        path: &str,
        handle: u64,
    ) -> Result<Arc<dyn super::super::KagemushaNativeCoreBootstrapSourceV1>, Error> {
        assert_eq!(path, "/durable/enrollment");
        assert_eq!(handle, 1);
        self.handoffs.fetch_add(1, Ordering::SeqCst);
        if self.fail_handoff.load(Ordering::SeqCst) {
            return Err(Error::Rejected);
        }
        Ok(Arc::new(MissingPhysicalInputs {
            trusted_native_time: self.trusted_native_time.load(Ordering::SeqCst),
        }))
    }
}

struct MissingPhysicalInputs {
    trusted_native_time: u64,
}
impl super::super::KagemushaNativeCoreBootstrapSourceV1 for MissingPhysicalInputs {
    fn recheck_originals(&self) -> Result<(), Error> {
        Ok(())
    }
    fn inputs_for_admission(
        &self,
        path: &str,
        admission: &FreshIssuerAdmissionV1,
    ) -> Result<super::super::KagemushaNativeCoreBootstrapInputsV1, Error> {
        assert_eq!(path, "/durable/enrollment");
        let (enrollment, possession) = admission
            .current_bootstrap_evidence(self.trusted_native_time)
            .map_err(|_| Error::Rejected)?;
        assert_eq!(
            enrollment.authenticated_at_ms(),
            possession.verified_at_ms()
        );
        assert_eq!(
            enrollment.certificate().subject.challenge_evidence_digest,
            possession.evidence_digest()
        );
        // Signed issuer fixtures and memory stores have no genuine recursive proof,
        // nonforking physical transport or native private-key custody.
        Err(Error::Unavailable)
    }
}

fn installed(source: Arc<Provisioner>) -> Arc<dyn KagemushaCoreCoordinatorBackendV1> {
    let mut state = InstallationState::new();
    let mut backend = None;
    state
        .install(source, "/durable/enrollment", |selected| {
            backend = Some(selected);
            Ok(())
        })
        .unwrap();
    backend.unwrap()
}

fn select(
    backend: &dyn KagemushaCoreCoordinatorBackendV1,
) -> KagemushaEnrollmentJournalSelectionV1 {
    assert_eq!(backend.open("/durable/enrollment"), Ok(1));
    let account = fixture::journal_account();
    let begin = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        account.as_bytes().to_vec(),
    ])
    .unwrap();
    let reply = backend.invoke_initial_enrollment(1, &begin).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&reply).unwrap();
    KagemushaEnrollmentJournalSelectionV1 {
        account_i105: account,
        ticket: u64::from_le_bytes(fields[0].as_slice().try_into().unwrap()),
        client_nonce: fields[1].as_slice().try_into().unwrap(),
        release_id: fields[2].as_slice().try_into().unwrap(),
        hardware_profile_id: fields[3].as_slice().try_into().unwrap(),
        lane_id: fields[4].as_slice().try_into().unwrap(),
    }
}

fn qualify(
    backend: &dyn KagemushaCoreCoordinatorBackendV1,
    selected: &KagemushaEnrollmentJournalSelectionV1,
) {
    let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
    let begin = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        command.clone(),
    ])
    .unwrap();
    let response = backend
        .invoke(
            1,
            KagemushaCoreCoordinatorMethodV1::BeginObservation,
            &begin,
        )
        .unwrap();
    let nonce = kagemusha_core_coordinator_decode_response_v1(&response).unwrap()[0]
        .as_slice()
        .try_into()
        .unwrap();
    let qualification = fixture::journal_qualification_fields(selected);
    backend
        .invoke(
            1,
            KagemushaCoreCoordinatorMethodV1::AcceptQualification,
            &kagemusha_core_coordinator_encode_request_v1(&qualification).unwrap(),
        )
        .unwrap();
    let original = fixture::journal_qualification_response(selected, nonce);
    let reply = kagemusha_decode_device_success_response_v1(&original, 1, nonce).unwrap();
    let mut fields = vec![
        1_u32.to_le_bytes().to_vec(),
        nonce.to_vec(),
        command,
        reply.payload.to_vec(),
        reply.authenticator.to_vec(),
    ];
    fields.extend(qualification[..5].iter().cloned());
    backend
        .invoke(
            1,
            KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply,
            &kagemusha_core_coordinator_encode_request_v1(&fields).unwrap(),
        )
        .unwrap();
}

fn prepare_finish(backend: &dyn KagemushaCoreCoordinatorBackendV1) -> Vec<u8> {
    let selection = select(backend);
    qualify(backend, &selection);
    let challenge = fixture::journal_challenge_fields(&selection);
    let preparation = kagemusha_core_coordinator_encode_request_v1(&[
        8_u32.to_le_bytes().to_vec(),
        selection.ticket.to_le_bytes().to_vec(),
        challenge[2].clone(),
    ])
    .unwrap();
    backend.invoke_initial_enrollment(1, &preparation).unwrap();
    backend
        .invoke_initial_enrollment(
            1,
            &kagemusha_core_coordinator_encode_request_v1(&challenge).unwrap(),
        )
        .unwrap();
    let proof = KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(
        &fixture::journal_proof_bytes(&selection),
    )
    .unwrap();
    let signature = Signature::from(proof.account_signature);
    backend
        .invoke_initial_enrollment(
            1,
            &kagemusha_core_coordinator_encode_request_v1(&[
                3_u32.to_le_bytes().to_vec(),
                selection.ticket.to_le_bytes().to_vec(),
                signature.payload().to_vec(),
                proof.device_response,
            ])
            .unwrap(),
        )
        .unwrap();
    let (certificate, _) = fixture::journal_certificate(&selection);
    kagemusha_core_coordinator_encode_request_v1(&[
        5_u32.to_le_bytes().to_vec(),
        selection.ticket.to_le_bytes().to_vec(),
        certificate,
    ])
    .unwrap()
}

#[test]
fn install_exact_path_is_idempotent_without_reopening_original_store() {
    let source = Arc::new(Provisioner::default());
    let mut state = InstallationState::new();
    state
        .install(source.clone(), "/durable/enrollment", |_| Ok(()))
        .unwrap();
    state
        .install(source.clone(), "/durable/enrollment", |_| {
            panic!("must not reinstall")
        })
        .unwrap();
    assert_eq!(
        state.install(source.clone(), "/other/owner", |_| panic!("must not adopt")),
        Err(Error::Rejected)
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    source.original_valid.store(false, Ordering::SeqCst);
    assert_eq!(
        state.install(source, "/durable/enrollment", |_| panic!(
            "changed original"
        )),
        Err(Error::Rejected)
    );
}

#[test]
fn uncertain_platform_or_global_install_never_retries_provisioning() {
    let source = Arc::new(Provisioner::default());
    source.fail_provision.store(true, Ordering::SeqCst);
    let mut state = InstallationState::new();
    assert_eq!(
        state.install(source.clone(), "/durable/enrollment", |_| panic!(
            "no backend"
        )),
        Err(Error::Unavailable)
    );
    source.fail_provision.store(false, Ordering::SeqCst);
    assert_eq!(
        state.install(source.clone(), "/durable/enrollment", |_| panic!(
            "uncertain retry"
        )),
        Err(Error::Rejected)
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    let mut second = InstallationState::new();
    assert_eq!(
        second.install(source.clone(), "/durable/enrollment", |_| Err(
            Error::Rejected
        )),
        Err(Error::Rejected)
    );
    assert_eq!(
        second.install(source.clone(), "/durable/enrollment", |_| panic!(
            "foreign backend"
        )),
        Err(Error::Rejected)
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn malformed_path_and_changed_originals_refuse_before_platform_io() {
    let source = Arc::new(Provisioner::default());
    let mut state = InstallationState::new();
    assert_eq!(
        state.install(source.clone(), "../other", |_| panic!("bad path")),
        Err(Error::Rejected)
    );
    source.original_valid.store(false, Ordering::SeqCst);
    assert_eq!(
        state.install(source.clone(), "/durable/enrollment", |_| panic!(
            "bad originals"
        )),
        Err(Error::Rejected)
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    assert!(!state.attempted);
}

#[test]
fn complete_native_enrollment_transfers_one_consuming_admission_and_replays_only_exact_finish() {
    let source = Arc::new(Provisioner::default());
    let backend = installed(source.clone());
    let finish = prepare_finish(backend.as_ref());
    let original = backend.invoke_initial_enrollment(1, &finish).unwrap();
    assert_eq!(backend.invoke_initial_enrollment(1, &finish), Ok(original));
    assert_eq!(source.handoffs.load(Ordering::SeqCst), 1);
    let mut changed = finish.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert_eq!(
        backend.invoke_initial_enrollment(1, &changed),
        Err(Error::Rejected)
    );
    // Successful enrollment is not a fabricated monetary owner or qualifying read authority.
    assert_eq!(
        backend.invoke(1, KagemushaCoreCoordinatorMethodV1::BeginObservation, &[]),
        Err(Error::Unavailable)
    );
}

#[test]
fn issuer_completion_cannot_reopen_recovery_without_concrete_authenticated_core() {
    let source = Arc::new(Provisioner::default());
    let backend = installed(source.clone());
    let finish = prepare_finish(backend.as_ref());
    backend.invoke_initial_enrollment(1, &finish).unwrap();
    backend.close(1).unwrap();
    assert_eq!(backend.open("/durable/enrollment"), Err(Error::Unavailable));
    // Retry neither reopens the issuer journal nor silently starts a new enrollment.
    assert_eq!(backend.open("/durable/enrollment"), Err(Error::Unavailable));
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(source.handoffs.load(Ordering::SeqCst), 1);
    let recovered =
        kagemusha_core_coordinator_encode_request_v1(&[9_u32.to_le_bytes().to_vec()]).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(1, &recovered),
        Err(Error::Rejected)
    );
}

#[test]
fn installed_wrapper_close_revokes_old_handle_without_a_second_initial_attempt() {
    let source = Arc::new(Provisioner::default());
    // Explicit test-only repeated inner handle; the production factory instead composes
    // NativeInitialSelectionBackend and never accepts this application-defined backend.
    let adapter = Arc::new(
        KagemushaEnrollmentPhaseOneBackendV1::new(
            Arc::new(Inner),
            Arc::new(
                KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default()))
                    .unwrap(),
            ),
            fixture::journal_pins(),
            "/durable/enrollment",
        )
        .unwrap(),
    );
    let backend = InstalledEnrollmentBackend {
        adapter,
        provisioner: source,
        path: "/durable/enrollment".into(),
        handoff: Mutex::new(None),
        bootstrap: Mutex::new(None),
        recovered: Mutex::new(None),
        routes: Mutex::new(InstalledRoutes {
            next: 1,
            opening: false,
            current: None,
        }),
    };
    assert_eq!(backend.open("/durable/enrollment"), Ok(1));
    backend.close(1).unwrap();
    assert_eq!(backend.open("/durable/enrollment"), Err(Error::Rejected));
    // This synthetic inner always returns7. Closing its sole initial attempt must
    // not revive either the old virtual handle or a guessed subsequent handle.
    let request = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        fixture::journal_account().as_bytes().to_vec(),
    ])
    .unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(1, &request),
        Err(Error::Rejected)
    );
    assert_eq!(backend.close(1), Err(Error::Rejected));
    assert_eq!(
        backend.invoke_initial_enrollment(2, &request),
        Err(Error::Rejected)
    );
}

#[test]
fn lost_admission_handoff_cannot_be_reconstructed_from_published_certificate() {
    let source = Arc::new(Provisioner::default());
    source.fail_handoff.store(true, Ordering::SeqCst);
    let backend = installed(source.clone());
    let finish = prepare_finish(backend.as_ref());
    assert_eq!(
        backend.invoke_initial_enrollment(1, &finish),
        Err(Error::Rejected)
    );
    source.fail_handoff.store(false, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(1, &finish),
        Err(Error::Rejected)
    );
    assert_eq!(source.handoffs.load(Ordering::SeqCst), 1);
}

#[test]
fn provisioning_policies_cannot_be_replaced_by_a_later_context_response() {
    let source = Arc::new(Provisioner::default());
    let backend = installed(source.clone());
    let _selection = select(backend.as_ref());
    source
        .context
        .substitute_policy
        .store(true, Ordering::SeqCst);
    let frame = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap(),
    ])
    .unwrap();
    assert_eq!(
        backend.invoke(
            1,
            KagemushaCoreCoordinatorMethodV1::BeginObservation,
            &frame
        ),
        Err(Error::Rejected)
    );
}

#[test]
fn historical_issuer_response_cannot_bootstrap_at_expired_native_service_time() {
    let source = Arc::new(Provisioner::default());
    let backend = installed(source.clone());
    let finish = prepare_finish(backend.as_ref());
    source.trusted_native_time.store(500_000, Ordering::SeqCst);
    // Historical issuer completion is retained, while current bootstrap evidence is checked
    // at the independently selected physical time when a new owner is requested.
    backend.invoke_initial_enrollment(1, &finish).unwrap();
    backend.close(1).unwrap();
    assert_eq!(backend.open("/durable/enrollment"), Err(Error::Rejected));
    source.trusted_native_time.store(1_500, Ordering::SeqCst);
    assert_eq!(backend.open("/durable/enrollment"), Err(Error::Rejected));
    // The retained selection is immutable; a later callback time cannot replace it.
    assert_eq!(source.handoffs.load(Ordering::SeqCst), 1);
}

#[test]
fn pinned_concrete_provisioner_consumes_only_original_path_and_selection() {
    struct TestCustody(AtomicBool);
    impl KagemushaNativeEnrollmentOriginalCustodyV1 for TestCustody {
        fn recheck_originals(&self) -> Result<(), Error> {
            if self.0.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(Error::Rejected)
            }
        }
    }
    let fixture = Provisioner::default();
    let selected = fixture.provision("/durable/enrollment").unwrap();
    let custody = Arc::new(TestCustody(AtomicBool::new(true)));
    let concrete = KagemushaPinnedNativeEnrollmentProvisionerV1::from_original_selection(
        "/durable/enrollment".into(),
        selected,
        custody.clone(),
        Arc::new(MissingPhysicalInputs {
            trusted_native_time: 1_500,
        }),
    )
    .unwrap();
    assert!(concrete.provision("/another-owner").is_err());
    custody.0.store(false, Ordering::SeqCst);
    assert!(concrete.provision("/durable/enrollment").is_err());
    custody.0.store(true, Ordering::SeqCst);
    assert!(concrete.provision("/durable/enrollment").is_ok());
    assert!(concrete.provision("/durable/enrollment").is_err());
    assert!(concrete.bootstrap_source("/durable/enrollment", 0).is_err());
    assert!(concrete.bootstrap_source("/another-owner", 1).is_err());
    assert!(concrete.bootstrap_source("/durable/enrollment", 1).is_ok());
}
