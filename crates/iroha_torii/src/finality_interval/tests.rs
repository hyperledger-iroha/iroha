//! Genuine public source, charged response framing and detached physical-worker controls.

#[path = "maximum_payment.rs"]
mod maximum_payment;

pub(crate) use maximum_payment::paid_chain as maximum_paid_chain_for_testing;

use super::*;
use iroha_core::{
    state::World,
    sumeragi::{
        finality::build_proof,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use tower::ServiceExt as _;

#[test]
fn finite_finality_heapless_rejections_preserve_missing_and_invalid_classification() {
    use iroha_core::sumeragi::finality::NativeFinalityProofRefusal as Refusal;

    for missing in [
        Refusal::NotCommitted { height: 49 },
        Refusal::NotInView { height: 49 },
        Refusal::MissingCertificate { height: 49 },
    ] {
        let error = source_error(FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Native(missing),
        ));
        assert!(matches!(
            error,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound
            ))
        ));
    }
    for invalid in [
        Refusal::Malformed { height: 49 },
        Refusal::Committee { height: 49 },
        Refusal::HeaderMismatch { height: 49 },
    ] {
        let error = source_error(FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Native(invalid),
        ));
        assert!(matches!(
            error,
            Error::Query(iroha_data_model::ValidationFail::InternalError(_))
        ));
    }
}

#[test]
fn borrowed_interval_identity_projects_only_its_root_to_the_owned_sequence() {
    use norito::{NoritoSchema as _, schema::identity::frame_hash};

    assert_eq!(
        BorrowedInterval::nominal_name(),
        "iroha_torii::finality_interval::BorrowedInterval<'_>"
    );
    assert_ne!(
        BorrowedInterval::nominal_name(),
        Vec::<SumeragiFinalityProof>::nominal_name()
    );
    assert_eq!(
        BorrowedInterval::frame_name(),
        Vec::<SumeragiFinalityProof>::frame_name()
    );
    assert_eq!(
        frame_hash::<BorrowedInterval<'_>>(),
        frame_hash::<Vec<SumeragiFinalityProof>>()
    );
    assert_ne!(
        frame_hash::<Option<BorrowedInterval<'_>>>(),
        frame_hash::<Option<Vec<SumeragiFinalityProof>>>()
    );
}

fn admission() -> (
    QueryAdmissionPermit,
    ByteWeightedMemoryPool,
    Arc<tokio::sync::Semaphore>,
) {
    let working = 48_000_000;
    let pool = ByteWeightedMemoryPool::new(working).unwrap();
    let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
        pool.try_acquire_parts([working as u64]).unwrap(),
        QueryFanoutMemoryEnvelope::for_bodyless_finality(working).unwrap(),
        pool.generation(),
    )
    .unwrap();
    let query = Arc::new(tokio::sync::Semaphore::new(1));
    (
        QueryAdmissionPermit {
            _query: Arc::clone(&query).try_acquire_owned().unwrap(),
            heavy: None,
            body: None,
            fanout_memory: Some(reservation),
        },
        pool,
        query,
    )
}
#[test]
fn query_admission_retires_original_resources_before_waking_next_worker() {
    use std::task::{Context, Poll, Wake, Waker};

    struct ObserveWake {
        pool: ByteWeightedMemoryPool,
        metadata: iroha_allocation::AllocationBudget,
        heavy: Arc<tokio::sync::Semaphore>,
        body: Arc<tokio::sync::Semaphore>,
        observed: std::sync::atomic::AtomicU8,
    }
    impl Wake for ObserveWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            // Semaphore release invokes this synchronously. Observe the actual
            // retirement boundary without sleeps, thread scheduling or a retry.
            let state = u8::from(self.pool.available_bytes() == 48_000_000)
                | (u8::from(self.metadata.reserved_bytes() == 0) << 1)
                | (u8::from(self.heavy.available_permits() == 1) << 2)
                | (u8::from(self.body.available_permits() == 1) << 3);
            self.observed.store(state, Ordering::SeqCst);
        }
    }

    let (mut admission, pool, query) = admission();
    let heavy = Arc::new(tokio::sync::Semaphore::new(1));
    let body = Arc::new(tokio::sync::Semaphore::new(1));
    admission.heavy = Some(Arc::clone(&heavy).try_acquire_owned().unwrap());
    admission.body = Some(Arc::clone(&body).try_acquire_owned().unwrap());
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let metadata = owner.response_metadata().clone();
    drop(owner);
    assert_eq!(pool.available_bytes(), 0);
    assert!(metadata.reserved_bytes() > 0);
    let observed = Arc::new(ObserveWake {
        pool: pool.clone(),
        metadata,
        heavy,
        body,
        observed: std::sync::atomic::AtomicU8::new(u8::MAX),
    });
    let waker = Waker::from(Arc::clone(&observed));
    let mut context = Context::from_waker(&waker);
    let mut next = Box::pin(Arc::clone(&query).acquire_owned());
    assert!(std::future::Future::poll(next.as_mut(), &mut context).is_pending());
    drop(admission);
    let next_permit = match std::future::Future::poll(next.as_mut(), &mut context) {
        Poll::Ready(Ok(permit)) => permit,
        _ => panic!("the original query permit must wake its actual next worker"),
    };
    drop(next_permit);
    drop(next);
    assert_eq!(
        observed.observed.load(Ordering::SeqCst),
        0b1111,
        "query wake must follow native allocation, byte credit, body and heavy permit retirement",
    );
    assert_eq!(query.available_permits(), 1);
    assert_eq!(pool.available_bytes(), 48_000_000);
}

