//! Adversarial journal tests; the memory store models monotonic CAS without hardware claims.
use super::*;
use crate::kagemusha_core_coordinator_v1::{
    kagemusha_core_coordinator_decode_response_v1, kagemusha_core_coordinator_encode_request_v1,
};
use iroha_crypto::KeyPair;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MemoryStore(Mutex<(Option<Vec<u8>>, bool)>);
impl MemoryStore {
    fn fail_after_write_once(&self) {
        self.0.lock().unwrap().1 = true;
    }
}
impl KagemushaEnrollmentJournalStoreV1 for MemoryStore {
    fn load_checked(&self) -> Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().0.clone())
    }
    fn compare_and_swap(&self, previous_revision: Option<u64>, next: &[u8]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        let current = state
            .0
            .as_ref()
            .map(|bytes| ImageV1::decode_checked(bytes).unwrap().revision);
        if current != previous_revision
            || ImageV1::decode_checked(next)?.revision != current.unwrap_or(0) + 1
        {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.0 = Some(next.to_vec());
        if state.1 {
            state.1 = false;
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        Ok(())
    }
}
fn pins() -> KagemushaEnrollmentJournalPinsV1 {
    super::super::initial_enrollment::tests::journal_pins()
}
fn account() -> String {
    super::super::initial_enrollment::tests::journal_account()
}
fn other_account() -> String {
    AccountId::new(KeyPair::random().public_key().clone())
        .canonical_i105()
        .unwrap()
}
fn frame(fields: &[Vec<u8>]) -> Vec<u8> {
    kagemusha_core_coordinator_encode_request_v1(fields).unwrap()
}
fn begin(identity: &str) -> Vec<u8> {
    frame(&[1_u32.to_le_bytes().to_vec(), identity.as_bytes().to_vec()])
}
fn select(
    journal: &KagemushaEnrollmentAttemptJournalV1,
    identity: &str,
) -> KagemushaEnrollmentJournalSelectionV1 {
    let request = begin(identity);
    let (selection, response) = journal.select(&request, pins()).unwrap();
    archive_boundary::validate_response(
        KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
        &request,
        &response,
    )
    .unwrap();
    selection
}

#[test]
fn phase_one_exports_original_owner_scope_and_fixed_native_continuous_expiry() {
    let journal =
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap();
    let request = begin(&account());
    let (selection, response) = journal.select(&request, pins()).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&response).unwrap();
    assert_eq!(fields.len(), 7);
    assert_eq!(fields[0], selection.ticket.to_le_bytes());
    assert_eq!(fields[1], selection.client_nonce);
    assert_eq!(fields[2], selection.release_id);
    assert_eq!(fields[3], selection.hardware_profile_id);
    assert_eq!(fields[4], selection.lane_id);
    assert_eq!(fields[5].len(), 32);
    assert!(fields[5].iter().any(|byte| *byte != 0));
    let exported = u64::from_le_bytes(fields[6].as_slice().try_into().unwrap());
    assert_eq!(
        exported,
        journal
            .state
            .lock()
            .unwrap()
            .deadlines
            .get(&selection.ticket)
            .unwrap()
            .expiry_continuous_ms()
            .unwrap(),
    );
    archive_boundary::validate_response(
        KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
        &request,
        &response,
    )
    .unwrap();
}

