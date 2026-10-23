//! Numeric capability/lifetime controls against the actual Core scheduler; no proof fixture.

use super::*;
use crate::kagemusha_wallet_ffi::tests::TestWallet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

fn fixture() -> (
    Arc<Mutex<Registry>>,
    Arc<Owner>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let owner = Arc::new(Owner {
        background: background::Background::default(),
        closing: Arc::new(closing::CloseState::default()),
        exchanges: Mutex::new(Tokens::default()),
        scheduler: state::Scheduler::new(),
        wallet: Mutex::new(Some(Box::new(TestWallet {
            calls: Arc::clone(&calls),
            drops: Arc::clone(&drops),
            expected_request: None,
        }))),
    });
    let mut registry = Registry::default();
    registry.owners.insert(1, Arc::clone(&owner));
    (Arc::new(Mutex::new(registry)), owner, calls, drops)
}

/// Discovery-only boundary fixture; it cannot issue a financial operation or proof.
struct DiscoveryWallet {
    inner: Box<dyn Wallet>,
    observed: mpsc::Sender<()>,
}
impl Wallet for DiscoveryWallet {
    fn maintenance(&mut self) -> Result<maintenance::View> {
        self.observed.send(()).unwrap();
        Ok(maintenance::View::BeforeBootstrap)
    }
    fn snapshot(&mut self) -> Result<state::Snapshot> {
        self.inner.snapshot()
    }
    fn setup(&mut self, origin: u64, input: setup::Setup) -> Result<Response> {
        self.inner.setup(origin, input)
    }
    fn execute(&mut self, input: state::OperationRequestV1) -> Result<Response> {
        self.inner.execute(input)
    }
    fn request_status(&mut self, id: &[u8; 32]) -> Result<Response> {
        self.inner.request_status(id)
    }
    fn retry(&mut self, id: &[u8; 32]) -> Result<Response> {
        self.inner.retry(id)
    }
    fn has_pending_operation(&mut self) -> Result<bool> {
        self.inner.has_pending_operation()
    }
    fn resume(&mut self) -> Result<Response> {
        self.inner.resume()
    }
    fn fold(&mut self) -> Result<ControlFlow<Response, FoldProof>> {
        self.inner.fold()
    }
    fn credit(&mut self, id: &[u8; 32], payment: &[u8; 32]) -> Result<Response> {
        self.inner.credit(id, payment)
    }
}

fn discovery_events(owner: &Arc<Owner>) -> mpsc::Receiver<()> {
    let (observed, events) = mpsc::channel();
    {
        let mut selected = owner.wallet.lock().unwrap();
        let inner = selected.take().unwrap();
        *selected = Some(Box::new(DiscoveryWallet { inner, observed }));
    }
    owner.background.start(owner).unwrap();
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    events
}

#[test]
fn exchange_end_notifies_inactive_discovery_after_actual_occupancy_release() {
    let (registry, owner, _, drops) = fixture();
    let token = begin_with(&registry, 1).unwrap().delivered();
    let events = discovery_events(&owner);
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::Ended
    );
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(owner.scheduler.prepare_archive_turn().unwrap());
    closing::close_with(&registry, 1).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn last_accepted_child_wakes_discovery_after_end_returned_ending() {
    let (registry, owner, _, drops) = fixture();
    let token = begin_with(&registry, 1).unwrap().delivered();
    let child = enter_with(&registry, 1, token).unwrap();
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::Ending
    );
    let events = discovery_events(&owner);
    assert!(matches!(
        owner.scheduler.prepare_archive_turn(),
        Err(state::ArchiveAdmissionErrorV1::ExchangeOpen)
    ));
    drop(child);
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(owner.scheduler.prepare_archive_turn().unwrap());
    closing::close_with(&registry, 1).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn undelivered_exchange_drop_wakes_discovery_after_lease_and_table_release() {
    let (registry, owner, _, drops) = fixture();
    let issued = begin_with(&registry, 1).unwrap();
    let events = discovery_events(&owner);
    assert!(matches!(
        owner.scheduler.prepare_archive_turn(),
        Err(state::ArchiveAdmissionErrorV1::ExchangeOpen)
    ));
    drop(issued);
    events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(owner.exchanges.lock().unwrap().live.is_empty());
    drop(owner.scheduler.prepare_archive_turn().unwrap());
    closing::close_with(&registry, 1).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn controls_admit_before_wallet_wait_without_invoking_wallet_work() {
    let (registry, owner, calls, drops) = fixture();
    let wallet = owner.wallet.lock().unwrap();
    let (send, receive) = mpsc::channel();
    let caller = {
        let registry = Arc::clone(&registry);
        std::thread::spawn(move || {
            let token = begin_with(&registry, 1).unwrap().delivered();
            let child = enter_with(&registry, 1, token).unwrap();
            send.send((token, child)).unwrap();
        })
    };
    let admitted = receive.recv_timeout(Duration::from_secs(5));
    drop(wallet);
    caller.join().unwrap();
    let (token, child) = admitted.expect("control must precede wallet ownership");
    assert!(Arc::ptr_eq(&child.owner(), &owner));
    child.validate_for(&owner).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::Ending
    );
    drop(child);
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::AlreadyEnded
    );
}