#[inline(never)]
fn chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < 4 {
        chain.commit(Vec::new());
    }
    chain
}
fn deadline() -> Instant {
    Instant::now() + route_timeout_for_path("/v1/bridge/finality/interval/1/4")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_and_interval_responses_keep_original_charged_bytes_and_query_credit_through_last_slice()
 {
    let chain = chain();
    let expected = (1..=4)
        .map(|height| build_proof(&chain.state().view(), height).unwrap())
        .collect::<Vec<_>>();
    for selection in [Selection::Single(4), Selection::Interval { from: 1, to: 4 }] {
        let (admission, pool, query) = admission();
        let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
        let frames = owner.cold_frames().clone();
        let limit = owner.response_bytes();
        drop(owner);
        let prepared = prepare(
            Arc::clone(chain.state()),
            selection,
            ResponseFormat::Norito,
            admission,
            limit,
            deadline(),
        )
        .await
        .unwrap();
        assert_eq!(prepared.response.status(), StatusCode::OK);
        let extent = match selection {
            Selection::Single(_) => norito::canonical_frame_len(&expected[3]).unwrap(),
            Selection::Interval { .. } => norito::canonical_frame_len(&expected).unwrap(),
        };
        assert_eq!(prepared.bytes, extent);
        assert_eq!(
            frames.reserved_bytes(),
            prepared.bytes,
            "only the exact canonical response remains after the physical prefix/DTO owners retire"
        );
        assert_eq!(
            query.available_permits(),
            1,
            "physical CPU admission retires at worker completion"
        );
        let bytes = axum::body::to_bytes(prepared.response.into_body(), limit)
            .await
            .unwrap();
        match selection {
            Selection::Single(_) => assert_eq!(
                norito::decode_canonical::<SumeragiFinalityProof>(&bytes).unwrap(),
                expected[3]
            ),
            Selection::Interval { .. } => assert_eq!(
                norito::decode_canonical::<Vec<SumeragiFinalityProof>>(&bytes).unwrap(),
                expected
            ),
        }
        let last = bytes.slice(..);
        drop(bytes);
        assert!(
            pool.try_acquire_parts([48_000_000]).is_none(),
            "single and interval last response slices must retain the actual original query memory credit"
        );
        assert!(frames.reserved_bytes() > 0);
        drop(last);
        assert_eq!(frames.reserved_bytes(), 0);
        assert!(pool.try_acquire_parts([48_000_000]).is_some());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interval_route_uses_real_state_and_counts_complete_outer_framing_for_both_formats() {
    let chain = chain();
    let expected = (1..=4)
        .map(|height| build_proof(&chain.state().view(), height).unwrap())
        .collect::<Vec<_>>();
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let fixture = Arc::get_mut(&mut app).unwrap();
    fixture.state = Arc::clone(chain.state());
    fixture.kura = Arc::clone(chain.kura());
    let router = axum::Router::new()
        .route(
            iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_INTERVAL.path(),
            axum::routing::get(handler_bridge_finality_interval),
        )
        .route(
            iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY.path(),
            axum::routing::get(handler_bridge_finality_proof),
        )
        .layer(axum::middleware::from_fn(enforce_route_timeout))
        .with_state(app);
    for format in [ResponseFormat::Norito, ResponseFormat::Json] {
        let request = axum::http::Request::builder()
            .uri("/v1/bridge/finality/interval/1/4")
            .header(
                axum::http::header::ACCEPT,
                match format {
                    ResponseFormat::Norito => "application/x-norito",
                    ResponseFormat::Json => "application/json",
                },
            )
            .body(Body::empty())
            .unwrap();
        let mut request = request;
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        match format {
            ResponseFormat::Norito => assert_eq!(
                norito::decode_canonical::<Vec<SumeragiFinalityProof>>(&bytes).unwrap(),
                expected
            ),
            ResponseFormat::Json => assert_eq!(
                norito::json::from_slice::<Vec<SumeragiFinalityProof>>(&bytes).unwrap(),
                expected
            ),
        }
        // These zero-copy response bytes still retain the first native request's
        // original reservation. Retire that response before admitting another one.
        drop(bytes);
        let mut request = axum::http::Request::builder()
            .uri("/v1/bridge/finality/4")
            .header(
                axum::http::header::ACCEPT,
                match format {
                    ResponseFormat::Norito => "application/x-norito",
                    ResponseFormat::Json => "application/json",
                },
            )
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        let actual = match format {
            ResponseFormat::Norito => {
                norito::decode_canonical::<SumeragiFinalityProof>(&bytes).unwrap()
            }
            ResponseFormat::Json => {
                norito::json::from_slice::<SumeragiFinalityProof>(&bytes).unwrap()
            }
        };
        assert_eq!(
            actual, expected[3],
            "the single-proof route must use the same funded native producer without changing its standalone schema"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interval_outer_frame_refusal_has_no_partial_body_and_refunds_original_destination() {
    let chain = chain();
    let expected = vec![build_proof(&chain.state().view(), 1).unwrap()];
    let extent = norito::canonical_frame_len(&expected).unwrap();
    let (admission, pool, _) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let frames = owner.cold_frames().clone();
    drop(owner);
    let result = prepare(
        Arc::clone(chain.state()),
        Selection::Interval { from: 1, to: 1 },
        ResponseFormat::Norito,
        admission,
        extent - 1,
        deadline(),
    )
    .await;
    let error = result
        .err()
        .expect("outer canonical header and sequence exceed the original response admission");
    // Exceeding the fixed response contract is terminal, unlike an occupied
    // allocation pool. Preserve the exact outer-frame demand and limit.
    let Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
    )) = error
    else {
        panic!("fixed outer-frame limit lost its original classification: {error}");
    };
    assert_eq!(
        message,
        format!(
            "native response requires {extent} bytes, exceeding its {}-byte bound",
            extent - 1
        )
    );
    assert_eq!(frames.reserved_bytes(), 0);
    assert!(pool.try_acquire_parts([48_000_000]).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_interval_http_future_keeps_original_worker_credit_until_physical_retirement() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    let (admission, pool, query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let frames = owner.cold_frames().clone();
    let (started, entered) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let (finished, result) = tokio::sync::oneshot::channel();
    let request = tokio::spawn(run_with_stop(admission, owner, move |owner, cancelled| {
        let budget = owner.finality_budget()?;
        let prepaid = owner.allocation_context().consumed_allocated_bytes();
        started.send(()).unwrap();
        held.recv().unwrap();
        let limits = NativeFinalityProofIntervalLimits::new(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(4).unwrap(),
            NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
            NonZeroUsize::new(owner.response_bytes()).unwrap(),
            deadline(),
        )
        .unwrap();
        let stopped = matches!(
            state.read_finality_proof_interval(&budget, &limits, &cancelled),
            Err(FinalityProofIntervalReadError::Proof(
                NativeFinalityProofIntervalError::Cancelled
            ))
        );
        let codec_debit = owner.allocation_context().consumed_allocated_bytes() - prepaid;
        finished.send((stopped, codec_debit)).unwrap();
        Ok(())
    }));
    entered.await.unwrap();
    request.abort(); // Cancel only this test's HTTP future; the native worker is never aborted.
    let cancelled_request = request.await.unwrap_err().is_cancelled();
    let physical_held =
        pool.try_acquire_parts([48_000_000]).is_none() && query.available_permits() == 0;
    release.send(()).unwrap();
    let (stopped, codec_debit) = result.await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        // The query permit can retire before the worker's final allocation owner.
        // Wait for both independent resources before checking physical retirement.
        while query.available_permits() != 1 || pool.available_bytes() != 48_000_000 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(cancelled_request && physical_held);
    assert!(
        stopped,
        "cancelled original interval must stop before native acquisition"
    );
    assert_eq!(codec_debit, 0);
    assert_eq!(frames.reserved_bytes(), 0);
    assert!(pool.try_acquire_parts([48_000_000]).is_some());
}

#[tokio::test]
async fn expired_interval_route_deadline_returns_original_timeout_without_native_work() {
    let chain = chain();
    let (admission, pool, query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let limit = owner.response_bytes();
    let prepared = prepare(
        Arc::clone(chain.state()),
        Selection::Interval { from: 1, to: 4 },
        ResponseFormat::Norito,
        admission,
        limit,
        Instant::now(),
    )
    .await
    .unwrap();
    assert_eq!(prepared.response.status(), StatusCode::REQUEST_TIMEOUT);
    assert_eq!(prepared.bytes, 0);
    assert_eq!(owner.allocation_context().consumed_allocated_bytes(), 0);
    drop(owner);
    assert_eq!(query.available_permits(), 1);
    assert!(pool.try_acquire_parts([48_000_000]).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_and_timed_out_workers_keep_original_credit_until_unpolled_stop_control_retires() {
    struct JoinCompleted(std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>);
    impl std::task::Wake for JoinCompleted {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            if let Some(completed) = self.0.lock().unwrap().take() {
                let _ = completed.send(());
            }
        }
    }

    let chain = chain();
    for expired in [false, true] {
        let (admission, pool, query) = admission();
        let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
        // A subpool observation retains no byte-weighted query permit.
        let metadata = owner.response_metadata().clone();
        let state = Arc::clone(chain.state());
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let (completed, notified) = tokio::sync::oneshot::channel();
        let waker = std::task::Waker::from(Arc::new(JoinCompleted(std::sync::Mutex::new(Some(
            completed,
        )))));
        let mut request = Box::pin(run_with_stop(admission, owner, move |owner, cancelled| {
            let _ = started.send(());
            let _ = held.recv();
            let height = if expired { 4 } else { 5 };
            let limits = NativeFinalityProofIntervalLimits::new(
                NonZeroU64::new(height).unwrap(),
                NonZeroU64::new(height).unwrap(),
                NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).unwrap(),
                NonZeroUsize::new(owner.response_bytes()).unwrap(),
                if expired { Instant::now() } else { deadline() },
            )
            .unwrap();
            // Exercise the actual native missing-height or original deadline refusal.
            match state.read_finality_proof_interval(&owner.finality_budget()?, &limits, &cancelled)
            {
                Err(FinalityProofIntervalReadError::Proof(
                    NativeFinalityProofIntervalError::Deadline,
                )) => Ok(PreparedResponse::deadline(ResponseFormat::Norito)),
                Err(error) => Err(source_error(error)),
                Ok(_) => Err(Error::Query(
                    iroha_data_model::ValidationFail::InternalError(
                        "the native refusal control unexpectedly completed".to_owned(),
                    ),
                )),
            }
        }));
        let initial = {
            let mut context = std::task::Context::from_waker(&waker);
            match std::future::Future::poll(request.as_mut(), &mut context) {
                std::task::Poll::Pending => None,
                std::task::Poll::Ready(result) => Some(result),
            }
        };
        let initially_pending = initial.is_none();
        let entered = tokio::time::timeout(std::time::Duration::from_secs(2), entered).await;
        let released = release.send(()).is_ok();
        // This is the sole JoinHandle's completion wake, after the physical operation
        // and the finality worker's admission/TLS owners have retired. The HTTP
        // future is deliberately not polled again until after the observations.
        let physically_closed = initially_pending
            && matches!(
                tokio::time::timeout(std::time::Duration::from_secs(2), notified).await,
                Ok(Ok(()))
            );
        let control_charge = metadata.reserved_bytes();
        let original_credit_held = pool.try_acquire_parts([48_000_000]).is_none();
        let cpu_credit_retired = query.available_permits() == 1;
        // Always join/retire the original future and its result before any assertion.
        let result = match initial {
            Some(result) => result,
            None => request.await,
        };
        let precise_result = if expired {
            matches!(&result, Ok(response) if response.bytes == 0 && response.response.status() == StatusCode::REQUEST_TIMEOUT)
        } else {
            matches!(
                &result,
                Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::NotFound,
                )))
            )
        };
        drop(result);
        let fully_refunded =
            metadata.reserved_bytes() == 0 && pool.try_acquire_parts([48_000_000]).is_some();
        assert!(
            initially_pending && matches!(entered, Ok(Ok(()))) && released && physically_closed
        );
        assert!(
            precise_result,
            "the control must retain the exact native refusal or original 408"
        );
        assert!(
            cpu_credit_retired,
            "physical worker admission must retire at actual completion"
        );
        assert!(control_charge >= ChargedShared::<AtomicBool>::allocation_layout().size());
        assert!(
            original_credit_held,
            "native worker error must retain original query credit until the unpolled HTTP stop control retires"
        );
        assert!(
            fully_refunded,
            "observing the original result must retire all stop-control storage and query credit"
        );
    }
}

// Hold the actual State publication on its own thread until released.
fn hold_publication(
    state: Arc<CoreState>,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    std::sync::mpsc::Sender<()>,
    std::thread::JoinHandle<()>,
) {
    let (held_tx, held_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        state.with_held_view_publication_for_reader_test(|_| {
            held_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
    });
    (held_rx, release_tx, holder)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interval_waits_out_a_busy_state_publication_that_completes_within_its_deadline() {
    let chain = chain();
    let expected = (1..=4)
        .map(|height| build_proof(&chain.state().view(), height).unwrap())
        .collect::<Vec<_>>();
    let (admission, pool, query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let limit = owner.response_bytes();
    drop(owner);
    let (held, release, holder) = hold_publication(Arc::clone(chain.state()));
    held.await.unwrap();
    let request = tokio::spawn(prepare(
        Arc::clone(chain.state()),
        Selection::Interval { from: 1, to: 4 },
        ResponseFormat::Norito,
        admission,
        limit,
        deadline(),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let waiting = !request.is_finished();
    release.send(()).unwrap();
    holder.join().unwrap();
    let prepared = request.await.unwrap().unwrap();
    assert!(
        waiting,
        "a held publication is waited out instead of refused"
    );
    assert_eq!(prepared.response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(prepared.response.into_body(), limit)
        .await
        .unwrap();
    assert_eq!(
        norito::decode_canonical::<Vec<SumeragiFinalityProof>>(&bytes).unwrap(),
        expected
    );
    drop(bytes);
    assert_eq!(query.available_permits(), 1);
    assert!(pool.try_acquire_parts([48_000_000]).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interval_refuses_as_busy_only_when_a_held_publication_outlasts_its_deadline() {
    let chain = chain();
    let (admission, pool, query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let limit = owner.response_bytes();
    let metadata = owner.response_metadata().clone();
    drop(owner);
    let (held, release, holder) = hold_publication(Arc::clone(chain.state()));
    held.await.unwrap();
    let wait = std::time::Duration::from_millis(200);
    let started = Instant::now();
    let result = prepare(
        Arc::clone(chain.state()),
        Selection::Interval { from: 1, to: 4 },
        ResponseFormat::Norito,
        admission,
        limit,
        started + wait,
    )
    .await;
    let waited = started.elapsed();
    release.send(()).unwrap();
    holder.join().unwrap();
    let Err(Error::AppServiceUnavailable { code, .. }) = result else {
        panic!("a publication held past the deadline keeps its busy refusal");
    };
    assert_eq!(code, "finality_history_busy");
    assert!(waited >= wait, "the worker waited out its whole deadline");
    assert_eq!(query.available_permits(), 1);
    assert_eq!(metadata.reserved_bytes(), 0);
    assert!(pool.try_acquire_parts([48_000_000]).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stopped_interval_request_unparks_its_worker_from_a_busy_state_wait() {
    let chain = chain();
    let (admission, pool, query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let limit = owner.response_bytes();
    drop(owner);
    let (held, release, holder) = hold_publication(Arc::clone(chain.state()));
    held.await.unwrap();
    let request = tokio::spawn(prepare(
        Arc::clone(chain.state()),
        Selection::Interval { from: 1, to: 4 },
        ResponseFormat::Norito,
        admission,
        limit,
        deadline(),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    request.abort(); // Cancel only the HTTP future; its native worker observes the stop.
    let cancelled = request.await.is_err_and(|error| error.is_cancelled());
    // The publication is still held, so only the stop control's unpark ends the wait.
    let retired = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while query.available_permits() != 1 || pool.available_bytes() != 48_000_000 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    release.send(()).unwrap();
    holder.join().unwrap();
    assert!(cancelled);
    assert!(
        retired,
        "a stopped request's worker must retire while the publication is still held"
    );
}

#[inline(never)]
fn chain_of(height: u64) -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < height {
        chain.commit(Vec::new());
    }
    chain
}

// One route-shaped read under its own admission, decoded from its exact Norito body.
async fn read_proofs(
    state: Arc<CoreState>,
    selection: Selection,
) -> Result<Vec<SumeragiFinalityProof>, Error> {
    let (admission, _pool, _query) = admission();
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
    let limit = owner.response_bytes();
    drop(owner);
    let prepared = prepare(
        state,
        selection,
        ResponseFormat::Norito,
        admission,
        limit,
        deadline(),
    )
    .await?;
    assert_eq!(prepared.response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(prepared.response.into_body(), limit)
        .await
        .unwrap();
    Ok(match selection {
        Selection::Single(_) => {
            vec![norito::decode_canonical::<SumeragiFinalityProof>(&bytes).unwrap()]
        }
        Selection::Interval { .. } => {
            norito::decode_canonical::<Vec<SumeragiFinalityProof>>(&bytes).unwrap()
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reads_wait_for_an_active_publication_and_both_succeed() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    let expected = (1..=4)
        .map(|height| build_proof(&state.view(), height).unwrap())
        .collect::<Vec<_>>();
    let (held, release, publisher) = hold_publication(Arc::clone(&state));
    held.await.unwrap();
    // Both selections meet the actual publisher. Each used to answer 503
    // finality_history_busy at once; now each waits for that publication's release.
    let interval = tokio::spawn(read_proofs(
        Arc::clone(&state),
        Selection::Interval { from: 1, to: 4 },
    ));
    let single = tokio::spawn(read_proofs(Arc::clone(&state), Selection::Single(4)));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let waited = !interval.is_finished() && !single.is_finished();
    release.send(()).unwrap();
    publisher.join().unwrap();
    let interval = interval.await.unwrap();
    let single = single.await.unwrap();
    assert!(
        waited,
        "busy reads must wait for the publication's release instead of answering busy"
    );
    assert_eq!(interval.unwrap(), expected);
    assert_eq!(single.unwrap(), expected[3..]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_interval_reads_all_succeed_while_publications_repeat() {
    let chain = chain();
    let state = Arc::clone(chain.state());
    let expected = (1..=4)
        .map(|height| build_proof(&state.view(), height).unwrap())
        .collect::<Vec<_>>();
    let stop = Arc::new(AtomicBool::new(false));
    // Actual State publications keep starting and completing under both readers,
    // so their first and final captures meet active publications at random.
    let publisher = std::thread::spawn({
        let (state, stop) = (Arc::clone(&state), Arc::clone(&stop));
        move || {
            let mut publications = 0_u32;
            while !stop.load(Ordering::Acquire) {
                state.with_held_view_publication_for_reader_test(|_| {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                });
                publications += 1;
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
            publications
        }
    });
    let mut results = Vec::new();
    for _ in 0..8 {
        let (whole, tail) = tokio::join!(
            read_proofs(Arc::clone(&state), Selection::Interval { from: 1, to: 4 }),
            read_proofs(Arc::clone(&state), Selection::Interval { from: 3, to: 4 }),
        );
        results.push((whole, tail));
    }
    stop.store(true, Ordering::Release);
    let publications = publisher.join().unwrap();
    assert!(publications > 0);
    for (whole, tail) in results {
        assert_eq!(whole.unwrap(), expected);
        assert_eq!(tail.unwrap(), expected[2..]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefix_ladder_worker_climbs_from_genesis_follows_the_tip_and_stops_at_shutdown() {
    let mut chain = chain_of(10);
    let state = Arc::clone(chain.state());
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let fixture = Arc::get_mut(&mut app).unwrap();
    fixture.state = Arc::clone(&state);
    fixture.kura = Arc::clone(chain.kura());
    // Retained prefixes are process-local: a started node's floor is signed genesis.
    assert_eq!(state.finality_ladder_floor(10), 1);
    let shutdown = ShutdownSignal::new();
    let worker = tokio::spawn(keep_prefix_ladder_warm_in_steps(
        Arc::clone(&app),
        shutdown.clone(),
        3,
    ));
    let reaches = |height: u64, floor: u64| {
        let state = Arc::clone(&state);
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(60), async {
                while state.finality_ladder_floor(height) < floor {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .is_ok()
        }
    };
    // Steps to 4 and 7 bring the ladder within one step of the tip; it then waits.
    assert!(reaches(10, 7).await);
    assert_eq!(state.finality_ladder_floor(10), 7);
    // Once the tip moves more than one step above the ladder, the worker follows it.
    for _ in 0..3 {
        chain.commit(Vec::new());
    }
    assert!(reaches(13, 10).await);
    shutdown.send();
    assert_eq!(
        worker.await.unwrap(),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
}
