//! Enrollment adapter tests use an explicit fake delegate; no test fixture qualifies hardware.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use iroha_crypto::Signature;
use iroha_data_model::kagemusha::{
    KagemushaRetailEnrollmentCertificateV1, KagemushaRetailEnrollmentPossessionProofV1,
};

use super::*;
use crate::kagemusha_core_coordinator_v1::{
    KagemushaEnrollmentContextProviderV1, KagemushaEnrollmentJournalResultV1,
    KagemushaEnrollmentJournalStoreV1, KagemushaEnrollmentProvisionedContextV1,
    KagemushaKernelEnrollmentDelegateV1, kagemusha_core_coordinator_decode_request_v1,
    kagemusha_core_coordinator_decode_response_v1, kagemusha_core_coordinator_encode_request_v1,
};

struct FixedContextProvider;

impl KagemushaEnrollmentContextProviderV1 for FixedContextProvider {
    fn context_for_selection(
        &self,
        _: u64,
        live: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<KagemushaEnrollmentProvisionedContextV1, KagemushaCoreCoordinatorBackendErrorV1>
    {
        let selected = live
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        Ok(super::super::initial_enrollment::tests::journal_context(
            selected,
        ))
    }
}

#[derive(Default)]
struct MemoryStore {
    state: Mutex<(Option<Vec<u8>>, Option<u64>)>,
    fail_previous_revision: AtomicU64,
}

impl KagemushaEnrollmentJournalStoreV1 for MemoryStore {
    fn load_checked(&self) -> KagemushaEnrollmentJournalResultV1<Option<Vec<u8>>> {
        Ok(self.state.lock().unwrap().0.clone())
    }

