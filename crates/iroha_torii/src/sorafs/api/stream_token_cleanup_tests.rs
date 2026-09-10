// Actual asynchronous enforcement over signed custody and the durable quota/callback fixture.
use crate::sorafs::stream_token_cleanup::{
    prepare as prepare_range_cleanup, register_worker as register_range_cleanup_worker,
    test_support::{GateRelease, ProbeProvider, wait_until as wait_for_range_cleanup},
};

struct RangeCleanupTestOwner {
    shutdown: iroha_futures::supervisor::ShutdownSignal,
    worker: Option<tokio::task::JoinHandle<crate::ToriiCriticalWorkerExit>>,
}
impl RangeCleanupTestOwner {
    fn install(app: &mut crate::AppState, capacity: u32) -> Self {
        let shutdown = app.shutdown_signal.clone();
        app.stream_token_cleanup = prepare_range_cleanup(
            app.stream_token_admission_capture.as_ref(),
            Some(capacity),
            shutdown.clone(),
        )
        .unwrap();
        let mut workers = Vec::new();
        register_range_cleanup_worker(app, &mut workers).unwrap();
        assert_eq!(workers.len(), 1);
        let registered = workers.pop().unwrap();
        assert_eq!(registered.name, "sorafs_stream_token_cleanup");
        let worker = registered.task;
        Self {
            shutdown,
            worker: Some(worker),
        }
    }
    async fn finish(mut self) {
        self.shutdown.send();
        let worker = self.worker.take().unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(10), worker)
                .await
                .expect("bounded fixture cleanup drain")
                .unwrap(),
            crate::ToriiCriticalWorkerExit::StoppedByShutdown,
        );
    }
}
impl Drop for RangeCleanupTestOwner {
    fn drop(&mut self) {
        self.shutdown.send();
    }
}

fn range_cleanup_context(
    capacity: u32,
) -> (
    TokenTestContext,
    Arc<SignedFixture>,
    ServingAdmissionFixture,
    Arc<ProbeProvider>,
    RangeCleanupTestOwner,
) {
    let mut context = token_test_context();
    let mut app = Arc::try_unwrap(context.app).unwrap_or_else(|_| panic!("exclusive test app"));
    let signed = SignedFixture::for_api([0xAB; 32], 7, TestSignerMode::Sign);
    app.stream_token_issuer = Some(Arc::new(signed.issuer().unwrap()));
    let durable = ServingAdmissionFixture::new();
    let probe = ProbeProvider::new(durable.provider());
    app.stream_token_admission_capture = Some(durable.capture_with_provider(probe.clone()));
    app.query_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app.query_queue_timeout = std::time::Duration::ZERO;
    let cleanup = RangeCleanupTestOwner::install(&mut app, capacity);
    context.app = Arc::new(app);
    (context, signed, durable, probe, cleanup)
}

