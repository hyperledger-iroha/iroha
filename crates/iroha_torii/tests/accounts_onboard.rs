//! Sponsored account-onboarding clean-break contract tests.
#![cfg(feature = "app_api")]
use axum::{
    body::{Body, to_bytes},
    extract::connect_info::ConnectInfo,
    http::{Request, header},
};
use http::StatusCode;
use iroha_core::{
    block::BlockBuilder,
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::{AcceptedTransaction, TransactionBuilder},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId, Registrable,
    account::{AccountAddress, AccountId},
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    level::Level,
    nexus::DataSpaceId,
    peer::PeerId,
    permission::Permission,
    prelude::{Account, Asset, AssetDefinition, Domain, Log},
    sns::{NameControllerV1, NameRecordV1},
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_torii::{json_entry, json_object};
use iroha_torii_shared::{
    AccountOnboardingCurrentStateResponseV1,
    route_catalog::{
        AuthenticationPolicy, CATALOGED_ROUTES,
        application_api::{
            ACCOUNTS_ONBOARD_PLAN_POST, ACCOUNTS_ONBOARD_POST, ACCOUNTS_ONBOARD_PREPARE_POST,
            ACCOUNTS_ONBOARDING_CURRENT_STATE_POST, ACCOUNTS_ONBOARDING_READINESS_GET,
        },
    },
};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    num::NonZeroU8,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "fixtures.rs"]
mod fixtures;
const ONBOARDING_API_TOKEN: &str = "torii-onboarding-test-token-32-bytes";
const ONBOARDING_WRONG_SCOPE_API_TOKEN: &str = "torii-onboarding-wrong-scope-token";
const ONBOARDING_SIGNER_PATH: &str = "/runtime-only/onboarding-test-signer.key";
static ONBOARDING_TORII_INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct OnboardingTestContext {
    app: iroha_torii::TestApiRouterRuntime,
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: iroha_data_model::ChainId,
    _data_dir: tempfile::TempDir,
}
impl OnboardingTestContext {
    async fn shutdown(self) {
        self.app.shutdown().await;
    }
}
struct JsonResponse {
    status: StatusCode,
    raw_body: String,
    payload: norito::json::Value,
}
fn checked_key_pair(seed: u8, algorithm: Algorithm, context: &str) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], algorithm)
        .unwrap_or_else(|error| panic!("{context}: {error}"))
}
fn install_account_alias_policy(
    world: &mut World,
    authority: &AccountId,
    payment_asset_id: &AssetDefinitionId,
) {
    let mut policy = iroha_data_model::sns::fixtures::default_policy();
    policy.suffix_id = iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID;
    policy.suffix = "account-alias".to_owned();
    policy.steward = authority.clone();
    policy.fund_splitter_account = authority.clone();
    policy.payment_asset_id = payment_asset_id.to_string();
    for tier in &mut policy.pricing {
        tier.label_regex = r"^[a-z0-9_@.-]{3,255}$".to_owned();
        tier.base_price.asset_id = policy.payment_asset_id.clone();
    }
    world.smart_contract_state_mut_for_testing().insert(
        iroha_core::sns::policy_storage_key(iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID),
        norito::codec::Encode::encode(&policy),
    );
}
fn install_universal_parent_lease(world: &mut World, authority: &AccountId) {
    let selector =
        iroha_core::sns::selector_for_dataspace_alias("universal").expect("universal selector");
    let controller = NameControllerV1::account(
        &AccountAddress::from_account_id(authority).expect("onboarding authority address"),
    );
    let mut metadata = iroha_data_model::metadata::Metadata::default();
    metadata.insert(
        iroha_core::sns::SNS_DATASPACE_ID_METADATA_KEY
            .parse()
            .expect("dataspace metadata key"),
        Json::new(DataSpaceId::UNIVERSAL.as_u64()),
    );
    let record = NameRecordV1::new(
        selector.clone(),
        authority.clone(),
        vec![controller],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        metadata,
    );
    world.smart_contract_state_mut_for_testing().insert(
        iroha_core::sns::record_storage_key(&selector),
        norito::codec::Encode::encode(&record),
    );
}
fn build_onboarding_test_context() -> OnboardingTestContext {
    build_onboarding_test_context_with(iroha_torii::test_utils::signed_query_network_id(), 0xD1)
}
fn build_onboarding_test_context_with(
    network_id: NetworkId,
    onboarding_signer_seed: u8,
) -> OnboardingTestContext {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let data_dir = tempfile::tempdir().expect("create isolated onboarding Torii data directory");
    cfg.torii.sorafs_storage.data_dir = data_dir.path().join("sorafs");
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let authority_key_pair = checked_key_pair(
        onboarding_signer_seed,
        Algorithm::Ed25519,
        "derive onboarding authority fixture",
    );
    let authority_id = AccountId::new(authority_key_pair.public_key().clone());
    let fee_domain = DomainId::try_new("universal", "universal").expect("fee domain");
    let fee_asset_id: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("default fee asset id");
    let domain = Domain::new(fee_domain).build(&authority_id);
    let authority = Account::new(authority_id.clone()).build(&authority_id);
    let fee_definition = AssetDefinition::numeric(
        fee_asset_id.clone(),
        "XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority_id);
    let fee_asset = Asset::new(
        AssetId::of(fee_asset_id.clone(), authority_id.clone()),
        Quantity::from(100_u32),
    );
    let mut world = World::with_assets([domain], [authority], [fee_definition], [fee_asset], []);
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    install_account_alias_policy(&mut world, &authority_id, &fee_asset_id);
    install_universal_parent_lease(&mut world, &authority_id);
    world.account_permissions_mut_for_testing().insert(
        authority_id.clone(),
        BTreeSet::from([Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        })]),
    );
    let chain_id = iroha_data_model::ChainId::from("onboarding-test-chain");
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        world,
        kura.clone(),
        query,
        chain_id.clone(),
        network_id,
    ));
    let nexus = state.nexus_snapshot();
    let lane_manifests = Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &nexus.registry,
    ));
    state.install_lane_manifests(&lane_manifests);
    let seed_tx = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "onboarding anchor".to_owned())])
    .sign(authority_key_pair.private_key());
    let leader = checked_key_pair(
        0xD2,
        Algorithm::BlsNormal,
        "derive onboarding block leader fixture",
    );
    let unverified = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
        seed_tx,
    ))])
    .chain(0, state.view().latest_block().as_deref())
    .sign(leader.private_key())
    .unpack(|_| {});
    let mut state_block = state.block(unverified.header());
    state_block.chain_id = chain_id.clone();
    let valid = unverified
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    iroha_torii::test_utils::finalize_committed_block(&state, state_block, committed);
    cfg.torii.account_onboarding = Some(iroha_config::parameters::actual::AccountOnboarding {
        authority: authority_id,
        private_key_file: ONBOARDING_SIGNER_PATH.into(),
        signer: authority_key_pair,
        credentials: vec![
            iroha_config::parameters::actual::AccountOnboardingCredential {
                id: "local-test".parse().expect("credential id"),
                scope:
                    iroha_config::parameters::actual::AccountOnboardingCredentialScope::Dataspace(
                        "universal".parse().expect("universal dataspace name"),
                    ),
                token_hash: *blake3::hash(ONBOARDING_API_TOKEN.as_bytes()).as_bytes(),
            },
            iroha_config::parameters::actual::AccountOnboardingCredential {
                id: "wrong-scope-test".parse().expect("credential id"),
                scope: iroha_config::parameters::actual::AccountOnboardingCredentialScope::Domain(
                    DomainId::try_new("restricted", "universal")
                        .expect("restricted fixture domain"),
                ),
                token_hash: *blake3::hash(ONBOARDING_WRONG_SCOPE_API_TOKEN.as_bytes()).as_bytes(),
            },
        ],
        additional_permissions: Vec::new(),
        fee_sponsor_program_id: None,
        lease_term_years: NonZeroU8::new(1).expect("non-zero lease term"),
        auto_renew: None,
    });
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender,
    ));
    queue.install_lane_manifests_with_state(&lane_manifests, &state);
    // Embedded SoraFS checkpoint stores intentionally fail fast when any writer is opening in
    // this process. Keep distinct fixture roots, but serialize only their short initialization.
    let init_guard = ONBOARDING_TORII_INIT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let torii = fixtures::ToriiHarness::new(
        &cfg,
        chain_id.clone(),
        network_id,
        &kura,
        &state,
        &queue,
        &local_peer_id,
        tokio::sync::broadcast::channel(1).0,
        iroha_config::parameters::actual::TelemetryProfile::Operator,
    );
    drop(init_guard);
    OnboardingTestContext {
        app: torii.router(),
        state,
        queue,
        chain_id,
        _data_dir: data_dir,
    }
}
fn distinct_onboarding_network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([seed])))
}
fn onboarding_plan_request(alias: &str, account_id: &AccountId) -> norito::json::Value {
    json_object(vec![
        json_entry("version", 1_u64),
        json_entry("alias", alias),
        json_entry("account_id", account_id.to_string()),
        json_entry("permissions", Vec::<String>::new()),
    ])
}
fn onboarding_current_state_request(alias: &str, account_id: &AccountId) -> norito::json::Value {
    json_object(vec![
        json_entry("version", 1_u64),
        json_entry("account_id", account_id.to_string()),
        json_entry("alias", alias),
    ])
}
fn mutation_binding(kind: &str, expires_at_unix_ms: u64) -> norito::json::Value {
    json_object(vec![
        json_entry("schema", "iroha.taira.public-reset.mutation-binding.v1"),
        json_entry("authorization_sha256", "11".repeat(32)),
        json_entry("authorization_nonce", "reset_nonce_00000000000000000000"),
        json_entry("kind", kind),
        json_entry("phase", format!("prepare_{kind}")),
        json_entry("idempotency_key", "22".repeat(32)),
        json_entry("execution_expires_at_unix_ms", expires_at_unix_ms),
    ])
}
fn onboarding_prepare_request(receipt: norito::json::Value) -> norito::json::Value {
    onboarding_prepare_request_with_expiry(receipt, u64::MAX)
}
fn onboarding_prepare_request_with_expiry(
    receipt: norito::json::Value,
    expires_at_unix_ms: u64,
) -> norito::json::Value {
    json_object(vec![
        json_entry("schema", "iroha.accounts.onboard.prepare.v1"),
        json_entry(
            "binding",
            mutation_binding("onboarding", expires_at_unix_ms),
        ),
        json_entry("receipt", receipt),
    ])
}
fn onboarding_http_request(
    path: &str,
    payload: &norito::json::Value,
    token: Option<&str>,
) -> Request<Body> {
    let body = norito::json::to_json(payload).expect("serialize onboarding request");
    assert!(!body.contains(ONBOARDING_API_TOKEN));
    assert!(!body.contains("private_key"));
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json");
    if let Some(token) = token {
        builder = builder.header("x-iroha-onboarding-token", token);
    }
    let mut request = builder.body(Body::from(body)).expect("onboarding request");
    request
        .extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));
    request
}
async fn send_onboarding_request(
    app: &axum::Router,
    path: &str,
    payload: &norito::json::Value,
) -> JsonResponse {
    send_onboarding_request_with_token(app, path, payload, ONBOARDING_API_TOKEN).await
}
async fn send_onboarding_request_with_token(
    app: &axum::Router,
    path: &str,
    payload: &norito::json::Value,
    token: &str,
) -> JsonResponse {
    let response = fixtures::request(app, onboarding_http_request(path, payload, Some(token)))
        .await
        .expect("onboarding route response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read onboarding response");
    let raw_body = String::from_utf8(bytes.to_vec()).expect("onboarding response is UTF-8 JSON");
    let payload = norito::json::from_str(&raw_body)
        .unwrap_or_else(|error| panic!("decode onboarding response: {error}; body={raw_body}"));
    JsonResponse {
        status,
        raw_body,
        payload,
    }
}
fn response_field<'a>(payload: &'a norito::json::Value, field: &str) -> &'a str {
    payload
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("response is missing string field `{field}`: {payload:?}"))
}
fn disposition_kind(payload: &norito::json::Value) -> &str {
    payload
        .as_object()
        .and_then(|object| object.get("disposition"))
        .and_then(norito::json::Value::as_object)
        .and_then(|disposition| disposition.get("kind"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("response is missing a typed disposition: {payload:?}"))
}
fn plan_disposition_kind(receipt: &norito::json::Value) -> &str {
    receipt
        .as_object()
        .and_then(|receipt| receipt.get("body"))
        .and_then(norito::json::Value::as_object)
        .and_then(|body| body.get("resource"))
        .and_then(norito::json::Value::as_object)
        .and_then(|resource| resource.get("disposition"))
        .and_then(norito::json::Value::as_object)
        .and_then(|disposition| disposition.get("kind"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("plan is missing a typed disposition: {receipt:?}"))
}
fn mutate_onboarding_receipt_body(
    receipt: &norito::json::Value,
    mutate: impl FnOnce(&mut norito::json::Map),
) -> norito::json::Value {
    let mut mutated = receipt.clone();
    let body = mutated
        .as_object_mut()
        .and_then(|receipt| receipt.get_mut("body"))
        .and_then(norito::json::Value::as_object_mut)
        .expect("onboarding plan body object");
    mutate(body);
    mutated
}
fn assert_secret_free(response: &JsonResponse) {
    for forbidden in [
        ONBOARDING_API_TOKEN,
        ONBOARDING_SIGNER_PATH,
        "private_key",
        "private_key_file",
    ] {
        assert!(
            !response.raw_body.contains(forbidden),
            "onboarding response leaked `{forbidden}`: {}",
            response.raw_body
        );
    }
}
#[test]
fn sponsored_onboarding_catalog_contains_plan_prepare_submit_and_readiness() {
    for route in [
        ACCOUNTS_ONBOARD_PLAN_POST,
        ACCOUNTS_ONBOARD_PREPARE_POST,
        ACCOUNTS_ONBOARD_POST,
        ACCOUNTS_ONBOARDING_READINESS_GET,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OnboardingToken,
            "{} must use the dedicated onboarding credential",
            route.stable_route_id()
        );
    }
    assert_eq!(
        ACCOUNTS_ONBOARDING_CURRENT_STATE_POST.authentication(),
        AuthenticationPolicy::ToriiDefault,
        "the atomic current-state read must use ordinary conditional Torii authentication"
    );
    for removed in [
        "/v1/accounts/onboard/multisig",
        "/v1/accounts/onboard/renew",
        "/v1/accounts/onboard/auto-renew",
    ] {
        assert!(
            CATALOGED_ROUTES.iter().all(|route| route.path() != removed),
            "removed onboarding route must not be cataloged: {removed}"
        );
    }
}
#[tokio::test]
async fn sponsored_onboarding_receipt_binds_exact_network_and_active_signer() {
    let local_network = distinct_onboarding_network_id(0xA1);
    let foreign_network = distinct_onboarding_network_id(0xA2);
    let origin = build_onboarding_test_context_with(local_network, 0xD1);
    let foreign_genesis = build_onboarding_test_context_with(foreign_network, 0xD1);
    let rotated_signer = build_onboarding_test_context_with(local_network, 0xE1);
    assert_eq!(origin.chain_id, foreign_genesis.chain_id);
    assert_eq!(origin.chain_id, rotated_signer.chain_id);
    let target = AccountId::new(
        checked_key_pair(
            0xD3,
            Algorithm::Ed25519,
            "derive cross-network onboarding target fixture",
        )
        .public_key()
        .clone(),
    );
    let plan = send_onboarding_request(
        &origin.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request("networkbound@universal", &target),
    )
    .await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);
    let body = plan
        .payload
        .as_object()
        .and_then(|receipt| receipt.get("body"))
        .and_then(norito::json::Value::as_object)
        .expect("onboarding plan body");
    let receipt_network: NetworkId =
        norito::json::from_value(body.get("network_id").expect("receipt network id").clone())
            .expect("canonical receipt network id");
    assert_eq!(receipt_network, local_network);
    for retired in ["chain", "chainId", "chain_id"] {
        assert!(!body.contains_key(retired));
    }
    let prepare = onboarding_prepare_request(plan.payload.clone());
    for (context, replay) in [
        (&foreign_genesis, "same-label foreign-genesis"),
        (&rotated_signer, "retired onboarding signer"),
    ] {
        let response =
            send_onboarding_request(&context.app, "/v1/accounts/onboard/prepare", &prepare).await;
        assert_eq!(
            response.status,
            StatusCode::CONFLICT,
            "{replay} replay escaped: {}",
            response.raw_body
        );
        assert_eq!(response_field(&response.payload, "code"), "conflict");
        assert_eq!(context.queue.active_len(), 0);
    }
    origin.shutdown().await;
    foreign_genesis.shutdown().await;
    rotated_signer.shutdown().await;
}
#[tokio::test]
async fn sponsored_onboarding_receipt_rejects_genesis_and_retired_network_keys() {
    let context = build_onboarding_test_context();
    let target = AccountId::new(
        checked_key_pair(
            0xD7,
            Algorithm::Ed25519,
            "derive onboarding receipt schema target fixture",
        )
        .public_key()
        .clone(),
    );
    let plan = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request("schemabound@universal", &target),
    )
    .await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);
    let genesis = mutate_onboarding_receipt_body(&plan.payload, |body| {
        body.insert(
            "network_id".to_owned(),
            norito::json::Value::String("genesis".to_owned()),
        );
    });
    let response = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(genesis),
    )
    .await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.raw_body
    );
    for retired in ["chain", "chainId", "chain_id"] {
        for keep_network_id in [false, true] {
            let mutated = mutate_onboarding_receipt_body(&plan.payload, |body| {
                if !keep_network_id {
                    body.remove("network_id");
                }
                body.insert(
                    retired.to_owned(),
                    norito::json::Value::String(context.chain_id.to_string()),
                );
            });
            let response = send_onboarding_request(
                &context.app,
                "/v1/accounts/onboard/prepare",
                &onboarding_prepare_request(mutated),
            )
            .await;
            assert_eq!(
                response.status,
                StatusCode::BAD_REQUEST,
                "retired key {retired} (keep_network_id={keep_network_id}) escaped: {}",
                response.raw_body
            );
        }
    }
    assert_eq!(context.queue.active_len(), 0);
    context.shutdown().await;
}

