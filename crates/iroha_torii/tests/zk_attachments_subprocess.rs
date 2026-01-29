//! Integration test for the subprocess attachment sanitizer path.
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs)]

use std::path::PathBuf;

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::AttachmentSanitizerMode;
use iroha_torii::MaybeTelemetry;

#[tokio::test]
async fn attachments_sanitize_via_subprocess() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));
    let sanitize_timeout_ms =
        iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS.max(5_000);
    iroha_torii::zk_attachments::configure(
        iroha_config::parameters::defaults::torii::ATTACHMENTS_TTL_SECS,
        iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_BYTES,
        iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
        8 * 1024 * 1024,
        iroha_config::parameters::defaults::torii::attachments_allowed_mime_types(),
        iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
        iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
        AttachmentSanitizerMode::Subprocess,
        sanitize_timeout_ms,
        Some(sanitizer_path),
        MaybeTelemetry::disabled(),
    );
    iroha_torii::zk_attachments::init_persistence();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    let body = axum::body::Bytes::from_static(br#"{"hello":"world"}"#);
    let response = iroha_torii::zk_attachments::handle_post_attachment(headers, body)
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);

    let meta_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let meta: iroha_torii::zk_attachments::AttachmentMeta =
        norito::json::from_slice(&meta_bytes).expect("attachment meta");
    let provenance = meta.provenance.expect("provenance");
    assert!(provenance.sanitizer.sandboxed);
}
