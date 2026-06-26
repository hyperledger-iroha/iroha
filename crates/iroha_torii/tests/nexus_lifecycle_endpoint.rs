#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level regression tests for `/v1/nexus/lifecycle`.
#![cfg(feature = "app_api")]

use std::{num::NonZeroU32, sync::Arc};

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
    queue::{ConfigLaneRouter, Queue, QueueLimits, SingleLaneRouter},
    state::State,
};
use iroha_crypto::KeyPair;
use iroha_data_model::nexus::{
    AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
    DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
};
use iroha_torii::Torii;
use iroha_torii_shared::{ErrorEnvelope, uri::NEXUS_LANE_LIFECYCLE};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn decode_norito_json(bytes: &[u8]) -> norito::json::Value {
    let decoded: String = norito::decode_from_bytes(bytes).expect("decode Norito JSON");
    norito::json::from_str(&decoded).expect("parse JSON payload")
}

async fn post_lifecycle(harness: &NexusHarness, body: &str) -> Response {
    let req = fixtures::operator_signed_request(
        &harness.key_pair,
        Request::builder()
            .method("POST")
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap(),
        body.as_bytes(),
    );
    harness.app.clone().oneshot(req).await.unwrap()
}

struct NexusHarness {
    app: Router,
    key_pair: KeyPair,
    state: Arc<State>,
}

fn build_app(nexus_enabled: bool) -> NexusHarness {
    build_app_with_nexus(nexus_enabled, |_| {})
}

fn build_app_with_nexus(
    nexus_enabled: bool,
    configure_nexus: impl FnOnce(&mut iroha_config::parameters::actual::Nexus),
) -> NexusHarness {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.nexus.enabled = nexus_enabled;
    configure_nexus(&mut cfg.nexus);
    let key_pair = cfg.common.key_pair.clone();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = iroha_core::prelude::World::with(
        Vec::new(),
        Vec::new(),
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );
    let mut state = State::new_for_testing(world, kura.clone(), query);
    state
        .set_nexus(cfg.nexus.clone())
        .expect("apply initial Nexus config");
    let state = Arc::new(state);

    let events_sender: EventsSender = tokio::sync::broadcast::channel(64).0;
    let router: Arc<dyn iroha_core::queue::LaneRouter> = if nexus_enabled {
        Arc::new(ConfigLaneRouter::new(
            cfg.nexus.routing_policy.clone(),
            cfg.nexus.dataspace_catalog.clone(),
            cfg.nexus.lane_catalog.clone(),
        ))
    } else {
        Arc::new(SingleLaneRouter::new())
    };
    let lane_catalog = Arc::new(cfg.nexus.lane_catalog.clone());
    let dataspace_catalog = Arc::new(cfg.nexus.dataspace_catalog.clone());
    let queue_limits = QueueLimits::from_nexus(&cfg.nexus);
    let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
        QueueConfig::default(),
        events_sender.clone(),
        router,
        queue_limits,
        &lane_catalog,
        &dataspace_catalog,
        None,
    ));
    {
        let view = state.view();
        queue.reconfigure_nexus(&state.nexus_snapshot(), &view, None);
    }

    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

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
            queue,
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
        queue,
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
        state,
    }
}

fn seed_valid_autoscale_lane(harness: &NexusHarness, lane_id: LaneId, created_height: u64) {
    seed_valid_autoscale_lane_with_autoscale(harness, lane_id, created_height, true);
}

fn seed_valid_autoscale_lane_with_autoscale(
    harness: &NexusHarness,
    lane_id: LaneId,
    created_height: u64,
    autoscale_enabled: bool,
) {
    seed_autoscale_lane(
        harness,
        lane_id,
        DataSpaceId::UNIVERSAL,
        created_height.to_string(),
        autoscale_enabled,
    );
}

fn seed_valid_autoscale_lane_in_dataspace(
    harness: &NexusHarness,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    created_height: u64,
) {
    seed_autoscale_lane(
        harness,
        lane_id,
        dataspace_id,
        created_height.to_string(),
        true,
    );
}

fn seed_invalid_autoscale_lane_with_height(harness: &NexusHarness, lane_id: LaneId, height: &str) {
    seed_autoscale_lane(
        harness,
        lane_id,
        DataSpaceId::UNIVERSAL,
        height.to_owned(),
        true,
    );
}