    fn compare_and_swap(
        &self,
        previous_revision: Option<u64>,
        next: &[u8],
    ) -> KagemushaEnrollmentJournalResultV1<()> {
        if previous_revision == Some(self.fail_previous_revision.load(Ordering::SeqCst)) {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        let mut state = self.state.lock().unwrap();
        if previous_revision != state.1 {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.0 = Some(next.to_vec());
        state.1 = Some(previous_revision.unwrap_or(0) + 1);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum DelegateResult {
    Valid,
    Unavailable,
    Invalid,
}

struct Delegate {
    result: DelegateResult,
    challenges: AtomicUsize,
    proofs: AtomicUsize,
    fail_proof: AtomicBool,
    substitute_proof: AtomicBool,
    generic_challenges: AtomicUsize,
    seen_selection: Mutex<
        Vec<(
            KagemushaEnrollmentJournalSelectionV1,
            KagemushaEnrollmentJournalPinsV1,
        )>,
    >,
}

impl KagemushaCoreCoordinatorBackendV1 for Delegate {
    fn open(&self, _: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
        Ok(7)
    }

    fn invoke(
        &self,
        _: u64,
        _: KagemushaCoreCoordinatorMethodV1,
        _: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    }

    fn invoke_initial_enrollment(
        &self,
        _: u64,
        _: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        self.generic_challenges.fetch_add(1, Ordering::SeqCst);
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    }

    fn close(&self, _: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
        Ok(())
    }
}

impl KagemushaQualifiedEnrollmentDelegateV1 for Delegate {
    fn accept_challenge(
        &self,
        _: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        request_frame: &[u8],
    ) -> Result<AcceptedIssuerChallengeV1, KagemushaCoreCoordinatorBackendErrorV1> {
        self.challenges.fetch_add(1, Ordering::SeqCst);
        let selected = live_selection
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
            .clone();
        self.seen_selection
            .lock()
            .unwrap()
            .push((selected.clone(), live_selection.pins()));
        match self.result {
            DelegateResult::Unavailable => Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable),
            DelegateResult::Invalid => {
                let mut changed = selected;
                changed.client_nonce[0] ^= 1;
                Ok(super::super::initial_enrollment::tests::journal_accepted_challenge(&changed))
            }
            DelegateResult::Valid => {
                let fields = kagemusha_core_coordinator_decode_request_v1(request_frame).unwrap();
                assert_eq!(fields[1], selected.ticket.to_le_bytes());
                Ok(super::super::initial_enrollment::tests::journal_accepted_challenge(&selected))
            }
        }
    }

    fn prepare_proof(
        &self,
        _: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        accepted: AcceptedIssuerChallengeV1,
        raw_account_signature: &[u8],
        complete_device_response: &[u8],
    ) -> Result<PreparedIssuerProofV1, KagemushaCoreCoordinatorBackendErrorV1> {
        self.proofs.fetch_add(1, Ordering::SeqCst);
        let selected = live_selection
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
            .clone();
        self.seen_selection
            .lock()
            .unwrap()
            .push((selected.clone(), live_selection.pins()));
        if self.fail_proof.load(Ordering::SeqCst) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable);
        }
        if self.substitute_proof.load(Ordering::SeqCst) {
            let mut changed = selected;
            changed.client_nonce[0] ^= 1;
            let alternate =
                super::super::initial_enrollment::tests::journal_accepted_challenge(&changed);
            let proof = KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(
                &super::super::initial_enrollment::tests::journal_proof_bytes(&changed),
            )
            .unwrap();
            let signature = Signature::from(proof.account_signature);
            return alternate
                .prepare_proof(signature.payload(), &proof.device_response)
                .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        accepted
            .prepare_proof(raw_account_signature, complete_device_response)
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    }
}

fn selected_backend_with_store(
    result: DelegateResult,
    qualified: bool,
) -> (
    KagemushaEnrollmentPhaseOneBackendV1,
    Arc<Delegate>,
    Vec<u8>,
    Arc<MemoryStore>,
) {
    let delegate = Arc::new(Delegate {
        result,
        challenges: AtomicUsize::new(0),
        proofs: AtomicUsize::new(0),
        fail_proof: AtomicBool::new(false),
        substitute_proof: AtomicBool::new(false),
        generic_challenges: AtomicUsize::new(0),
        seen_selection: Mutex::new(Vec::new()),
    });
    let store = Arc::new(MemoryStore::default());
    let journal = Arc::new(KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap());
    let pins = super::super::initial_enrollment::tests::journal_pins();
    let backend = if qualified {
        KagemushaEnrollmentPhaseOneBackendV1::new_with_qualified_enrollment(
            delegate.clone(),
            delegate.clone(),
            journal,
            pins,
            "/durable/enrollment",
        )
        .unwrap()
    } else {
        KagemushaEnrollmentPhaseOneBackendV1::new(
            delegate.clone(),
            journal,
            pins,
            "/durable/enrollment",
        )
        .unwrap()
    };
    assert_eq!(backend.open("/durable/enrollment"), Ok(7));
    let begin = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        super::super::initial_enrollment::tests::journal_account().into_bytes(),
    ])
    .unwrap();
    backend.invoke_initial_enrollment(7, &begin).unwrap();
    let selection = backend.owner.lock().unwrap().selection.clone().unwrap();
    let challenge_fields =
        super::super::initial_enrollment::tests::journal_challenge_fields(&selection);
    let challenge = kagemusha_core_coordinator_encode_request_v1(&challenge_fields).unwrap();
    (backend, delegate, challenge, store)
}

fn selected_backend(
    result: DelegateResult,
    qualified: bool,
) -> (KagemushaEnrollmentPhaseOneBackendV1, Arc<Delegate>, Vec<u8>) {
    let (backend, delegate, challenge, _store) = selected_backend_with_store(result, qualified);
    (backend, delegate, challenge)
}

fn proof_request(backend: &KagemushaEnrollmentPhaseOneBackendV1) -> Vec<u8> {
    let selected = backend.owner.lock().unwrap().selection.clone().unwrap();
    let canonical = super::super::initial_enrollment::tests::journal_proof_bytes(&selected);
    let proof =
        KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(&canonical).unwrap();
    kagemusha_core_coordinator_encode_request_v1(&[
        3_u32.to_le_bytes().to_vec(),
        selected.ticket.to_le_bytes().to_vec(),
        Signature::from(proof.account_signature).payload().to_vec(),
        proof.device_response,
    ])
    .unwrap()
}

fn read_proof_request(backend: &KagemushaEnrollmentPhaseOneBackendV1) -> Vec<u8> {
    let ticket = backend
        .owner
        .lock()
        .unwrap()
        .selection
        .as_ref()
        .unwrap()
        .ticket;
    kagemusha_core_coordinator_encode_request_v1(&[
        4_u32.to_le_bytes().to_vec(),
        ticket.to_le_bytes().to_vec(),
    ])
    .unwrap()
}

fn finish_request(backend: &KagemushaEnrollmentPhaseOneBackendV1) -> Vec<u8> {
    let selected = backend.owner.lock().unwrap().selection.clone().unwrap();
    let (certificate, _) = super::super::initial_enrollment::tests::journal_certificate(&selected);
    kagemusha_core_coordinator_encode_request_v1(&[
        5_u32.to_le_bytes().to_vec(),
        selected.ticket.to_le_bytes().to_vec(),
        certificate,
    ])
    .unwrap()
}

fn cancel_request(backend: &KagemushaEnrollmentPhaseOneBackendV1) -> Vec<u8> {
    let ticket = backend
        .owner
        .lock()
        .unwrap()
        .selection
        .as_ref()
        .unwrap()
        .ticket;
    kagemusha_core_coordinator_encode_request_v1(&[
        6_u32.to_le_bytes().to_vec(),
        ticket.to_le_bytes().to_vec(),
    ])
    .unwrap()
}

#[test]
fn phase_two_persists_one_checked_challenge_then_replays_exact_bytes() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    let selection = backend.owner.lock().unwrap().selection.clone().unwrap();
    let first = backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&first).unwrap();
    assert_eq!(fields.len(), 4);
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
    assert_eq!(delegate.generic_challenges.load(Ordering::SeqCst), 0);
    assert_eq!(
        delegate.seen_selection.lock().unwrap().as_slice(),
        &[(selection, backend.pins)]
    );
    assert_eq!(backend.invoke_initial_enrollment(7, &challenge), Ok(first));
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);

    let mut changed = kagemusha_core_coordinator_decode_request_v1(&challenge).unwrap();
    changed[2][49] ^= 1;
    let changed = kagemusha_core_coordinator_encode_request_v1(&changed).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &changed),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
    assert_eq!(backend.close(7), Ok(()));
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
}