#[test]
fn ending_preserves_all_admitted_children_and_retires_only_after_the_last_drop() {
    let (registry, owner, _, _) = fixture();
    let token = begin_with(&registry, 1).unwrap().delivered();
    let first = enter_with(&registry, 1, token).unwrap();
    let second = enter_with(&registry, 1, token).unwrap();
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::Ending
    );
    assert_eq!(
        enter_with(&registry, 1, token).err().unwrap().status,
        CLOSED
    );
    first.validate_for(&owner).unwrap();
    second.validate_for(&owner).unwrap();
    drop(first);
    assert!(owner.exchanges.lock().unwrap().live.contains_key(&token));
    second.validate_for(&owner).unwrap();
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::Ending
    );
    drop(second);
    let table = owner.exchanges.lock().unwrap();
    assert!(!table.live.contains_key(&token));
    assert_eq!(
        table.ended.iter().filter(|value| **value == token).count(),
        1
    );
    drop(table);
    assert_eq!(
        end_with(&registry, 1, token).unwrap(),
        state::ExchangeEndV1::AlreadyEnded
    );
    assert_eq!(
        enter_with(&registry, 1, token).err().unwrap().status,
        CLOSED
    );
}

#[test]
fn live_capacity_includes_ending_tokens_and_recovers_after_core_child_release() {
    let (registry, owner, _, _) = fixture();
    let tokens: Vec<_> = (0..MAX_LIVE)
        .map(|_| begin_with(&registry, 1).unwrap().delivered())
        .collect();
    assert_eq!(begin_with(&registry, 1).err().unwrap().status, RESOURCE);
    let child = enter_with(&registry, 1, tokens[0]).unwrap();
    assert_eq!(
        end_with(&registry, 1, tokens[0]).unwrap(),
        state::ExchangeEndV1::Ending
    );
    assert_eq!(begin_with(&registry, 1).err().unwrap().status, RESOURCE);
    drop(child);
    let next = begin_with(&registry, 1).unwrap().delivered();
    assert!(!tokens.contains(&next));
    assert_eq!(owner.exchanges.lock().unwrap().live.len(), MAX_LIVE);
    for token in tokens.into_iter().skip(1).chain([next]) {
        assert_eq!(
            end_with(&registry, 1, token).unwrap(),
            state::ExchangeEndV1::Ended
        );
    }
    assert!(owner.exchanges.lock().unwrap().live.is_empty());
}

#[test]
fn tombstones_are_bounded_and_evicted_ids_never_end_a_replacement() {
    let (registry, owner, _, _) = fixture();
    let first = begin_with(&registry, 1).unwrap().delivered();
    end_with(&registry, 1, first).unwrap();
    let mut last = first;
    for _ in 1..=MAX_ENDED {
        last = begin_with(&registry, 1).unwrap().delivered();
        assert_ne!(first, last);
        end_with(&registry, 1, last).unwrap();
    }
    let replacement = begin_with(&registry, 1).unwrap().delivered();
    assert_eq!(end_with(&registry, 1, first).unwrap_err().status, INVALID);
    assert_eq!(
        end_with(&registry, 1, last).unwrap(),
        state::ExchangeEndV1::AlreadyEnded
    );
    let table = owner.exchanges.lock().unwrap();
    assert_eq!(table.ended.len(), MAX_ENDED);
    assert_eq!(table.live.len(), 1);
    assert!(table.live.contains_key(&replacement));
}

#[test]
fn undisclosed_control_failure_retires_only_its_own_core_lease() {
    let (registry, owner, _, _) = fixture();
    let existing = begin_with(&registry, 1).unwrap().delivered();
    let undisclosed = begin_with(&registry, 1).unwrap();
    let failed = undisclosed.token;
    drop(undisclosed);
    assert_eq!(end_with(&registry, 1, failed).unwrap_err().status, INVALID);
    assert!(owner.exchanges.lock().unwrap().live.contains_key(&existing));
    let child = enter_with(&registry, 1, existing).unwrap();
    child.validate_for(&owner).unwrap();
    assert_eq!(owner.exchanges.lock().unwrap().live.len(), 1);
}

