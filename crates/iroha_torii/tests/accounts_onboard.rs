//! Sponsored account-onboarding clean-break contract tests.

#![cfg(feature = "app_api")]

use std::{borrow::Cow, collections::BTreeSet, num::NonZeroU8, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    extract::connect_info::ConnectInfo,
    http::{Request, header},
};
use http::StatusCode;
use iroha_core::{
    block::BlockBuilder,
    governance::manifest::LaneManifestRegistry,
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute as _,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::{AcceptedTransaction, TransactionBuilder},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId, Registrable,
    account::{AccountAddress, AccountId},
    alias_setup::{
        AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
        AliasIntentV1, ResolvedAccountAliasV1,
    },
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    isi::Revoke,
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
use iroha_torii::{Torii, json_entry, json_object};
use iroha_torii_shared::route_catalog::{
    AuthenticationPolicy, CATALOGED_ROUTES,
    application_api::{
        ACCOUNTS_ONBOARD_PLAN_POST, ACCOUNTS_ONBOARD_POST, ACCOUNTS_ONBOARDING_READINESS_GET,
    },
};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

const ONBOARDING_API_TOKEN: &str = "torii-onboarding-test-token-32-bytes";
const ONBOARDING_SIGNER_PATH: &str = "/runtime-only/onboarding-test-signer.key";

struct OnboardingTestContext {
    app: axum::Router,
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: iroha_data_model::ChainId,
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
    let (kiso, _child) = KisoHandle::start(cfg.clone());
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

    cfg.torii.account_onboarding =
        Some(iroha_config::parameters::actual::AccountOnboarding {
            authority: authority_id,
            private_key_file: ONBOARDING_SIGNER_PATH.into(),
            signer: authority_key_pair,
            credentials: vec![iroha_config::parameters::actual::AccountOnboardingCredential {
                id: "local-test".parse().expect("credential id"),
                scope:
                    iroha_config::parameters::actual::AccountOnboardingCredentialScope::Dataspace(
                        "universal".parse().expect("universal dataspace name"),
                    ),
                token_hash: *blake3::hash(ONBOARDING_API_TOKEN.as_bytes()).as_bytes(),
            }],
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
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) =
            iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            ts,
            false,
        )
        .0
    };
    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                chain_id.clone(),
                network_id,
                kiso,
                cfg.torii.clone(),
                queue.clone(),
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state.clone(),
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                chain_id.clone(),
                network_id,
                kiso,
                cfg.torii.clone(),
                queue.clone(),
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state.clone(),
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    OnboardingTestContext {
        app: torii.api_router_for_tests(),
        state,
        queue,
        chain_id,
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

fn onboarding_apply_request(receipt: norito::json::Value) -> norito::json::Value {
    json_object(vec![json_entry("receipt", receipt)])
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
    let response = app
        .clone()
        .oneshot(onboarding_http_request(
            path,
            payload,
            Some(ONBOARDING_API_TOKEN),
        ))
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

fn remove_exact_alias_permission(
    context: &OnboardingTestContext,
    alias: &str,
    account_id: &AccountId,
    account_key_pair: &KeyPair,
) -> Permission {
    let resolved = ResolvedAccountAliasV1::new(
        alias.parse::<AccountAliasName>().expect("account alias"),
        DataSpaceId::UNIVERSAL,
    );
    let intent = AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
        alias: resolved,
        target_account: account_id.clone(),
        provision: AccountProvisionV1::Create,
        role: AccountAliasRoleV1::Primary,
    });
    let permission = iroha_core::alias_setup::exact_alias_permission_bundle(&intent)[0].clone();
    let fixture_tx = TransactionBuilder::new(
        *context.state.network_id_ref(),
        account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "remove derived alias permission for repair fixture".to_owned(),
    )])
    .sign(account_key_pair.private_key());
    let leader = checked_key_pair(
        0xD6,
        Algorithm::BlsNormal,
        "derive repair fixture block leader",
    );
    let unverified = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
        fixture_tx,
    ))])
    .chain(0, context.state.view().latest_block().as_deref())
    .sign(leader.private_key())
    .unpack(|_| {});
    let mut state_block = context.state.block(unverified.header());
    state_block.chain_id = context.chain_id.clone();
    {
        let mut transaction = state_block.transaction();
        Revoke::account_permission(permission.clone(), account_id.clone())
            .execute(account_id, &mut transaction)
            .expect("remove one derived alias permission");
        transaction.apply();
    }
    let valid = unverified
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    iroha_torii::test_utils::finalize_committed_block(&context.state, state_block, committed);
    permission
}

