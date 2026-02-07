#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for /v1/zk/prover/reports endpoints (`app_api`).
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs, clippy::similar_names, unused_imports)]

use std::sync::Once;

use axum::{
    Router,
    response::IntoResponse,
    routing::{delete, get},
};
use http_body_util::BodyExt as _;
use iroha_core::zk::test_utils::halo2_fixture_envelope;
use iroha_data_model::proof::ProofAttachment;
use iroha_torii::zk_attachments::AttachmentTenant;
use tower::ServiceExt as _;

fn ensure_quota_config() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        iroha_torii::zk_attachments::configure(
            iroha_config::parameters::defaults::torii::ATTACHMENTS_TTL_SECS,
            iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_BYTES,
            iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
            8 * 1024 * 1024,
            iroha_config::parameters::defaults::torii::attachments_allowed_mime_types(),
            iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
            iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
            iroha_config::parameters::actual::AttachmentSanitizerMode::InProcess,
            iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS,
            None,
            iroha_torii::MaybeTelemetry::disabled(),
        );
    });
}

fn fixture_attachment_bytes() -> Vec<u8> {
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let proof = fixture.proof_box("halo2/ipa");
    let vk = fixture.vk_box("halo2/ipa").expect("fixture vk bytes");
    let attachment = ProofAttachment::new_inline("halo2/ipa".into(), proof, vk);
    norito::to_bytes(&attachment).expect("proof attachment bytes")
}

#[tokio::test]
async fn prover_reports_list_get_delete() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();
    // Build a minimal router wiring prover endpoints
    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            get(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_get_report(id).await
            }),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            delete(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_delete_report(id).await
            }),
        );

    // Create an attachment
    iroha_torii::zk_attachments::init_persistence();
    let body = fixture_attachment_bytes();
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-norito"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Trigger scan once
    let created = iroha_torii::zk_prover::scan_once();
    assert!(created >= 1);

    // List reports
    let req_list = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), http::StatusCode::OK);
    let list_bytes = resp_list.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&list_bytes).unwrap();
    assert!(
        arr.iter()
            .any(|m| m.get("id").and_then(|v| v.as_str()) == Some(&id))
    );

    // Get specific report
    let req_get = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), http::StatusCode::OK);
    let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(v.get("backend").and_then(|x| x.as_str()), Some("halo2/ipa"));
    assert!(v.get("proof_hash").and_then(|x| x.as_str()).is_some());

    // Delete the report
    let req_del = http::Request::builder()
        .method("DELETE")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_del = app.clone().oneshot(req_del).await.unwrap();
    assert_eq!(resp_del.status(), http::StatusCode::NO_CONTENT);

    // Getting it again should 404
    let req_get2 = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get2 = app.clone().oneshot(req_get2).await.unwrap();
    assert_eq!(resp_get2.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn prover_reports_zk1_tags_present_for_norito() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();
    // Build a minimal router wiring prover endpoints

    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            get(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_get_report(id).await
            }),
        );

    // Create a ZK1 attachment with minimal envelope and one TLV: PROF (0 bytes)
    iroha_torii::zk_attachments::init_persistence();
    let mut body = b"ZK1\0".to_vec();
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&0u32.to_le_bytes());
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Trigger scan once
    let created = iroha_torii::zk_prover::scan_once();
    assert!(created >= 1);

    // List reports and assert zk1_tags present with ["PROF"]
    let req_list = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), http::StatusCode::OK);
    let list_bytes = resp_list.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&list_bytes).unwrap();
    let rec = arr
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(&id))
        .cloned()
        .expect("report exists");
    assert_eq!(
        rec.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let err = rec.get("error").and_then(|x| x.as_str()).unwrap_or("");
    assert!(err.contains("unsupported ZK1"));
    let tags = rec.get("zk1_tags").and_then(|v| v.as_array()).cloned();
    assert!(tags.is_some(), "zk1_tags must be present for ZK1 envelope");
    assert_eq!(tags.unwrap(), vec![norito::json::Value::from("PROF")]);

    // Get specific report and assert zk1_tags again
    let req_get = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), http::StatusCode::OK);
    let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let tags = v.get("zk1_tags").and_then(|x| x.as_array()).cloned();
    assert!(tags.is_some());
    assert_eq!(tags.unwrap(), vec![norito::json::Value::from("PROF")]);
}

