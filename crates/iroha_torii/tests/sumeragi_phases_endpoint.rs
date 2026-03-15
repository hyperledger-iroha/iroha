#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Router-level test for GET /v2/sumeragi/phases (compact per-phase latencies)"]
#![cfg(feature = "telemetry")]

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sumeragi_phases_endpoint_shape() {
    use axum::{Router, routing::get};
    use tower::ServiceExt;

    // Seed snapshot values
    iroha_core::sumeragi::status::set_phase_propose_ms(11);
    iroha_core::sumeragi::status::set_phase_collect_da_ms(22);
    iroha_core::sumeragi::status::set_phase_collect_prevote_ms(33);
    iroha_core::sumeragi::status::set_phase_collect_precommit_ms(44);
    iroha_core::sumeragi::status::set_phase_collect_aggregator_ms(49);
    iroha_core::sumeragi::status::set_phase_commit_ms(77);
    iroha_core::sumeragi::status::set_phase_propose_ema_ms(15);
    iroha_core::sumeragi::status::set_phase_collect_da_ema_ms(26);
    iroha_core::sumeragi::status::set_phase_collect_prevote_ema_ms(37);
    iroha_core::sumeragi::status::set_phase_collect_precommit_ema_ms(48);
    iroha_core::sumeragi::status::set_phase_collect_aggregator_ema_ms(58);
    iroha_core::sumeragi::status::set_phase_commit_ema_ms(80);
    iroha_core::sumeragi::status::set_phase_pipeline_total_ema_ms(206);
    iroha_core::sumeragi::status::inc_gossip_fallback();
    iroha_core::sumeragi::status::inc_block_created_dropped_by_lock();
    iroha_core::sumeragi::status::inc_block_created_hint_mismatch();
    iroha_core::sumeragi::status::inc_block_created_proposal_mismatch();

    // Build a tiny router with the phases endpoint handler
    let app = Router::new().route(
        "/v2/sumeragi/phases",
        get(|| async move { iroha_torii::handle_v1_sumeragi_phases(None).await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/phases")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8(body.to_vec()).unwrap();

    // Parse JSON and check expected keys exist
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    for k in [
        "propose_ms",
        "collect_da_ms",
        "collect_prevote_ms",
        "collect_precommit_ms",
        "collect_aggregator_ms",
        "commit_ms",
        "pipeline_total_ms",
        "collect_aggregator_gossip_total",
        "block_created_dropped_by_lock_total",
        "block_created_hint_mismatch_total",
        "block_created_proposal_mismatch_total",
    ] {
        assert!(v.get(k).is_some(), "missing key: {k}");
    }
    let ema = v
        .get("ema_ms")
        .and_then(|x| x.as_object())
        .expect("ema_ms object present");
    for k in [
        "propose_ms",
        "collect_da_ms",
        "collect_prevote_ms",
        "collect_precommit_ms",
        "collect_aggregator_ms",
        "commit_ms",
        "pipeline_total_ms",
    ] {
        assert!(ema.get(k).is_some(), "missing ema key: {k}");
    }
    assert_eq!(
        v.get("pipeline_total_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        187
    );
    assert_eq!(
        ema.get("pipeline_total_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        206
    );
    assert_eq!(
        v.get("block_created_dropped_by_lock_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        1
    );
    assert_eq!(
        v.get("block_created_hint_mismatch_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        1
    );
    assert_eq!(
        v.get("block_created_proposal_mismatch_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        1
    );
}