fn seed_autoscale_lane(
    harness: &NexusHarness,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    created_height: String,
    autoscale_enabled: bool,
) {
    let mut lane = LaneConfig {
        id: lane_id,
        dataspace_id,
        alias: format!("elastic-lane-{}", lane_id.as_u32()),
        ..LaneConfig::default()
    };
    lane.metadata
        .insert(AUTOSCALE_META_MANAGED.to_owned(), "true".to_owned());
    lane.metadata
        .insert(AUTOSCALE_META_CREATED_HEIGHT.to_owned(), created_height);
    let lane_count =
        NonZeroU32::new(lane_id.as_u32().saturating_add(1)).expect("lane count is non-zero");
    let lane_catalog = LaneCatalog::new(lane_count, vec![LaneConfig::default(), lane])
        .expect("valid lane catalog");

    let mut nexus = harness.state.nexus.write();
    nexus.autoscale.enabled = autoscale_enabled;
    if dataspace_id != DataSpaceId::UNIVERSAL {
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: dataspace_id,
                alias: format!("test-dataspace-{}", dataspace_id.as_u64()),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");
    }
    nexus.lane_catalog = lane_catalog;
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
}

#[tokio::test]
async fn nexus_lifecycle_applies_plan_and_reports_lane_count() {
    let harness = build_app(true);
    let body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let req = fixtures::operator_signed_request(
        &harness.key_pair,
        Request::builder()
            .method("POST")
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap(),
        body.as_bytes(),
    );
    let resp = harness.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["ok"].as_bool(), Some(true));
    assert_eq!(payload["lane_count"].as_u64(), Some(2));
}

#[tokio::test]
async fn nexus_lifecycle_rejects_when_disabled() {
    let harness = build_app(false);
    let body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let req = fixtures::operator_signed_request(
        &harness.key_pair,
        Request::builder()
            .method("POST")
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap(),
        body.as_bytes(),
    );
    let resp = harness.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nexus_lifecycle_rejects_duplicate_additions_without_mutating_catalog() {
    let valid_add = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;

    for (body, expected_message) in [
        (
            r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}},{"id":1,"dataspace_id":0,"alias":"gamma","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#,
            "duplicate lane id 1",
        ),
        (
            r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}},{"id":2,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#,
            "duplicate lane alias beta",
        ),
    ] {
        let harness = build_app(true);
        let resp = post_lifecycle(&harness, body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let payload =
            norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
        assert_eq!(payload.code, "lane_lifecycle_error");
        assert!(
            payload.message.contains(expected_message),
            "expected error message to contain {expected_message:?}, got {:?}",
            payload.message
        );

        let valid_resp = post_lifecycle(&harness, valid_add).await;
        assert_eq!(valid_resp.status(), StatusCode::ACCEPTED);
        let bytes = valid_resp.into_body().collect().await.unwrap().to_bytes();
        let payload = decode_norito_json(&bytes);
        assert_eq!(payload["lane_count"].as_u64(), Some(2));
    }
}

