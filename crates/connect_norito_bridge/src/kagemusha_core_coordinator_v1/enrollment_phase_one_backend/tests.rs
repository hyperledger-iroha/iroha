//! Phase-1 backend tests with explicit fake dependencies and no hardware authority.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use iroha_crypto::KeyPair;
use iroha_data_model::account::AccountId;

use super::*;
use crate::kagemusha_core_coordinator_v1::{
    KagemushaEnrollmentJournalResultV1, KagemushaEnrollmentJournalStoreV1,
    kagemusha_core_coordinator_decode_response_v1, kagemusha_core_coordinator_encode_request_v1,
};

#[derive(Default)]
struct MemoryStore(Mutex<(Option<Vec<u8>>, Option<u64>)>);

impl KagemushaEnrollmentJournalStoreV1 for MemoryStore {
    fn load_checked(&self) -> KagemushaEnrollmentJournalResultV1<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().0.clone())
    }

    fn compare_and_swap(
        &self,
        previous_revision: Option<u64>,
        next: &[u8],
    ) -> KagemushaEnrollmentJournalResultV1<()> {
        let mut state = self.0.lock().unwrap();
        if previous_revision != state.1 {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.0 = Some(next.to_vec());
        state.1 = Some(previous_revision.unwrap_or(0) + 1);
        Ok(())
    }
}

struct FailingStore;

impl KagemushaEnrollmentJournalStoreV1 for FailingStore {
    fn load_checked(&self) -> KagemushaEnrollmentJournalResultV1<Option<Vec<u8>>> {
        Ok(None)
    }

    fn compare_and_swap(&self, _: Option<u64>, _: &[u8]) -> KagemushaEnrollmentJournalResultV1<()> {
        Err(KagemushaEnrollmentJournalErrorV1::Store)
    }
}

#[derive(Default)]
struct Delegate {
    opens: AtomicUsize,
    generic_invokes: AtomicUsize,
    closes: AtomicUsize,
}

impl KagemushaCoreCoordinatorBackendV1 for Delegate {
    fn open(&self, _: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(7)
    }

    fn invoke(
        &self,
        _: u64,
        _: KagemushaCoreCoordinatorMethodV1,
        _: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        self.generic_invokes.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }

    fn close(&self, _: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn pins() -> KagemushaEnrollmentJournalPinsV1 {
    KagemushaEnrollmentJournalPinsV1 {
        release_id: [1; 32],
        hardware_profile_id: [2; 32],
        issuer_policy_id: [3; 32],
        app_policy_digest: [4; 32],
    }
}

fn begin() -> Vec<u8> {
    let account = AccountId::new(KeyPair::random().public_key().clone())
        .canonical_i105()
        .unwrap();
    kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        account.into_bytes(),
    ])
    .unwrap()
}

#[test]
fn phase_one_uses_original_pins_and_deadline_once_then_fails_closed() {
    let inner = Arc::new(Delegate::default());
    let journal = Arc::new(
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap(),
    );
    let backend = KagemushaEnrollmentPhaseOneBackendV1::new(
        inner.clone(),
        journal.clone(),
        pins(),
        "/durable/enrollment",
    )
    .unwrap();
    assert_eq!(
        backend.open("/durable/another"),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(inner.opens.load(Ordering::SeqCst), 0);
    assert_eq!(backend.open("/durable/enrollment"), Ok(7));
    assert_eq!(inner.opens.load(Ordering::SeqCst), 1);
    let request = begin();
    let response = backend.invoke_initial_enrollment(7, &request).unwrap();
    let fields = kagemusha_core_coordinator_decode_response_v1(&response).unwrap();
    assert_eq!(fields.len(), 7);
    assert_eq!(fields[2], pins().release_id);
    assert_eq!(fields[3], pins().hardware_profile_id);
    assert!(fields[5].iter().any(|byte| *byte != 0));
    assert_ne!(
        u64::from_le_bytes(fields[6].as_slice().try_into().unwrap()),
        0
    );
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
    let cancel = kagemusha_core_coordinator_encode_request_v1(&[
        6_u32.to_le_bytes().to_vec(),
        fields[0].clone(),
    ])
    .unwrap();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &cancel),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable),
    );
    assert_eq!(
        backend.invoke(
            7,
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
            &request,
        ),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
    assert_eq!(inner.generic_invokes.load(Ordering::SeqCst), 0);
    let selected = backend.owner.lock().unwrap().selection.clone().unwrap();
    let live = journal.retain_live(selected, pins()).unwrap();
    assert!(live.require_live().is_ok());
    assert_eq!(backend.close(7), Ok(()));
    assert_eq!(inner.closes.load(Ordering::SeqCst), 1);
    assert!(live.require_live().is_err());
    assert_eq!(
        backend.open("/durable/enrollment"),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
}

#[test]
fn phase_one_rejects_invalid_provisioning_and_never_retries_uncertain_store() {
    let inner = Arc::new(Delegate::default());
    let journal =
        Arc::new(KagemushaEnrollmentAttemptJournalV1::open(Arc::new(FailingStore)).unwrap());
    assert!(
        KagemushaEnrollmentPhaseOneBackendV1::new(
            inner.clone(),
            journal.clone(),
            pins(),
            "relative/path",
        )
        .is_err()
    );
    let mut missing = pins();
    missing.app_policy_digest = [0; 32];
    assert!(
        KagemushaEnrollmentPhaseOneBackendV1::new(
            inner.clone(),
            journal.clone(),
            missing,
            "/durable/enrollment",
        )
        .is_err()
    );
    let backend =
        KagemushaEnrollmentPhaseOneBackendV1::new(inner, journal, pins(), "/durable/enrollment")
            .unwrap();
    assert_eq!(backend.open("/durable/enrollment"), Ok(7));
    let request = begin();
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
    assert_eq!(
        backend.invoke_initial_enrollment(7, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
    assert_eq!(
        backend.invoke_initial_enrollment(8, &request),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected),
    );
}
