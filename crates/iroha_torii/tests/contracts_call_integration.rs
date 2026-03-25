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
use iroha_data_model::{asset::AssetDefinitionId, name::Name};
use mv::storage::StorageReadOnly;
use norito::json;
use tower::ServiceExt as _;

fn contract_call_dispatch_program() -> Vec<u8> {
    let src = format!(
        r#"
seiyaku ContractCallDispatchTest {{
  meta {{ abi_version: 1; }}

  kotoage fn main() {{}}

  kotoage fn credit_by_payload() {{
    let ev = trigger_event();
    let amount = json_get_int(ev, name("amount"));
    state_set(name("call_amount"), encode_int(amount));
  }}

  kotoage fn record_asset_by_payload() {{
    let ev = trigger_event();
    let asset = json_get_asset_definition_id(ev, name("asset_definition_id"));
    state_set(name("call_asset"), pointer_to_norito(asset));
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

  kotoage fn credit_by_payload() {{
    let ev = trigger_event();
    let amount = json_get_int(ev, name("amount"));
    CallAmount = amount;
  }}

  kotoage fn record_asset_by_payload() {{
    let ev = trigger_event();
    let asset = json_get_asset_definition_id(ev, name("asset_definition_id"));
    CallAsset = asset;
  }}

  kotoage fn mirror_declared_state() {{
    state_set(name("call_amount_readback"), encode_int(CallAmount));
    state_set(name("call_asset_readback"), pointer_to_norito(CallAsset));
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

  kotoage fn write_with_isi() permission(Admin) {{
    let ev = trigger_event();
    let amount = json_get_int(ev, name("amount"));
    set_account_detail(authority(), name("cursor"), json("{{\"phase\":\"write_with_isi\"}}"));
    CallAmount = amount;
  }}

  kotoage fn mirror_declared_state() {{
    state_set(name("call_amount_readback"), encode_int(CallAmount));
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

  kotoage fn write_with_mint() permission(Admin) {{
    let ev = trigger_event();
    let amount = json_get_int(ev, name("amount"));
    let user = json_get_account_id(ev, name("user"));
    let asset = json_get_asset_definition_id(ev, name("asset_definition_id"));
    mint_asset(user, asset, 1);
    CallAmount = amount;
  }}

  kotoage fn mirror_declared_state() {{
    state_set(name("call_amount_readback"), encode_int(CallAmount));
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

  kotoage fn deposit_like() permission(Admin) {{
    let ev = trigger_event();
    deposit_impl(
      json_get_account_id(ev, name("user")),
      json_get_asset_definition_id(ev, name("asset_definition_id")),
      json_get_int(ev, name("usdt_in")),
      json_get_int(ev, name("usdc_in")),
      json_get_int(ev, name("kusd_in"))
    );
  }}

  kotoage fn burn_like() permission(Admin) {{
    let ev = trigger_event();
    let user = json_get_account_id(ev, name("user"));
    let asset = json_get_asset_definition_id(ev, name("asset_definition_id"));
    let n3x_amount = json_get_int(ev, name("n3x_amount"));
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

  kotoage fn mirror_state() {{
    state_set(name("hub_initialized_readback"), encode_int(HubInitialized));
    state_set(name("basket_usdt_readback"), encode_int(BasketUsdt));
    state_set(name("basket_usdc_readback"), encode_int(BasketUsdc));
    state_set(name("basket_kusd_readback"), encode_int(BasketKusd));
    state_set(name("total_n3x_readback"), encode_int(TotalN3x));
  }}
}}
"#
    );
    ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile contract call n3x-like test program")
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
    let abi_hash_hex = deploy_json
        .get("abi_hash_hex")
        .and_then(json::Value::as_str)
        .expect("abi_hash_hex present")
        .to_owned();

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let missing_limit_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
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
        "apps",
        "calc.v1",
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

    let payload = norito::json!({ "note": "integration-test" });
    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("main"),
            payload: Some(&payload),
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
            .get("namespace")
            .and_then(json::Value::as_str)
            .unwrap(),
        "apps"
    );
    assert_eq!(
        call_json
            .get("contract_id")
            .and_then(json::Value::as_str)
            .unwrap(),
        "calc.v1"
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
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_call, 1);

    let draft_payload = norito::json!({ "note": "client-signed" });
    let draft_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("payload", draft_payload.clone()),
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
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("payload", draft_payload),
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
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_detached_submit, 1);
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_dispatch_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "dispatch.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let payload = norito::json!({ "amount": 7 });
    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "dispatch.v1",
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
        "apps",
        "dispatch.v1",
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_declared_state_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let credit_payload = norito::json!({ "amount": 7 });
    let credit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared.v1",
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
        "apps",
        "declared.v1",
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

    let mirror_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("mirror_declared_state"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let mirror_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(mirror_body))
        .unwrap();
    let mirror_resp = app.clone().oneshot(mirror_req).await.unwrap();
    assert_eq!(mirror_resp.status(), http::StatusCode::OK);

    let applied_mirror =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 5);
    assert_eq!(applied_mirror, 1);

    let call_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let call_asset_path: Name = "CallAsset".parse().expect("declared asset path");
    let amount_readback_path: Name = "call_amount_readback"
        .parse()
        .expect("readback amount path");
    let asset_readback_path: Name = "call_asset_readback".parse().expect("readback asset path");

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

    let amount_readback = view
        .world
        .smart_contract_state()
        .get(&amount_readback_path)
        .expect("stored amount readback");
    let amount_readback_tlv =
        ivm::pointer_abi::validate_tlv_bytes(amount_readback).expect("amount readback tlv");
    let readback_amount: i64 =
        norito::decode_from_bytes(amount_readback_tlv.payload).expect("decode readback amount");
    assert_eq!(readback_amount, 7);

    let asset_readback = view
        .world
        .smart_contract_state()
        .get(&asset_readback_path)
        .expect("stored asset readback");
    let asset_readback_outer =
        ivm::pointer_abi::validate_tlv_bytes(asset_readback).expect("asset readback tlv");
    assert_eq!(asset_readback_outer.type_id, ivm::PointerType::NoritoBytes);
    let asset_readback_inner = ivm::pointer_abi::validate_tlv_bytes(asset_readback_outer.payload)
        .expect("inner asset readback tlv");
    assert_eq!(
        asset_readback_inner.type_id,
        ivm::PointerType::AssetDefinitionId
    );
    let readback_asset: AssetDefinitionId =
        norito::decode_from_bytes(asset_readback_inner.payload).expect("decode readback asset");
    assert_eq!(
        readback_asset,
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_declared_state_with_isi_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_isi.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let write_payload = norito::json!({ "amount": 7 });
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_isi.v1",
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

    let mirror_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_isi.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("mirror_declared_state"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let mirror_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(mirror_body))
        .unwrap();
    let mirror_resp = app.clone().oneshot(mirror_req).await.unwrap();
    assert_eq!(mirror_resp.status(), http::StatusCode::OK);

    let applied_mirror =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_mirror, 1);

    let declared_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let readback_path: Name = "call_amount_readback"
        .parse()
        .expect("readback amount path");
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

    let readback_amount = view
        .world
        .smart_contract_state()
        .get(&readback_path)
        .expect("stored readback amount");
    let readback_tlv =
        ivm::pointer_abi::validate_tlv_bytes(readback_amount).expect("readback amount tlv");
    let mirrored_amount: i64 =
        norito::decode_from_bytes(readback_tlv.payload).expect("decode mirrored amount");
    assert_eq!(mirrored_amount, 7);
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
        "wonderland".parse().expect("domain id"),
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_declared_state_with_mint_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_mint.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let write_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("amount", 7),
        iroha_torii::json_entry("user", creds.account.clone()),
        iroha_torii::json_entry("asset_definition_id", asset_definition_id.to_string()),
    ]);
    let write_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_mint.v1",
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

    let mirror_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "declared_mint.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("mirror_declared_state"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let mirror_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(mirror_body))
        .unwrap();
    let mirror_resp = app.clone().oneshot(mirror_req).await.unwrap();
    assert_eq!(mirror_resp.status(), http::StatusCode::OK);

    let applied_mirror =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_mirror, 1);

    let declared_amount_path: Name = "CallAmount".parse().expect("declared amount path");
    let readback_path: Name = "call_amount_readback"
        .parse()
        .expect("readback amount path");
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

    let readback_amount = view
        .world
        .smart_contract_state()
        .get(&readback_path)
        .expect("stored readback amount");
    let readback_tlv =
        ivm::pointer_abi::validate_tlv_bytes(readback_amount).expect("readback amount tlv");
    let mirrored_amount: i64 =
        norito::decode_from_bytes(readback_tlv.payload).expect("decode mirrored amount");
    assert_eq!(mirrored_amount, 7);
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
        "wonderland".parse().expect("domain id"),
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_n3x_like_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_like.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_like.v1",
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
        "apps",
        "n3x_like.v1",
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

    let mirror_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_like.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("mirror_state"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let mirror_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(mirror_body))
        .unwrap();
    let mirror_resp = app.clone().oneshot(mirror_req).await.unwrap();
    assert_eq!(mirror_resp.status(), http::StatusCode::OK);

    let applied_mirror =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 5);
    assert_eq!(applied_mirror, 1);

    let view = state.view();
    let check_readback = |path: &str, expected: i64| {
        let state_path: Name = path.parse().expect("state path");
        let stored = view
            .world
            .smart_contract_state()
            .get(&state_path)
            .expect("stored readback");
        let tlv = ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored tlv");
        let decoded: i64 = norito::decode_from_bytes(tlv.payload).expect("decode readback");
        assert_eq!(decoded, expected, "unexpected value for {path}");
    };

    check_readback("hub_initialized_readback", 1);
    check_readback("basket_usdt_readback", 1);
    check_readback("basket_usdc_readback", 2);
    check_readback("basket_kusd_readback", 3);
    check_readback("total_n3x_readback", 6);
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
        "wonderland".parse().expect("domain id"),
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

    let app = Router::new()
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
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
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
        );

    let program = contract_call_n3x_like_program();
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

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_burn.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let init_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_burn.v1",
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
        "apps",
        "n3x_burn.v1",
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
        "apps",
        "n3x_burn.v1",
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

    let mirror_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "n3x_burn.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("mirror_state"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 10_000,
        },
    );
    let mirror_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(mirror_body))
        .unwrap();
    let mirror_resp = app.clone().oneshot(mirror_req).await.unwrap();
    assert_eq!(mirror_resp.status(), http::StatusCode::OK);

    let applied_mirror =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 6);
    assert_eq!(applied_mirror, 1);

    let view = state.view();
    let check_readback = |path: &str, expected: i64| {
        let state_path: Name = path.parse().expect("state path");
        let stored = view
            .world
            .smart_contract_state()
            .get(&state_path)
            .expect("stored readback");
        let tlv = ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored tlv");
        let decoded: i64 = norito::decode_from_bytes(tlv.payload).expect("decode readback");
        assert_eq!(decoded, expected, "unexpected value for {path}");
    };

    check_readback("hub_initialized_readback", 1);
    check_readback("basket_usdt_readback", 0);
    check_readback("basket_usdc_readback", 0);
    check_readback("basket_kusd_readback", 0);
    check_readback("total_n3x_readback", 0);
}