#[tokio::test]
async fn sponsored_onboarding_submit_rejects_old_and_tampered_envelopes() {
    let context = build_onboarding_test_context();
    let target = AccountId::new(
        checked_key_pair(0xD8, Algorithm::Ed25519, "derive tamper target")
            .public_key()
            .clone(),
    );
    let plan = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request("tamperproof@universal", &target),
    )
    .await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);

    let old = json_object(vec![json_entry("receipt", plan.payload.clone())]);
    let rejected = send_onboarding_request(&context.app, "/v1/accounts/onboard", &old).await;
    assert_eq!(
        rejected.status,
        StatusCode::BAD_REQUEST,
        "{}",
        rejected.raw_body
    );

    let prepared = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(plan.payload),
    )
    .await;
    assert_eq!(prepared.status, StatusCode::OK, "{}", prepared.raw_body);
    for (field, replacement) in [
        ("transaction_hash_hex", "00".repeat(32)),
        ("signed_transaction_wire_sha256", "33".repeat(32)),
        ("signed_transaction_wire_hex", "00".repeat(8)),
    ] {
        let mut tampered = prepared.payload.clone();
        tampered
            .as_object_mut()
            .expect("prepared object")
            .insert(field.to_owned(), norito::json::Value::String(replacement));
        let rejected =
            send_onboarding_request(&context.app, "/v1/accounts/onboard", &tampered).await;
        assert_eq!(
            rejected.status,
            StatusCode::BAD_REQUEST,
            "tampered {field} escaped: {}",
            rejected.raw_body
        );
    }
    let mut binding_tampered = prepared.payload;
    let substituted_signature = binding_tampered
        .as_object()
        .and_then(|body| body.get("receipt"))
        .and_then(norito::json::Value::as_object)
        .and_then(|receipt| receipt.get("signature"))
        .cloned()
        .expect("receipt signature");
    let mut signature_tampered = binding_tampered.clone();
    signature_tampered
        .as_object_mut()
        .expect("prepared object")
        .insert("server_signature".to_owned(), substituted_signature);
    let rejected =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &signature_tampered).await;
    assert_eq!(
        rejected.status,
        StatusCode::BAD_REQUEST,
        "{}",
        rejected.raw_body
    );

    binding_tampered
        .as_object_mut()
        .and_then(|body| body.get_mut("binding"))
        .and_then(norito::json::Value::as_object_mut)
        .expect("prepared binding")
        .insert(
            "phase".to_owned(),
            norito::json::Value::String("substituted_phase".to_owned()),
        );
    let rejected =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &binding_tampered).await;
    assert_eq!(
        rejected.status,
        StatusCode::BAD_REQUEST,
        "{}",
        rejected.raw_body
    );
    assert_eq!(context.queue.active_len(), 0);
    context.shutdown().await;
}

