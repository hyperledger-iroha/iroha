//! Smoke tests for Torii asset definitions endpoints.
#![cfg(feature = "app_api")]
use axum::extract::connect_info::ConnectInfo;
use axum::http::Request;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle, query::store::LiveQueryStore, state::World,
    sumeragi::test_chain::CertifiedTestChain,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::Registrable as _;
use iroha_data_model::isi::SetAssetDefinitionAlias;
use iroha_data_model::prelude as dm;
use iroha_executor_data_model::permission::asset_definition::{
    AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::topology::DataSpaceId;
use iroha_torii::Torii;
use std::sync::Arc;
use tower::ServiceExt as _;
#[path = "fixtures.rs"]
mod fixtures;
fn checked_asset_definition_ed25519_key_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked asset-definition Ed25519 fixture keypair")
}
#[test]
fn asset_definition_authority_fixture_uses_checked_ed25519_key_generation() {
    let key_pair = checked_asset_definition_ed25519_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture asset-definition public key has a valid algorithm");
    assert_eq!(algorithm, Algorithm::Ed25519);
}
/// Apply a signed genesis whose authority owns both definitions and binds the `cbdc` alias.
///
/// Only a definition's owner may bind its alias. Genesis needs no namespace grant for that, and
/// its grant lets the same owner take a lease in a later block.
fn seeded_chain(
    cfg: &mut iroha_config::parameters::actual::Root,
) -> (
    CertifiedTestChain,
    dm::AssetDefinitionId,
    dm::AssetDefinitionId,
) {
    cfg.common.key_pair = checked_asset_definition_ed25519_key_fixture();
    let authority = dm::AccountId::new(cfg.common.key_pair.public_key().clone());
    let domain_id: iroha_model_base::domain::DomainId =
        DomainId::try_new("wonderland", "universal").expect("valid domain");
    let domain = dm::Domain::new(domain_id.clone()).build(&authority);
    let account = dm::Account::new(authority.clone()).build(&authority);
    let cbdc_id = dm::AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "cbdc".parse().expect("asset name"),
    );
    let cbdc = dm::AssetDefinition::numeric(
        cbdc_id.clone(),
        "CBDC".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let usd_id = dm::AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "usd".parse().expect("asset name"),
    );
    let usd = dm::AssetDefinition::numeric(
        usd_id.clone(),
        "USD".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let native_chain = fixtures::commit_signed_genesis(
        cfg,
        World::with([domain], [account], [cbdc, usd]),
        vec![
            SetAssetDefinitionAlias::bind(
                cbdc_id.clone(),
                "cbdc#universal".parse().expect("asset alias"),
                None,
            )
            .into(),
            dm::Grant::account_permission(
                CanManageAssetDefinitionAlias {
                    scope: AssetDefinitionAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
                },
                authority,
            )
            .into(),
        ],
    );
    (native_chain, cbdc_id, usd_id)
}
fn build_app(
    cfg: &iroha_config::parameters::actual::Root,
    native_chain: &CertifiedTestChain,
) -> iroha_torii::TestApiRouterRuntime {
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    Torii::new_with_handle(
        cfg.common.chain.clone(),
        native_chain.network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        native_chain.kura().clone(),
        native_chain.state().clone(),
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        iroha_torii::test_utils::with_private_settlement_test_signers(
            iroha_torii::ToriiRuntimeDeps::new(
                build_identity_test_fixture::build_identity(),
                iroha_torii::MaybeTelemetry::disabled(),
            ),
            &iroha_torii::test_utils::private_settlement_test_node_key(),
        ),
    )
    .expect("valid Torii asset-definition fixture")
    .with_local_peer_id(native_chain.validators()[0].0.clone())
    .api_router_for_tests()
    .expect("test Torii router initializes")
}
fn loopback_connect_info() -> ConnectInfo<std::net::SocketAddr> {
    ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}
