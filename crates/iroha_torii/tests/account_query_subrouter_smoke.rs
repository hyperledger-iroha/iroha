#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test that Torii exposes account query routes via the merged sub-router.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::http::Request;
use http::StatusCode;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::peer::PeerId;
use iroha_data_model::{Registrable, account::Account, domain::Domain};
#[cfg(feature = "telemetry")]
use iroha_primitives::time::TimeSource;
use iroha_test_samples::ALICE_ID;
use iroha_torii::Torii;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn account_query_subrouter_exposes_endpoints() {
    // Minimal Torii setup
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let account_id = ALICE_ID.clone();
    let domain_id: iroha_data_model::domain::DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let mut world = World::with([domain], [account], []);
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            ts,
            false,
        )
        .0
    };
    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    let app = torii.api_router_for_tests();
    let account_segment = fixtures::TX_QUERY_ACCOUNT.canonical.clone();

    // GET assets
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/{account_segment}/assets"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // POST assets/query
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{account_segment}/assets/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // POST transactions/query
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/accounts/{account_segment}/transactions/query"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // GET permissions
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/accounts/{account_segment}/permissions?limit=10"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
}
