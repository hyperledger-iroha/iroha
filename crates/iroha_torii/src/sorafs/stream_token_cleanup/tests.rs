//! Physical-worker, capacity, cancellation and exact durable lease regression tests.
use super::{test_support::*, *};
use std::time::Duration;

#[test]
fn invalid_capacity_and_unstarted_cleanup_fail_before_admission() {
    for capacity in [0, 1_000_001, u32::MAX] {
        assert!(StreamTokenCleanupV1::new(capacity, ShutdownSignal::new()).is_err());
    }
    let cleanup = StreamTokenCleanupV1::new(1, ShutdownSignal::new()).unwrap();
    assert!(cleanup.try_reserve().is_err());
    assert!(
        cleanup.start().is_err(),
        "never create an inline async runtime"
    );
    assert!(prepare(None, Some(1), ShutdownSignal::new()).is_err());
    assert!(
        prepare(None, None, ShutdownSignal::new())
            .unwrap()
            .is_none()
    );
    let (_, _, capture) = fixture();
    assert!(prepare(Some(&capture), None, ShutdownSignal::new()).is_err());
    assert!(prepare(Some(&capture), Some(0), ShutdownSignal::new()).is_err());
    assert!(
        prepare(Some(&capture), Some(1_000_000), ShutdownSignal::new())
            .unwrap()
            .is_some()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reserved_drop_enqueues_and_retains_one_physical_release_through_shutdown() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(2, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    assert!(cleanup.start().is_err(), "one retained receiver only");
    let first = cleanup.try_reserve().unwrap();
    let second = cleanup.try_reserve().unwrap();
    assert!(cleanup.try_reserve().is_err());
    let one = capture.admit(&request("cleanup-one")).unwrap();
    let two = capture.admit(&request("cleanup-two")).unwrap();
    assert_eq!(fixture.active_leases(), 2);
    let first = first.arm(capture.clone(), one);
    let second = second.arm(capture.clone(), two);
    provider.gate_point.store(1, Ordering::Release);
    let release = GateRelease(provider.gate.clone());
    drop(first); // On the async executor: this must only enqueue.
    wait_until(|| provider.gate.entered.load(Ordering::Acquire) == 1).await;
    assert_eq!(cleanup.state.physical.load(Ordering::Acquire), 1);
    // The receiver freed one queue slot, but the physical release remains occupied.
    let third = cleanup.try_reserve().unwrap();
    assert!(cleanup.try_reserve().is_err());
    let three = capture.admit(&request("cleanup-three")).unwrap();
    let third = third.arm(capture, three);
    drop(second);
    drop(third);
    tokio::time::sleep(Duration::from_millis(10)).await; // Executor progresses while release blocks.
    assert_eq!(*provider.release_calls.lock().unwrap(), [one]);
    assert_eq!(cleanup.state.physical.load(Ordering::Acquire), 1);
    shutdown.send();
    assert!(cleanup.try_reserve().is_err());
    assert!(
        !worker.is_finished(),
        "shutdown cannot detach the physical release"
    );
    drop(release);
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
    assert_eq!(*provider.release_calls.lock().unwrap(), [one, two, three]);
    assert_eq!(fixture.active_leases(), 0);
    assert_eq!(cleanup.state.physical.load(Ordering::Acquire), 0);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn closing_receiver_drains_previously_reserved_tickets_and_late_results() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let ticket = cleanup.try_reserve().unwrap();
    let record = capture.admit(&request("late-owned-result")).unwrap();
    shutdown.send();
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(
        !worker.is_finished(),
        "outstanding ticket must keep drain alive"
    );
    assert!(cleanup.try_reserve().is_err());
    drop(ticket.arm(capture, record));
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
    assert_eq!(*provider.release_calls.lock().unwrap(), [record]);
    assert_eq!(fixture.active_leases(), 0);
}

#[tokio::test]
async fn unused_ticket_drain_does_not_fabricate_a_release() {
    let (_, provider, _) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let unused = cleanup.try_reserve().unwrap();
    shutdown.send();
    drop(unused);
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
    assert!(provider.release_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ambiguous_release_fences_admission_and_drains_without_retry_or_success_claim() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(2, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let first = cleanup.try_reserve().unwrap();
    let second = cleanup.try_reserve().unwrap();
    let one = capture.admit(&request("ambiguous-one")).unwrap();
    let two = capture.admit(&request("ambiguous-two")).unwrap();
    provider.release_fault.store(1, Ordering::Release);
    drop(first.arm(capture.clone(), one));
    wait_until(|| shutdown.is_sent()).await;
    assert!(cleanup.try_reserve().is_err());
    assert!(!worker.is_finished());
    drop(second.arm(capture, two));
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::UnexpectedExit
    );
    assert_eq!(*provider.release_calls.lock().unwrap(), [one, two]);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 2);
    assert_eq!(
        fixture.active_leases(),
        2,
        "authoritative expiry, not local success"
    );
}

#[tokio::test]
async fn provider_panic_is_observable_and_does_not_drop_the_receiver() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(2, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let first = cleanup.try_reserve().unwrap();
    let second = cleanup.try_reserve().unwrap();
    let one = capture.admit(&request("panic-one")).unwrap();
    let two = capture.admit(&request("panic-two")).unwrap();
    provider.release_fault.store(2, Ordering::Release);
    drop(first.arm(capture.clone(), one));
    wait_until(|| shutdown.is_sent()).await;
    provider.release_fault.store(0, Ordering::Release);
    drop(second.arm(capture, two));
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::UnexpectedExit
    );
    assert_eq!(*provider.release_calls.lock().unwrap(), [one, two]);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 1);
    assert_eq!(fixture.active_leases(), 1);
}