#[tokio::test]
async fn prover_reports_zk1_tags_order_prof_ipak() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();
    // Build a minimal router wiring prover endpoints

    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            get(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_get_report(id).await
            }),
        );

    // Create a ZK1 attachment with envelope containing two TLVs: PROF (0), IPAK (4 -> k=5)
    iroha_torii::zk_attachments::init_persistence();
    let mut body = b"ZK1\0".to_vec();
    // PROF TLV
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&0u32.to_le_bytes());
    // IPAK TLV
    body.extend_from_slice(b"IPAK");
    body.extend_from_slice(&4u32.to_le_bytes());
    body.extend_from_slice(&5u32.to_le_bytes());
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Trigger scan once
    let created = iroha_torii::zk_prover::scan_once();
    assert!(created >= 1);

    // List and check exact order
    let req_list = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), http::StatusCode::OK);
    let list_bytes = resp_list.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&list_bytes).unwrap();
    let rec = arr
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(&id))
        .cloned()
        .expect("report exists");
    assert_eq!(
        rec.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let err = rec.get("error").and_then(|x| x.as_str()).unwrap_or("");
    assert!(err.contains("unsupported ZK1"));
    let tags = rec.get("zk1_tags").and_then(|v| v.as_array()).cloned();
    assert_eq!(
        tags.unwrap(),
        vec![
            norito::json::Value::from("PROF"),
            norito::json::Value::from("IPAK")
        ]
    );

    // Get and check order again
    let req_get = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), http::StatusCode::OK);
    let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let tags = v.get("zk1_tags").and_then(|x| x.as_array()).cloned();
    assert_eq!(
        tags.unwrap(),
        vec![
            norito::json::Value::from("PROF"),
            norito::json::Value::from("IPAK")
        ]
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
#[allow(clippy::items_after_statements)]
async fn prover_reports_error_for_truncated_tlv() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();
    // Build a minimal router wiring prover endpoints

    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            get(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_get_report(id).await
            }),
        );

    // Create a ZK1 attachment with truncated TLV payload
    iroha_torii::zk_attachments::init_persistence();
    let mut body = b"ZK1\0".to_vec();
    // TLV header present: PROF + len=10, but payload missing
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&10u32.to_le_bytes());
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Scan once
    let created = iroha_torii::zk_prover::scan_once();
    assert!(created >= 1);

    // List and assert error present and ok=false
    let req_list = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), http::StatusCode::OK);
    let list_bytes = resp_list.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&list_bytes).unwrap();
    let rec = arr
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(&id))
        .cloned()
        .expect("report exists");
    assert_eq!(
        rec.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let err = rec.get("error").and_then(|v| v.as_str()).unwrap_or("");
    assert!(err.contains("truncated TLV"));
    assert!(rec.get("zk1_tags").is_none(), "tags not present on error");

    // Get and assert error again
    let req_get = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), http::StatusCode::OK);
    let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let err2 = v.get("error").and_then(|x| x.as_str()).unwrap_or("");
    assert!(err2.contains("truncated TLV"));

    // Query errors_only filter should include this report
    let req_errs = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports?errors_only=true")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_errs = app.clone().oneshot(req_errs).await.unwrap();
    assert_eq!(resp_errs.status(), http::StatusCode::OK);
    let bytes = resp_errs.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert!(
        arr.iter()
            .any(|m| m.get("id").and_then(|x| x.as_str()) == Some(&id))
    );

    // messages_only projection returns array of { id, error }
    let req_msgs = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports?messages_only=true")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_msgs = app.clone().oneshot(req_msgs).await.unwrap();
    assert_eq!(resp_msgs.status(), http::StatusCode::OK);
    let bytes = resp_msgs.into_body().collect().await.unwrap().to_bytes();
    let msgs: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert!(
        msgs.iter()
            .any(|m| m.get("id").and_then(|x| x.as_str()) == Some(&id))
    );
    let errv = msgs
        .iter()
        .find(|m| m.get("id").and_then(|x| x.as_str()) == Some(&id))
        .and_then(|m| m.get("error"))
        .and_then(|x| x.as_str())
        .unwrap_or("");
    assert!(errv.contains("truncated TLV"));
}