#[test]
fn jni_begin_requires_successful_fence_and_response_handoff_to_retain_core_occupancy() {
    for (fence_ok, object_delivered) in [(false, true), (true, false), (true, true)] {
        let (registry, owner, calls, _) = fixture();
        let mut retained = None;
        let mut token = 0;
        let result = run_with_fence(
            || {
                let issued = begin_with(&registry, 1)?;
                token = issued.token;
                let response = issued.response();
                retained = Some(issued);
                Ok(response)
            },
            || {
                if fence_ok {
                    Ok(())
                } else {
                    Err(std::io::Error::other("begin-exchange final fence refusal"))
                }
            },
        );
        let result = deliver_issued(retained, result, |result| (result, object_delivered));
        assert!(token > 0);
        assert_eq!(result.is_ok(), fence_ok);
        if !fence_ok {
            assert_eq!(result.unwrap_err().status, UNAVAILABLE);
        }
        let delivered = fence_ok && object_delivered;
        assert_eq!(
            owner.exchanges.lock().unwrap().live.contains_key(&token),
            delivered
        );
        if delivered {
            assert_eq!(
                owner.scheduler.prepare_archive_turn().err().unwrap(),
                state::ArchiveAdmissionErrorV1::ExchangeOpen
            );
            let child = enter_with(&registry, 1, token).unwrap();
            child.validate_for(&owner).unwrap();
            drop(child);
            end_with(&registry, 1, token).unwrap();
        } else {
            assert_eq!(
                enter_with(&registry, 1, token).err().unwrap().status,
                INVALID
            );
        }
        // This actual Core control refuses outstanding exchange occupancy; table cleanup
        // alone would not make it succeed if an undisclosed lease were still retained.
        drop(owner.scheduler.prepare_archive_turn().unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn wrong_owner_and_wrong_purpose_tokens_cannot_mutate_an_exchange() {
    let (registry, owner, _, _) = fixture();
    let (_, other, _, _) = fixture();
    registry
        .lock()
        .unwrap()
        .owners
        .insert(2, Arc::clone(&other));
    let token = begin_with(&registry, 1).unwrap().delivered();
    let unrelated = review::allocate_token().unwrap();
    for invalid in [0, unrelated] {
        assert_eq!(
            enter_with(&registry, 1, invalid).err().unwrap().status,
            INVALID
        );
        assert_eq!(end_with(&registry, 1, invalid).unwrap_err().status, INVALID);
    }
    assert_eq!(
        enter_with(&registry, 2, token).err().unwrap().status,
        INVALID
    );
    assert_eq!(end_with(&registry, 2, token).unwrap_err().status, INVALID);
    let child = enter_with(&registry, 1, token).unwrap();
    assert_eq!(child.validate_for(&other).unwrap_err().status, INVALID);
    child.validate_for(&owner).unwrap();
    assert_eq!(owner.exchanges.lock().unwrap().live.len(), 1);
    assert!(other.exchanges.lock().unwrap().live.is_empty());
}

#[test]
fn close_fences_capabilities_without_waiting_for_user_held_leases_or_queued_children() {
    let (registry, owner, _, drops) = fixture();
    let token = begin_with(&registry, 1).unwrap().delivered();
    let child = enter_with(&registry, 1, token).unwrap();
    closing::close_with(&registry, 1).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(child.validate_for(&owner).unwrap_err().status, CLOSED);
    assert_eq!(begin_with(&registry, 1).err().unwrap().status, CLOSED);
    assert_eq!(end_with(&registry, 1, token).unwrap_err().status, CLOSED);
    let (_, replacement, _, _) = fixture();
    registry
        .lock()
        .unwrap()
        .owners
        .insert(1, Arc::clone(&replacement));
    let next = begin_with(&registry, 1).unwrap().delivered();
    assert_ne!(next, token);
    assert_eq!(
        child.validate_for(&replacement).unwrap_err().status,
        INVALID
    );
    assert_eq!(end_with(&registry, 1, token).unwrap_err().status, INVALID);
    drop(child);
    let admitted = enter_with(&registry, 1, next).unwrap();
    admitted.validate_for(&replacement).unwrap();
}

#[test]
fn core_control_errors_preserve_closed_invalid_and_resource_meanings() {
    use state::ExchangeErrorV1 as E;
    for (input, expected) in [
        (E::Closed, CLOSED),
        (E::Ended, CLOSED),
        (E::Foreign, INVALID),
        (E::Capacity, RESOURCE),
        (E::Exhausted, RESOURCE),
    ] {
        assert_eq!(failure(input), Failure::code(expected));
    }
}

fn registered() -> (u64, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let id = install(
        Box::new(TestWallet {
            calls: Arc::clone(&calls),
            drops: Arc::new(AtomicUsize::new(0)),
            expected_request: None,
        }),
        state::Scheduler::new(),
    )
    .unwrap();
    (id, calls)
}

#[test]
fn c_controls_initialize_closed_geometry_and_preserve_admitted_children() {
    let (id, calls) = registered();
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_begin_exchange_v1(id, std::ptr::null_mut()) },
        INVALID
    );
    assert!(owner(id).unwrap().exchanges.lock().unwrap().live.is_empty());
    let mut output = WalletResult::default();
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_begin_exchange_v1(id, &mut output) },
        0
    );
    assert_eq!(
        (
            output.status,
            output.sequence_high,
            output.detail,
            output.length
        ),
        (60, 0, 0, 0)
    );
    assert!(output.sequence_low > 0 && output.sequence_low <= i64::MAX as u64);
    assert!(output.bytes.is_null());
    let token = output.sequence_low;
    let admitted = Invocation::admit(id, token).unwrap();
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_end_exchange_v1(id, token, &mut output) },
        0
    );
    assert_eq!(
        (
            output.status,
            output.sequence_low,
            output.sequence_high,
            output.detail,
            output.length
        ),
        (61, 0, 0, 1, 0)
    );
    assert!(output.bytes.is_null());
    assert_eq!(Invocation::admit(id, token).err().unwrap().status, CLOSED);
    admitted.validate().unwrap();
    drop(admitted);
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_end_exchange_v1(id, token, &mut output) },
        0
    );
    assert_eq!((output.status, output.detail), (61, 2));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    close(id).unwrap();
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_begin_exchange_v1(id, &mut output) },
        CLOSED
    );
    assert_eq!(
        (
            output.status,
            output.sequence_low,
            output.sequence_high,
            output.detail,
            output.length
        ),
        (CLOSED, 0, 0, 0, 0)
    );
}

