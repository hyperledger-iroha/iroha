//! Real worker/supervisor concurrency, physical ownership and failure propagation.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc as blocking_mpsc,
};
use tokio::{sync::oneshot, time::timeout};

struct OwnedWork {
    drops: Arc<AtomicUsize>,
    _memory: tokio::sync::OwnedSemaphorePermit,
}
impl Drop for OwnedWork {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

async fn serve_until_shutdown(shutdown: ShutdownSignal) -> std::io::Result<()> {
    shutdown.receive().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn blocked_publication_keeps_response_delivery_running() {
    let shutdown = ShutdownSignal::new();
    let (input_tx, input_rx) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = blocking_mpsc::channel();
    let (response_tx, response_rx) = oneshot::channel();
    input_tx.send(()).await.unwrap();
    let mut entered_tx = Some(entered_tx);
    let publication = tokio::spawn(run_work(
        input_rx,
        shutdown.clone(),
        runtime().unwrap(),
        move |()| {
            entered_tx.take().unwrap().send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("response task must execute while publication blocks");
            std::future::ready(())
        },
    ));
    let response_shutdown = shutdown.clone();
    let response = tokio::spawn(async move {
        entered_rx.await.unwrap();
        tokio::task::yield_now().await;
        response_tx.send(()).unwrap();
        release_tx.send(()).unwrap();
        response_shutdown.receive().await;
        ToriiCriticalWorkerExit::StoppedByShutdown
    });
    timeout(Duration::from_secs(1), response_rx)
        .await
        .unwrap()
        .unwrap();
    shutdown.send();
    let workers = vec![
        ToriiCriticalWorker {
            name: "publication",
            task: publication,
        },
        ToriiCriticalWorker {
            name: "response",
            task: response,
        },
    ];
    timeout(
        Duration::from_secs(3),
        supervise_torii_critical_workers(shutdown.clone(), workers, serve_until_shutdown(shutdown)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn request_receives_response_while_retaining_its_single_proxy_slot() {
    let shutdown = ShutdownSignal::new();
    let memory = Arc::new(tokio::sync::Semaphore::new(1));
    let drops = Arc::new(AtomicUsize::new(0));
    let (input_tx, input_rx) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (response_tx, response_rx) = oneshot::channel();
    let mut response_rx = Some(response_rx);
    let mut entered_tx = Some(entered_tx);
    input_tx
        .send(OwnedWork {
            drops: drops.clone(),
            _memory: memory.clone().acquire_owned().await.unwrap(),
        })
        .await
        .unwrap();
    let request_shutdown = shutdown.clone();
    let request = tokio::spawn(run_work(
        input_rx,
        shutdown.clone(),
        runtime().unwrap(),
        move |work| {
            let entered = entered_tx.take().unwrap();
            let response = response_rx.take().unwrap();
            let shutdown = request_shutdown.clone();
            async move {
                entered.send(()).unwrap();
                response.await.unwrap();
                drop(work);
                shutdown.send();
            }
        },
    ));
    let response_shutdown = shutdown.clone();
    let response_memory = memory.clone();
    let response = tokio::spawn(async move {
        entered_rx.await.unwrap();
        assert_eq!(response_memory.available_permits(), 0);
        response_tx.send(()).unwrap();
        response_shutdown.receive().await;
        ToriiCriticalWorkerExit::StoppedByShutdown
    });
    let workers = vec![
        ToriiCriticalWorker {
            name: "request",
            task: request,
        },
        ToriiCriticalWorker {
            name: "response",
            task: response,
        },
    ];
    timeout(
        Duration::from_secs(3),
        supervise_torii_critical_workers(shutdown.clone(), workers, serve_until_shutdown(shutdown)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(memory.available_permits(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shutdown_joins_physical_work_before_releasing_its_owner() {
    let shutdown = ShutdownSignal::new();
    let memory = Arc::new(tokio::sync::Semaphore::new(1));
    let drops = Arc::new(AtomicUsize::new(0));
    let (input_tx, input_rx) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = blocking_mpsc::channel();
    let mut entered_tx = Some(entered_tx);
    input_tx
        .send(OwnedWork {
            drops: drops.clone(),
            _memory: memory.clone().acquire_owned().await.unwrap(),
        })
        .await
        .unwrap();
    let work = tokio::spawn(run_work(
        input_rx,
        shutdown.clone(),
        runtime().unwrap(),
        move |work| {
            entered_tx.take().unwrap().send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("release physical worker");
            drop(work);
            std::future::ready(())
        },
    ));
    let workers = vec![ToriiCriticalWorker {
        name: "physical",
        task: work,
    }];
    let mut supervision = tokio::spawn(supervise_torii_critical_workers(
        shutdown.clone(),
        workers,
        serve_until_shutdown(shutdown.clone()),
    ));
    timeout(Duration::from_secs(1), entered_rx)
        .await
        .unwrap()
        .unwrap();
    shutdown.send();
    assert!(
        timeout(Duration::from_millis(50), &mut supervision)
            .await
            .is_err()
    );
    assert_eq!(memory.available_permits(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    release_tx.send(()).unwrap();
    timeout(Duration::from_secs(3), supervision)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(memory.available_permits(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn physical_panic_reaches_critical_supervisor() {
    let shutdown = ShutdownSignal::new();
    let (input_tx, input_rx) = mpsc::channel(1);
    input_tx.send(()).await.unwrap();
    let task = tokio::spawn(run_work(
        input_rx,
        shutdown.clone(),
        runtime().unwrap(),
        |()| -> std::future::Ready<()> {
            // Measure unwind propagation and supervision, excluding the
            // process-global panic hook's diagnostic I/O and symbolization.
            std::panic::resume_unwind(Box::new("deliberate proxy physical-worker panic"));
        },
    ));
    let workers = vec![ToriiCriticalWorker {
        name: "physical",
        task,
    }];
    let result = timeout(
        Duration::from_secs(2),
        supervise_torii_critical_workers(shutdown.clone(), workers, serve_until_shutdown(shutdown)),
    )
    .await
    .unwrap();
    assert_eq!(
        result.unwrap_err(),
        ToriiCriticalWorkerFailure::Panicked("physical")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unexpected_channel_closure_fails_critical_supervision() {
    let shutdown = ShutdownSignal::new();
    let (input_tx, input_rx) = mpsc::channel::<()>(1);
    drop(input_tx);
    let task = tokio::spawn(run_work(
        input_rx,
        shutdown.clone(),
        runtime().unwrap(),
        |()| std::future::ready(()),
    ));
    let workers = vec![ToriiCriticalWorker {
        name: "physical",
        task,
    }];
    let result = timeout(
        Duration::from_secs(2),
        supervise_torii_critical_workers(shutdown.clone(), workers, serve_until_shutdown(shutdown)),
    )
    .await
    .unwrap();
    assert_eq!(
        result.unwrap_err(),
        ToriiCriticalWorkerFailure::ExitedUnexpectedly("physical")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn signalled_shutdown_drops_queued_owner_without_processing() {
    let shutdown = ShutdownSignal::new();
    let memory = Arc::new(tokio::sync::Semaphore::new(1));
    let drops = Arc::new(AtomicUsize::new(0));
    let (input_tx, input_rx) = mpsc::channel(1);
    input_tx
        .send(OwnedWork {
            drops: drops.clone(),
            _memory: memory.clone().acquire_owned().await.unwrap(),
        })
        .await
        .unwrap();
    shutdown.send();
    let result = run_work(
        input_rx,
        shutdown,
        runtime().unwrap(),
        |_| -> std::future::Ready<()> {
            panic!("queued work must not start after shutdown");
        },
    )
    .await;
    assert_eq!(result, ToriiCriticalWorkerExit::StoppedByShutdown);
    assert_eq!(memory.available_permits(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn missing_runtime_returns_precise_error() {
    assert_eq!(
        runtime().unwrap_err(),
        "Torii proxy workers require an active Tokio runtime"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_runtime_returns_precise_error() {
    assert_eq!(
        runtime().unwrap_err(),
        "Torii proxy workers require a multithreaded Tokio runtime"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn response_dispatch_ignores_full_admission_queues_and_owned_proxy_slot() {
    let app = crate::mk_app_state_for_tests();
    let network = iroha_core::IrohaNetwork::closed_for_tests();
    let keypair =
        iroha_crypto::KeyPair::from_seed(vec![0x79; 32], iroha_crypto::Algorithm::Ed25519);
    let peer = Peer::new(
        "127.0.0.1:1337".parse().unwrap(),
        keypair.public_key().clone(),
    );
    let (requests, request_rx) = mpsc::channel(1);
    let (publications, publication_rx) = mpsc::channel(1);
    let route = iroha_core::queue::RoutingDecision::new(
        iroha_model_base::topology::LaneId::SINGLE,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    );
    let request = Arc::new(ToriiProxyRequestV1 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V1,
        request_id: Hash::new(b"held-request"),
        deadline_unix_ms: crate::torii_proxy_test_deadline_unix_ms(),
        hop_count: 0,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV1::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: iroha_core::torii_proxy::ToriiRouteHintV1::from(route),
            response_format: iroha_core::torii_proxy::ToriiProxyResponseFormatV1::Norito,
        },
    });
    let request_payload = iroha_core::NetworkMessage::ToriiProxyRequest(request);
    let bytes = norito::to_bytes(&request_payload).unwrap().len();
    dispatch(
        &app,
        &network,
        &requests,
        &publications,
        PeerMessage::new(peer.clone(), request_payload, bytes),
    )
    .await;
    assert_eq!(requests.capacity(), 0);
    assert_eq!(app.torii_proxy_memory_inflight.available_permits(), 0);
    // This deliberately unvalidated publication only fills the bounded queue;
    // no physical persistence worker is started by this dispatcher test.
    let publication_payload = iroha_core::NetworkMessage::QueuePlanAdmissionPublication(Arc::new(
        QueuePlanAdmissionPublicationV1 {
            schema_version: 1,
            certificate: vec![0; 8],
        },
    ));
    let bytes = norito::to_bytes(&publication_payload).unwrap().len();
    dispatch(
        &app,
        &network,
        &requests,
        &publications,
        PeerMessage::new(peer.clone(), publication_payload, bytes),
    )
    .await;
    assert_eq!(publications.capacity(), 0);
    let request_id = Hash::new(b"independent-response");
    let (response_tx, response_rx) = oneshot::channel();
    let _waiter = crate::register_torii_proxy_pending_waiter(
        &app,
        (request_id, peer.id().clone()),
        response_tx,
        usize::MAX,
        false,
    );
    let expected = ToriiProxyHttpResponseV1 {
        status_code: 202,
        headers: Vec::new(),
        body: b"response".to_vec(),
    };
    let response = iroha_core::NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
        schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
        request_id,
        response: expected.clone(),
    }));
    let bytes = norito::to_bytes(&response).unwrap().len();
    timeout(
        Duration::from_secs(1),
        dispatch(
            &app,
            &network,
            &requests,
            &publications,
            PeerMessage::new(peer, response, bytes),
        ),
    )
    .await
    .unwrap();
    assert_eq!(response_rx.await.unwrap(), expected);
    assert_eq!(requests.capacity(), 0);
    assert_eq!(publications.capacity(), 0);
    assert_eq!(app.torii_proxy_memory_inflight.available_permits(), 0);
    drop(request_rx);
    drop(publication_rx);
    assert_eq!(app.torii_proxy_memory_inflight.available_permits(), 1);
}