#[test]
fn sponsored_onboarding_catalog_contains_only_plan_apply_and_readiness() {
    for route in [
        ACCOUNTS_ONBOARD_PLAN_POST,
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
    let local_network_literal = local_network.to_string();
    let body = plan
        .payload
        .as_object()
        .and_then(|receipt| receipt.get("body"))
        .and_then(norito::json::Value::as_object)
        .expect("onboarding plan body");
    assert_eq!(
        body.get("network_id").and_then(norito::json::Value::as_str),
        Some(local_network_literal.as_str())
    );
    for retired in ["chain", "chainId", "chain_id"] {
        assert!(!body.contains_key(retired));
    }

    let apply = onboarding_apply_request(plan.payload.clone());
    for (context, replay) in [
        (&foreign_genesis, "same-label foreign-genesis"),
        (&rotated_signer, "retired onboarding signer"),
    ] {
        let response = send_onboarding_request(&context.app, "/v1/accounts/onboard", &apply).await;
        assert_eq!(
            response.status,
            StatusCode::CONFLICT,
            "{replay} replay escaped: {}",
            response.raw_body
        );
        assert_eq!(
            response_field(&response.payload, "code"),
            "alias.onboarding.receipt_context_mismatch"
        );
        assert_eq!(context.queue.active_len(), 0);
    }
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
        "/v1/accounts/onboard",
        &onboarding_apply_request(genesis),
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
                "/v1/accounts/onboard",
                &onboarding_apply_request(mutated),
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
}

#[tokio::test]
async fn sponsored_onboarding_create_replay_and_repair_use_the_real_handlers() {
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

    let apply_request = onboarding_apply_request(plan.payload.clone());
    let created =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &apply_request).await;
    assert_eq!(created.status, StatusCode::ACCEPTED, "{}", created.raw_body);
    assert_eq!(response_field(&created.payload, "status"), "Queued");
    assert_eq!(disposition_kind(&created.payload), "create");
    assert!(
        created
            .payload
            .as_object()
            .and_then(|payload| payload.get("tx_hash_hex"))
            .and_then(norito::json::Value::as_str)
            .is_some(),
        "create response must identify its one queued transaction"
    );
    assert_secret_free(&created);

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
        "create apply must submit exactly one atomic transaction"
    );

    let replay =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &apply_request).await;
    assert_eq!(replay.status, StatusCode::OK, "{}", replay.raw_body);
    assert_eq!(response_field(&replay.payload, "status"), "Unchanged");
    assert_eq!(disposition_kind(&replay.payload), "no_op");
    assert!(
        replay
            .payload
            .as_object()
            .is_some_and(|payload| !payload.contains_key("tx_hash_hex")),
        "an exact replay must not report or queue another transaction"
    );
    assert_eq!(context.queue.active_len(), 0);
    assert_secret_free(&replay);

    let removed_permission =
        remove_exact_alias_permission(&context, alias, &target_id, &target_key_pair);
    let repair_plan =
        send_onboarding_request(&context.app, "/v1/accounts/onboard/plan", &plan_request).await;
    assert_eq!(
        repair_plan.status,
        StatusCode::OK,
        "{}",
        repair_plan.raw_body
    );
    assert_eq!(plan_disposition_kind(&repair_plan.payload), "repair");
    assert_secret_free(&repair_plan);

    let repair_request = onboarding_apply_request(repair_plan.payload.clone());
    let repaired =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &repair_request).await;
    assert_eq!(
        repaired.status,
        StatusCode::ACCEPTED,
        "{}",
        repaired.raw_body
    );
    assert_eq!(response_field(&repaired.payload, "status"), "Repaired");
    assert_eq!(disposition_kind(&repaired.payload), "repair");
    assert_eq!(context.queue.active_len(), 1);
    assert_secret_free(&repaired);

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
        "repair apply must submit exactly one atomic transaction"
    );
    assert!(
        context
            .state
            .view()
            .world()
            .account_contains_inherent_permission(&target_id, &removed_permission),
        "repair must restore the exact derived alias permission"
    );

    let repair_replay =
        send_onboarding_request(&context.app, "/v1/accounts/onboard", &repair_request).await;
    assert_eq!(
        repair_replay.status,
        StatusCode::OK,
        "{}",
        repair_replay.raw_body
    );
    assert_eq!(
        response_field(&repair_replay.payload, "status"),
        "Unchanged"
    );
    assert_eq!(disposition_kind(&repair_replay.payload), "no_op");
    assert!(
        repair_replay
            .payload
            .as_object()
            .is_some_and(|payload| !payload.contains_key("tx_hash_hex")),
        "an exact repair replay must not report or queue another transaction"
    );
    assert_eq!(context.queue.active_len(), 0);
    assert_secret_free(&repair_replay);
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

    let conflicting_apply = send_onboarding_request(
        &context.app,
        "/v1/accounts/onboard",
        &onboarding_apply_request(conflicting_plan.payload.clone()),
    )
    .await;
    assert_eq!(
        conflicting_apply.status,
        StatusCode::ACCEPTED,
        "{}",
        conflicting_apply.raw_body
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
        "/v1/accounts/onboard",
        &onboarding_apply_request(original_plan.payload.clone()),
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{}", stale.raw_body);
    assert_eq!(
        response_field(&stale.payload, "code"),
        "alias.owner.conflict"
    );
    assert_eq!(context.queue.active_len(), 0);
    assert_secret_free(&stale);

    for removed in [
        "/v1/accounts/onboard/multisig",
        "/v1/accounts/onboard/renew",
        "/v1/accounts/onboard/auto-renew",
    ] {
        let empty_body = norito::json::Value::Object(norito::json::Map::new());
        let response = context
            .app
            .clone()
            .oneshot(onboarding_http_request(
                removed,
                &empty_body,
                Some(ONBOARDING_API_TOKEN),
            ))
            .await
            .expect("removed onboarding route response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "removed onboarding route must remain absent: {removed}"
        );
    }
}