#[test]
fn mandatory_setup_exchange_and_zero_only_exemptions_are_a_closed_partition() {
    let (id, calls) = registered();
    let token = begin(id).unwrap().delivered();
    for selector in 0..=59 {
        if setup::bounds(selector).is_err() {
            assert_eq!(
                Invocation::setup(id, token, selector).err().unwrap().status,
                INVALID
            );
        } else if setup_exempt(selector) {
            Invocation::setup(id, 0, selector)
                .unwrap()
                .validate()
                .unwrap();
            assert_eq!(
                Invocation::setup(id, token, selector).err().unwrap().status,
                INVALID
            );
        } else {
            assert_eq!(
                Invocation::setup(id, 0, selector).err().unwrap().status,
                INVALID
            );
            Invocation::setup(id, token, selector)
                .unwrap()
                .validate()
                .unwrap();
        }
    }
    end(id, token).unwrap();
    for selector in [6, 51] {
        Invocation::setup(id, token, selector)
            .unwrap()
            .validate()
            .unwrap();
    }
    assert_eq!(
        Invocation::setup(id, token, 2).err().unwrap().status,
        CLOSED
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    close(id).unwrap();
}

#[test]
fn ledger_attempt_mutations_require_an_exchange_and_outcome_reads_do_not() {
    for selector in [34, 36, 46, 53] {
        assert!(setup::bounds(selector).is_ok());
        assert!(!setup_exempt(selector));
    }
    for selector in [33, 37, 52] {
        assert!(setup::bounds(selector).is_ok());
        assert!(setup_exempt(selector));
    }
    for selector in [3, 30, 35, 41, 54] {
        assert!(setup::bounds(selector).is_err());
    }
}

#[test]
fn accepted_core_child_survives_until_foreign_response_allocation_finishes() {
    let (id, calls) = registered();
    let token = begin(id).unwrap().delivered();
    let request_id = [1; 32];
    let request = exports::WalletSetupRequest {
        setup_id: request_id.as_ptr(),
        selector: 2,
        amount: WalletU128 { low: 1, high: 0 },
        token: 0,
        first: std::ptr::null(),
        first_length: 0,
        second: std::ptr::null(),
        second_length: 0,
        third: std::ptr::null(),
        third_length: 0,
    };
    let mut output = WalletResult::default();
    let result = unsafe {
        exports::setup_output_with(id, token, &request, &mut output, |_, bytes| {
            assert_eq!(bytes, [0, 255, 7]);
            assert_eq!(end(id, token).unwrap(), state::ExchangeEndV1::Ending);
            Err(())
        })
    };
    assert_eq!(result, RESOURCE);
    assert_eq!(
        (
            output.status,
            output.sequence_low,
            output.sequence_high,
            output.detail,
            output.length
        ),
        (RESOURCE, 0, 0, 0, 0)
    );
    assert!(output.bytes.is_null());
    assert_eq!(end(id, token).unwrap(), state::ExchangeEndV1::AlreadyEnded);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    close(id).unwrap();
}
