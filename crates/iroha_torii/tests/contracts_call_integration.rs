#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for the contract call endpoint.
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs, clippy::too_many_lines)]
use axum::{Router, routing::post};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_crypto::Signature;
use iroha_data_model::{
    DomainId,
    asset::AssetDefinitionId,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use ivm::kotodama::session::{CompileRequest, CompilerSession};
use mv::storage::StorageReadOnly;
use norito::json;
use std::{num::NonZeroU64, sync::Arc, time::Duration};
use tower::ServiceExt as _;
fn can_modify_account_metadata(
    account: &iroha_data_model::account::AccountId,
) -> iroha_data_model::permission::Permission {
    iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
        account: account.clone(),
    }
    .into()
}
fn can_mint_asset_definition(
    asset_definition: &AssetDefinitionId,
) -> iroha_data_model::permission::Permission {
    iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition {
        asset_definition: asset_definition.clone(),
    }
    .into()
}
fn can_burn_asset_definition(
    asset_definition: &AssetDefinitionId,
) -> iroha_data_model::permission::Permission {
    iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition {
        asset_definition: asset_definition.clone(),
    }
    .into()
}
fn contract_call_noop_program() -> Vec<u8> {
    let src = r#"
seiyaku ContractCallNoopTest {

  kotoage fn main() authorize("CanEnactGovernance") {}
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract call no-op test program")
}
fn contract_call_dispatch_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDispatchTest {{

  state int call_amount;
  state AssetDefinitionId call_asset;

  hajimari(AssetDefinitionId asset_definition_id) {{
    call_amount = 0;
    call_asset = asset_definition_id;
  }}

  kotoage fn credit_by_payload(int amount) authorize("CanEnactGovernance") {{
    call_amount = amount;
  }}

  kotoage fn record_asset_by_payload(AssetDefinitionId asset_definition_id) authorize("CanEnactGovernance") {{
    call_asset = asset_definition_id;
  }}

  view fn call_state() -> (int, AssetDefinitionId) {{
    return (call_amount, call_asset);
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call dispatch test program")
}
fn contract_call_declared_state_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDeclaredStateTest {{

  state int CallAmount;
  state AssetDefinitionId CallAsset;

  hajimari(AssetDefinitionId asset_definition_id) {{
    CallAmount = 0;
    CallAsset = asset_definition_id;
  }}

  kotoage fn credit_by_payload(int amount) authorize("CanEnactGovernance") {{
    CallAmount = amount;
  }}

  kotoage fn record_asset_by_payload(AssetDefinitionId asset_definition_id) authorize("CanEnactGovernance") {{
    CallAsset = asset_definition_id;
  }}

  view fn declared_state() -> (int, AssetDefinitionId) {{
    return (CallAmount, CallAsset);
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call declared state test program")
}
fn contract_call_declared_state_with_isi_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDeclaredStateWithIsiTest {{

  state int CallAmount;

  hajimari() {{
    CallAmount = 0;
  }}

  kotoage fn write_with_isi(int amount) authorize("CanEnactGovernance") {{
    ledger::account::set_detail(account: context::authority(), key: Name::parse("cursor"), value: Json::parse("{{\"phase\":\"write_with_isi\"}}"));
    CallAmount = amount;
  }}

  view fn declared_state() -> int {{
    return CallAmount;
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call declared state with isi test program")
}
fn contract_call_declared_state_with_mint_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDeclaredStateWithMintTest {{

  state int CallAmount;

  hajimari() {{
    CallAmount = 0;
  }}

  kotoage fn write_with_mint(int amount,
                           AccountId user,
                           AssetDefinitionId asset_definition_id) authorize("CanEnactGovernance") {{
    ledger::asset::mint(account: user, asset_definition: asset_definition_id, amount: 1);
    CallAmount = amount;
  }}

  view fn declared_state() -> int {{
    return CallAmount;
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call declared state with mint test program")
}
fn contract_call_n3x_like_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallN3xLikeTest {{

  error enum HubError {{
    NotInitialized = 1,
    EmptyHub = 2,
    InvalidAmount = 3,
    InsufficientSupply = 4,
    ZeroRedemption = 5
  }}

  state int HubInitialized;
  state quantity BasketUsdt;
  state quantity BasketUsdc;
  state quantity BasketKusd;
  state quantity TotalN3x;

  fn init_impl() {{
    HubInitialized = 1;
    BasketUsdt = 0;
    BasketUsdc = 0;
    BasketKusd = 0;
    TotalN3x = 0;
  }}

  hajimari() {{
    init_impl();
  }}

  kotoage fn init_hub() authorize("CanEnactGovernance") {{
    init_impl();
  }}

  fn deposit_impl(AccountId user,
                  AssetDefinitionId asset,
                  quantity usdt_in,
                  quantity usdc_in,
                  quantity kusd_in) {{
    require(HubInitialized == 1, HubError::NotInitialized);
    let minted = usdt_in + usdc_in + kusd_in;
    ledger::asset::mint(account: user, asset_definition: asset, amount: minted);
    BasketUsdt = BasketUsdt + usdt_in;
    BasketUsdc = BasketUsdc + usdc_in;
    BasketKusd = BasketKusd + kusd_in;
    TotalN3x = TotalN3x + minted;
  }}

  kotoage fn deposit_like(AccountId user,
                        AssetDefinitionId asset_definition_id,
                        quantity usdt_in,
                        quantity usdc_in,
                        quantity kusd_in) authorize("CanEnactGovernance") {{
    deposit_impl(
      user: user,
      asset: asset_definition_id,
      usdt_in: usdt_in,
      usdc_in: usdc_in,
      kusd_in: kusd_in
    );
  }}

  kotoage fn burn_like(AccountId user,
                     AssetDefinitionId asset_definition_id,
                     quantity n3x_amount) authorize("CanEnactGovernance") {{
    let total = TotalN3x;
    require(total > 0, HubError::EmptyHub);
    require(n3x_amount > 0, HubError::InvalidAmount);
    require(n3x_amount <= total, HubError::InsufficientSupply);
    let decimal redemption_ratio = n3x_amount / total;
    let quantity usdt_out = BasketUsdt * redemption_ratio;
    let quantity usdc_out = BasketUsdc * redemption_ratio;
    let quantity kusd_out = BasketKusd * redemption_ratio;
    let redeemed = usdt_out + usdc_out + kusd_out;
    require(redeemed > 0, HubError::ZeroRedemption);
    ledger::asset::burn(account: user, asset_definition: asset_definition_id, amount: n3x_amount);
    BasketUsdt = BasketUsdt - usdt_out;
    BasketUsdc = BasketUsdc - usdc_out;
    BasketKusd = BasketKusd - kusd_out;
    TotalN3x = total - n3x_amount;
  }}

  view fn state_snapshot() -> (int, quantity, quantity, quantity, quantity) {{
    return (HubInitialized, BasketUsdt, BasketUsdc, BasketKusd, TotalN3x);
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call n3x-like test program")
}
fn contract_view_trap_program_with_source_path(source_path: &str) -> Vec<u8> {
    let src = r#"
seiyaku ContractViewTrapTest {

  error enum ViewError {
    Boom = 1
  }

  view fn explode() -> int {
    require(false, ViewError::Boom);
    return 1;
  }
}
"#;
    CompilerSession::default()
        .build(CompileRequest {
            source: src,
            source_name: Some(source_path),
        })
        .expect("compile contract view trap test program")
        .artifact
}
fn contract_view_bytes_program() -> Vec<u8> {
    let src = r#"
seiyaku ContractViewBytesTest {

  state AssetDefinitionId Asset;
  state bytes Target;

  fn configure_impl(AssetDefinitionId asset_input, bytes target_bytes) {
    Asset = asset_input;
    Target = target_bytes;
  }

  hajimari(AssetDefinitionId asset_input) {
    Asset = asset_input;
    Target = b"";
  }

  kotoage fn configure(AssetDefinitionId asset_input, bytes target_bytes) authorize("CanEnactGovernance") {
    configure_impl(asset_input: asset_input, target_bytes: target_bytes);
  }

  view fn literal() -> bytes {
    return b"risk";
  }

  view fn target() -> bytes {
    return Target;
  }

  view fn config() -> (AssetDefinitionId, bytes) {
    return (Asset, Target);
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract view bytes test program")
}
fn contract_view_account_id_program() -> Vec<u8> {
    let src = r#"
seiyaku ContractViewAccountIdTest {

  state AccountId Stored;

  fn bind_impl(AccountId account_id) {
    Stored = account_id;
  }

  hajimari(AccountId account_id) {
    bind_impl(account_id: account_id);
  }

  kotoage fn bind(AccountId account_id) authorize("CanEnactGovernance") {
    bind_impl(account_id: account_id);
  }

  view fn literal() -> AccountId {
    return context::authority();
  }

  view fn stored() -> AccountId {
    return Stored;
  }

  view fn stored_tuple() -> (AccountId, int) {
    return (Stored, 1);
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract view AccountId test program")
}
fn contract_call_configure_account_map_program() -> Vec<u8> {
    let src = r#"
seiyaku ContractCallConfigureAccountMapTest {

  error enum ConfigureError {
    UnauthorizedExisting = 1,
    UnauthorizedInitial = 2
  }

  state StateMap<Name, AccountId> ConfigAccount;
  state StateMap<Name, int> ConfigInt;

  fn key_admin() -> Name {
    return Name::parse("admin");
  }

  fn key_inori() -> Name {
    return Name::parse("inori");
  }

  fn key_paused() -> Name {
    return Name::parse("paused");
  }

  fn initialize_config_defaults() {
    if (!ConfigInt.contains(key_paused())) {
      ConfigInt[key_paused()] = 0;
    }
  }

  kotoage fn configure(AccountId admin_account, AccountId inori_account) authorize("CanEnactGovernance") {
    let has_admin = ConfigAccount.contains(key_admin());
    if (has_admin) {
      require(context::authority() == ConfigAccount.get(key_admin()).unwrap_or(admin_account), ConfigureError::UnauthorizedExisting);
    } else {
      require(context::authority() == admin_account, ConfigureError::UnauthorizedInitial);
    }
    ConfigAccount[key_admin()] = admin_account;
    ConfigAccount[key_inori()] = inori_account;
    initialize_config_defaults();
  }

  view fn admin() -> AccountId {
    return ConfigAccount.get(key_admin()).unwrap_or(context::authority());
  }

  view fn inori() -> AccountId {
    return ConfigAccount.get(key_inori()).unwrap_or(context::authority());
  }

  view fn paused() -> int {
    return ConfigInt.get(key_paused()).unwrap_or(0);
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract call configure account-map test program")
}
fn contract_test_app(
    state: Arc<State>,
    _kura: Arc<Kura>,
    queue: Arc<Queue>,
    telemetry: iroha_torii::MaybeTelemetry,
) -> Router {
    Router::new()
        .route(
            "/v1/contracts/call",
            post({
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ContractCallDto,
                >| async move {
                    iroha_torii::handle_post_contract_call(
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        iroha_torii::NoritoJson(req),
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/contracts/view",
            post({
                let state = state.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ContractViewDto,
                >| async move {
                    iroha_torii::handle_post_contract_view(
                        state.clone(),
                        iroha_torii::NoritoJson(req),
                    )
                }
            }),
        )
}
fn contract_test_state() -> (
    iroha_torii::test_utils::AuthorityCreds,
    Arc<State>,
    Arc<Kura>,
) {
    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    (creds, state, kura)
}
fn contract_test_queue_and_app(
    state: &Arc<State>,
    kura: &Arc<Kura>,
) -> (Arc<Queue>, iroha_data_model::ChainId, Router) {
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id = "chain".parse().expect("valid test chain ID");
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let app = contract_test_app(
        Arc::clone(state),
        Arc::clone(kura),
        Arc::clone(&queue),
        telemetry,
    );
    (queue, chain_id, app)
}
async fn run_contract_view(
    app: &Router,
    authority: &iroha_data_model::account::AccountId,
    contract_address: &str,
    entrypoint: &str,
    payload: Option<&norito::json::Value>,
) -> json::Value {
    let (status, body) =
        run_contract_view_response(app, authority, contract_address, entrypoint, payload).await;
    assert_eq!(status, http::StatusCode::OK, "{body:?}");
    body
}
async fn run_contract_view_response(
    app: &Router,
    authority: &iroha_data_model::account::AccountId,
    contract_address: &str,
    entrypoint: &str,
    payload: Option<&norito::json::Value>,
) -> (http::StatusCode, json::Value) {
    let body = iroha_torii::test_utils::contract_view_request_json(
        authority,
        contract_address,
        iroha_torii::test_utils::ContractViewOptions {
            entrypoint,
            payload,
            gas_limit: 1_500_000,
        },
    );
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/view")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        json::from_slice(&bytes).expect("decode contract view response"),
    )
}
async fn run_contract_hajimari_and_apply(
    app: &Router,
    state: &Arc<State>,
    queue: &Arc<Queue>,
    chain_id: &iroha_data_model::ChainId,
    creds: &iroha_torii::test_utils::AuthorityCreds,
    contract_address: &str,
    payload: Option<&json::Value>,
    block_height: u64,
) {
    let body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address,
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "hajimari",
            payload,
            gas_limit: 1_500_000,
        },
    );
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let response_body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&response_body)
    );
    let applied =
        iroha_torii::test_utils::apply_queued_in_one_block(state, queue, chain_id, block_height);
    assert_eq!(applied, 1, "hajimari transaction must apply exactly once");
}
#[tokio::test]
async fn contracts_call_enqueues_transaction() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_noop_program();
    let (contract_address, code_hash_hex, abi_hash_hex) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let missing_limit_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("contract_address", contract_address.as_str()),
    ]);
    let missing_limit_body = json::to_json(&missing_limit_payload).expect("serialize call request");
    let missing_limit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(missing_limit_body))
        .unwrap();
    let missing_limit_resp = app.clone().oneshot(missing_limit_req).await.unwrap();
    assert_eq!(missing_limit_resp.status(), http::StatusCode::BAD_REQUEST);
    let zero_limit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "main",
            payload: None,
            gas_limit: 0,
        },
    );
    let zero_limit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(zero_limit_body))
        .unwrap();
    let zero_limit_resp = app.clone().oneshot(zero_limit_req).await.unwrap();
    assert_eq!(zero_limit_resp.status(), http::StatusCode::BAD_REQUEST);
    let transaction_ttl_ms = 900_000_u64;
    let call_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("contract_address", contract_address.as_str()),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("transaction_ttl_ms", transaction_ttl_ms),
        iroha_torii::json_entry(
            "fee_payment",
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000)),
        ),
    ]);
    let call_body = json::to_json(&call_payload).expect("serialize call request");
    let call_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(call_body))
        .unwrap();
    let call_resp = app.clone().oneshot(call_req).await.unwrap();
    assert_eq!(call_resp.status(), http::StatusCode::OK);
    let call_bytes = call_resp.into_body().collect().await.unwrap().to_bytes();
    let call_json: json::Value = json::from_slice(&call_bytes).unwrap();
    assert!(
        call_json
            .get("ok")
            .and_then(json::Value::as_bool)
            .unwrap_or(false)
    );
    assert_eq!(
        call_json
            .get("dataspace")
            .and_then(json::Value::as_str)
            .unwrap(),
        "universal"
    );
    assert_eq!(
        call_json
            .get("contract_address")
            .and_then(json::Value::as_str)
            .unwrap(),
        contract_address
    );
    assert_eq!(
        call_json
            .get("transaction_ttl_ms")
            .and_then(json::Value::as_u64),
        Some(transaction_ttl_ms)
    );
    assert_eq!(
        call_json
            .get("code_hash_hex")
            .and_then(json::Value::as_str)
            .unwrap(),
        code_hash_hex
    );
    assert_eq!(
        call_json
            .get("abi_hash_hex")
            .and_then(json::Value::as_str)
            .unwrap(),
        abi_hash_hex
    );
    let tx_hash_hex = call_json
        .get("tx_hash_hex")
        .and_then(json::Value::as_str)
        .expect("tx_hash_hex present");
    assert_eq!(tx_hash_hex.len(), 64);
    assert_eq!(
        call_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(false),
        true
    );
    let call_receipt = call_json
        .get("operation_receipt")
        .and_then(json::Value::as_object)
        .expect("operation_receipt present");
    assert_eq!(
        call_receipt
            .get("operation_kind")
            .and_then(json::Value::as_str),
        Some("contract_call")
    );
    assert_eq!(
        call_receipt.get("status").and_then(json::Value::as_str),
        Some("submitted")
    );
    assert_eq!(
        call_receipt.get("transport").and_then(json::Value::as_str),
        Some("torii")
    );
    assert_eq!(
        call_receipt.get("dataspace").and_then(json::Value::as_str),
        Some("universal")
    );
    assert_eq!(
        call_receipt
            .get("contract_address")
            .and_then(json::Value::as_str),
        Some(contract_address.as_str())
    );
    assert_eq!(
        call_receipt
            .get("tx_hash_hex")
            .and_then(json::Value::as_str),
        Some(tx_hash_hex)
    );
    assert_eq!(
        call_receipt
            .get("payload_digest_hex")
            .and_then(json::Value::as_str)
            .map(str::len),
        Some(64)
    );
    assert!(!call_receipt.contains_key("private_key"));
    assert!(!call_receipt.contains_key("payload"));
    let applied_call =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_call, 1);
    let draft_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("contract_address", contract_address.as_str()),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("transaction_ttl_ms", transaction_ttl_ms),
        iroha_torii::json_entry(
            "fee_payment",
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000)),
        ),
    ]);
    let draft_body = json::to_json(&draft_body).expect("serialize draft call request");
    let draft_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(draft_body))
        .unwrap();
    let draft_resp = app.clone().oneshot(draft_req).await.unwrap();
    assert_eq!(draft_resp.status(), http::StatusCode::OK);
    let draft_bytes = draft_resp.into_body().collect().await.unwrap().to_bytes();
    let draft_json: json::Value = json::from_slice(&draft_bytes).unwrap();
    assert_eq!(
        draft_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(true),
        false
    );
    assert!(
        draft_json
            .get("transaction_payload_b64")
            .and_then(json::Value::as_str)
            .is_some(),
        "expected canonical unsigned contract-call payload when signing material is omitted"
    );
    assert!(draft_json.get("transaction_scaffold_b64").is_none());
    assert!(draft_json.get("signed_transaction_b64").is_none());
    assert!(
        draft_json.get("entrypoint_hash_hex").is_none()
            || draft_json
                .get("entrypoint_hash_hex")
                .is_some_and(json::Value::is_null)
    );
    assert_eq!(
        draft_json
            .get("transaction_ttl_ms")
            .and_then(json::Value::as_u64),
        Some(transaction_ttl_ms)
    );
    let draft_receipt = draft_json
        .get("operation_receipt")
        .and_then(json::Value::as_object)
        .expect("draft operation_receipt present");
    assert_eq!(
        draft_receipt
            .get("operation_kind")
            .and_then(json::Value::as_str),
        Some("contract_call")
    );
    assert_eq!(
        draft_receipt.get("status").and_then(json::Value::as_str),
        Some("pending_signature")
    );
    assert_eq!(
        draft_receipt.get("transport").and_then(json::Value::as_str),
        Some("torii")
    );
    assert!(!draft_receipt.contains_key("private_key"));
    assert!(!draft_receipt.contains_key("payload"));
    let transaction_payload_b64 = draft_json
        .get("transaction_payload_b64")
        .and_then(json::Value::as_str)
        .expect("transaction_payload_b64 present");
    let transaction_payload_bytes = base64::engine::general_purpose::STANDARD
        .decode(transaction_payload_b64)
        .expect("decode transaction payload");
    assert_eq!(
        base64::engine::general_purpose::STANDARD.encode(&transaction_payload_bytes),
        transaction_payload_b64,
        "draft payload must use canonical padded base64"
    );
    let transaction_builder = TransactionBuilder::decode_payload(&transaction_payload_bytes)
        .expect("strictly decode exact canonical transaction payload");
    assert_eq!(
        transaction_builder.payload().time_to_live(),
        Some(Duration::from_millis(transaction_ttl_ms))
    );
    assert_eq!(
        transaction_builder.encode_payload(),
        transaction_payload_bytes,
        "draft payload must round-trip byte-for-byte"
    );
    let signing_message_b64 = draft_json
        .get("signing_message_b64")
        .and_then(json::Value::as_str)
        .expect("signing_message_b64 present");
    let creation_time_ms = draft_json
        .get("creation_time_ms")
        .and_then(json::Value::as_u64)
        .expect("creation_time_ms present");
    let signing_message = base64::engine::general_purpose::STANDARD
        .decode(signing_message_b64)
        .expect("decode signing message");
    assert_eq!(
        base64::engine::general_purpose::STANDARD.encode(&signing_message),
        signing_message_b64,
        "signing message must use canonical padded base64"
    );
    assert_eq!(
        signing_message,
        transaction_builder.payload_hash_bytes(),
        "signing message must be the exact unsigned payload hash"
    );
    let detached_signature =
        Signature::try_new(&creds.private_key.0, &signing_message).expect("sign detached call");
    let detached_submit_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry(
            "public_key_hex",
            hex::encode_upper(
                iroha_crypto::PublicKey::from(creds.private_key.0.clone())
                    .to_bytes()
                    .1,
            ),
        ),
        iroha_torii::json_entry(
            "signature_b64",
            base64::engine::general_purpose::STANDARD.encode(detached_signature.payload()),
        ),
        iroha_torii::json_entry("contract_address", contract_address.as_str()),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("creation_time_ms", creation_time_ms),
        iroha_torii::json_entry("transaction_ttl_ms", transaction_ttl_ms),
        iroha_torii::json_entry(
            "fee_payment",
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(5_000)),
        ),
    ]);
    let detached_submit_body =
        json::to_json(&detached_submit_body).expect("serialize detached submit request");
    let detached_submit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(detached_submit_body))
        .unwrap();
    let detached_submit_resp = app.clone().oneshot(detached_submit_req).await.unwrap();
    assert_eq!(detached_submit_resp.status(), http::StatusCode::OK);
    let detached_submit_bytes = detached_submit_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let detached_submit_json: json::Value = json::from_slice(&detached_submit_bytes).unwrap();
    assert_eq!(
        detached_submit_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(false),
        true
    );
    let detached_submit_hash = detached_submit_json
        .get("tx_hash_hex")
        .and_then(json::Value::as_str)
        .expect("detached submit tx hash present");
    assert_eq!(detached_submit_hash.len(), 64);
    for field in ["transaction_payload_b64", "signing_message_b64"] {
        assert!(
            detached_submit_json.get(field).is_none()
                || detached_submit_json
                    .get(field)
                    .is_some_and(json::Value::is_null),
            "submitted response must not contain unsigned draft field {field}"
        );
    }
    let detached_receipt = detached_submit_json
        .get("operation_receipt")
        .and_then(json::Value::as_object)
        .expect("detached operation_receipt present");
    assert_eq!(
        detached_receipt
            .get("operation_kind")
            .and_then(json::Value::as_str),
        Some("contract_call")
    );
    assert_eq!(
        detached_receipt.get("status").and_then(json::Value::as_str),
        Some("submitted")
    );
    assert_eq!(
        detached_receipt
            .get("tx_hash_hex")
            .and_then(json::Value::as_str),
        Some(detached_submit_hash)
    );
    assert!(!detached_receipt.contains_key("private_key"));
    assert!(!detached_receipt.contains_key("payload"));
    assert_eq!(
        detached_submit_json
            .get("transaction_ttl_ms")
            .and_then(json::Value::as_u64),
        Some(transaction_ttl_ms)
    );
    assert!(
        draft_json.get("tx_hash_hex").is_none()
            || draft_json
                .get("tx_hash_hex")
                .is_some_and(json::Value::is_null)
    );
    let applied_detached_submit =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_detached_submit, 1);
}
#[tokio::test]
async fn contracts_view_omits_unverified_source_path_from_vm_diagnostic() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let source_path = "contracts/view_trap_test.ko";
    let program = contract_view_trap_program_with_source_path(source_path);
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let body = iroha_torii::test_utils::contract_view_request_json(
        &creds.account,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractViewOptions {
            entrypoint: "explode",
            payload: None,
            gas_limit: 10_000,
        },
    );
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/view")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value: json::Value = json::from_slice(&bytes).expect("decode contract view error");
    assert_eq!(value.get("ok").and_then(json::Value::as_bool), Some(false));
    assert_eq!(
        value.get("entrypoint").and_then(json::Value::as_str),
        Some("explode")
    );
    assert!(
        value
            .get("vm_diagnostic")
            .and_then(json::Value::as_object)
            .and_then(|diag| diag.get("source_path"))
            .is_some_and(json::Value::is_null),
        "deployable artifacts exclude compiler source maps; a verified hash-keyed sidecar is required"
    );
}
#[tokio::test]
async fn contracts_view_decodes_literal_and_persisted_bytes_returns() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_view_bytes_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let asset_definition_id = "6qLb5RYJbzychndCXgFa9aZzjWyx"
        .parse::<AssetDefinitionId>()
        .expect("asset definition id");
    let init_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset_input", asset_definition_id.to_string()),
        iroha_torii::json_entry(
            "target_bytes",
            "0x7269736b5f7661756c743a3a7269736b2e756e6976657273616c",
        ),
    ]);
    let hajimari_payload = iroha_torii::json_object(vec![iroha_torii::json_entry(
        "asset_input",
        asset_definition_id.to_string(),
    )]);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        Some(&hajimari_payload),
        2,
    )
    .await;
    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "configure",
            payload: Some(&init_payload),
            gas_limit: 1_500_000,
        },
    );
    let init_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(init_body))
        .unwrap();
    let init_resp = app.clone().oneshot(init_req).await.unwrap();
    assert_eq!(init_resp.status(), http::StatusCode::OK);
    let applied_init =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_init, 1);
    let literal = run_contract_view(&app, &creds.account, &contract_address, "literal", None).await;
    assert_eq!(
        literal.get("result").and_then(json::Value::as_str),
        Some("0x7269736b")
    );
    let target = run_contract_view(&app, &creds.account, &contract_address, "target", None).await;
    assert_eq!(
        target.get("result").and_then(json::Value::as_str),
        Some("0x7269736b5f7661756c743a3a7269736b2e756e6976657273616c")
    );
    let config = run_contract_view(&app, &creds.account, &contract_address, "config", None).await;
    assert_eq!(
        config.get("result"),
        Some(&json::Value::Array(vec![
            json::Value::String(asset_definition_id.to_string()),
            json::Value::String(
                "0x7269736b5f7661756c743a3a7269736b2e756e6976657273616c".to_owned()
            ),
        ]))
    );
}
#[tokio::test]
async fn contracts_call_honors_requested_entrypoint_and_payload() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_dispatch_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let initial_asset_literal = "6qLb5RYJbzychndCXgFa9aZzjWyx";
    let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let hajimari_payload = norito::json!({ "asset_definition_id": initial_asset_literal });
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        Some(&hajimari_payload),
        2,
    )
    .await;
    let payload = norito::json!({ "amount": "7" });
    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "credit_by_payload",
            payload: Some(&payload),
            gas_limit: 1_500_000,
        },
    );
    let call_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(call_body))
        .unwrap();
    let call_resp = app.clone().oneshot(call_req).await.unwrap();
    assert_eq!(call_resp.status(), http::StatusCode::OK);
    let applied_call =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_call, 1);
    let state_after_credit = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "call_state",
        None,
    )
    .await;
    assert_eq!(
        state_after_credit.get("result"),
        Some(&json::Value::Array(vec![
            json::Value::String("7".to_owned()),
            json::Value::String(initial_asset_literal.to_owned()),
        ]))
    );
    let asset_payload = norito::json!({ "asset_definition_id": asset_literal });
    let asset_call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "record_asset_by_payload",
            payload: Some(&asset_payload),
            gas_limit: 1_500_000,
        },
    );
    let asset_call_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(asset_call_body))
        .unwrap();
    let asset_call_resp = app.clone().oneshot(asset_call_req).await.unwrap();
    assert_eq!(asset_call_resp.status(), http::StatusCode::OK);
    let applied_asset_call =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_asset_call, 1);
    let state_after_asset = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "call_state",
        None,
    )
    .await;
    assert_eq!(
        state_after_asset.get("result"),
        Some(&json::Value::Array(vec![
            json::Value::String("7".to_owned()),
            json::Value::String(asset_literal.to_owned()),
        ]))
    );
}
#[tokio::test]
async fn contracts_view_roundtrips_account_id_literals_and_persisted_state() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_view_account_id_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let initial_account = contract_address.subject_id().to_string();
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let hajimari_payload = iroha_torii::json_object(vec![iroha_torii::json_entry(
        "account_id",
        initial_account.clone(),
    )]);
    let bind_payload = iroha_torii::json_object(vec![iroha_torii::json_entry(
        "account_id",
        creds.account.to_string(),
    )]);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        Some(&hajimari_payload),
        2,
    )
    .await;
    let literal = run_contract_view(&app, &creds.account, &contract_address, "literal", None).await;
    assert_eq!(
        literal.get("result").and_then(json::Value::as_str),
        Some(creds.account.to_string().as_str())
    );
    let initialized =
        run_contract_view(&app, &creds.account, &contract_address, "stored", None).await;
    assert_eq!(
        initialized.get("result").and_then(json::Value::as_str),
        Some(initial_account.as_str())
    );
    let bind_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "bind",
            payload: Some(&bind_payload),
            gas_limit: 1_500_000,
        },
    );
    let bind_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(bind_body))
        .unwrap();
    let bind_resp = app.clone().oneshot(bind_req).await.unwrap();
    assert_eq!(bind_resp.status(), http::StatusCode::OK);
    let applied_bind =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_bind, 1);
    let stored = run_contract_view(&app, &creds.account, &contract_address, "stored", None).await;
    assert_eq!(
        stored.get("result").and_then(json::Value::as_str),
        Some(creds.account.to_string().as_str())
    );
    let stored_tuple = run_contract_view(
        &app,
        &creds.account,
        &contract_address,
        "stored_tuple",
        None,
    )
    .await;
    assert_eq!(
        stored_tuple.get("result"),
        Some(&json::Value::Array(vec![
            json::Value::String(creds.account.to_string()),
            json::Value::String("1".to_owned()),
        ]))
    );
}
#[tokio::test]
async fn contracts_call_configure_roundtrips_account_id_map_state() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_configure_account_map_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1, "expected locally signed deployment");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after epoch")
        .as_millis() as u64;
    let deploy_alias: iroha_data_model::smart_contract::ContractAlias =
        "fixture0::universal".parse().expect("deployment alias");
    let post_apply_view = state.view();
    let alias_target = post_apply_view
        .world
        .contract_address_by_alias_at(&deploy_alias, now_ms);
    let alias_active = alias_target.as_ref().is_some_and(|address| {
        post_apply_view
            .world
            .contract_instances()
            .get(address)
            .is_some()
    });
    assert!(
        alias_active,
        "post-apply alias state missing or inactive: alias_target={alias_target:?}"
    );
    let configure_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("admin_account", creds.account.to_string()),
        iroha_torii::json_entry("inori_account", creds.account.to_string()),
    ]);
    let configure_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "configure",
            payload: Some(&configure_payload),
            gas_limit: 1_500_000,
        },
    );
    let configure_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(configure_body))
        .unwrap();
    let configure_resp = app.clone().oneshot(configure_req).await.unwrap();
    let configure_status = configure_resp.status();
    let configure_bytes = configure_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    if configure_status != http::StatusCode::OK {
        panic!(
            "configure call failed with status {configure_status}: body_lossy={:?} body_hex={}",
            String::from_utf8_lossy(&configure_bytes),
            hex::encode(&configure_bytes)
        );
    }
    let applied_configure =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_configure, 1);
    let admin = run_contract_view(&app, &creds.account, &contract_address, "admin", None).await;
    assert_eq!(
        admin.get("result").and_then(json::Value::as_str),
        Some(creds.account.to_string().as_str())
    );
    let inori = run_contract_view(&app, &creds.account, &contract_address, "inori", None).await;
    assert_eq!(
        inori.get("result").and_then(json::Value::as_str),
        Some(creds.account.to_string().as_str())
    );
    let paused = run_contract_view(&app, &creds.account, &contract_address, "paused", None).await;
    assert_eq!(
        paused.get("result").and_then(json::Value::as_str),
        Some("0")
    );
}
#[tokio::test]
async fn contracts_call_persists_declared_state_fields_across_calls() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_declared_state_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    let initial_asset_literal = "6qLb5RYJbzychndCXgFa9aZzjWyx";
    let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let hajimari_payload = norito::json!({ "asset_definition_id": initial_asset_literal });
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        Some(&hajimari_payload),
        2,
    )
    .await;
    let credit_payload = norito::json!({ "amount": "7" });
    let credit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "credit_by_payload",
            payload: Some(&credit_payload),
            gas_limit: 1_500_000,
        },
    );
    let credit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(credit_body))
        .unwrap();
    let credit_resp = app.clone().oneshot(credit_req).await.unwrap();
    assert_eq!(credit_resp.status(), http::StatusCode::OK);
    let applied_credit =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_credit, 1);
    let asset_payload = norito::json!({ "asset_definition_id": asset_literal });
    let asset_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "record_asset_by_payload",
            payload: Some(&asset_payload),
            gas_limit: 1_500_000,
        },
    );
    let asset_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(asset_body))
        .unwrap();
    let asset_resp = app.clone().oneshot(asset_req).await.unwrap();
    assert_eq!(asset_resp.status(), http::StatusCode::OK);
    let applied_asset =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_asset, 1);
    let view_json = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "declared_state",
        None,
    )
    .await;
    let view_result = view_json
        .get("result")
        .and_then(json::Value::as_array)
        .expect("view result array");
    assert_eq!(
        view_result.first().and_then(json::Value::as_str),
        Some("7"),
        "unexpected declared amount from view",
    );
    assert_eq!(
        view_result.get(1).and_then(json::Value::as_str),
        Some(asset_literal),
        "unexpected declared asset from view",
    );
}
#[tokio::test]
async fn contracts_call_persists_declared_state_after_emitting_isi() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_declared_state_with_isi_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment_with_subject_permissions(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
            [can_modify_account_metadata(&creds.account)],
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        None,
        2,
    )
    .await;
    let write_payload = norito::json!({ "amount": "7" });
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "write_with_isi",
            payload: Some(&write_payload),
            gas_limit: 1_500_000,
        },
    );
    let write_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(write_body))
        .unwrap();
    let write_resp = app.clone().oneshot(write_req).await.unwrap();
    assert_eq!(write_resp.status(), http::StatusCode::OK);
    let applied_write =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_write, 1);
    let view_json = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "declared_state",
        None,
    )
    .await;
    assert_eq!(
        view_json
            .get("result")
            .and_then(json::Value::as_str)
            .expect("view int result"),
        "7"
    );
}
#[tokio::test]
async fn contracts_call_persists_declared_state_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "minted".parse().expect("asset definition name"),
    );
    let mut seed_block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height > 0"),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut seed_tx = seed_block.transaction();
    iroha_data_model::prelude::Register::asset_definition(
        iroha_data_model::asset::AssetDefinition::numeric(
            asset_definition_id.clone(),
            "minted".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_declared_state_with_mint_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment_with_subject_permissions(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
            [can_mint_asset_definition(&asset_definition_id)],
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        None,
        2,
    )
    .await;
    let write_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("amount", "7"),
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
    ]);
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "write_with_mint",
            payload: Some(&write_payload),
            gas_limit: 1_500_000,
        },
    );
    let write_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(write_body))
        .unwrap();
    let write_resp = app.clone().oneshot(write_req).await.unwrap();
    assert_eq!(write_resp.status(), http::StatusCode::OK);
    let applied_write =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_write, 1);
    let view_json = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "declared_state",
        None,
    )
    .await;
    assert_eq!(
        view_json
            .get("result")
            .and_then(json::Value::as_str)
            .expect("view int result"),
        "7"
    );
}
#[tokio::test]
async fn contracts_call_persists_n3x_like_state_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "n3x_like".parse().expect("asset definition name"),
    );
    let mut seed_block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height > 0"),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut seed_tx = seed_block.transaction();
    iroha_data_model::prelude::Register::asset_definition(
        iroha_data_model::asset::AssetDefinition::numeric(
            asset_definition_id.clone(),
            "n3x_like".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_n3x_like_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment_with_subject_permissions(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
            [can_mint_asset_definition(&asset_definition_id)],
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        None,
        2,
    )
    .await;
    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "init_hub",
            payload: None,
            gas_limit: 10_000,
        },
    );
    let init_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(init_body))
        .unwrap();
    let init_resp = app.clone().oneshot(init_req).await.unwrap();
    assert_eq!(init_resp.status(), http::StatusCode::OK);
    let applied_init =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_init, 1);
    let deposit_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
        iroha_torii::json_entry("usdt_in", "1"),
        iroha_torii::json_entry("usdc_in", "2"),
        iroha_torii::json_entry("kusd_in", "3"),
    ]);
    let deposit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "deposit_like",
            payload: Some(&deposit_payload),
            gas_limit: 1_500_000,
        },
    );
    let deposit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deposit_body))
        .unwrap();
    let deposit_resp = app.clone().oneshot(deposit_req).await.unwrap();
    assert_eq!(deposit_resp.status(), http::StatusCode::OK);
    let applied_deposit =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_deposit, 1);
    let view_json = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "state_snapshot",
        None,
    )
    .await;
    let snapshot = view_json
        .get("result")
        .and_then(json::Value::as_array)
        .expect("state snapshot array");
    assert_eq!(snapshot.first().and_then(json::Value::as_str), Some("1"));
    assert_eq!(snapshot.get(1).and_then(json::Value::as_str), Some("1"));
    assert_eq!(snapshot.get(2).and_then(json::Value::as_str), Some("2"));
    assert_eq!(snapshot.get(3).and_then(json::Value::as_str), Some("3"));
    assert_eq!(snapshot.get(4).and_then(json::Value::as_str), Some("6"));
}
#[tokio::test]
async fn contracts_call_executes_n3x_like_burn_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let (creds, state, kura) = contract_test_state();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "n3x_burn".parse().expect("asset definition name"),
    );
    let mut seed_block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("height > 0"),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut seed_tx = seed_block.transaction();
    iroha_data_model::prelude::Register::asset_definition(
        iroha_data_model::asset::AssetDefinition::numeric(
            asset_definition_id.clone(),
            "n3x_burn".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");
    let (queue, chain_id, app) = contract_test_queue_and_app(&state, &kura);
    let program = contract_call_n3x_like_program();
    let (contract_address, _, _) =
        iroha_torii::test_utils::enqueue_locally_signed_contract_deployment_with_subject_permissions(
            &state,
            &queue,
            &creds.account,
            &creds.private_key,
            &program,
            [
                can_mint_asset_definition(&asset_definition_id),
                can_burn_asset_definition(&asset_definition_id),
            ],
        );
    let contract_address = contract_address.to_string();
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);
    run_contract_hajimari_and_apply(
        &app,
        &state,
        &queue,
        &chain_id,
        &creds,
        contract_address.as_str(),
        None,
        2,
    )
    .await;
    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "init_hub",
            payload: None,
            gas_limit: 10_000,
        },
    );
    let init_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(init_body))
        .unwrap();
    let init_resp = app.clone().oneshot(init_req).await.unwrap();
    assert_eq!(init_resp.status(), http::StatusCode::OK);
    let applied_init =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_init, 1);
    let deposit_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
        iroha_torii::json_entry("usdt_in", "1"),
        iroha_torii::json_entry("usdc_in", "2"),
        iroha_torii::json_entry("kusd_in", "3"),
    ]);
    let deposit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "deposit_like",
            payload: Some(&deposit_payload),
            gas_limit: 1_500_000,
        },
    );
    let deposit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deposit_body))
        .unwrap();
    let deposit_resp = app.clone().oneshot(deposit_req).await.unwrap();
    assert_eq!(deposit_resp.status(), http::StatusCode::OK);
    let applied_deposit =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_deposit, 1);
    let burn_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
        iroha_torii::json_entry("n3x_amount", "6"),
    ]);
    let burn_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: "burn_like",
            payload: Some(&burn_payload),
            gas_limit: 1_500_000,
        },
    );
    let burn_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(burn_body))
        .unwrap();
    let burn_resp = app.clone().oneshot(burn_req).await.unwrap();
    assert_eq!(burn_resp.status(), http::StatusCode::OK);
    let applied_burn =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 5);
    assert_eq!(applied_burn, 1);
    let view_json = run_contract_view(
        &app,
        &creds.account,
        contract_address.as_str(),
        "state_snapshot",
        None,
    )
    .await;
    let snapshot = view_json
        .get("result")
        .and_then(json::Value::as_array)
        .expect("state snapshot array");
    assert_eq!(snapshot.first().and_then(json::Value::as_str), Some("1"));
    assert_eq!(snapshot.get(1).and_then(json::Value::as_str), Some("0"));
    assert_eq!(snapshot.get(2).and_then(json::Value::as_str), Some("0"));
    assert_eq!(snapshot.get(3).and_then(json::Value::as_str), Some("0"));
    assert_eq!(snapshot.get(4).and_then(json::Value::as_str), Some("0"));
}