/// Bind a leased alias in a certified block at `block_time_ms`, signed by the definition owner.
fn commit_alias_lease(
    native_chain: &mut CertifiedTestChain,
    owner: &KeyPair,
    definition_id: &dm::AssetDefinitionId,
    alias: &str,
    lease_expiry_ms: u64,
    block_time_ms: u64,
) {
    let lease = native_chain.sign(
        owner,
        [SetAssetDefinitionAlias::bind(
            definition_id.clone(),
            alias.parse().expect("valid asset alias"),
            Some(lease_expiry_ms),
        )
        .into()],
        block_time_ms,
    );
    assert_eq!(
        native_chain.commit_at(block_time_ms, vec![lease]),
        vec![true],
        "bind leased alias"
    );
}
#[tokio::test]
async fn asset_definitions_endpoints_return_name_and_alias() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (native_chain, cbdc_id, _) = seeded_chain(&mut cfg);
    let runtime = build_app(&cfg, &native_chain);
    let app = runtime.router();
    // GET /v1/assets/definitions
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/assets/definitions?include_total=true")
                .extension(loopback_connect_info())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let items = doc["items"].as_array().expect("items");
    assert_eq!(doc["total"].as_u64(), Some(2));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("CBDC")
            && item["alias"].as_str() == Some("cbdc#universal")
            && item["alias_binding"]["status"].as_str() == Some("permanent")
    }));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("USD")
            && item["alias"].is_null()
            && item["alias_binding"].is_null()
    }));
    // GET /v1/assets/definitions/{asset}
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/assets/definitions/{cbdc_id}"))
                .extension(loopback_connect_info())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let cbdc_literal = cbdc_id.to_string();
    assert_eq!(doc["id"].as_str(), Some(cbdc_literal.as_str()));
    assert_eq!(doc["name"].as_str(), Some("CBDC"));
    assert_eq!(doc["alias"].as_str(), Some("cbdc#universal"));
    assert_eq!(
        doc["alias_binding"]["alias"].as_str(),
        Some("cbdc#universal")
    );
    assert_eq!(doc["alias_binding"]["status"].as_str(), Some("permanent"));
    // POST /v1/assets/definitions/query
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/assets/definitions/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(loopback_connect_info())
                .body(axum::body::Body::from(r#"{"include_total":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let items = doc["items"].as_array().expect("items");
    assert_eq!(doc["total"].as_u64(), Some(2));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("CBDC")
            && item["alias"].as_str() == Some("cbdc#universal")
            && item["alias_binding"]["status"].as_str() == Some("permanent")
    }));
    assert!(items.iter().any(|item| {
        item["name"].as_str() == Some("USD")
            && item["alias"].is_null()
            && item["alias_binding"].is_null()
    }));
    runtime.shutdown().await;
}
#[tokio::test]
async fn asset_definitions_query_supports_alias_binding_sort() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (mut native_chain, _, usd_id) = seeded_chain(&mut cfg);
    let genesis_time_ms = native_chain.genesis().header().creation_time_ms;
    commit_alias_lease(
        &mut native_chain,
        &cfg.common.key_pair,
        &usd_id,
        "usd#universal",
        genesis_time_ms + 5_000,
        genesis_time_ms + 1_000,
    );
    let runtime = build_app(&cfg, &native_chain);
    let app = runtime.router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/assets/definitions/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(loopback_connect_info())
                .body(axum::body::Body::from(
                    r#"{"sort":["-alias_binding.bound_at_ms"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(
        status == StatusCode::OK,
        "unexpected status {status}: {}",
        String::from_utf8_lossy(body.as_ref())
    );
    let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid json");
    let items = doc["items"].as_array().expect("items");
    let usd_literal = usd_id.to_string();
    assert_eq!(
        items.first().and_then(|item| item["id"].as_str()),
        Some(usd_literal.as_str())
    );
    assert_eq!(
        items
            .first()
            .and_then(|item| item["alias_binding"]["status"].as_str()),
        Some("leased_active")
    );
    runtime.shutdown().await;
}

use crate::build_identity_test_fixture;
