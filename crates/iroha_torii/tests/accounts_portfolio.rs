//! Torii UAID portfolio endpoint tests.
#![cfg(feature = "app_api")]
use axum::extract::connect_info::ConnectInfo;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::state::World;
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
use iroha_data_model::{
    account::NewAccount,
    asset::{AssetDefinition, AssetId},
    isi::{Mint, Register},
    nexus::UniversalAccountId,
    prelude::*,
};
use iroha_model_base::domain::DomainId;
use norito::json::{self, Value};
use tower::ServiceExt as _;
#[path = "fixtures.rs"]
mod fixtures;
const ACCOUNT_SIGNATORY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
#[tokio::test]
async fn accounts_portfolio_endpoint_returns_snapshot() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let (_torii, app, uaid) = setup_portfolio_app(seed_portfolio_accounts());
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let mut req = fixtures::get_request(&(format!("/v1/accounts/uaid:{uaid_hex}/portfolio")));
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    let resp = app.router().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&body)
    );
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    assert_eq!(value["totals"]["accounts"], Value::from(1));
    assert_eq!(value["totals"]["positions"], Value::from(2));
    let mut observed_quantities: Vec<String> = value["dataspaces"][0]["accounts"]
        .as_array()
        .expect("accounts array")
        .iter()
        .filter_map(|account| account["assets"].as_array())
        .flat_map(|assets| assets.iter())
        .filter_map(|asset| asset["quantity"].as_str())
        .map(ToOwned::to_owned)
        .collect();
    observed_quantities.sort();
    assert_eq!(
        observed_quantities,
        vec!["250".to_string(), "500".to_string()]
    );
    app.shutdown().await;
}
#[tokio::test]
async fn accounts_portfolio_snapshot_matches_fixture() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let (_torii, app, uaid) = setup_portfolio_app(seed_fixture_portfolio_accounts());
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let mut req = fixtures::get_request(&(format!("/v1/accounts/uaid:{uaid_hex}/portfolio")));
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    let resp = app.router().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&body)
    );
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    let fixture: Value = json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/nexus/uaid_portfolio/global_default_portfolio.json"
    )))
    .expect("portfolio fixture JSON");
    assert_eq!(value, fixture);
    app.shutdown().await;
}
#[tokio::test]
async fn accounts_portfolio_filters_by_asset_id() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let (_torii, app, uaid) = setup_portfolio_app(seed_portfolio_accounts());
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let mut baseline_req =
        fixtures::get_request(&(format!("/v1/accounts/uaid:{uaid_hex}/portfolio")));
    baseline_req
        .extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    let baseline = fixtures::request(&app, baseline_req).await.unwrap();
    let baseline_status = baseline.status();
    let baseline_body = baseline.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        baseline_status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&baseline_body)
    );
    let baseline_value: Value = json::from_slice(&baseline_body).expect("valid portfolio payload");
    let asset_id = baseline_value["dataspaces"][0]["accounts"][0]["assets"][0]["asset_id"]
        .as_str()
        .expect("baseline asset id")
        .to_owned();
    let mut req = fixtures::get_request(
        &(format!(
            "/v1/accounts/uaid:{uaid_hex}/portfolio?asset_id={}",
            urlencoding::encode(&asset_id)
        )),
    );
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    let resp = app.router().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&body)
    );
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    assert_eq!(value["totals"]["accounts"], Value::from(1));
    assert_eq!(value["totals"]["positions"], Value::from(1));
    let assets = value["dataspaces"][0]["accounts"][0]["assets"]
        .as_array()
        .expect("assets array");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["asset_id"], Value::from(asset_id));
    app.shutdown().await;
}
/// Serve the portfolio that an original signed genesis authors, as on a running ledger.
fn setup_portfolio_app(
    (uaid, instructions): (UniversalAccountId, Vec<InstructionBox>),
) -> (
    fixtures::StandardToriiHarness,
    iroha_torii::TestApiRouterRuntime,
    UniversalAccountId,
) {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let torii = fixtures::StandardToriiHarness::with_genesis_instructions(
        &cfg,
        World::default(),
        instructions,
    );
    let app = torii.router();
    (torii, app, uaid)
}
fn seed_portfolio_accounts() -> (UniversalAccountId, Vec<InstructionBox>) {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii_portfolio"));
    let domain_id: DomainId = DomainId::try_new("portfolio", "universal").unwrap();
    let cash_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "cash".parse().unwrap());
    let points_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "points".parse().unwrap());
    let first_account = account_id_from_signatory(domain_id.clone(), ACCOUNT_SIGNATORY);
    let register: Vec<InstructionBox> = vec![
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(first_account.clone()).with_uaid(Some(uaid))).into(),
        Register::asset_definition(AssetDefinition::numeric(
            cash_id.clone(),
            "cash".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            points_id.clone(),
            "points".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Mint::asset_quantity(500u64, AssetId::new(cash_id, first_account.clone())).into(),
        Mint::asset_quantity(250u64, AssetId::new(points_id, first_account.clone())).into(),
    ];
    (uaid, register)
}
fn seed_fixture_portfolio_accounts() -> (UniversalAccountId, Vec<InstructionBox>) {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii_fixture"));
    let domain_id: DomainId = DomainId::try_new("portfolio_fixture", "universal").unwrap();
    let cash_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "cash".parse().unwrap());
    let points_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "points".parse().unwrap());
    let first_account = account_id_from_seed(&domain_id, 0x11);
    let register: Vec<InstructionBox> = vec![
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new(first_account.clone()).with_uaid(Some(uaid))).into(),
        Register::asset_definition(AssetDefinition::numeric(
            cash_id.clone(),
            "cash".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            points_id.clone(),
            "points".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Mint::asset_quantity(875u64, AssetId::new(cash_id, first_account.clone())).into(),
        Mint::asset_quantity(320u64, AssetId::new(points_id, first_account.clone())).into(),
    ];
    (uaid, register)
}
fn account_id_from_signatory(_domain_id: DomainId, signatory_hex: &str) -> AccountId {
    let normalized = signatory_hex
        .strip_prefix("ed0120")
        .unwrap_or(signatory_hex);
    let public_key = PublicKey::from_hex(Algorithm::Ed25519, normalized)
        .expect("signatory literal parses into a public key");
    AccountId::new(public_key)
}
fn account_id_from_seed(_domain_id: &DomainId, seed_byte: u8) -> AccountId {
    let seed = vec![seed_byte; 32];
    let (public, _) = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
        .expect("derive portfolio account key")
        .into_parts();
    AccountId::new(public)
}
