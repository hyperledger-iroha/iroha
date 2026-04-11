#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for the contract call endpoint.
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs, clippy::too_many_lines)]

use std::sync::Arc;

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
    DomainId, Registrable,
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    name::Name,
    smart_contract::ContractAddress,
};
use ivm::kotodama::compiler::CompilerOptions;
use mv::storage::StorageReadOnly;
use norito::json;
use tower::ServiceExt as _;

fn contract_call_dispatch_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDispatchTest {{
  meta {{ abi_version: 1; }}

  kotoage fn main() {{}}

  kotoage fn credit_by_payload(amount: int) {{
    state_set(name("call_amount"), encode_int(amount));
  }}

  kotoage fn record_asset_by_payload(asset_definition_id: AssetDefinitionId) {{
    state_set(name("call_asset"), pointer_to_norito(asset_definition_id));
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
  meta {{ abi_version: 1; }}

  state int CallAmount;
  state AssetDefinitionId CallAsset;

  kotoage fn main() {{}}

  kotoage fn credit_by_payload(amount: int) {{
    CallAmount = amount;
  }}

  kotoage fn record_asset_by_payload(asset_definition_id: AssetDefinitionId) {{
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
  meta {{ abi_version: 1; }}

  state int CallAmount;

  kotoage fn main() {{}}

  kotoage fn write_with_isi(amount: int) permission(Admin) {{
    set_account_detail(authority(), name("cursor"), json("{{\"phase\":\"write_with_isi\"}}"));
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
  meta {{ abi_version: 1; }}

  state int CallAmount;

  kotoage fn main() {{}}

  kotoage fn write_with_mint(amount: int,
                             user: AccountId,
                             asset_definition_id: AssetDefinitionId) permission(Admin) {{
    mint_asset(user, asset_definition_id, 1);
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
  meta {{ abi_version: 1; }}

  state int HubInitialized;
  state int BasketUsdt;
  state int BasketUsdc;
  state int BasketKusd;
  state int TotalN3x;

  kotoage fn main() {{}}

  fn init_impl() {{
    HubInitialized = 1;
    BasketUsdt = 0;
    BasketUsdc = 0;
    BasketKusd = 0;
    TotalN3x = 0;
  }}

  kotoage fn init_hub() permission(Admin) {{
    init_impl();
  }}

  fn deposit_impl(user: AccountId,
                  asset: AssetDefinitionId,
                  usdt_in: int,
                  usdc_in: int,
                  kusd_in: int) {{
    assert(HubInitialized == 1, "hub not initialized");
    let minted = usdt_in + usdc_in + kusd_in;
    mint_asset(user, asset, minted);
    BasketUsdt = BasketUsdt + usdt_in;
    BasketUsdc = BasketUsdc + usdc_in;
    BasketKusd = BasketKusd + kusd_in;
    TotalN3x = TotalN3x + minted;
  }}

  kotoage fn deposit_like(user: AccountId,
                          asset_definition_id: AssetDefinitionId,
                          usdt_in: int,
                          usdc_in: int,
                          kusd_in: int) permission(Admin) {{
    deposit_impl(user, asset_definition_id, usdt_in, usdc_in, kusd_in);
  }}

  kotoage fn burn_like(user: AccountId,
                       asset_definition_id: AssetDefinitionId,
                       n3x_amount: int) permission(Admin) {{
    let total = TotalN3x;
    assert(total > 0, "empty hub");
    assert(n3x_amount > 0, "invalid n3x_amount");
    assert(n3x_amount <= total, "insufficient supply");
    let usdt_out = (BasketUsdt * n3x_amount) / total;
    let usdc_out = (BasketUsdc * n3x_amount) / total;
    let kusd_out = (BasketKusd * n3x_amount) / total;
    let redeemed = usdt_out + usdc_out + kusd_out;
    assert(redeemed > 0, "zero redemption");
    burn_asset(user, asset, n3x_amount);
    BasketUsdt = BasketUsdt - usdt_out;
    BasketUsdc = BasketUsdc - usdc_out;
    BasketKusd = BasketKusd - kusd_out;
    TotalN3x = total - n3x_amount;
  }}

  view fn state_snapshot() -> (int, int, int, int, int) {{
    return (HubInitialized, BasketUsdt, BasketUsdc, BasketKusd, TotalN3x);
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call n3x-like test program")
}

fn contract_call_nested_transfer_caller_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallNestedTransferCallerTest {{
  meta {{ abi_version: 1; }}

  state AccountId CallerAccount;
  state bytes VaultContract;
  state AssetDefinitionId SettlementAsset;

  kotoage fn main() {{}}

  kotoage fn bind(caller_account: AccountId,
                  vault_contract: bytes,
                  settlement_asset: AssetDefinitionId) {{
    CallerAccount = caller_account;
    VaultContract = vault_contract;
    SettlementAsset = settlement_asset;
  }}

  kotoage fn open(amount: int) -> int permission(AssetOps) {{
    transfer_asset(authority(), CallerAccount, SettlementAsset, amount);
    let payload = json_object();
    let payload = json_set_int(payload, name("amount"), amount);
    return decode_int(call_contract(VaultContract, "deposit", payload));
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile nested transfer caller program")
}

fn contract_call_nested_transfer_vault_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallNestedTransferVaultTest {{
  meta {{ abi_version: 1; }}

  state AccountId VaultAccount;
  state AssetDefinitionId SettlementAsset;

  kotoage fn main() {{}}

  kotoage fn bind(vault_account: AccountId,
                  settlement_asset: AssetDefinitionId) {{
    VaultAccount = vault_account;
    SettlementAsset = settlement_asset;
  }}

  kotoage fn deposit(amount: int) -> int permission(AssetOps) {{
    transfer_asset(authority(), VaultAccount, SettlementAsset, amount);
    return amount;
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile nested transfer vault program")
}

fn contract_view_trap_program_with_source_path(source_path: &str) -> Vec<u8> {
    let src = r#"
seiyaku ContractViewTrapTest {
  meta { abi_version: 1; }

  kotoage fn main() {}

  view fn explode() -> int {
    assert(false, "boom");
    return 1;
  }
}
"#;
    ivm::KotodamaCompiler::new_with_options(CompilerOptions {
        debug_source_name: Some(source_path.to_owned()),
        ..CompilerOptions::default()
    })
    .compile_source(src)
    .expect("compile contract view trap test program")
}

fn contract_view_bytes_program() -> Vec<u8> {
    let src = r#"
seiyaku ContractViewBytesTest {
  meta { abi_version: 1; }

  state AssetDefinitionId Asset;
  state bytes Target;

  kotoage fn main() {}

  kotoage fn init(asset: AssetDefinitionId, target: bytes) {
    Asset = asset;
    Target = target;
  }

  view fn literal() -> bytes {
    return blob("risk");
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
  meta { abi_version: 1; }

  state AccountId Stored;

  kotoage fn main() {}

  kotoage fn bind(account_id: AccountId) {
    Stored = account_id;
  }

  view fn literal() -> AccountId {
    return authority();
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

fn contract_test_app(
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: iroha_data_model::ChainId,
    telemetry: iroha_torii::MaybeTelemetry,
) -> Router {
    Router::new()
        .route(
            "/v1/contracts/deploy",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::DeployContractDto,
                >| async move {
                    iroha_torii::handle_post_contract_deploy(
                        chain_id.clone(),
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
            "/v1/contracts/call",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ContractCallDto,
                >| async move {
                    iroha_torii::handle_post_contract_call(
                        chain_id.clone(),
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
                    .await
                }
            }),
        )
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
            entrypoint: Some(entrypoint),
            payload,
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
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        json::from_slice(&bytes).expect("decode contract view response"),
    )
}

fn deployed_contract_address(response: &json::Value) -> String {
    response
        .get("contract_address")
        .and_then(json::Value::as_str)
        .expect("contract_address present in deploy response")
        .to_owned()
}

#[tokio::test]
async fn contracts_call_enqueues_transaction() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);
    let abi_hash_hex = deploy_json
        .get("abi_hash_hex")
        .and_then(json::Value::as_str)
        .expect("abi_hash_hex present")
        .to_owned();

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
            entrypoint: Some("main"),
            payload: None,
            gas_asset_id: None,
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

    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("main"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 5_000,
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

    let applied_call =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_call, 1);

    let draft_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("contract_address", contract_address.as_str()),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("gas_limit", 5_000u64),
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
            .get("signed_transaction_b64")
            .and_then(json::Value::as_str)
            .is_some(),
        "expected contract call draft scaffold when private_key is omitted"
    );
    let transaction_scaffold_b64 = draft_json
        .get("transaction_scaffold_b64")
        .and_then(json::Value::as_str)
        .expect("transaction_scaffold_b64 present");
    assert_eq!(
        draft_json
            .get("signed_transaction_b64")
            .and_then(json::Value::as_str)
            .expect("signed_transaction_b64 present"),
        transaction_scaffold_b64
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
    let detached_signature = Signature::new(&creds.private_key.0, &signing_message);
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
        iroha_torii::json_entry("gas_limit", 5_000u64),
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
async fn contracts_view_surfaces_source_path_in_vm_diagnostic() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let source_path = "contracts/view_trap_test.ko";
    let program = contract_view_trap_program_with_source_path(source_path);
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let body = iroha_torii::test_utils::contract_view_request_json(
        &creds.account,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractViewOptions {
            entrypoint: Some("explode"),
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
    assert_eq!(
        value
            .get("vm_diagnostic")
            .and_then(json::Value::as_object)
            .and_then(|diag| diag.get("source_path"))
            .and_then(json::Value::as_str),
        Some(source_path)
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

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_view_bytes_program();
    let deploy_body = iroha_torii::test_utils::deploy_request_json(
        &creds.account,
        &creds.private_key,
        &base64::engine::general_purpose::STANDARD.encode(&program),
    );
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let asset_definition_id = "6qLb5RYJbzychndCXgFa9aZzjWyx"
        .parse::<AssetDefinitionId>()
        .expect("asset definition id");
    let init_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset", asset_definition_id.to_string()),
        iroha_torii::json_entry("target", "risk_vault::risk.universal"),
    ]);
    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("init"),
            payload: Some(&init_payload),
            gas_asset_id: None,
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
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
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

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_dispatch_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let payload = norito::json!({ "amount": 7 });
    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("credit_by_payload"),
            payload: Some(&payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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

    let state_path: Name = "call_amount".parse().expect("state path");
    let view = state.view();
    let stored = view
        .world
        .smart_contract_state()
        .get(&state_path)
        .expect("recorded state payload");
    let tlv = ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored tlv");
    let recorded: i64 = norito::decode_from_bytes(tlv.payload).expect("decode state payload");
    assert_eq!(recorded, 7);

    let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let asset_payload = norito::json!({ "asset_definition_id": asset_literal });
    let asset_call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("record_asset_by_payload"),
            payload: Some(&asset_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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

    let asset_state_path: Name = "call_asset".parse().expect("asset state path");
    let asset_view = state.view();
    let stored_asset = asset_view
        .world
        .smart_contract_state()
        .get(&asset_state_path)
        .expect("recorded asset payload");
    let outer = ivm::pointer_abi::validate_tlv_bytes(stored_asset).expect("outer asset tlv");
    assert_eq!(outer.type_id, ivm::PointerType::NoritoBytes);
    let inner = ivm::pointer_abi::validate_tlv_bytes(outer.payload).expect("inner asset tlv");
    assert_eq!(inner.type_id, ivm::PointerType::AssetDefinitionId);
    let recorded_asset: AssetDefinitionId =
        norito::decode_from_bytes(inner.payload).expect("decode asset payload");
    assert_eq!(
        recorded_asset,
        AssetDefinitionId::parse_address_literal(asset_literal).expect("asset definition literal")
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

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_view_account_id_program();
    let deploy_body = iroha_torii::test_utils::deploy_request_json(
        &creds.account,
        &creds.private_key,
        &base64::engine::general_purpose::STANDARD.encode(&program),
    );
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let literal = run_contract_view(&app, &creds.account, &contract_address, "literal", None).await;
    assert_eq!(
        literal.get("result").and_then(json::Value::as_str),
        Some(creds.account.to_string().as_str())
    );

    let bind_payload = iroha_torii::json_object(vec![iroha_torii::json_entry(
        "account_id",
        creds.account.to_string(),
    )]);
    let bind_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("bind"),
            payload: Some(&bind_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_bind, 1);

    let parsed_contract_address: ContractAddress = contract_address
        .parse()
        .expect("parse deployed contract address");
    let state_scope = hex::encode(
        iroha_crypto::Hash::new(parsed_contract_address.to_string().as_bytes()).as_ref(),
    );
    let stored_path: Name = format!("sc/{state_scope}/Stored")
        .parse()
        .expect("scoped stored path");
    let persisted_view = state.view();
    let stored_bytes = persisted_view
        .world
        .smart_contract_state()
        .get(&stored_path)
        .expect("persisted scoped Stored state");
    let outer = ivm::pointer_abi::validate_tlv_bytes(stored_bytes).expect("outer stored tlv");
    assert_eq!(outer.type_id, ivm::PointerType::NoritoBytes);
    let inner = ivm::pointer_abi::validate_tlv_bytes(outer.payload).expect("inner stored tlv");
    assert_eq!(inner.type_id, ivm::PointerType::AccountId);
    let persisted: iroha_data_model::account::AccountId =
        norito::decode_from_bytes(inner.payload).expect("decode persisted account id");
    assert_eq!(persisted, creds.account);

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
            json::Value::from(1),
        ]))
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

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_declared_state_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let credit_payload = norito::json!({ "amount": 7 });
    let credit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("credit_by_payload"),
            payload: Some(&credit_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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

    let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let asset_payload = norito::json!({ "asset_definition_id": asset_literal });
    let asset_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("record_asset_by_payload"),
            payload: Some(&asset_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
        view_result.first().and_then(json::Value::as_i64),
        Some(7),
        "unexpected declared amount from view",
    );
    assert_eq!(
        view_result.get(1).and_then(json::Value::as_str),
        Some(asset_literal),
        "unexpected declared asset from view",
    );

    let call_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let call_asset_path: Name = "CallAsset".parse().expect("declared asset path");

    let view = state.view();

    let stored_amount = view
        .world
        .smart_contract_state()
        .get(&call_amount_path)
        .expect("stored declared amount");
    let amount_tlv = ivm::pointer_abi::validate_tlv_bytes(stored_amount).expect("amount tlv");
    let declared_amount: i64 =
        norito::decode_from_bytes(amount_tlv.payload).expect("decode declared amount");
    assert_eq!(declared_amount, 7);

    let stored_asset = view
        .world
        .smart_contract_state()
        .get(&call_asset_path)
        .expect("stored declared asset");
    let asset_outer = ivm::pointer_abi::validate_tlv_bytes(stored_asset).expect("asset tlv");
    assert_eq!(asset_outer.type_id, ivm::PointerType::NoritoBytes);
    let asset_inner =
        ivm::pointer_abi::validate_tlv_bytes(asset_outer.payload).expect("inner asset tlv");
    assert_eq!(asset_inner.type_id, ivm::PointerType::AssetDefinitionId);
    let declared_asset: AssetDefinitionId =
        norito::decode_from_bytes(asset_inner.payload).expect("decode declared asset");
    assert_eq!(
        declared_asset,
        AssetDefinitionId::parse_address_literal(asset_literal).expect("asset definition literal")
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

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_declared_state_with_isi_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let write_payload = norito::json!({ "amount": 7 });
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("write_with_isi"),
            payload: Some(&write_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
            .and_then(json::Value::as_i64)
            .expect("view int result"),
        7
    );

    let declared_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let view = state.view();

    let stored_amount = view
        .world
        .smart_contract_state()
        .get(&declared_amount_path)
        .expect("stored declared amount");
    let amount_tlv = ivm::pointer_abi::validate_tlv_bytes(stored_amount).expect("amount tlv");
    let declared_amount: i64 =
        norito::decode_from_bytes(amount_tlv.payload).expect("decode declared amount");
    assert_eq!(declared_amount, 7);
}

#[tokio::test]
async fn contracts_call_persists_declared_state_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);

    let asset_definition_id = AssetDefinitionId::new(
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
        iroha_data_model::asset::AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");

    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_declared_state_with_mint_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let write_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("amount", 7),
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
    ]);
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("write_with_mint"),
            payload: Some(&write_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
            .and_then(json::Value::as_i64)
            .expect("view int result"),
        7
    );

    let declared_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let view = state.view();

    let stored_amount = view
        .world
        .smart_contract_state()
        .get(&declared_amount_path)
        .expect("stored declared amount");
    let amount_tlv = ivm::pointer_abi::validate_tlv_bytes(stored_amount).expect("amount tlv");
    let declared_amount: i64 =
        norito::decode_from_bytes(amount_tlv.payload).expect("decode declared amount");
    assert_eq!(declared_amount, 7);
}

#[tokio::test]
async fn contracts_call_persists_n3x_like_state_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);

    let asset_definition_id = AssetDefinitionId::new(
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
        iroha_data_model::asset::AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");

    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_n3x_like_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("init_hub"),
            payload: None,
            gas_asset_id: None,
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
        iroha_torii::json_entry("usdt_in", 1),
        iroha_torii::json_entry("usdc_in", 2),
        iroha_torii::json_entry("kusd_in", 3),
    ]);
    let deposit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("deposit_like"),
            payload: Some(&deposit_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
    assert_eq!(snapshot.first().and_then(json::Value::as_i64), Some(1));
    assert_eq!(snapshot.get(1).and_then(json::Value::as_i64), Some(1));
    assert_eq!(snapshot.get(2).and_then(json::Value::as_i64), Some(2));
    assert_eq!(snapshot.get(3).and_then(json::Value::as_i64), Some(3));
    assert_eq!(snapshot.get(4).and_then(json::Value::as_i64), Some(6));
}

#[tokio::test]
async fn contracts_call_executes_n3x_like_burn_after_mint_asset() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);

    let asset_definition_id = AssetDefinitionId::new(
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
        iroha_data_model::asset::AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    )
    .execute(&creds.account, &mut seed_tx)
    .expect("register asset definition");
    seed_tx.apply();
    seed_block.commit().expect("commit seeded asset definition");

    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let program = contract_call_n3x_like_program();
    let _code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let contract_address = deployed_contract_address(&deploy_json);

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("init_hub"),
            payload: None,
            gas_asset_id: None,
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
        iroha_torii::json_entry("usdt_in", 1),
        iroha_torii::json_entry("usdc_in", 2),
        iroha_torii::json_entry("kusd_in", 3),
    ]);
    let deposit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("deposit_like"),
            payload: Some(&deposit_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
        iroha_torii::json_entry("n3x_amount", 6),
    ]);
    let burn_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("burn_like"),
            payload: Some(&burn_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
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
    assert_eq!(snapshot.first().and_then(json::Value::as_i64), Some(1));
    assert_eq!(snapshot.get(1).and_then(json::Value::as_i64), Some(0));
    assert_eq!(snapshot.get(2).and_then(json::Value::as_i64), Some(0));
    assert_eq!(snapshot.get(3).and_then(json::Value::as_i64), Some(0));
    assert_eq!(snapshot.get(4).and_then(json::Value::as_i64), Some(0));
}

#[tokio::test]
async fn contracts_call_preserves_root_and_nested_transfer_authorities() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let creds = iroha_torii::test_utils::random_authority();
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let asset_definition_id =
        AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
    let authority_asset_id = AssetId::of(asset_definition_id.clone(), creds.account.clone());
    let authority_asset = Asset::new(
        authority_asset_id.clone(),
        iroha_primitives::numeric::Numeric::new(5_u32, 0),
    );
    let world = iroha_core::state::World::with_assets(
        [iroha_data_model::prelude::Domain::new(domain_id.clone()).build(&creds.account)],
        [iroha_data_model::prelude::Account::new(creds.account.clone()).build(&creds.account)],
        [AssetDefinition::numeric(asset_definition_id.clone()).build(&creds.account)],
        [authority_asset],
        [],
    );

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);

    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let app = contract_test_app(
        state.clone(),
        queue.clone(),
        chain_id.clone(),
        telemetry.clone(),
    );

    let vault_program = contract_call_nested_transfer_vault_program();
    let vault_deploy_body = iroha_torii::test_utils::deploy_request_json(
        &creds.account,
        &creds.private_key,
        &base64::engine::general_purpose::STANDARD.encode(&vault_program),
    );
    let vault_deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(vault_deploy_body))
        .unwrap();
    let vault_deploy_resp = app.clone().oneshot(vault_deploy_req).await.unwrap();
    assert_eq!(vault_deploy_resp.status(), http::StatusCode::OK);
    let vault_deploy_bytes = vault_deploy_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let vault_deploy_json: json::Value = json::from_slice(&vault_deploy_bytes).unwrap();
    let vault_contract_address = deployed_contract_address(&vault_deploy_json);
    let applied_vault_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_vault_deploy, 1);

    let caller_program = contract_call_nested_transfer_caller_program();
    let caller_deploy_body = iroha_torii::test_utils::deploy_request_json(
        &creds.account,
        &creds.private_key,
        &base64::engine::general_purpose::STANDARD.encode(&caller_program),
    );
    let caller_deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(caller_deploy_body))
        .unwrap();
    let caller_deploy_resp = app.clone().oneshot(caller_deploy_req).await.unwrap();
    assert_eq!(caller_deploy_resp.status(), http::StatusCode::OK);
    let caller_deploy_bytes = caller_deploy_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let caller_deploy_json: json::Value = json::from_slice(&caller_deploy_bytes).unwrap();
    let caller_contract_address = deployed_contract_address(&caller_deploy_json);
    let applied_caller_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_caller_deploy, 1);

    let vault_contract_subject = vault_contract_address
        .parse::<ContractAddress>()
        .expect("vault contract address")
        .subject_id();
    let caller_contract_subject = caller_contract_address
        .parse::<ContractAddress>()
        .expect("caller contract address")
        .subject_id();

    let vault_bind_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("vault_account", vault_contract_subject.clone()),
        iroha_torii::json_entry("settlement_asset", asset_definition_id.to_string()),
    ]);
    let vault_bind_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        vault_contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("bind"),
            payload: Some(&vault_bind_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let vault_bind_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(vault_bind_body))
        .unwrap();
    let vault_bind_resp = app.clone().oneshot(vault_bind_req).await.unwrap();
    assert_eq!(vault_bind_resp.status(), http::StatusCode::OK);
    let applied_vault_bind =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_vault_bind, 1);

    let caller_bind_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("caller_account", caller_contract_subject.clone()),
        iroha_torii::json_entry("vault_contract", vault_contract_address.as_str()),
        iroha_torii::json_entry("settlement_asset", asset_definition_id.to_string()),
    ]);
    let caller_bind_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        caller_contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("bind"),
            payload: Some(&caller_bind_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let caller_bind_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(caller_bind_body))
        .unwrap();
    let caller_bind_resp = app.clone().oneshot(caller_bind_req).await.unwrap();
    assert_eq!(caller_bind_resp.status(), http::StatusCode::OK);
    let applied_caller_bind =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_caller_bind, 1);

    let open_payload = iroha_torii::json_object(vec![iroha_torii::json_entry("amount", 3)]);
    let open_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        caller_contract_address.as_str(),
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("open"),
            payload: Some(&open_payload),
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let open_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(open_body))
        .unwrap();
    let open_resp = app.clone().oneshot(open_req).await.unwrap();
    assert_eq!(open_resp.status(), http::StatusCode::OK);
    let applied_open =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 5);
    assert_eq!(applied_open, 1);

    let view = state.view();
    let authority_balance = view
        .world()
        .asset(&authority_asset_id)
        .expect("authority asset remains")
        .value()
        .clone();
    let caller_asset_id = AssetId::of(asset_definition_id.clone(), caller_contract_subject);
    let vault_asset_id = AssetId::of(asset_definition_id, vault_contract_subject);
    let vault_balance = view
        .world()
        .asset(&vault_asset_id)
        .expect("vault asset exists")
        .value()
        .clone();
    assert_eq!(
        authority_balance.as_ref(),
        &iroha_primitives::numeric::Numeric::new(2_u32, 0)
    );
    assert!(
        view.world().asset(&caller_asset_id).is_err(),
        "fully drained caller balance should remove the asset entry"
    );
    assert_eq!(
        vault_balance.as_ref(),
        &iroha_primitives::numeric::Numeric::new(3_u32, 0)
    );
}