#[tokio::test]
async fn sponsored_onboarding_prepare_is_non_mutating_and_exact_submit_is_replay_safe() {
    let context = build_onboarding_test_context();
    let target_key_pair =
        checked_key_pair(0xD3, Algorithm::Ed25519, "derive onboarding target fixture");
    let target_id = AccountId::new(target_key_pair.public_key().clone());
    let alias = "replayuser@universal";
    let plan_request = onboarding_plan_request(alias, &target_id);
    let plan =
        send_onboarding_request(&context.app, "/v1/accounts/onboard/plan", &plan_request).await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);
    assert_eq!(plan_disposition_kind(&plan.payload), "create");
    assert_secret_free(&plan);
    let candidate_a = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(plan.payload.clone()),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let prepared = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(plan.payload.clone()),
    )
    .await;
    assert_eq!(
        candidate_a.status,
        StatusCode::OK,
        "{}",
        candidate_a.raw_body
    );
    assert_eq!(prepared.status, StatusCode::OK, "{}", prepared.raw_body);
    assert_eq!(
        response_field(&prepared.payload, "schema"),
        "iroha.taira.prepared-transaction.v1"
    );
    assert_eq!(response_field(&prepared.payload, "operation"), "onboarding");
    assert_eq!(disposition_kind(&prepared.payload), "create");
    assert_eq!(
        context.queue.active_len(),
        0,
        "prepare must not mutate queue"
    );
    assert!(context.state.view().world().account(&target_id).is_err());
    let selected_hash = response_field(&prepared.payload, "transaction_hash_hex").to_owned();
    let unselected_hash = response_field(&candidate_a.payload, "transaction_hash_hex");
    assert_ne!(
        selected_hash, unselected_hash,
        "distinct prepare calls must identify distinct exact candidates"
    );
    assert_secret_free(&prepared);

    let submitted =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &prepared.payload).await;
    assert_eq!(
        submitted.status,
        StatusCode::ACCEPTED,
        "{}",
        submitted.raw_body
    );
    assert_eq!(response_field(&submitted.payload, "outcome"), "Pending");
    assert_eq!(
        response_field(&submitted.payload, "transaction_hash_hex"),
        selected_hash
    );
    assert_eq!(context.queue.active_len(), 1);

    let response_loss_replay =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &prepared.payload).await;
    assert_eq!(
        response_loss_replay.status,
        StatusCode::OK,
        "{}",
        response_loss_replay.raw_body
    );
    assert_eq!(
        response_field(&response_loss_replay.payload, "outcome"),
        "Pending"
    );
    assert_eq!(
        context.queue.active_len(),
        1,
        "replay must not enqueue twice"
    );
    let wrong_scope_replay = send_onboarding_request_with_token(
        &context.app,
        "/v1/accounts/onboard",
        &prepared.payload,
        ONBOARDING_WRONG_SCOPE_API_TOKEN,
    )
    .await;
    assert_eq!(
        wrong_scope_replay.status,
        StatusCode::FORBIDDEN,
        "a known hash must not bypass receipt credential scope: {}",
        wrong_scope_replay.raw_body
    );
    assert_eq!(context.queue.active_len(), 1);

    let expected_height = u64::try_from(context.state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    assert_eq!(
        iroha_torii::test_utils::apply_queued_in_one_block(
            &context.state,
            &context.queue,
            &context.chain_id,
            expected_height,
        ),
        1,
        "exact submit must enqueue one atomic transaction"
    );
    let applied_replay =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &prepared.payload).await;
    assert_eq!(
        applied_replay.status,
        StatusCode::OK,
        "{}",
        applied_replay.raw_body
    );
    assert_eq!(
        response_field(&applied_replay.payload, "outcome"),
        "Applied"
    );
    assert_eq!(context.queue.active_len(), 0);
    let proof_required = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(plan.payload.clone()),
    )
    .await;
    assert_eq!(
        proof_required.status,
        StatusCode::OK,
        "{}",
        proof_required.raw_body
    );
    assert_eq!(
        response_field(&proof_required.payload, "schema"),
        "iroha.accounts.onboard.prepare-proof-required.v1"
    );
    assert_eq!(
        response_field(&proof_required.payload, "outcome"),
        "ProofRequired"
    );
    assert_eq!(
        response_field(&proof_required.payload, "proof_kind"),
        "account_alias_current_state"
    );
    assert_eq!(disposition_kind(&proof_required.payload), "no_op");
    assert!(
        proof_required
            .payload
            .as_object()
            .is_some_and(|payload| !payload.contains_key("signed_transaction_wire_hex")),
        "a proof-required prepare result must not fabricate a transaction"
    );
    assert_eq!(context.queue.active_len(), 0);
    assert_secret_free(&proof_required);

    let current_state = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboarding/current-state",
        &onboarding_current_state_request(alias, &target_id),
    )
    .await;
    assert_eq!(
        current_state.status,
        StatusCode::OK,
        "{}",
        current_state.raw_body
    );
    let current_state: AccountOnboardingCurrentStateResponseV1 =
        norito::json::from_value(current_state.payload)
            .expect("strict atomic onboarding current-state response");
    let request = iroha_torii_shared::AccountOnboardingCurrentStateRequestV1::new(
        &target_id,
        &alias.parse().expect("canonical onboarding alias"),
    );
    let (observed_height, alias_target) = current_state
        .validate_for(&request, context.state.network_id_ref())
        .expect("snapshot response binds the exact request and network");
    assert_eq!(
        observed_height.get(),
        u64::try_from(context.state.view().height()).expect("fixture height fits u64")
    );
    assert_eq!(
        current_state.observed_block_hash,
        context
            .state
            .view()
            .latest_block_hash()
            .expect("fixture has a committed block")
    );
    assert!(current_state.account_exists);
    assert_eq!(alias_target, Some(target_id));
    context.shutdown().await;
}