#[test]
fn consuming_kernel_delegate_completes_the_durable_phase_adapter_and_revokes_with_ticket() {
    let (mut backend, fake_delegate, challenge, _) =
        selected_backend_with_store(DelegateResult::Valid, true);
    backend.qualified_enrollment = Some(Arc::new(KagemushaKernelEnrollmentDelegateV1::new(
        Arc::new(FixedContextProvider),
    )));
    let selection = backend.owner.lock().unwrap().selection.clone().unwrap();

    let accepted = backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&accepted).unwrap();
    let original = kagemusha_core_coordinator_decode_request_v1(&challenge).unwrap();
    assert_eq!(fields[0], selection.ticket.to_le_bytes());
    assert_eq!(fields[1], original[7]);
    assert_eq!(fields[2], original[8]);
    assert_eq!(fields[3], original[9]);
    let proof = backend
        .invoke_initial_enrollment(7, &proof_request(&backend))
        .unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &read_proof_request(&backend)),
        Ok(proof)
    );
    let finish = finish_request(&backend);
    let response = backend.invoke_initial_enrollment(7, &finish).unwrap();
    let result = kagemusha_core_coordinator_decode_response_v1(&response).unwrap();
    let admission = backend.take_fresh_admission(7).unwrap();
    assert_eq!(result[1], admission.enrollment_binding().enrollment_id);
    assert_eq!(fake_delegate.challenges.load(Ordering::SeqCst), 0);
    assert_eq!(fake_delegate.proofs.load(Ordering::SeqCst), 0);

    let cancel = cancel_request(&backend);
    backend.invoke_initial_enrollment(7, &cancel).unwrap();
    assert!(admission.require_live().is_err());
}

