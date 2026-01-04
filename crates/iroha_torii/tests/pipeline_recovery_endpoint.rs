//! Tests for the pipeline recovery endpoint: `/v1/pipeline/recovery/{height}`.

use axum::{Router, routing::get};
use http_body_util::{BodyExt as _, Full};
use iroha_config::parameters::actual::LaneConfig;
use iroha_core::kura::{Kura, PipelineDagSnapshot, PipelineRecoverySidecar};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::BlockHeader;
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn recovery_endpoint_serves_sidecar_and_404_on_missing() {
    // Initialize a persistent Kura in a temp dir so sidecars can be written/read
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let (kura, _count) = Kura::new(
        &iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        },
        &LaneConfig::default(),
    )
    .expect("kura init");

    // Wire only the tested route, mimicking the closure in lib.rs
    let app = Router::new().route(
        "/v1/pipeline/recovery/{height}",
        get({
            let kura = kura.clone();
            move |axum::extract::Path(height): axum::extract::Path<u64>| async move {
                kura.read_pipeline_metadata(height).map_or_else(
                    || {
                        Err(iroha_torii::Error::Query(
                            iroha_data_model::ValidationFail::QueryFailed(
                                iroha_data_model::query::error::QueryExecutionFail::NotFound,
                            ),
                        ))
                    },
                    |sidecar| {
                        let json = sidecar.to_json_value();
                        let serialized = norito::json::to_json_pretty(&json)
                            .expect("serialize pipeline metadata");
                        let body = Full::from(serialized.into_bytes());
                        Ok::<_, iroha_torii::Error>(
                            axum::http::Response::builder()
                                .status(axum::http::StatusCode::OK)
                                .header(axum::http::header::CONTENT_TYPE, "application/json")
                                .body(body)
                                .unwrap(),
                        )
                    },
                )
            }
        }),
    );

    // Prepare a sidecar for height=1
    let mut fingerprint = [0u8; 32];
    fingerprint[..3].copy_from_slice(&[0xAB, 0xCD, 0xEF]);
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32]));
    let block_hash_str = block_hash.to_string();
    let sidecar = PipelineRecoverySidecar::new_v1(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint,
            key_count: 0,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);

    // Query existing height
    let req_ok = http::Request::builder()
        .method("GET")
        .uri("/v1/pipeline/recovery/1")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_ok = app.clone().oneshot(req_ok).await.unwrap();
    assert_eq!(resp_ok.status(), http::StatusCode::OK);
    let resp_body = resp_ok.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&resp_body).unwrap();
    assert_eq!(
        v.get("format").and_then(|x| x.as_str()),
        Some("pipeline.recovery.v1")
    );
    assert_eq!(
        v.get("block_hash").and_then(|x| x.as_str()),
        Some(block_hash_str.as_str())
    );

    // Query missing height
    let req_missing = http::Request::builder()
        .method("GET")
        .uri("/v1/pipeline/recovery/2")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_missing = app.clone().oneshot(req_missing).await.unwrap();
    assert_eq!(resp_missing.status(), http::StatusCode::NOT_FOUND);
}
