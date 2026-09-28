//! Original remote witness ownership under actual pool pressure and bounded driver retry.

use super::*;
use iroha_sumeragi::message::ResultWitness;
use mv::allocation::{AllocationBudget, ChargedBuffer};

fn fixture(
    budget: &AllocationBudget,
) -> (DriverHandle, mpsc::Receiver<Input>, PublicKey, WireMessage) {
    let block = tests::block(2, Hash32([1; 32]), Hash32([2; 32]), Vec::new());
    let mut qc = tests::commit_qc(&block, Hash32([3; 32]));
    qc.attestation_witness = Some(ResultWitness::from_untrusted(vec![7; 200]).unwrap());
    let shared = Arc::new(Shared {
        allocation_budget: budget.clone(),
        pending_admission: Mutex::new(None),
        instance: block.header.instance,
        own: Vec::new(),
        ingress: Arc::new(Mutex::new(Ingress::new(IngressLimits::default()))),
        frame_limit: 1 << 20,
        status: Mutex::new(None),
        backlog: Mutex::new(Backlog::default()),
        wake_pending: AtomicBool::new(false),
        alive: AtomicBool::new(true),
        stopped: Mutex::new(None),
    });
    let (inputs, rx) = mpsc::channel();
    (
        DriverHandle { shared, inputs },
        rx,
        PublicKey::new(vec![9; 48]).unwrap(),
        WireMessage::Qc(qc),
    )
}

fn witness(message: &WireMessage) -> &ResultWitness {
    let WireMessage::Qc(qc) = message else {
        panic!("fixture QC")
    };
    qc.attestation_witness.as_ref().unwrap()
}

#[test]
fn pending_remote_frame_preserves_original_backing_and_never_enters_core_early() {
    let budget = AllocationBudget::new(4096);
    let occupied = ChargedBuffer::<u8>::new(3896, &budget).unwrap();
    let (handle, _rx, peer, message) = fixture(&budget);
    let another = message.clone();
    let canonical = message.encode().unwrap();
    assert!(handle.deliver_message(peer.clone(), message));
    assert!(handle.shared.ingress.lock().is_empty());
    let pointer = {
        let slot = handle.shared.pending_admission.lock();
        let pending = slot.as_ref().unwrap();
        assert_eq!(pending.message.encode().unwrap(), canonical);
        assert!(!witness(&pending.message).admitted_to(&budget));
        witness(&pending.message).as_slice().as_ptr()
    };
    assert_eq!(budget.reserved_bytes(), 4096);
    assert!(!handle.deliver_message(peer.clone(), another));
    assert!(!handle.shared.retry_pending_message());
    assert!(handle.shared.ingress.lock().is_empty());
    assert_eq!(budget.reserved_bytes(), 4096);
    {
        let slot = handle.shared.pending_admission.lock();
        assert_eq!(
            witness(&slot.as_ref().unwrap().message).as_slice().as_ptr(),
            pointer
        );
    }
    // Source-independent control traffic continues while the one admission slot is occupied.
    let block = tests::block(2, Hash32([1; 32]), Hash32([2; 32]), Vec::new());
    let ordinary = WireMessage::Qc(tests::commit_qc(&block, Hash32([3; 32])));
    assert!(ordinary.attestation_witnesses_admitted_to(&budget));
    assert!(handle.deliver_message(peer, ordinary));
    assert_eq!(handle.shared.ingress.lock().len(), 1);
    handle.shared.ingress.lock().pop();
    drop(occupied);
    assert!(handle.shared.retry_pending_message());
    assert!(handle.shared.pending_admission.lock().is_none());
    let (_, admitted) = handle.shared.ingress.lock().pop().unwrap();
    assert!(witness(&admitted).admitted_to(&budget));
    assert_eq!(witness(&admitted).as_slice().as_ptr(), pointer);
    assert_eq!(admitted.encode().unwrap(), canonical);
    assert!(!handle.shared.retry_pending_message());
    drop(admitted);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn pending_slot_rejects_foreign_owners_and_retained_handle_cannot_revive_stopped_instance() {
    let budget = AllocationBudget::new(4096);
    let foreign = AllocationBudget::new(4096);
    let (handle, _rx, peer, mut message) = fixture(&budget);
    message.admit_attestation_witnesses(&foreign).unwrap();
    assert!(!handle.deliver_message(peer.clone(), message));
    assert_eq!(foreign.reserved_bytes(), 0);
    assert!(handle.shared.pending_admission.lock().is_none());
    let occupied = ChargedBuffer::<u8>::new(3896, &budget).unwrap();
    let (_, _, _, message) = fixture(&budget);
    assert!(handle.deliver_message(peer.clone(), message.clone()));
    assert_eq!(budget.reserved_bytes(), 4096);
    assert!(handle.shared.pending_admission.lock().is_some());
    drop(LoopGuard {
        shared: Arc::clone(&handle.shared),
        observer: Arc::new(tests::fakes::RecordingObserver::default()),
    });
    assert!(!handle.shared.alive.load(Ordering::Acquire));
    assert_eq!(budget.reserved_bytes(), 3896);
    assert!(!handle.deliver_message(peer, message));
    assert!(handle.shared.pending_admission.lock().is_none());
    assert!(handle.shared.ingress.lock().is_empty());
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), 0);
}