#[tokio::test]
async fn prover_reports_server_side_filters() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();

    let app = Router::new().route(
        "/v1/zk/prover/reports",
        get(
            |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                iroha_torii::zk_prover::handle_list_reports(q).await
            },
        ),
    );

    // Create two attachments: proof (ok) and ZK1 (unsupported but tagged)
    iroha_torii::zk_attachments::init_persistence();
    let headers_norito = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-norito"),
        );
        h
    };
    let headers_zk1 = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let proof_body = fixture_attachment_bytes();
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_norito.clone(),
        axum::body::Bytes::from(proof_body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    // ZK1 with PROF
    let mut body = b"ZK1\0".to_vec();
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&0u32.to_le_bytes());
    let resp2 = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_zk1,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp2.status(), axum::http::StatusCode::CREATED);

    // Scan
    let _ = iroha_torii::zk_prover::scan_once();

    // Filter: content_type application/x-zk1 and has_tag=PROF
    let req = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports?content_type=application/x-zk1&has_tag=PROF")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1, "should return only the ZK1 report");
    let ct = arr[0]
        .get("content_type")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    assert!(ct.contains("application/x-zk1"));

    // Filter ids_only
    let req_ids = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports?content_type=application/x-zk1&ids_only=true")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_ids = app.clone().oneshot(req_ids).await.unwrap();
    assert_eq!(resp_ids.status(), http::StatusCode::OK);
    let bytes = resp_ids.into_body().collect().await.unwrap().to_bytes();
    let ids: Vec<String> = norito::json::from_slice(&bytes).unwrap();
    assert!(!ids.is_empty());
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
#[allow(clippy::items_after_statements)]
async fn prover_reports_server_side_paging_limit_since() {
    use std::time::Duration;
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();

    let app = Router::new().route(
        "/v1/zk/prover/reports",
        get(
            |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                iroha_torii::zk_prover::handle_list_reports(q).await
            },
        ),
    );

    // Create three attachments and process in order with delays to ensure increasing processed_ms
    iroha_torii::zk_attachments::init_persistence();
    let mk_json = |s: &str| axum::body::Bytes::from(format!("{{\"v\":\"{s}\"}}"));
    let headers_json = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        h
    };
    let post = |b: axum::body::Bytes| async {
        iroha_torii::zk_attachments::handle_post_attachment(
            AttachmentTenant::anonymous(),
            headers_json.clone(),
            b,
        )
        .await
        .into_response()
    };
    let r1 = post(mk_json("a")).await;
    assert_eq!(r1.status(), axum::http::StatusCode::CREATED);
    let (_p1, b1) = r1.into_parts();
    let id1: String = {
        let bytes = b1.collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        v.get("id").and_then(|x| x.as_str()).unwrap().to_string()
    };

    let r2 = post(mk_json("b")).await;
    assert_eq!(r2.status(), axum::http::StatusCode::CREATED);
    let (_p2, b2) = r2.into_parts();
    let id2: String = {
        let bytes = b2.collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        v.get("id").and_then(|x| x.as_str()).unwrap().to_string()
    };

    let r3 = post(mk_json("c")).await;
    assert_eq!(r3.status(), axum::http::StatusCode::CREATED);
    let (_p3, b3) = r3.into_parts();
    let id3: String = {
        let bytes = b3.collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        v.get("id").and_then(|x| x.as_str()).unwrap().to_string()
    };

    // Process in order with sleep to ensure processed_ms increases
    let rep1 = iroha_torii::zk_prover::process_attachment_once(&id1).expect("rep1");
    std::thread::sleep(Duration::from_millis(2));
    let rep2 = iroha_torii::zk_prover::process_attachment_once(&id2).expect("rep2");
    std::thread::sleep(Duration::from_millis(2));
    let rep3 = iroha_torii::zk_prover::process_attachment_once(&id3).expect("rep3");

    assert!(rep1.processed_ms <= rep2.processed_ms && rep2.processed_ms <= rep3.processed_ms);

    // Query since_ms just after rep1, limit=1 => should return only rep2
    let uri = format!(
        "/v1/zk/prover/reports?since_ms={}&limit=1",
        rep1.processed_ms + 1
    );
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1);
    let got_id = arr[0].get("id").and_then(|x| x.as_str());
    assert_eq!(got_id, Some(id2.as_str()));

    // Query since_ms after rep2, limit=10 => should return rep3 only
    let uri = format!(
        "/v1/zk/prover/reports?since_ms={}&limit=10",
        rep2.processed_ms + 1
    );
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1);
    let got_id = arr[0].get("id").and_then(|x| x.as_str());
    assert_eq!(got_id, Some(id3.as_str()));

    // Order desc, limit=1 should return latest (rep3)
    let uri = "/v1/zk/prover/reports?order=desc&limit=1";
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1);
    let got_id = arr[0].get("id").and_then(|x| x.as_str());
    assert_eq!(got_id, Some(id3.as_str()));

    // latest=true should also return rep3
    let uri = "/v1/zk/prover/reports?latest=true";
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1);
    let got_id = arr[0].get("id").and_then(|x| x.as_str());
    assert_eq!(got_id, Some(id3.as_str()));

    // Query before_ms just before rep2 => should return rep1 only
    let uri = format!(
        "/v1/zk/prover/reports?before_ms={}",
        rep2.processed_ms.saturating_sub(1)
    );
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(arr.len(), 1);
    let got_id = arr[0].get("id").and_then(|x| x.as_str());
    assert_eq!(got_id, Some(id1.as_str()));
}

