//! Regression test ensuring `Torii::start` waits for shutdown.
use iroha_core::{
    kiso::KisoHandle, query::store::LiveQueryStore, queue::Queue, state::World,
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use std::{sync::Arc, time::Duration};
use crate::fixtures;
#[tokio::test]
async fn torii_start_blocks_until_shutdown_signal() {
    // Some sandboxed test environments forbid binding sockets (for example, when networking is
    // disabled). This regression test exercises `Torii::start`, which binds a listener; if the
    // platform rejects `bind(2)` with `EPERM`, skip rather than failing the entire suite.
    if let Err(err) = std::net::TcpListener::bind("127.0.0.1:0") {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            eprintln!(
                "skipping torii_start_blocks_until_shutdown_signal: socket bind not permitted: {err}"
            );
            return;
        }
        panic!("failed to probe socket bind capability: {err}");
    }
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    let native_chain = fixtures::commit_signed_genesis(&mut cfg, World::default(), Vec::new());
    let (kiso, _kiso_child) = KisoHandle::start(cfg.clone());
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events.clone()));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    drop(peers_tx);
    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        native_chain.network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        events,
        LiveQueryStore::start_test(),
        native_chain.kura().clone(),
        native_chain.state().clone(),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        iroha_torii::test_utils::with_private_settlement_test_signers(
            iroha_torii::ToriiRuntimeDeps::new(
                build_identity_test_fixture::build_identity(),
                MaybeTelemetry::disabled(),
            ),
            &iroha_torii::test_utils::private_settlement_test_node_key(),
        ),
    )
    .expect("valid Torii startup fixture");
    let shutdown = ShutdownSignal::new();
    let shutdown_for_task = shutdown.clone();
    let join_handle =
        tokio::spawn(async move { Box::pin(torii.start(shutdown_for_task, None)).await });
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

use crate::build_identity_test_fixture;