#[tokio::test]
async fn expired_onboarding_envelope_only_reconciles_an_already_known_hash() {
    let context = build_onboarding_test_context();
    let target = AccountId::new(
        checked_key_pair(0xD9, Algorithm::Ed25519, "derive expiry target")
            .public_key()
            .clone(),
    );
    let plan = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request("expiryuser@universal", &target),
    )
    .await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);
    let now_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_millis()
        .try_into()
        .expect("millisecond clock fits u64");
    let deadline = now_ms.saturating_add(1_000);
    let request = onboarding_prepare_request_with_expiry(plan.payload, deadline);
    let known =
        send_onboarding_request(&context.app, "/v1/accounts/onboard/prepare", &request).await;
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let unknown =
        send_onboarding_request(&context.app, "/v1/accounts/onboard/prepare", &request).await;
    for prepared in [&known, &unknown] {
        assert_eq!(prepared.status, StatusCode::OK, "{}", prepared.raw_body);
    }
    let submitted =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &known.payload).await;
    assert_eq!(
        submitted.status,
        StatusCode::ACCEPTED,
        "{}",
        submitted.raw_body
    );
    tokio::time::sleep(std::time::Duration::from_millis(1_050)).await;
    let reconciled =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &known.payload).await;
    assert_eq!(reconciled.status, StatusCode::OK, "{}", reconciled.raw_body);
    assert_eq!(response_field(&reconciled.payload, "outcome"), "Pending");
    let expired_unknown =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &unknown.payload).await;
    assert_eq!(
        expired_unknown.status,
        StatusCode::BAD_REQUEST,
        "{}",
        expired_unknown.raw_body
    );
    assert_eq!(context.queue.active_len(), 1);
    context.shutdown().await;
}