#[tokio::test]
async fn nexus_lifecycle_rejects_reserved_autoscale_metadata() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    let rejected = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"spoofed-autoscale-owner","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{"autoscale.managed":"false"}}],"retire":[]}"#;

    let resp = post_lifecycle(&harness, rejected).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("lane 8 uses reserved autoscale-managed metadata"),
        "expected reserved autoscale metadata error, got {:?}",
        payload.message
    );

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_rejects_manual_retire_of_valid_autoscale_lane() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    seed_valid_autoscale_lane(&harness, LaneId::new(1), 2);

    let rejected = r#"{"additions":[],"retire":[1]}"#;
    let resp = post_lifecycle(&harness, rejected).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("lane 1 uses reserved autoscale-managed metadata"),
        "expected reserved autoscale metadata error, got {:?}",
        payload.message
    );

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_rejects_manual_lane_inside_active_autoscale_range() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    let rejected = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"manual-elastic-range","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;

    let resp = post_lifecycle(&harness, rejected).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("reserved autoscale elastic lane id range [1, 8)"),
        "expected reserved range error, got {:?}",
        payload.message
    );

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_allows_repair_retire_of_invalid_autoscale_lane() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    seed_invalid_autoscale_lane_with_height(&harness, LaneId::new(1), "0");

    let unrelated = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-before-repair","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, unrelated).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("autoscale.created_height must be a positive integer"),
        "expected invalid autoscale metadata error, got {:?}",
        payload.message
    );

    let repair = r#"{"additions":[],"retire":[1]}"#;
    let resp = post_lifecycle(&harness, repair).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_allows_repair_retire_of_out_of_range_autoscale_lane() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    seed_valid_autoscale_lane(&harness, LaneId::new(8), 2);

    let unrelated = r#"{"additions":[{"id":9,"dataspace_id":0,"alias":"outside-range-before-repair","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, unrelated).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload.message.contains(
            "autoscale-managed lane 8 is outside configured autoscale lane id range [1, 8)"
        ),
        "expected out-of-range autoscale metadata error, got {:?}",
        payload.message
    );

    let repair = r#"{"additions":[],"retire":[8]}"#;
    let resp = post_lifecycle(&harness, repair).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));

    let accepted = r#"{"additions":[{"id":9,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(10));
}

#[tokio::test]
async fn nexus_lifecycle_allows_repair_retire_of_off_default_autoscale_lane() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = true;
    });
    let off_default_dataspace = DataSpaceId::new(9);
    seed_valid_autoscale_lane_in_dataspace(&harness, LaneId::new(1), off_default_dataspace, 2);

    let unrelated = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-before-repair","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, unrelated).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("autoscale owns only default dataspace"),
        "expected off-default autoscale ownership error, got {:?}",
        payload.message
    );

    let repair = r#"{"additions":[],"retire":[1]}"#;
    let resp = post_lifecycle(&harness, repair).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_allows_repair_retire_of_disabled_autoscale_owned_lane() {
    let harness = build_app_with_nexus(true, |nexus| {
        nexus.autoscale.enabled = false;
    });
    seed_valid_autoscale_lane_with_autoscale(&harness, LaneId::new(1), 2, false);

    let unrelated = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-before-repair","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, unrelated).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&bytes).expect("decode error payload");
    assert_eq!(payload.code, "lane_lifecycle_error");
    assert!(
        payload
            .message
            .contains("autoscale-managed lane 1 requires nexus.autoscale.enabled=true"),
        "expected disabled autoscale ownership error, got {:?}",
        payload.message
    );

    let repair = r#"{"additions":[],"retire":[1]}"#;
    let resp = post_lifecycle(&harness, repair).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));

    let accepted = r#"{"additions":[{"id":8,"dataspace_id":0,"alias":"outside-range-manual","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let resp = post_lifecycle(&harness, accepted).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(9));
}

#[tokio::test]
async fn nexus_lifecycle_supports_retire_after_add() {
    let harness = build_app(true);
    let add_body = r#"{"additions":[{"id":1,"dataspace_id":0,"alias":"beta","description":null,"visibility":"public","lane_type":null,"governance":null,"settlement":null,"storage":"full_replica","proof_scheme":"merkle_sha256","metadata":{}}],"retire":[]}"#;
    let add_req = fixtures::operator_signed_request(
        &harness.key_pair,
        Request::builder()
            .method("POST")
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("content-type", "application/json")
            .body(Body::from(add_body))
            .unwrap(),
        add_body.as_bytes(),
    );
    let add_resp = harness.app.clone().oneshot(add_req).await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::ACCEPTED);

    let retire_body = r#"{"additions":[],"retire":[1]}"#;
    let retire_req = fixtures::operator_signed_request(
        &harness.key_pair,
        Request::builder()
            .method("POST")
            .uri(NEXUS_LANE_LIFECYCLE)
            .header("content-type", "application/json")
            .body(Body::from(retire_body))
            .unwrap(),
        retire_body.as_bytes(),
    );
    let retire_resp = harness.app.clone().oneshot(retire_req).await.unwrap();
    assert_eq!(retire_resp.status(), StatusCode::ACCEPTED);
    let bytes = retire_resp.into_body().collect().await.unwrap().to_bytes();
    let payload = decode_norito_json(&bytes);
    assert_eq!(payload["lane_count"].as_u64(), Some(1));
}