#[tokio::test]
async fn receiver_loss_marks_outstanding_work_unresolved_without_calling_from_drop() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let ticket = cleanup.try_reserve().unwrap();
    let record = capture.admit(&request("receiver-loss")).unwrap();
    // Fault-inject loss of this test's own idle receiver; no physical call is in flight.
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    drop(ticket.arm(capture, record));
    assert!(shutdown.is_sent());
    assert!(cleanup.try_reserve().is_err());
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 1);
    assert!(provider.release_calls.lock().unwrap().is_empty());
    assert_eq!(fixture.active_leases(), 1);
}

#[tokio::test]
async fn dropping_owner_does_not_form_an_app_state_or_sender_receiver_cycle() {
    let cleanup = Arc::new(StreamTokenCleanupV1::new(1, ShutdownSignal::new()).unwrap());
    let weak = Arc::downgrade(&cleanup);
    let worker = cleanup.start().unwrap();
    drop(cleanup);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
}

#[tokio::test(flavor = "current_thread")]
async fn receiver_abort_before_first_poll_fences_without_waiting_for_senders() {
    let shutdown = ShutdownSignal::new();
    let cleanup = Arc::new(StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap());
    let retained_sender = Arc::clone(&cleanup);
    let worker = cleanup.start().unwrap();
    // No await occurs between spawn and abort: the receiver future has never been polled.
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert!(shutdown.is_sent());
    assert!(cleanup.state.receiver_failed.load(Ordering::Acquire));
    assert!(!cleanup.state.receiver_alive.load(Ordering::Acquire));
    assert!(retained_sender.try_reserve().is_err());
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 0);
    assert_eq!(cleanup.state.unsettled.load(Ordering::Acquire), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn receiver_loss_between_drop_precheck_and_send_settles_pending_exactly_once() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let ticket = cleanup.try_reserve().unwrap();
    let record = capture.admit(&request("send-loss-race")).unwrap();
    let mut lease = ticket.arm(capture, record);
    let permit = lease.permit.take().unwrap();
    let work = lease.work.take().unwrap();
    let settlement = Arc::clone(&work.settlement);
    assert!(settlement.state.receiver_alive.load(Ordering::Acquire));
    // Deterministically interleave the two real Drop primitives around actual receiver loss.
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    drop(permit.send(work));
    assert!(!settlement.state.receiver_alive.load(Ordering::Acquire));
    settlement.unresolved_if_pending();
    assert!(shutdown.is_sent());
    assert_eq!(settlement.disposition.load(Ordering::Acquire), 3);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 1);
    assert_eq!(cleanup.state.unsettled.load(Ordering::Acquire), 0);
    assert!(provider.release_calls.lock().unwrap().is_empty());
    assert_eq!(fixture.active_leases(), 1);
    // Tokio may retain the raced message until the last Sender drops. Its later Work::drop
    // must not duplicate the already accounted unresolved record.
    drop(cleanup);
    assert_eq!(settlement.state.unresolved.load(Ordering::Acquire), 1);
    settlement.acknowledged();
    assert_eq!(settlement.disposition.load(Ordering::Acquire), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn receiver_loss_after_claim_preserves_late_physical_acknowledgement() {
    let (fixture, provider, capture) = fixture();
    let shutdown = ShutdownSignal::new();
    let cleanup = StreamTokenCleanupV1::new(1, shutdown.clone()).unwrap();
    let worker = cleanup.start().unwrap();
    let ticket = cleanup.try_reserve().unwrap();
    let record = capture.admit(&request("claimed-loss-race")).unwrap();
    let lease = ticket.arm(capture, record);
    let settlement = Arc::clone(&lease.work.as_ref().unwrap().settlement);
    provider.gate_point.store(1, Ordering::Release);
    let release = GateRelease(provider.gate.clone());
    drop(lease);
    wait_until(|| provider.gate.entered.load(Ordering::Acquire) == 1).await;
    assert_eq!(settlement.disposition.load(Ordering::Acquire), 1);
    worker.abort(); // This test's receiver only; the physical closure must still finish.
    assert!(worker.await.unwrap_err().is_cancelled());
    assert!(cleanup.state.receiver_failed.load(Ordering::Acquire));
    assert!(shutdown.is_sent());
    settlement.unresolved_if_pending(); // The racing sender must not settle InFlight.
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 0);
    assert_eq!(cleanup.state.unsettled.load(Ordering::Acquire), 1);
    assert_eq!(cleanup.state.physical.load(Ordering::Acquire), 1);
    assert!(cleanup.try_reserve().is_err());
    drop(release);
    wait_until(|| cleanup.state.physical.load(Ordering::Acquire) == 0).await;
    assert_eq!(*provider.release_calls.lock().unwrap(), [record]);
    assert_eq!(fixture.active_leases(), 0);
    assert_eq!(settlement.disposition.load(Ordering::Acquire), 2);
    assert_eq!(cleanup.state.unsettled.load(Ordering::Acquire), 0);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 0);
    settlement.unresolved_if_pending(); // A later loss observation cannot overwrite the ack.
    settlement.unresolved();
    assert_eq!(settlement.disposition.load(Ordering::Acquire), 2);
    assert_eq!(cleanup.state.unresolved.load(Ordering::Acquire), 0);
    assert!(cleanup.state.receiver_failed.load(Ordering::Acquire));
    assert!(
        cleanup.try_reserve().is_err(),
        "receiver health stays fenced after a real late ack"
    );
}