#[test]
fn phase_two_uncertain_or_invalid_delegate_result_freezes_original_intent() {
    for result in [DelegateResult::Unavailable, DelegateResult::Invalid] {
        let (backend, delegate, challenge) = selected_backend(result, true);
        assert!(backend.invoke_initial_enrollment(7, &challenge).is_err());
        assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
        assert_eq!(
            backend.invoke_initial_enrollment(7, &challenge),
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        );
        assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn phase_two_uncertain_publication_never_retains_or_rebuilds_kernel_challenge() {
    let (backend, delegate, challenge, store) =
        selected_backend_with_store(DelegateResult::Valid, true);
    // Selection=revision 1, challenge intent=2. Reject the result CAS after the typed
    // accepted challenge has been built but before it can become process-local authority.
    store.fail_previous_revision.store(2, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert!(backend.owner.lock().unwrap().accepted.is_none());
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
}

#[test]
fn restart_cannot_reconstruct_process_local_challenge_from_retained_frame() {
    let (backend, delegate, challenge, store) =
        selected_backend_with_store(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let selected = backend.owner.lock().unwrap().selection.clone().unwrap();
    let pins = backend.pins;
    drop(backend); // Simulate process death without close or cancellation.

    let reopened = Arc::new(KagemushaEnrollmentAttemptJournalV1::open(store).unwrap());
    assert!(reopened.retain_live(selected, pins).is_err());
    let next = KagemushaEnrollmentPhaseOneBackendV1::new_with_qualified_enrollment(
        delegate.clone(),
        delegate.clone(),
        reopened,
        pins,
        "/durable/enrollment",
    )
    .unwrap();
    assert_eq!(next.open("/durable/enrollment"), Ok(7));
    assert_eq!(
        next.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
}

#[test]
fn phase_two_rejects_wrong_ticket_and_proof_before_challenge_without_delegate_dispatch() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    let mut wrong = kagemusha_core_coordinator_decode_request_v1(&challenge).unwrap();
    wrong[1] = 9_u64.to_le_bytes().to_vec();
    let wrong = kagemusha_core_coordinator_encode_request_v1(&wrong).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &wrong),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    let proof = proof_request(&backend);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &proof),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
    assert!(backend.invoke_initial_enrollment(7, &challenge).is_ok());
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
}

#[test]
fn phase_three_consumes_authenticated_challenge_once_and_phase_four_reads_exact_proof() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let proof_request = proof_request(&backend);
    let proof = backend
        .invoke_initial_enrollment(7, &proof_request)
        .unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&proof).unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &proof_request),
        Ok(proof.clone())
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &read_proof_request(&backend)),
        Ok(proof)
    );
    let mut changed = kagemusha_core_coordinator_decode_request_v1(&proof_request).unwrap();
    changed[2][0] ^= 1;
    let changed = kagemusha_core_coordinator_encode_request_v1(&changed).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &changed),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(delegate.seen_selection.lock().unwrap().len(), 2);
}

#[test]
fn phase_three_lost_result_and_invalid_signature_never_repeat_possession() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let request = proof_request(&backend);
    delegate.fail_proof.store(true, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
    );
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert!(
        backend
            .invoke_initial_enrollment(7, &read_proof_request(&backend))
            .is_err()
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);

    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let mut fields =
        kagemusha_core_coordinator_decode_request_v1(&proof_request(&backend)).unwrap();
    fields[2][0] ^= 1;
    let invalid = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &invalid),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(
        backend.invoke_initial_enrollment(7, &invalid),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
}

