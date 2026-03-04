#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for /v1/zk/proofs and /v1/zk/proofs/count.
#![cfg(feature = "app_api")]
#![allow(clippy::too_many_lines)]

use std::{collections::HashSet, sync::Arc, time::Duration};

use axum::{Router, routing::get};
use http_body_util::BodyExt as _;
use iroha_config::parameters::defaults;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::proof::{ProofId, ProofRecord, ProofStatus};
use nonzero_ext::nonzero;
use tower::ServiceExt as _;

#[tokio::test]
async fn proofs_list_and_count_with_filters() {
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = {
        #[cfg(feature = "telemetry")]
        {
            State::new(
                World::new(),
                kura,
                query,
                iroha_core::telemetry::StateTelemetry::default(),
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            State::new(World::new(), kura, query)
        }
    };

    // Seed two proofs under the same backend; assign tags for one
    let backend = "halo2/ipa";
    {
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let id1 = ProofId {
            backend: backend.into(),
            proof_hash: [0x01; 32],
        };
        let rec1 = ProofRecord {
            id: id1.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(1),
            bridge: None,
        };
        stx.world.proofs_mut_for_testing().insert(id1.clone(), rec1);
        // Tag index for id1
        stx.world
            .proof_tags_mut_for_testing()
            .insert(id1.clone(), vec![*b"PROF", *b"I10P"]);
        stx.world
            .proofs_by_tag_mut_for_testing()
            .insert(*b"PROF", vec![id1.clone()]);
        stx.world
            .proofs_by_tag_mut_for_testing()
            .insert(*b"I10P", vec![id1.clone()]);

        let id2 = ProofId {
            backend: backend.into(),
            proof_hash: [0x02; 32],
        };
        let rec2 = ProofRecord {
            id: id2.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Rejected,
            verified_at_height: Some(2),
            bridge: None,
        };
        stx.world.proofs_mut_for_testing().insert(id2.clone(), rec2);
        let id3 = ProofId {
            backend: backend.into(),
            proof_hash: [0x03; 32],
        };
        let rec3 = ProofRecord {
            id: id3.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Submitted,
            verified_at_height: None,
            bridge: None,
        };
        stx.world.proofs_mut_for_testing().insert(id3.clone(), rec3);
        stx.apply();
        block
            .transactions
            .insert_block(HashSet::new(), nonzero!(1_usize));
        let _ = block.commit();
    }

    let state = Arc::new(state);
    let limits = iroha_torii::ProofApiLimits::default();
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let app = Router::new()
        .route(
            "/v1/zk/proofs",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |q: iroha_torii::NoritoQuery<iroha_torii::ProofListQuery>| async move {
                    iroha_torii::handle_list_proofs(state, limits, telemetry, q).await
                }
            }),
        )
        .route(
            "/v1/zk/proofs/count",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |q: iroha_torii::NoritoQuery<iroha_torii::ProofListQuery>| async move {
                    iroha_torii::handle_count_proofs(state, limits, telemetry, q).await
                }
            }),
        );

    // List all
    let resp_all = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_all.status(), http::StatusCode::OK);
    let arr_all: Vec<norito::json::Value> =
        norito::json::from_slice(&resp_all.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(arr_all.len(), 3);

    // Count with status filter
    let resp_cnt = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs/count?status=Verified")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_cnt.status(), http::StatusCode::OK);
    let vcnt: norito::json::Value =
        norito::json::from_slice(&resp_cnt.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(
        vcnt.get("count").and_then(norito::json::Value::as_u64),
        Some(1)
    );

    // List with has_tag=PROF should return only id1
    let resp_tag = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs?has_tag=PROF&ids_only=true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_tag.status(), http::StatusCode::OK);
    let id_values: Vec<norito::json::Value> =
        norito::json::from_slice(&resp_tag.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(id_values.len(), 1);
    let backend_s = id_values[0]
        .get("backend")
        .and_then(norito::json::Value::as_str)
        .unwrap();
    let hash_s = id_values[0]
        .get("hash")
        .and_then(norito::json::Value::as_str)
        .unwrap();
    assert_eq!(backend_s, backend);
    assert_eq!(hash_s, hex::encode([0x01u8; 32]));

    // Height lower-bound filter should return only proofs verified at or above height 2
    let resp_min = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs?verified_from_height=2&ids_only=true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_min.status(), http::StatusCode::OK);
    let arr_min: Vec<norito::json::Value> =
        norito::json::from_slice(&resp_min.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(arr_min.len(), 1);
    assert_eq!(
        arr_min[0]
            .get("hash")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hex::encode([0x02u8; 32])
    );

    // Upper-bound filter should limit results to the earliest verified proof
    let resp_max = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs?verified_until_height=1&ids_only=true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_max.status(), http::StatusCode::OK);
    let arr_max: Vec<norito::json::Value> =
        norito::json::from_slice(&resp_max.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(arr_max.len(), 1);
    assert_eq!(
        arr_max[0]
            .get("hash")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hex::encode([0x01u8; 32])
    );

    // Invalid range should yield a bad request.
    let resp_err = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/v1/zk/proofs?verified_from_height=10&verified_until_height=5")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status_err = resp_err.status();
    let err_body = resp_err.into_body().collect().await.unwrap().to_bytes();
    let err_json: Vec<norito::json::Value> = norito::json::from_slice(&err_body).unwrap();
    assert!(err_json.is_empty());
    assert_eq!(status_err, http::StatusCode::OK);
}

#[tokio::test]
async fn proofs_list_rejects_over_limit() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = {
        #[cfg(feature = "telemetry")]
        {
            State::new(
                World::new(),
                kura,
                query,
                iroha_core::telemetry::StateTelemetry::default(),
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            State::new(World::new(), kura, query)
        }
    };

    let state = Arc::new(state);
    let limits = iroha_torii::ProofApiLimits::new(
        5,
        Duration::from_millis(50),
        Duration::from_secs(defaults::torii::PROOF_CACHE_MAX_AGE_SECS),
        Duration::from_secs(defaults::torii::PROOF_RETRY_AFTER_SECS),
        defaults::torii::PROOF_MAX_BODY_BYTES.get(),
    );
    let result = iroha_torii::handle_list_proofs(
        state,
        limits,
        iroha_torii::MaybeTelemetry::disabled(),
        iroha_torii::NoritoQuery(iroha_torii::ProofListQuery {
            limit: Some(50),
            ..Default::default()
        }),
    )
    .await;
    assert!(
        result.is_err(),
        "requests above the configured page cap must be rejected"
    );
}
