//! Behavior of the real W permit across response consumption, cancellation and physical work.
use super::*;
use crate::Body;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Semaphore, oneshot},
    time::timeout,
};

#[tokio::test]
async fn finalization_retains_w_after_intermediate_body_and_through_final_body() {
    let memory = Arc::new(Semaphore::new(1));
    let reservation = ToriiProxyMemoryReservation::new(memory.clone().try_acquire_owned().unwrap());
    let intermediate = hold_torii_proxy_memory_in_response_body(
        Response::new(Body::from("certificate")),
        reservation.clone(),
    );
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let work = tokio::spawn(complete(intermediate, reservation, |response| async move {
        let bytes = axum::body::to_bytes(response.into_body(), 32)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"certificate");
        entered_tx.send(()).unwrap();
        release_rx.await.unwrap();
        Response::new(Body::from("durable receipt"))
    }));
    timeout(Duration::from_secs(2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        memory.available_permits(),
        0,
        "consuming the intermediate Body must not release the complete admission owner"
    );
    assert!(
        memory.clone().try_acquire_owned().is_err(),
        "another W-sized graph cannot start during finalization"
    );
    release_tx.send(()).unwrap();
    let response = timeout(Duration::from_secs(2), work)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        memory.available_permits(),
        0,
        "the final response Body owns W after the finalizer returns"
    );
    let bytes = axum::body::to_bytes(response.into_body(), 32)
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), b"durable receipt");
    assert_eq!(memory.available_permits(), 1);
}

#[tokio::test]
async fn cancellation_during_async_finalization_releases_the_owned_slot() {
    let memory = Arc::new(Semaphore::new(1));
    let reservation = ToriiProxyMemoryReservation::new(memory.clone().try_acquire_owned().unwrap());
    let (entered_tx, entered_rx) = oneshot::channel();
    let work = tokio::spawn(complete(
        Response::new(Body::empty()),
        reservation,
        |response| async move {
            axum::body::to_bytes(response.into_body(), 1).await.unwrap();
            entered_tx.send(()).unwrap();
            std::future::pending::<()>().await;
            Response::new(Body::empty())
        },
    ));
    timeout(Duration::from_secs(2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(memory.available_permits(), 0);
    work.abort();
    assert!(
        timeout(Duration::from_secs(2), work)
            .await
            .unwrap()
            .unwrap_err()
            .is_cancelled()
    );
    assert_eq!(
        memory.available_permits(),
        1,
        "cancelling an idle inline finalizer leaves no detached owner"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cancellation_cannot_release_w_before_physical_finalization_returns() {
    let memory = Arc::new(Semaphore::new(1));
    let reservation = ToriiProxyMemoryReservation::new(memory.clone().try_acquire_owned().unwrap());
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let work = tokio::spawn(complete(
        Response::new(Body::empty()),
        reservation,
        |response| async move {
            axum::body::to_bytes(response.into_body(), 1).await.unwrap();
            tokio::task::block_in_place(|| {
                entered_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(3))
                    .expect("physical operation release");
            });
            Response::new(Body::empty())
        },
    ));
    timeout(Duration::from_secs(2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    work.abort();
    tokio::task::yield_now().await;
    assert_eq!(
        memory.available_permits(),
        0,
        "cancellation cannot detach a still-running physical finalizer from W"
    );
    release_tx.send(()).unwrap();
    // Cancellation may race an already-ready return; dropping either outcome must release W.
    drop(timeout(Duration::from_secs(3), work).await.unwrap());
    assert_eq!(memory.available_permits(), 1);
}
