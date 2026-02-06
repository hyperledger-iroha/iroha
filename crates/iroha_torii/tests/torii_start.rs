#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Regression test ensuring `Torii::start` waits for shutdown.

use std::{sync::Arc, time::Duration};

use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};

#[tokio::test]
async fn torii_start_blocks_until_shutdown_signal() {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _kiso_child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events.clone()));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    drop(peers_tx);

    let _data_dir = test_utils::TestDataDirGuard::new();

    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        kiso,
        cfg.torii.clone(),
        queue,
        events,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    let shutdown = ShutdownSignal::new();
    let shutdown_for_task = shutdown.clone();
    let join_handle = tokio::spawn(async move { torii.start(shutdown_for_task).await });

    tokio::time::sleep(Duration::from_millis(100)).await;
    if join_handle.is_finished() {
        let early = join_handle
            .await
            .expect("join handle should complete without panicking");
        panic!("Torii::start returned before receiving shutdown signal: {early:?}");
    }

    shutdown.send();
    let join_result = tokio::time::timeout(Duration::from_secs(5), join_handle)
        .await
        .expect("Torii::start should exit after shutdown");
    if let Err(err) = join_result {
        panic!("Torii::start should terminate successfully: {err:?}");
    }
}