#[tokio::test]
async fn prover_reports_server_side_count_matches_filtered_list() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();

    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/count",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_count_reports(q).await
                },
            ),
        );

    // Create attachments: one JSON and two ZK1 (PROF and IPAK)
    iroha_torii::zk_attachments::init_persistence();
    // JSON
    let headers_json = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        h
    };
    let _ = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_json,
        axum::body::Bytes::from_static(br#"{"a":1}"#),
    )
    .await
    .into_response();

    // ZK1 with PROF
    let headers_zk1 = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let mut body = b"ZK1\0".to_vec();
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&0u32.to_le_bytes());
    let _ = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_zk1.clone(),
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    // ZK1 with IPAK
    let mut body2 = b"ZK1\0".to_vec();
    body2.extend_from_slice(b"IPAK");
    body2.extend_from_slice(&4u32.to_le_bytes());
    body2.extend_from_slice(&5u32.to_le_bytes());
    let _ = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_zk1,
        axum::body::Bytes::from(body2),
    )
    .await
    .into_response();

    // Scan
    let _ = iroha_torii::zk_prover::scan_once();

    // List with has_tag=PROF and count with same filter should match length
    let uri_list = "/v1/zk/prover/reports?content_type=application/x-zk1&has_tag=PROF";
    let req_list = http::Request::builder()
        .method("GET")
        .uri(uri_list)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), http::StatusCode::OK);
    let bytes = resp_list.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    let expected_len = arr.len() as u64;

    let uri_count = "/v1/zk/prover/reports/count?content_type=application/x-zk1&has_tag=PROF";
    let req_count = http::Request::builder()
        .method("GET")
        .uri(uri_count)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_count = app.clone().oneshot(req_count).await.unwrap();
    assert_eq!(resp_count.status(), http::StatusCode::OK);
    let bytes = resp_count.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let got = v
        .get("count")
        .and_then(norito::json::Value::as_u64)
        .unwrap();
    assert_eq!(got, expected_len);
}

#[tokio::test]
async fn prover_reports_server_side_bulk_delete() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();

    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports",
            delete(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_delete_reports(q).await
                },
            ),
        );

    // Create attachments: one JSON and one ZK1
    iroha_torii::zk_attachments::init_persistence();
    // JSON
    let headers_json = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        h
    };
    let _ = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_json,
        axum::body::Bytes::from_static(br#"{"a":1}"#),
    )
    .await
    .into_response();
    // ZK1
    let headers_zk1 = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let mut body = b"ZK1\0".to_vec();
    body.extend_from_slice(b"PROF");
    body.extend_from_slice(&0u32.to_le_bytes());
    let _ = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers_zk1,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();

    // Scan
    let _ = iroha_torii::zk_prover::scan_once();

    // Delete JSON reports only
    let req_del = http::Request::builder()
        .method("DELETE")
        .uri("/v1/zk/prover/reports?content_type=application/json")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_del = app.clone().oneshot(req_del).await.unwrap();
    assert_eq!(resp_del.status(), http::StatusCode::OK);

    // Verify only Norito report(s) remain
    let req_all = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/prover/reports")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_all = app.clone().oneshot(req_all).await.unwrap();
    assert_eq!(resp_all.status(), http::StatusCode::OK);
    let bytes = resp_all.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<norito::json::Value> = norito::json::from_slice(&bytes).unwrap();
    assert!(arr.iter().all(
        |v| v.get("content_type").and_then(|x| x.as_str()).unwrap_or("") != "application/json"
    ));
}