#[tokio::test(flavor = "current_thread")]
async fn range_admission_cancellation_keeps_physical_permits_and_releases_discarded_result() {
    use std::sync::atomic::Ordering;
    let (context, signed, durable, probe, cleanup) = range_cleanup_context(2);
    let encoded = issue_token_base64(&context, TokenOverrides::default()).await;
    let manifest = context.manifest();
    let headers = enforcement_headers(&encoded);
    probe.gate_point.store(2, Ordering::Release);
    let release = GateRelease(probe.gate.clone());
    let app = context.app.clone();
    let admission = tokio::spawn(async move {
        enforce_stream_token_for_request(
            &app,
            &headers,
            &manifest,
            "cancel-admission",
            enforcement_route(1),
        )
        .await
    });
    wait_for_range_cleanup(|| probe.gate.entered.load(Ordering::Acquire) == 1).await;
    assert_eq!(context.app.query_inflight.available_permits(), 0);
    assert_eq!(context.app.query_heavy_inflight.available_permits(), 0);
    admission.abort();
    assert!(admission.await.unwrap_err().is_cancelled());
    let observed = signed.observer_calls.load(Ordering::SeqCst);
    let rejected = enforce_stream_token_for_request(
        &context.app,
        &enforcement_headers(&encoded),
        &context.manifest(),
        "rejected-before-physical-finish",
        enforcement_route(1),
    )
    .await
    .expect_err("physical admission still occupies both gates");
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(signed.observer_calls.load(Ordering::SeqCst), observed);
    assert_eq!(probe.admission_calls.load(Ordering::Acquire), 1);
    assert_eq!(context.app.query_inflight.available_permits(), 0);
    assert_eq!(context.app.query_heavy_inflight.available_permits(), 0);
    drop(release);
    wait_for_range_cleanup(|| {
        probe.release_calls.lock().unwrap().len() == 1 && durable.active_leases() == 0
    })
    .await;
    wait_for_range_cleanup(|| {
        context.app.query_inflight.available_permits() == 1
            && context.app.query_heavy_inflight.available_permits() == 1
    })
    .await;
    assert_eq!(durable.requests().len(), 1);
    assert_eq!(durable.outcomes().len(), 1);
    assert_eq!(
        signed.calls.load(Ordering::SeqCst),
        1,
        "no token re-sign during range recovery"
    );
    cleanup.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn static_rejection_callback_uses_physical_worker_without_a_cleanup_ticket() {
    use std::sync::atomic::Ordering;
    let (context, signed, durable, probe, cleanup) = range_cleanup_context(1);
    let ticket = context
        .app
        .stream_token_cleanup
        .as_ref()
        .unwrap()
        .try_reserve()
        .unwrap();
    let observed = signed.observer_calls.load(Ordering::SeqCst);
    probe.gate_point.store(2, Ordering::Release);
    let release = GateRelease(probe.gate.clone());
    let app = context.app.clone();
    let manifest = context.manifest();
    let rejected = tokio::spawn(async move {
        enforce_stream_token_for_request(
            &app,
            &enforcement_headers("malformed%%%"),
            &manifest,
            "static-with-full-cleanup",
            enforcement_route(1),
        )
        .await
    });
    wait_for_range_cleanup(|| probe.gate.entered.load(Ordering::Acquire) == 1).await;
    assert_eq!(context.app.query_inflight.available_permits(), 0);
    assert_eq!(context.app.query_heavy_inflight.available_permits(), 0);
    assert!(
        context
            .app
            .stream_token_cleanup
            .as_ref()
            .unwrap()
            .try_reserve()
            .is_err()
    );
    assert_eq!(signed.observer_calls.load(Ordering::SeqCst), observed);
    drop(release);
    let response = rejected
        .await
        .unwrap()
        .expect_err("malformed token remains rejected");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(durable.requests().len(), 1);
    assert_eq!(durable.outcomes().len(), 1);
    assert_eq!(durable.active_leases(), 0);
    assert!(probe.release_calls.lock().unwrap().is_empty());
    drop(ticket);
    cleanup.finish().await;
}

#[tokio::test]
async fn cleanup_capacity_rejects_before_accepted_side_effect_and_external_quota_frees_ticket() {
    use std::sync::atomic::Ordering;
    let (context, _, durable, probe, cleanup) = range_cleanup_context(2);
    let encoded = issue_token_base64(
        &context,
        TokenOverrides {
            max_streams: Some(1),
            ..TokenOverrides::default()
        },
    )
    .await;
    let manifest = context.manifest();
    let headers = enforcement_headers(&encoded);
    let (guard, _) = enforce_stream_token_for_request(
        &context.app,
        &headers,
        &manifest,
        "first-lease",
        enforcement_route(1),
    )
    .await
    .unwrap();
    assert!(guard.lease_window().is_some());
    assert_eq!(context.app.query_inflight.available_permits(), 1);
    assert_eq!(context.app.query_heavy_inflight.available_permits(), 1);
    let reserved = context
        .app
        .stream_token_cleanup
        .as_ref()
        .unwrap()
        .try_reserve()
        .unwrap();
    let before = probe.admission_calls.load(Ordering::Acquire);
    let full = enforce_stream_token_for_request(
        &context.app,
        &headers,
        &manifest,
        "full-cleanup",
        enforcement_route(1),
    )
    .await
    .expect_err("local ticket ceiling");
    assert_eq!(full.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(full.headers().get(RETRY_AFTER).unwrap(), "1");
    assert_eq!(probe.admission_calls.load(Ordering::Acquire), before);
    drop(reserved);
    let quota = enforce_stream_token_for_request(
        &context.app,
        &headers,
        &manifest,
        "quota-terminal",
        enforcement_route(1),
    )
    .await
    .expect_err("external concurrency limit");
    assert_eq!(quota.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(probe.admission_calls.load(Ordering::Acquire), before + 1);
    let unused = context
        .app
        .stream_token_cleanup
        .as_ref()
        .unwrap()
        .try_reserve()
        .expect("quota terminal returned its reservation");
    assert_eq!(durable.active_leases(), 1);
    drop(unused);
    drop(guard);
    cleanup.finish().await;
    assert_eq!(durable.active_leases(), 0);
    assert_eq!(probe.release_calls.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_accepted_guard_does_not_call_provider_or_hold_query_gates() {
    use std::sync::atomic::Ordering;
    let (context, _, durable, probe, cleanup) = range_cleanup_context(1);
    let encoded = issue_token_base64(&context, TokenOverrides::default()).await;
    let (guard, _) = enforce_stream_token_for_request(
        &context.app,
        &enforcement_headers(&encoded),
        &context.manifest(),
        "nonblocking-guard-drop",
        enforcement_route(1),
    )
    .await
    .unwrap();
    probe.gate_point.store(1, Ordering::Release);
    let release = GateRelease(probe.gate.clone());
    drop(guard);
    wait_for_range_cleanup(|| probe.gate.entered.load(Ordering::Acquire) == 1).await;
    assert_eq!(context.app.query_inflight.available_permits(), 1);
    assert_eq!(context.app.query_heavy_inflight.available_permits(), 1);
    // Holding both ordinary gates must not prevent the independently owned release operation.
    let query = context
        .app
        .query_inflight
        .clone()
        .try_acquire_owned()
        .unwrap();
    let heavy = context
        .app
        .query_heavy_inflight
        .clone()
        .try_acquire_owned()
        .unwrap();
    assert_eq!(durable.active_leases(), 1);
    drop(release);
    cleanup.finish().await;
    assert_eq!(durable.active_leases(), 0);
    assert_eq!(probe.release_calls.lock().unwrap().len(), 1);
    drop((query, heavy));
}