#[test]
fn phase_three_uncertain_publication_drops_consumed_challenge_and_freezes_retries() {
    let (backend, delegate, challenge, store) =
        selected_backend_with_store(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let request = proof_request(&backend);
    // Selected=revision 1, challenge intent/result=2/3, proof intent=4. Reject only the
    // proof-result CAS so the typed kernel state has already been consumed.
    store.fail_previous_revision.store(4, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    let owner = backend.owner.lock().unwrap();
    assert!(owner.accepted.is_none());
    assert!(owner.prepared.is_none());
    drop(owner);
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert!(
        backend
            .invoke_initial_enrollment(7, &read_proof_request(&backend))
            .is_err()
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
}

#[test]
fn phase_three_rejects_a_different_kernel_challenge_even_from_qualified_delegate() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    let request = proof_request(&backend);
    delegate.substitute_proof.store(true, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert!(
        backend
            .invoke_initial_enrollment(7, &read_proof_request(&backend))
            .is_err()
    );
}

#[test]
fn close_and_account_switch_drop_retained_kernel_states_and_revoke_proof_reads() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    assert!(backend.owner.lock().unwrap().accepted.is_some());
    let proof = proof_request(&backend);
    backend.invoke_initial_enrollment(7, &proof).unwrap();
    assert!(backend.owner.lock().unwrap().prepared.is_some());
    let read = read_proof_request(&backend);
    assert_eq!(backend.close(7), Ok(()));
    let owner = backend.owner.lock().unwrap();
    assert!(owner.selection.is_none());
    assert!(owner.accepted.is_none());
    assert!(owner.prepared.is_none());
    assert!(owner.challenge_id.is_none());
    drop(owner);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &proof),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(
        backend.invoke_initial_enrollment(7, &read),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);

    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    assert!(backend.owner.lock().unwrap().accepted.is_some());
    let proof = proof_request(&backend);
    assert_eq!(
        backend.open("/durable/another-account"),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    let owner = backend.owner.lock().unwrap();
    assert!(owner.selection.is_none());
    assert!(owner.accepted.is_none());
    assert!(owner.prepared.is_none());
    assert!(owner.challenge_id.is_none());
    drop(owner);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &proof),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 0);
}

