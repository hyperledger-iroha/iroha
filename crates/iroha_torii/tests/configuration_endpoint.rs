#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level regression tests for `/v2/configuration`.

#[path = "fixtures.rs"]
mod fixtures;
#[path = "common/norito_rpc_harness.rs"]
mod norito_rpc_harness;

use axum::http::Request;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_config::{
    client_api::ConfigGetDTO,
    parameters::actual::{ConsensusMode, NoritoRpcStage, StreamingSoranetAccessKind},
};
use norito_rpc_harness::NoritoRpcHarness;
use tower::ServiceExt as _;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn configuration_endpoint_includes_transport_summary() {
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.enabled = true;
        cfg.torii.transport.norito_rpc.require_mtls = true;
        cfg.torii.transport.norito_rpc.allowed_clients =
            vec!["alpha-canary".into(), "beta-canary".into()];
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Ga;
    });
    let cfg = &harness.cfg;
    let expected_stage = harness
        .cfg
        .torii
        .transport
        .norito_rpc
        .stage
        .label()
        .to_string();
    let expected_allowlist = cfg.torii.transport.norito_rpc.allowed_clients.len();
    let expected_access_kind = cfg.streaming.soranet.access_kind.as_str();

    let req = fixtures::operator_signed_request(
        &harness.cfg.common.key_pair,
        Request::builder()
            .uri(iroha_torii_shared::uri::CONFIGURATION)
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    );
    let response = harness.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let dto: ConfigGetDTO = norito::json::from_slice(&body).expect("valid configuration payload");
    let summary = dto.transport.norito_rpc;
    assert!(summary.enabled, "expected Norito-RPC flag to propagate");
    assert!(summary.require_mtls, "require_mtls flag missing");
    assert_eq!(
        summary.stage, expected_stage,
        "stage label differs from config"
    );
    assert_eq!(
        summary.canary_allowlist_size, expected_allowlist,
        "allowlist size must match config"
    );

    let streaming = &dto.transport.streaming.soranet;
    assert_eq!(
        streaming.enabled, cfg.streaming.soranet.enabled,
        "SoraNet streaming enable flag should propagate"
    );
    assert_eq!(
        streaming.stream_tag, "norito-stream",
        "stream tag should match the Norito SoraNet label"
    );
    assert_eq!(
        streaming.exit_multiaddr, cfg.streaming.soranet.exit_multiaddr,
        "exit multiaddr must mirror streaming.soranet config"
    );
    assert_eq!(
        streaming.padding_budget_ms, cfg.streaming.soranet.padding_budget_ms,
        "padding budget should round-trip"
    );
    assert_eq!(
        streaming.access_kind, expected_access_kind,
        "access kind label must match config"
    );
    let expected_gar = match cfg.streaming.soranet.access_kind {
        StreamingSoranetAccessKind::Authenticated => "stream.norito.authenticated",
        StreamingSoranetAccessKind::ReadOnly => "stream.norito.read_only",
    };
    assert_eq!(
        streaming.gar_category, expected_gar,
        "GAR category should map to the configured access kind"
    );
    assert_eq!(
        streaming.channel_salt, cfg.streaming.soranet.channel_salt,
        "channel salt should propagate"
    );
    assert_eq!(
        streaming.provision_spool_dir,
        cfg.streaming
            .soranet
            .provision_spool_dir
            .to_string_lossy()
            .as_ref(),
        "spool directory must be exposed to clients"
    );
    assert_eq!(
        streaming.provision_window_segments, cfg.streaming.soranet.provision_window_segments,
        "provision window segments should be exposed to clients"
    );
    assert_eq!(
        streaming.provision_queue_capacity, cfg.streaming.soranet.provision_queue_capacity,
        "provision queue capacity should be exposed to clients"
    );
    let expected_mode = match cfg.sumeragi.consensus_mode {
        ConsensusMode::Permissioned => "permissioned",
        ConsensusMode::Npos => "npos",
    };
    assert_eq!(
        dto.consensus.mode, expected_mode,
        "consensus mode should be surfaced"
    );
    assert_eq!(
        dto.consensus.mode_flip_enabled, cfg.sumeragi.mode_flip.enabled,
        "mode flip kill switch should propagate"
    );
}