#[test]
fn live_selection_retains_original_ticket_and_deadline_until_revoked() {
    let journal = Arc::new(
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap(),
    );
    let selected = select(&journal, &account());
    let original = selected.clone();
    let live = journal.retain_live(selected, pins()).unwrap();
    assert_eq!(live.require_live().unwrap(), &original);
    assert_eq!(live.pins(), pins());
    assert!(live.deadline().unwrap().check().is_ok());
    let mut substituted = original.clone();
    substituted.client_nonce[0] ^= 1;
    assert!(journal.retain_live(substituted, pins()).is_err());
    journal.revoke_all().unwrap();
    assert!(matches!(
        live.require_live(),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
    assert!(matches!(
        live.deadline(),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
}

#[test]
fn live_selection_cannot_transfer_an_expired_original_deadline() {
    let journal = Arc::new(
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap(),
    );
    let selected = select(&journal, &account());
    let live = journal.retain_live(selected.clone(), pins()).unwrap();
    journal
        .state
        .lock()
        .unwrap()
        .deadlines
        .insert(selected.ticket, NativeDeadlineV1::expired_for_test());
    assert!(matches!(
        live.deadline(),
        Err(KagemushaEnrollmentJournalErrorV1::Expired)
    ));
    assert!(matches!(
        live.require_live(),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
}

#[test]
fn revision_exhaustion_during_revocation_poisoned_live_selection() {
    let store = Arc::new(MemoryStore::default());
    store.0.lock().unwrap().0 = Some(
        ImageV1 {
            version: VERSION,
            revision: u64::MAX - 1,
            records: Vec::new(),
        }
        .encode_checked()
        .unwrap(),
    );
    let journal = Arc::new(KagemushaEnrollmentAttemptJournalV1::open(store).unwrap());
    let selected = select(&journal, &account());
    let live = journal.retain_live(selected, pins()).unwrap();
    assert!(live.require_live().is_ok());

    assert_eq!(
        journal.revoke_all(),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    );
    assert_eq!(
        live.require_live(),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    );
    assert_eq!(
        journal.select(&begin(&other_account()), pins()).err(),
        Some(KagemushaEnrollmentJournalErrorV1::Store)
    );
}

fn challenge_request(selected: &KagemushaEnrollmentJournalSelectionV1) -> Vec<u8> {
    frame(&super::super::initial_enrollment::tests::journal_challenge_fields(selected))
}
fn challenge_response(request: &[u8]) -> Vec<u8> {
    let f = kagemusha_core_coordinator_decode_request_v1(request).unwrap();
    kagemusha_core_coordinator_encode_response_v1(&[
        f[1].clone(),
        f[7].clone(),
        f[8].clone(),
        f[9].clone(),
    ])
    .unwrap()
}
fn reservation(
    dispatch: KagemushaEnrollmentJournalDispatchV1,
) -> KagemushaEnrollmentJournalReservationV1 {
    match dispatch {
        KagemushaEnrollmentJournalDispatchV1::Execute(value) => value,
        KagemushaEnrollmentJournalDispatchV1::Retained(_) => panic!("unexpected cached result"),
    }
}
#[test]
fn selection_is_unique_and_latches_account_lane_and_policy() {
    let journal =
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap();
    let identity = account();
    let s = select(&journal, &identity);
    assert_ne!(s.ticket, 0);
    assert_ne!(s.client_nonce, [0; 32]);
    assert_ne!(s.lane_id, [0; 32]);
    assert_eq!(s.release_id, pins().release_id);
    assert_eq!(s.hardware_profile_id, pins().hardware_profile_id);
    assert!(matches!(
        journal.select(&begin(&identity), pins()),
        Err(KagemushaEnrollmentJournalErrorV1::Conflict)
    ));
    assert!(journal.select(&begin("not-an-account"), pins()).is_err());
}
#[test]
fn crash_after_intent_never_dispatches_second_challenge_or_lane() {
    let store = Arc::new(MemoryStore::default());
    let journal = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let identity = account();
    let s = select(&journal, &identity);
    let request = challenge_request(&s);
    let _intent = reservation(journal.reserve(&s, &request).unwrap());
    assert!(matches!(
        journal.reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
    drop(journal);
    let reopened = KagemushaEnrollmentAttemptJournalV1::open(store).unwrap();
    assert!(matches!(
        reopened.reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
    assert!(matches!(
        reopened.select(&begin(&identity), pins()),
        Err(KagemushaEnrollmentJournalErrorV1::Conflict)
    ));
}
#[test]
fn exact_published_result_survives_restart_then_revocation_stops_recovery() {
    let store = Arc::new(MemoryStore::default());
    let journal = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let s = select(&journal, &account());
    let request = challenge_request(&s);
    let response = challenge_response(&request);
    journal
        .publish(
            reservation(journal.reserve(&s, &request).unwrap()),
            &response,
        )
        .unwrap();
    drop(journal);
    let reopened = KagemushaEnrollmentAttemptJournalV1::open(store).unwrap();
    assert!(
        matches!(reopened.reserve(&s, &request), Ok(KagemushaEnrollmentJournalDispatchV1::Retained(bytes)) if bytes == response)
    );
    let mut altered = kagemusha_core_coordinator_decode_request_v1(&request).unwrap();
    altered[2][49] ^= 1;
    assert!(matches!(
        reopened.reserve(&s, &frame(&altered)),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen | KagemushaEnrollmentJournalErrorV1::Invalid)
    ));
    reopened
        .cancel(
            &s,
            &frame(&[
                6_u32.to_le_bytes().to_vec(),
                s.ticket.to_le_bytes().to_vec(),
            ]),
        )
        .unwrap();
    assert!(matches!(
        reopened.reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
}
#[test]
fn uncertain_cas_poisons_process_and_reopen_cannot_repeat_io() {
    let store = Arc::new(MemoryStore::default());
    let journal = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let s = select(&journal, &account());
    let request = challenge_request(&s);
    store.fail_after_write_once();
    assert!(matches!(
        journal.reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
    assert!(matches!(
        journal.reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
    drop(journal);
    assert!(matches!(
        KagemushaEnrollmentAttemptJournalV1::open(store)
            .unwrap()
            .reserve(&s, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
}
#[test]
fn wrong_preparation_lane_fails_before_intent() {
    let journal =
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap();
    let s = select(&journal, &account());
    let request = challenge_request(&s);
    let mut fields = kagemusha_core_coordinator_decode_request_v1(&request).unwrap();
    fields[2][177] ^= 1;
    assert!(matches!(
        journal.reserve(&s, &frame(&fields)),
        Err(KagemushaEnrollmentJournalErrorV1::Invalid)
    ));
    assert!(matches!(
        journal.reserve(&s, &request),
        Ok(KagemushaEnrollmentJournalDispatchV1::Execute(_))
    ));
}
#[test]
fn stale_parallel_owner_cannot_allocate_second_ticket() {
    let store = Arc::new(MemoryStore::default());
    let first = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let second = KagemushaEnrollmentAttemptJournalV1::open(store).unwrap();
    let _ = select(&first, &account());
    let request = begin(&other_account());
    assert!(matches!(
        second.select(&request, pins()),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
    assert!(matches!(
        second.select(&request, pins()),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
}

#[test]
fn all_published_phase_results_recover_exactly_after_later_phases_and_restart() {
    let store = Arc::new(MemoryStore::default());
    let journal = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let selected = select(&journal, &account());
    let challenge = challenge_request(&selected);
    let challenge_result = challenge_response(&challenge);
    journal
        .publish(
            reservation(journal.reserve(&selected, &challenge).unwrap()),
            &challenge_result,
        )
        .unwrap();
    let proof = frame(&[
        3_u32.to_le_bytes().to_vec(),
        selected.ticket.to_le_bytes().to_vec(),
        vec![21; 64],
        vec![22; 32],
    ]);
    let proof_result = kagemusha_core_coordinator_encode_response_v1(&[
        selected.ticket.to_le_bytes().to_vec(),
        kagemusha_core_coordinator_decode_request_v1(&challenge).unwrap()[6].clone(),
        super::super::initial_enrollment::tests::journal_proof_bytes(&selected),
    ])
    .unwrap();
    journal
        .publish(
            reservation(journal.reserve(&selected, &proof).unwrap()),
            &proof_result,
        )
        .unwrap();
    let read = frame(&[
        4_u32.to_le_bytes().to_vec(),
        selected.ticket.to_le_bytes().to_vec(),
    ]);
    assert_eq!(journal.read_proof(&selected, &read).unwrap(), proof_result);
    let (certificate, enrollment_id) =
        super::super::initial_enrollment::tests::journal_certificate(&selected);
    let finish = frame(&[
        5_u32.to_le_bytes().to_vec(),
        selected.ticket.to_le_bytes().to_vec(),
        certificate,
    ]);
    let finish_result = kagemusha_core_coordinator_encode_response_v1(&[
        selected.ticket.to_le_bytes().to_vec(),
        enrollment_id.to_vec(),
    ])
    .unwrap();
    journal
        .publish(
            reservation(journal.reserve(&selected, &finish).unwrap()),
            &finish_result,
        )
        .unwrap();
    drop(journal);
    let reopened = KagemushaEnrollmentAttemptJournalV1::open(store).unwrap();
    for (request, expected) in [
        (&challenge, &challenge_result),
        (&proof, &proof_result),
        (&finish, &finish_result),
    ] {
        assert!(matches!(reopened.reserve(&selected, request),
            Ok(KagemushaEnrollmentJournalDispatchV1::Retained(bytes)) if bytes == *expected));
    }
    assert_eq!(reopened.read_proof(&selected, &read).unwrap(), proof_result);
}

#[test]
fn invalid_publish_consumes_intent_and_never_retries() {
    let journal =
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap();
    let selected = select(&journal, &account());
    let request = challenge_request(&selected);
    let intent = reservation(journal.reserve(&selected, &request).unwrap());
    assert!(matches!(
        journal.publish(intent, b"invalid response"),
        Err(KagemushaEnrollmentJournalErrorV1::Invalid)
    ));
    assert!(matches!(
        journal.reserve(&selected, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Frozen)
    ));
}

#[test]
fn wrong_owner_scope_cannot_recover_or_consume_selected_lane() {
    let journal =
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap();
    let selected = select(&journal, &account());
    let request = challenge_request(&selected);
    let mut wrong = selected.clone();
    wrong.lane_id[0] ^= 1;
    assert!(matches!(
        journal.reserve(&wrong, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Invalid)
    ));
    wrong = selected.clone();
    wrong.account_i105 = other_account();
    assert!(matches!(
        journal.reserve(&wrong, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Invalid)
    ));
    assert!(matches!(
        journal.reserve(&selected, &request),
        Ok(KagemushaEnrollmentJournalDispatchV1::Execute(_))
    ));
}

#[test]
fn concurrent_revocation_invalidates_a_stale_cached_result() {
    let store = Arc::new(MemoryStore::default());
    let journal = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let selected = select(&journal, &account());
    let request = challenge_request(&selected);
    journal
        .publish(
            reservation(journal.reserve(&selected, &request).unwrap()),
            &challenge_response(&request),
        )
        .unwrap();
    drop(journal);
    let stale = KagemushaEnrollmentAttemptJournalV1::open(store.clone()).unwrap();
    let revoker = KagemushaEnrollmentAttemptJournalV1::open(store).unwrap();
    revoker
        .cancel(
            &selected,
            &frame(&[
                6_u32.to_le_bytes().to_vec(),
                selected.ticket.to_le_bytes().to_vec(),
            ]),
        )
        .unwrap();
    assert!(matches!(
        stale.reserve(&selected, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
    assert!(matches!(
        stale.reserve(&selected, &request),
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    ));
}