#[test]
fn phase_five_authenticates_one_certificate_and_hands_admission_to_rust_once() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    backend
        .invoke_initial_enrollment(7, &proof_request(&backend))
        .unwrap();
    let finish = finish_request(&backend);
    let response = backend.invoke_initial_enrollment(7, &finish).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&response).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(backend.invoke_initial_enrollment(7, &finish), Ok(response));
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    let admission = backend.take_fresh_admission(7).unwrap();
    assert_eq!(fields[1], admission.enrollment_binding().enrollment_id);
    let request_fields = kagemusha_core_coordinator_decode_request_v1(&finish).unwrap();
    assert_eq!(admission.canonical_certificate(), request_fields[2]);
    assert_eq!(
        backend.take_fresh_admission(7).err(),
        Some(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
}

#[test]
fn phase_five_invalid_certificate_and_uncertain_publication_consume_original_proof() {
    let (backend, _delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    backend
        .invoke_initial_enrollment(7, &proof_request(&backend))
        .unwrap();
    let mut fields =
        kagemusha_core_coordinator_decode_request_v1(&finish_request(&backend)).unwrap();
    let mut certificate =
        KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(&fields[2]).unwrap();
    certificate.subject.challenge_evidence_digest[0] ^= 1;
    fields[2] = certificate.canonical_bytes().unwrap();
    let invalid = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &invalid),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert!(backend.owner.lock().unwrap().prepared.is_none());
    assert_eq!(
        backend.invoke_initial_enrollment(7, &invalid),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(
        backend.take_fresh_admission(7).err(),
        Some(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );

    let (backend, _delegate, challenge, store) =
        selected_backend_with_store(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    backend
        .invoke_initial_enrollment(7, &proof_request(&backend))
        .unwrap();
    let finish = finish_request(&backend);
    // Proof result is revision 5; reject the phase-5 result CAS after intent revision 6.
    store.fail_previous_revision.store(6, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &finish),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    let owner = backend.owner.lock().unwrap();
    assert!(owner.prepared.is_none());
    assert!(owner.fresh_admission.is_none());
    drop(owner);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &finish),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
}

#[test]
fn phase_six_revokes_ticket_and_all_kernel_states_with_exact_cancel_retry() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    backend.invoke_initial_enrollment(7, &challenge).unwrap();
    backend
        .invoke_initial_enrollment(7, &proof_request(&backend))
        .unwrap();
    backend
        .invoke_initial_enrollment(7, &finish_request(&backend))
        .unwrap();
    let admission = backend.take_fresh_admission(7).unwrap();
    // The fixed test kernel uses its test-only deadline constructor. Production's
    // `begin_selected` admission carries the original live journal ticket.
    assert!(admission.require_live().is_ok());
    let original_selection = backend.owner.lock().unwrap().selection.clone().unwrap();
    let cancel = cancel_request(&backend);
    let response = backend.invoke_initial_enrollment(7, &cancel).unwrap();
    assert!(
        kagemusha_core_coordinator_decode_response_v1(&response)
            .unwrap()
            .is_empty()
    );
    assert_eq!(backend.invoke_initial_enrollment(7, &cancel), Ok(response));
    assert!(
        backend
            .journal
            .retain_live(original_selection, backend.pins)
            .is_err()
    );
    let owner = backend.owner.lock().unwrap();
    assert!(owner.selection.is_none());
    assert!(owner.accepted.is_none());
    assert!(owner.prepared.is_none());
    assert!(owner.fresh_admission.is_none());
    drop(owner);
    assert_eq!(delegate.proofs.load(Ordering::SeqCst), 1);
    assert_eq!(
        backend.take_fresh_admission(7).err(),
        Some(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
}

#[test]
fn phase_six_uncertain_revocation_drops_all_process_local_authority() {
    let (backend, delegate, _challenge, store) =
        selected_backend_with_store(DelegateResult::Valid, true);
    let cancel = cancel_request(&backend);
    // The phase-1 selection is revision 1; fail the cancellation CAS while retaining
    // the process-local one-use attempt as consumed.
    store.fail_previous_revision.store(1, Ordering::SeqCst);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &cancel),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    let owner = backend.owner.lock().unwrap();
    assert!(owner.selection.is_none());
    assert!(owner.accepted.is_none());
    assert!(owner.prepared.is_none());
    assert!(owner.fresh_admission.is_none());
    assert_eq!(owner.cancelled_ticket, None);
    drop(owner);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &cancel),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
}

#[test]
fn phase_two_requires_typed_delegate_and_original_policy_pins() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, false);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
    assert_eq!(delegate.generic_challenges.load(Ordering::SeqCst), 0);

    let (mut backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    let original = backend.pins;
    backend.pins.app_policy_digest[0] ^= 1;
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
    backend.pins = original;
    assert!(backend.invoke_initial_enrollment(7, &challenge).is_ok());
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 1);
}

#[test]
fn phase_two_expired_original_deadline_never_reaches_typed_delegate() {
    let (backend, delegate, challenge) = selected_backend(DelegateResult::Valid, true);
    let ticket = backend
        .owner
        .lock()
        .unwrap()
        .selection
        .as_ref()
        .unwrap()
        .ticket;
    backend.journal.expire_ticket_for_test(ticket);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &challenge),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
    assert_eq!(delegate.generic_challenges.load(Ordering::SeqCst), 0);
}

#[test]
fn phase_seven_rejects_original_selection_after_native_deadline_expires() {
    let (backend, delegate, _) = selected_backend(DelegateResult::Valid, true);
    let request = kagemusha_core_coordinator_encode_request_v1(&[
        INITIAL_ENROLLMENT_READ_SELECTION_V1.to_le_bytes().to_vec(),
        super::super::initial_enrollment::tests::journal_account().into_bytes(),
    ])
    .unwrap();
    let ticket = backend
        .owner
        .lock()
        .unwrap()
        .selection
        .as_ref()
        .unwrap()
        .ticket;
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Ok(backend
            .owner
            .lock()
            .unwrap()
            .selection_response
            .clone()
            .unwrap())
    );
    backend.journal.expire_ticket_for_test(ticket);
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(delegate.challenges.load(Ordering::SeqCst), 0);
    assert_eq!(delegate.generic_challenges.load(Ordering::SeqCst), 0);
}
