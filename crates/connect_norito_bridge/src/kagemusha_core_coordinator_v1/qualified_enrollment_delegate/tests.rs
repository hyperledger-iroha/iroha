//! Consuming delegate tests use fixed signed catalog fixtures and a test-only journal store.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use iroha_crypto::Signature;
use iroha_data_model::kagemusha::KagemushaRetailEnrollmentPossessionProofV1;

use super::*;
use crate::kagemusha_core_coordinator_v1::{
    KagemushaEnrollmentAttemptJournalV1, KagemushaEnrollmentJournalErrorV1,
    KagemushaEnrollmentJournalResultV1, KagemushaEnrollmentJournalSelectionV1,
    KagemushaEnrollmentJournalStoreV1, kagemusha_core_coordinator_encode_request_v1,
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
        if state.1 != previous_revision {
            return Err(KagemushaEnrollmentJournalErrorV1::Store);
        }
        state.0 = Some(next.to_vec());
        state.1 = Some(previous_revision.unwrap_or(0) + 1);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ContextMode {
    Valid,
    WrongOwnerLane,
    AlteredPlatformEvidence,
    OversizedPlatformEvidence,
}

struct ContextProvider {
    mode: ContextMode,
    calls: AtomicUsize,
}

impl KagemushaEnrollmentContextProviderV1 for ContextProvider {
    fn context_for_selection(
        &self,
        _: u64,
        live: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<KagemushaEnrollmentProvisionedContextV1, KagemushaCoreCoordinatorBackendErrorV1>
    {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let selected = live
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        let mut context = super::super::initial_enrollment::tests::journal_context(selected);
        match self.mode {
            ContextMode::Valid => {}
            ContextMode::WrongOwnerLane => context.owner.lane_id[0] ^= 1,
            ContextMode::AlteredPlatformEvidence => context.raw_platform_evidence[0] ^= 1,
            ContextMode::OversizedPlatformEvidence => {
                context.raw_platform_evidence = vec![1; 128 * 1024 + 1]
            }
        }
        Ok(context)
    }
}

fn selected_journal() -> (
    Arc<KagemushaEnrollmentAttemptJournalV1>,
    KagemushaEnrollmentJournalSelectionV1,
) {
    let journal = Arc::new(
        KagemushaEnrollmentAttemptJournalV1::open(Arc::new(MemoryStore::default())).unwrap(),
    );
    let account = super::super::initial_enrollment::tests::journal_account();
    let begin = kagemusha_core_coordinator_encode_request_v1(&[
        1_u32.to_le_bytes().to_vec(),
        account.into_bytes(),
    ])
    .unwrap();
    let (selection, _) = journal
        .select(
            &begin,
            super::super::initial_enrollment::tests::journal_pins(),
        )
        .unwrap();
    (journal, selection)
}

fn challenge_request(selection: &KagemushaEnrollmentJournalSelectionV1) -> Vec<u8> {
    kagemusha_core_coordinator_encode_request_v1(
        &super::super::initial_enrollment::tests::journal_challenge_fields(selection),
    )
    .unwrap()
}

#[test]
fn delegate_consumes_original_selection_and_prepares_exact_proof() {
    let (journal, selection) = selected_journal();
    let provider = Arc::new(ContextProvider {
        mode: ContextMode::Valid,
        calls: AtomicUsize::new(0),
    });
    let delegate = KagemushaKernelEnrollmentDelegateV1::new(provider.clone());
    let request = challenge_request(&selection);
    let accepted = delegate
        .accept_challenge(
            7,
            journal
                .retain_live(
                    selection.clone(),
                    super::super::initial_enrollment::tests::journal_pins(),
                )
                .unwrap(),
            &request,
        )
        .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let canonical = super::super::initial_enrollment::tests::journal_proof_bytes(&selection);
    let proof =
        KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(&canonical).unwrap();
    let account_signature = Signature::from(proof.account_signature);
    let prepared = delegate
        .prepare_proof(
            7,
            journal
                .retain_live(
                    selection,
                    super::super::initial_enrollment::tests::journal_pins(),
                )
                .unwrap(),
            accepted,
            account_signature.payload(),
            &proof.device_response,
        )
        .unwrap();
    assert_eq!(prepared.canonical_proof().unwrap(), canonical);
}

#[test]
fn delegate_rejects_replaced_ticket_before_requesting_context() {
    let (journal, selection) = selected_journal();
    let provider = Arc::new(ContextProvider {
        mode: ContextMode::Valid,
        calls: AtomicUsize::new(0),
    });
    let delegate = KagemushaKernelEnrollmentDelegateV1::new(provider.clone());
    let mut fields = super::super::initial_enrollment::tests::journal_challenge_fields(&selection);
    fields[1] = selection.ticket.wrapping_add(1).to_le_bytes().to_vec();
    let request = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
    let result = delegate.accept_challenge(
        7,
        journal
            .retain_live(
                selection,
                super::super::initial_enrollment::tests::journal_pins(),
            )
            .unwrap(),
        &request,
    );
    assert!(matches!(
        result,
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn delegate_rejects_independently_provisioned_context_substitution() {
    for mode in [
        ContextMode::WrongOwnerLane,
        ContextMode::AlteredPlatformEvidence,
        ContextMode::OversizedPlatformEvidence,
    ] {
        let (journal, selection) = selected_journal();
        let provider = Arc::new(ContextProvider {
            mode,
            calls: AtomicUsize::new(0),
        });
        let delegate = KagemushaKernelEnrollmentDelegateV1::new(provider.clone());
        let request = challenge_request(&selection);
        let result = delegate.accept_challenge(
            7,
            journal
                .retain_live(
                    selection,
                    super::super::initial_enrollment::tests::journal_pins(),
                )
                .unwrap(),
            &request,
        );
        assert!(matches!(
            result,
            Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn delegate_rejects_a_different_live_journal_for_an_accepted_challenge() {
    let (first_journal, first_selection) = selected_journal();
    let provider = Arc::new(ContextProvider {
        mode: ContextMode::Valid,
        calls: AtomicUsize::new(0),
    });
    let delegate = KagemushaKernelEnrollmentDelegateV1::new(provider);
    let accepted = delegate
        .accept_challenge(
            7,
            first_journal
                .retain_live(
                    first_selection.clone(),
                    super::super::initial_enrollment::tests::journal_pins(),
                )
                .unwrap(),
            &challenge_request(&first_selection),
        )
        .unwrap();
    let (other_journal, other_selection) = selected_journal();
    let result = delegate.prepare_proof(
        7,
        other_journal
            .retain_live(
                other_selection,
                super::super::initial_enrollment::tests::journal_pins(),
            )
            .unwrap(),
        accepted,
        &[],
        &[],
    );
    assert!(matches!(
        result,
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    ));
}

#[test]
fn fixed_projection_and_kernel_error_mapping_fail_closed() {
    assert_eq!(fixed_32(&[7; 32]), Ok([7; 32]));
    assert_eq!(
        fixed_32(&[7; 31]),
        Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
    );
    assert_eq!(
        map_kernel_error(InitialEnrollmentErrorV1::Authority),
        KagemushaCoreCoordinatorBackendErrorV1::Rejected
    );
    assert_eq!(
        map_kernel_error(InitialEnrollmentErrorV1::RandomUnavailable),
        KagemushaCoreCoordinatorBackendErrorV1::Unavailable
    );
}
