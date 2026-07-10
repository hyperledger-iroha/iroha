#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level regressions for the read-only Nexus lifecycle status surface.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    EventsSender,
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, Queue, QueueLimits},
    state::State,
};
use iroha_crypto::KeyPair;
use iroha_data_model::nexus::{LaneId, LaneLifecycleStatusV1};
use iroha_torii::Torii;
use iroha_torii_shared::{ErrorEnvelope, uri::NEXUS_LANE_LIFECYCLE};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

struct NexusHarness {
    app: Router,
    key_pair: KeyPair,
    queue: Arc<Queue>,
    state: Arc<State>,
}

fn build_app() -> NexusHarness {
    build_app_with_api_token(None)
}

fn build_app_with_api_token(api_token: Option<&str>) -> NexusHarness {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.nexus.enabled = true;
    if let Some(api_token) = api_token {
        cfg.torii.require_api_token = true;
        cfg.torii.api_tokens = vec![api_token.to_owned()];
    }
    let key_pair = cfg.common.key_pair.clone();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let world = iroha_core::prelude::World::with(
        Vec::new(),
        Vec::new(),
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );
    let mut state = State::new_for_testing(world, kura.clone(), LiveQueryStore::start_test());
    state
        .set_nexus(cfg.nexus.clone())
        .expect("apply initial Nexus config");
    let state = Arc::new(state);

    let events_sender: EventsSender = tokio::sync::broadcast::channel(64).0;
    let router = Arc::new(ConfigLaneRouter::new(
        cfg.nexus.routing_policy.clone(),
        cfg.nexus.dataspace_catalog.clone(),
        cfg.nexus.lane_catalog.clone(),
    ));
    let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
        QueueConfig::default(),
        events_sender.clone(),
        router,
        QueueLimits::from_nexus(&cfg.nexus),
        &Arc::new(cfg.nexus.lane_catalog.clone()),
        &Arc::new(cfg.nexus.dataspace_catalog.clone()),
        None,
    ));
    {
        let view = state.view();
        queue.reconfigure_nexus(&state.nexus_snapshot(), &view, None);
    }

    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let da_receipt_signer = cfg.common.key_pair.clone();
    #[cfg(feature = "telemetry")]
    let torii = {
        let telemetry = iroha_core::telemetry::Telemetry::new(
            Arc::new(iroha_telemetry::metrics::Metrics::default()),
            false,
        );
        Torii::new(
            iroha_data_model::ChainId::from("test-chain"),
            kiso,
            cfg.torii.clone(),
            Arc::clone(&queue),
            events_sender,
            LiveQueryStore::start_test(),
            kura,
            Arc::clone(&state),
            da_receipt_signer,
            iroha_torii::OnlinePeersProvider::new(peers_rx),
            telemetry,
            false,
        )
    };
    #[cfg(not(feature = "telemetry"))]
    let torii = Torii::new(
        iroha_data_model::ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        Arc::clone(&queue),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
    );

    NexusHarness {
        app: torii.api_router_for_tests(),
        key_pair,
        queue,
        state,
    }
}

async fn response_bytes(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn lifecycle_get_returns_valid_exact_json_status() {
    let harness = build_app();
    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("accept", "application/json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    let status: LaneLifecycleStatusV1 =
        norito::json::from_slice(&response_bytes(response).await).expect("decode status JSON");
    assert!(status.nexus_enabled);
    assert_eq!(
        status.validate().expect("validate status"),
        harness.state.nexus_snapshot().lane_catalog
    );
}

#[tokio::test]
async fn lifecycle_get_returns_valid_exact_norito_status() {
    let harness = build_app();
    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("accept", "application/x-norito")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let status =
        norito::decode_from_bytes::<LaneLifecycleStatusV1>(&response_bytes(response).await)
            .expect("decode status Norito");
    assert_eq!(
        status.validate().expect("validate status"),
        harness.state.nexus_snapshot().lane_catalog
    );
}

#[tokio::test]
async fn lifecycle_get_honors_api_token_access_policy() {
    let harness = build_app_with_api_token(Some("lifecycle-status-token"));
    for supplied_token in [None, Some("wrong-token")] {
        let mut request = Request::builder()
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("accept", "application/json");
        if let Some(token) = supplied_token {
            request = request.header("x-api-token", token);
        }
        let response = harness
            .app
            .clone()
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(NEXUS_LANE_LIFECYCLE)
                .header("accept", "application/json")
                .header("x-api-token", "lifecycle-status-token")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

async fn assert_deprecated_post_never_mutates(signed: bool) {
    let harness = build_app();
    let lane = LaneId::new(1);
    let before_catalog = harness.state.nexus_snapshot().lane_catalog;
    let before_limits = harness.queue.queue_limits().for_lane(lane);
    let body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"forbidden-local","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let request = Request::builder()
        .method("POST")
        .uri(NEXUS_LANE_LIFECYCLE)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("request");
    let request = if signed {
        fixtures::operator_signed_request(&harness.key_pair, request, body.as_bytes())
    } else {
        request
    };
    let response = harness
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error = norito::decode_from_bytes::<ErrorEnvelope>(&response_bytes(response).await)
        .expect("decode deprecated endpoint error");
    assert_eq!(error.code, "local_lane_lifecycle_disabled");
    assert!(error.message.contains("nexus_lane_lifecycle_v1"));
    assert_eq!(harness.state.nexus_snapshot().lane_catalog, before_catalog);
    assert_eq!(harness.queue.queue_limits().for_lane(lane), before_limits);
}

#[tokio::test]
async fn lifecycle_post_is_explicitly_deprecated_for_unsigned_requests() {
    assert_deprecated_post_never_mutates(false).await;
}

#[tokio::test]
async fn lifecycle_post_is_explicitly_deprecated_for_operator_signed_requests() {
    assert_deprecated_post_never_mutates(true).await;
}

#[tokio::test]
async fn empty_malformed_and_oversized_deprecated_posts_never_decode_or_mutate() {
    let harness = build_app();
    let before = harness.state.nexus_snapshot().lane_catalog;
    for body in [Vec::new(), b"{".to_vec(), vec![b'x'; 4 * 1024 * 1024]] {
        let response = harness
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(NEXUS_LANE_LIFECYCLE)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(harness.state.nexus_snapshot().lane_catalog, before);
}