#[tokio::test]
async fn sponsored_onboarding_stale_create_receipt_returns_redacted_conflict() {
    let context = build_onboarding_test_context();
    let original_target = AccountId::new(
        checked_key_pair(
            0xD4,
            Algorithm::Ed25519,
            "derive original onboarding target fixture",
        )
        .public_key()
        .clone(),
    );
    let conflicting_target = AccountId::new(
        checked_key_pair(
            0xD5,
            Algorithm::Ed25519,
            "derive conflicting onboarding target fixture",
        )
        .public_key()
        .clone(),
    );
    let alias = "driftuser@universal";
    let original_plan = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request(alias, &original_target),
    )
    .await;
    let conflicting_plan = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/plan",
        &onboarding_plan_request(alias, &conflicting_target),
    )
    .await;
    for plan in [&original_plan, &conflicting_plan] {
        assert_eq!(plan.status, StatusCode::OK, "{}", plan.raw_body);
        assert_eq!(plan_disposition_kind(&plan.payload), "create");
        assert_secret_free(plan);
    }
    let conflicting_prepared = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(conflicting_plan.payload.clone()),
    )
    .await;
    assert_eq!(
        conflicting_prepared.status,
        StatusCode::OK,
        "{}",
        conflicting_prepared.raw_body
    );
    let conflicting_submit = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard",
        &conflicting_prepared.payload,
    )
    .await;
    assert_eq!(
        conflicting_submit.status,
        StatusCode::ACCEPTED,
        "{}",
        conflicting_submit.raw_body
    );
    let expected_height = u64::try_from(context.state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    assert_eq!(
        iroha_torii::test_utils::apply_queued_in_one_block(
            &context.state,
            &context.queue,
            &context.chain_id,
            expected_height,
        ),
        1,
        "the racing create must commit before stale receipt revalidation"
    );
    let stale = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard/prepare",
        &onboarding_prepare_request(original_plan.payload.clone()),
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{}", stale.raw_body);
    assert_eq!(response_field(&stale.payload, "code"), "conflict");
    assert_eq!(context.queue.active_len(), 0);
    assert_secret_free(&stale);
    for removed in [
        "/v1/accounts/onboard/multisig",
        "/v1/accounts/onboard/renew",
        "/v1/accounts/onboard/auto-renew",
    ] {
        let empty_body = norito::json::Value::Object(norito::json::Map::new());
        let response = fixtures::request(
            &context.app,
            onboarding_http_request(removed, &empty_body, Some(ONBOARDING_API_TOKEN)),
        )
        .await
        .expect("removed onboarding route response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "removed onboarding route must remain absent: {removed}"
        );
    }
    context.shutdown().await;
}