#[tokio::test]
async fn prover_reports_error_for_oversized_tlv() {
    // Use a temp dir for persistence
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();
    // Build a minimal router wiring prover endpoints
    let app = Router::new()
        .route(
            "/v1/zk/prover/reports",
            get(
                |q: iroha_torii::NoritoQuery<iroha_torii::zk_prover::ProverListQuery>| async move {
                    iroha_torii::zk_prover::handle_list_reports(q).await
                },
            ),
        )
        .route(
            "/v1/zk/prover/reports/{id}",
            get(|id: axum::extract::Path<String>| async move {
                iroha_torii::zk_prover::handle_get_report(id).await
            }),
        );

    // Create a ZK1 attachment with oversized TLV payload length (> 8 MiB)
    iroha_torii::zk_attachments::init_persistence();
    let mut body = b"ZK1\0".to_vec();
    body.extend_from_slice(b"PROF");
    let oversized: u32 = (8 * 1024 * 1024 + 1) as u32; // 8 MiB + 1
    body.extend_from_slice(&oversized.to_le_bytes());
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Scan once
    let created = iroha_torii::zk_prover::scan_once();
    assert!(created >= 1);

    // Fetch report and assert error about payload too large
    let req_get = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/zk/prover/reports/{id}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), http::StatusCode::OK);
    let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(false)
    );
    let err = v
        .get("error")
        .and_then(norito::json::Value::as_str)
        .unwrap_or("");
    assert!(err.contains("too large"), "unexpected error: {err}");
}

#[tokio::test]
async fn prover_reports_ttl_gc_deletes_old_reports() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    // Use a temp dir for persistence
    let data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    ensure_quota_config();

    // Create a simple JSON attachment and produce a report
    iroha_torii::zk_attachments::init_persistence();
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        h
    };
    let resp = iroha_torii::zk_attachments::handle_post_attachment(
        AttachmentTenant::anonymous(),
        headers,
        axum::body::Bytes::from_static(br#"{"a":1}"#),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let (_parts, body) = resp.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let meta: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let id = meta.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // Generate report
    let _ = iroha_torii::zk_prover::scan_once();

    // Overwrite report's processed_ms to an old timestamp (now - 10 seconds)
    let base = iroha_torii::zk_prover::handle_get_report(axum::extract::Path(id.clone()))
        .await
        .into_response();
    assert_eq!(
        base.status(),
        axum::http::StatusCode::OK,
        "report must exist"
    );
    let report_path = data_dir
        .path()
        .join("zk_prover")
        .join("reports")
        .join(format!("{id}.json"));
    // Load, edit processed_ms, and write back
    let mut rep: norito::json::Value =
        norito::json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
    let now_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    // Make it older than default TTL (7 days)
    let old = now_ms.saturating_sub(8 * 24 * 60 * 60 * 1000);
    if let Some(obj) = rep.as_object_mut() {
        obj.insert("processed_ms".to_string(), norito::json::Value::from(old));
    }
    std::fs::write(&report_path, norito::json::to_vec_pretty(&rep).unwrap()).unwrap();

    // Run GC once with default TTL; should delete the old report
    let deleted = iroha_torii::zk_prover::gc_reports_once();
    assert!(deleted >= 1, "expected GC to delete at least one report");

    // Ensure fetching the report now fails (404)
    let get = iroha_torii::zk_prover::handle_get_report(axum::extract::Path(id.clone())).await;
    assert_eq!(
        get.into_response().status(),
        axum::http::StatusCode::NOT_FOUND
    );
}
