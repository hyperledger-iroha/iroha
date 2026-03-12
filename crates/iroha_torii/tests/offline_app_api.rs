#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for offline app API write endpoints.
#![cfg(feature = "app_api")]

mod offline_balance_proof_utils;

use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::{Queue as QueueConfig, ToriiOfflineIssuer};
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    account::{Account, AccountId},
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::Domain,
    isi::{
        Mint,
        offline::{RegisterOfflineAllowance, SubmitOfflineToOnlineTransfer},
    },
    metadata::Metadata,
    offline::{
        ANDROID_PROVISIONED_APP_ID_KEY, AndroidMarkerKeyProof, AndroidProvisionedProof,
        AppleAppAttestProof, OFFLINE_ASSET_ENABLED_METADATA_KEY,
        OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY, OFFLINE_LINEAGE_EPOCH_KEY,
        OFFLINE_LINEAGE_SCOPE_KEY, OfflineAllowanceCommitment, OfflineBuildClaim,
        OfflineBuildClaimPlatform, OfflinePlatformProof, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineWalletCertificate, OfflineWalletPolicy,
        compute_receipts_root,
    },
    transaction::executable::Executable,
};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use nonzero_ext::nonzero;
use norito::json::{self, Value};
use offline_balance_proof_utils::{build_balance_proof_for_allowance, scalar_bytes};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

struct Harness {
    app: Router,
    fixtures: Fixtures,
    state: Arc<State>,
    queue: Arc<Queue>,
}

struct Fixtures {
    controller: AccountId,
    controller_keys: KeyPair,
    receiver: AccountId,
    receiver_keys: KeyPair,
    certificate: OfflineWalletCertificate,
    receipt: OfflineSpendReceipt,
    transfer: OfflineToOnlineTransfer,
}

struct AppAttestRealDeviceSettlementReplayFixture {
    chain_id: ChainId,
    authority: AccountId,
    private_key: String,
    certificate: OfflineWalletCertificate,
    transfer: OfflineToOnlineTransfer,
    expect_non_strict_ok: bool,
    expect_non_strict_reject_code: Option<String>,
    expect_strict_ok: bool,
    expect_strict_reject_code: Option<String>,
    expect_non_strict_final_status: Option<String>,
    expect_non_strict_final_reject_code: Option<String>,
    expect_strict_final_status: Option<String>,
    expect_strict_final_reject_code: Option<String>,
}

struct ReplaySubmissionOutcome {
    response: axum::response::Response,
    queued_tx_hash_hex: Option<String>,
    pipeline_status_response: Option<axum::response::Response>,
}

const IOS_TEAM_ID_KEY: &str = "ios.app_attest.team_id";
const IOS_BUNDLE_ID_KEY: &str = "ios.app_attest.bundle_id";
const IOS_ENVIRONMENT_KEY: &str = "ios.app_attest.environment";
const ANDROID_INTEGRITY_POLICY_KEY: &str = "android.integrity.policy";
const ANDROID_PACKAGE_NAMES_KEY: &str = "android.attestation.package_names";
const ANDROID_SIGNATURE_DIGESTS_KEY: &str = "android.attestation.signing_digests_sha256";
const ANDROID_PROVISIONED_DEVICE_ID_KEY: &str = "android.provisioned.device_id";
const ANDROID_PROVISIONED_INSPECTOR_KEY: &str = "android.provisioned.inspector_public_key";
const ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY: &str = "android.provisioned.manifest_schema";
const ANDROID_PROVISIONED_MANIFEST_VERSION_KEY: &str = "android.provisioned.manifest_version";
const ANDROID_PROVISIONED_MAX_AGE_KEY: &str = "android.provisioned.max_manifest_age_ms";

fn build_harness() -> Harness {
    let mut cfg = test_utils::mk_minimal_root_cfg();
    let issuer_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    cfg.torii.offline_issuer = Some(ToriiOfflineIssuer {
        operator_private_key: iroha_crypto::ExposedPrivateKey(issuer_keys.private_key().clone()),
        legacy_operator_private_keys: vec![],
        allowed_controllers: vec![],
    });
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let fixtures = build_fixtures();
    let world = world_from_fixtures(&fixtures);
    let chain_id = ChainId::from("test-chain");
    let mut state =
        State::new_with_chain_for_testing(world, Arc::clone(&kura), query, chain_id.clone());
    state.settlement.offline.skip_platform_attestation = true;
    state.settlement.offline.proof_mode =
        iroha_config::parameters::actual::OfflineProofMode::Optional;
    let state = Arc::new(state);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    Harness {
        app: torii.api_router_for_tests(),
        fixtures,
        state,
        queue,
    }
}

fn build_harness_without_offline_issuer() -> Harness {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let fixtures = build_fixtures();
    let world = world_from_fixtures(&fixtures);
    let chain_id = ChainId::from("test-chain");
    let mut state =
        State::new_with_chain_for_testing(world, Arc::clone(&kura), query, chain_id.clone());
    state.settlement.offline.skip_platform_attestation = true;
    state.settlement.offline.proof_mode =
        iroha_config::parameters::actual::OfflineProofMode::Optional;
    let state = Arc::new(state);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    Harness {
        app: torii.api_router_for_tests(),
        fixtures,
        state,
        queue,
    }
}

fn build_harness_without_offline_issuer_skip_build_claim_verification() -> Harness {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let fixtures = build_fixtures();
    let world = world_from_fixtures(&fixtures);
    let chain_id = ChainId::from("test-chain");
    let mut state =
        State::new_with_chain_for_testing(world, Arc::clone(&kura), query, chain_id.clone());
    state.settlement.offline.skip_platform_attestation = true;
    state.settlement.offline.proof_mode =
        iroha_config::parameters::actual::OfflineProofMode::Optional;
    state.settlement.offline.skip_build_claim_verification = true;
    let state = Arc::new(state);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    Harness {
        app: torii.api_router_for_tests(),
        fixtures,
        state,
        queue,
    }
}

fn build_harness_with_issuer(primary_seed: u8, legacy_seeds: &[u8]) -> Harness {
    let mut cfg = test_utils::mk_minimal_root_cfg();
    let primary_keys = KeyPair::from_seed(vec![primary_seed; 32], Algorithm::Ed25519);
    let legacy_operator_private_keys = legacy_seeds
        .iter()
        .map(|seed| {
            let legacy_keys = KeyPair::from_seed(vec![*seed; 32], Algorithm::Ed25519);
            iroha_crypto::ExposedPrivateKey(legacy_keys.private_key().clone())
        })
        .collect();
    cfg.torii.offline_issuer = Some(ToriiOfflineIssuer {
        operator_private_key: iroha_crypto::ExposedPrivateKey(primary_keys.private_key().clone()),
        legacy_operator_private_keys,
        allowed_controllers: vec![],
    });
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let fixtures = build_fixtures();
    let world = world_from_fixtures(&fixtures);
    let chain_id = ChainId::from("test-chain");
    let mut state =
        State::new_with_chain_for_testing(world, Arc::clone(&kura), query, chain_id.clone());
    state.settlement.offline.skip_platform_attestation = true;
    state.settlement.offline.proof_mode =
        iroha_config::parameters::actual::OfflineProofMode::Optional;
    let state = Arc::new(state);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    Harness {
        app: torii.api_router_for_tests(),
        fixtures,
        state,
        queue,
    }
}

fn build_harness_for_app_attest_fixture(
    chain_id: ChainId,
    fixtures: Fixtures,
    strict_signature: bool,
) -> Harness {
    let mut cfg = test_utils::mk_minimal_root_cfg();
    let issuer_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    cfg.torii.offline_issuer = Some(ToriiOfflineIssuer {
        operator_private_key: iroha_crypto::ExposedPrivateKey(issuer_keys.private_key().clone()),
        legacy_operator_private_keys: vec![],
        allowed_controllers: vec![],
    });
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = world_from_fixtures(&fixtures);
    let mut state =
        State::new_with_chain_for_testing(world, Arc::clone(&kura), query, chain_id.clone());
    state.settlement.offline.skip_platform_attestation = false;
    state.settlement.offline.apple_app_attest_strict_signature = strict_signature;
    state.settlement.offline.skip_build_claim_verification = true;
    state.settlement.offline.proof_mode =
        iroha_config::parameters::actual::OfflineProofMode::Optional;
    let state = Arc::new(state);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        chain_id,
        kiso,
        cfg.torii.clone(),
        queue.clone(),
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        Arc::clone(&state),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    Harness {
        app: torii.api_router_for_tests(),
        fixtures,
        state,
        queue,
    }
}

fn parse_real_device_replay_fixture(path: &str) -> AppAttestRealDeviceSettlementReplayFixture {
    let fixture_bytes = fs::read(path).expect("read real-device replay fixture");
    let root: Value = json::from_slice(&fixture_bytes).expect("parse real-device replay fixture");
    let object = root
        .as_object()
        .expect("real-device replay fixture must be a JSON object");

    let chain_id = ChainId::from(
        object
            .get("chain_id")
            .and_then(Value::as_str)
            .expect("fixture.chain_id must be present"),
    );
    let authority: AccountId = json::from_value(
        object
            .get("authority")
            .cloned()
            .expect("fixture.authority must be present"),
    )
    .expect("fixture.authority must decode as AccountId");
    let private_key = object
        .get("private_key")
        .and_then(Value::as_str)
        .expect("fixture.private_key must be present")
        .to_owned();
    let certificate: OfflineWalletCertificate = json::from_value(
        object
            .get("certificate")
            .cloned()
            .expect("fixture.certificate must be present"),
    )
    .expect("fixture.certificate must decode as OfflineWalletCertificate");
    let transfer: OfflineToOnlineTransfer = json::from_value(
        object
            .get("transfer")
            .cloned()
            .expect("fixture.transfer must be present"),
    )
    .expect("fixture.transfer must decode as OfflineToOnlineTransfer");

    assert!(
        !transfer.receipts.is_empty(),
        "fixture.transfer must contain at least one receipt"
    );
    assert_eq!(
        transfer.receiver, authority,
        "fixture.authority must match fixture.transfer.receiver"
    );

    let expect_non_strict_ok = object
        .get("expect_non_strict_ok")
        .and_then(Value::as_bool)
        .expect("fixture.expect_non_strict_ok must be present");
    let expect_non_strict_reject_code = object
        .get("expect_non_strict_reject_code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expect_strict_ok = object
        .get("expect_strict_ok")
        .and_then(Value::as_bool)
        .expect("fixture.expect_strict_ok must be present");
    let expect_strict_reject_code = object
        .get("expect_strict_reject_code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expect_non_strict_final_status = object
        .get("expect_non_strict_final_status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expect_non_strict_final_reject_code = object
        .get("expect_non_strict_final_reject_code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expect_strict_final_status = object
        .get("expect_strict_final_status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let expect_strict_final_reject_code = object
        .get("expect_strict_final_reject_code")
        .and_then(Value::as_str)
        .map(str::to_owned);

    AppAttestRealDeviceSettlementReplayFixture {
        chain_id,
        authority,
        private_key,
        certificate,
        transfer,
        expect_non_strict_ok,
        expect_non_strict_reject_code,
        expect_strict_ok,
        expect_strict_reject_code,
        expect_non_strict_final_status,
        expect_non_strict_final_reject_code,
        expect_strict_final_status,
        expect_strict_final_reject_code,
    }
}

async fn submit_replay_fixture(
    fixture: &AppAttestRealDeviceSettlementReplayFixture,
    strict_signature: bool,
) -> ReplaySubmissionOutcome {
    submit_replay_fixture_with_harness(fixture, strict_signature)
        .await
        .1
}

async fn submit_replay_fixture_with_harness(
    fixture: &AppAttestRealDeviceSettlementReplayFixture,
    strict_signature: bool,
) -> (Harness, ReplaySubmissionOutcome) {
    let fixtures = Fixtures {
        controller: fixture.certificate.controller.clone(),
        controller_keys: KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519),
        receiver: fixture.authority.clone(),
        receiver_keys: KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519),
        certificate: fixture.certificate.clone(),
        receipt: fixture
            .transfer
            .receipts
            .first()
            .cloned()
            .expect("fixture.transfer contains at least one receipt"),
        transfer: fixture.transfer.clone(),
    };
    let harness =
        build_harness_for_app_attest_fixture(fixture.chain_id.clone(), fixtures, strict_signature);
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&fixture.authority).expect("fixture authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(fixture.private_key.clone()),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&fixture.transfer).expect("fixture transfer value"),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize fixture request");

    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    if response.status() == StatusCode::OK {
        let queued_tx_hash_hex = queued_settlement_transaction_hash_hex(&harness);
        let pipeline_status_response = harness
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::GET)
                    .uri(format!(
                        "/v1/pipeline/transactions/status?hash={queued_tx_hash_hex}"
                    ))
                    .body(Body::empty())
                    .expect("pipeline status request"),
            )
            .await
            .expect("pipeline status response");
        (
            harness,
            ReplaySubmissionOutcome {
                response,
                queued_tx_hash_hex: Some(queued_tx_hash_hex),
                pipeline_status_response: Some(pipeline_status_response),
            },
        )
    } else {
        (
            harness,
            ReplaySubmissionOutcome {
                response,
                queued_tx_hash_hex: None,
                pipeline_status_response: None,
            },
        )
    }
}

fn replay_settlement_block_timestamp_ms(
    fixture: &AppAttestRealDeviceSettlementReplayFixture,
) -> u64 {
    let min_timestamp = fixture
        .transfer
        .receipts
        .iter()
        .map(|receipt| receipt.issued_at_ms)
        .max()
        .unwrap_or(fixture.certificate.issued_at_ms)
        .saturating_add(1_000);
    if fixture.certificate.expires_at_ms > min_timestamp {
        min_timestamp
    } else {
        fixture.certificate.issued_at_ms.saturating_add(1)
    }
}

fn materialize_replay_settlement(
    harness: &Harness,
    fixture: &AppAttestRealDeviceSettlementReplayFixture,
) {
    let header = BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        replay_settlement_block_timestamp_ms(fixture),
        0,
    );
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: fixture.transfer.clone(),
    }
    .execute(&fixture.authority, &mut tx)
    .expect("materialize replay settlement row");
    tx.apply();
    block.commit().expect("commit replay settlement row");
}

async fn query_settlement_record_by_bundle(harness: &Harness, bundle_id_hex: &str) -> Value {
    let mut query_envelope = json::Map::new();
    query_envelope.insert(
        "filter".into(),
        eq_filter("bundle_id_hex", Value::from(bundle_id_hex.to_owned())),
    );
    query_envelope.insert(
        "sort".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("key".into(), Value::from("recorded_at_ms"));
            map.insert("order".into(), Value::from("desc"));
            map
        })]),
    );
    query_envelope.insert(
        "pagination".into(),
        Value::Object({
            let mut map = json::Map::new();
            map.insert("limit".into(), Value::from(1u64));
            map.insert("offset".into(), Value::from(0u64));
            map
        }),
    );
    query_envelope.insert("fetch_size".into(), Value::from(32u64));
    let query_body = json::to_vec(&Value::Object(query_envelope)).expect("serialize envelope");

    let query_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(query_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(query_resp.status(), StatusCode::OK);
    let query_bytes = query_resp
        .into_body()
        .collect()
        .await
        .expect("query response body")
        .to_bytes();
    let query_json: Value = json::from_slice(&query_bytes).expect("query response json");
    assert_eq!(query_json["total"].as_u64(), Some(1));
    query_json["items"][0].clone()
}

async fn assert_replay_final_settlement_status(
    harness: &Harness,
    fixture: &AppAttestRealDeviceSettlementReplayFixture,
    expected_status: &str,
    expected_reject_code: Option<&str>,
) {
    let bundle_id_hex = hex::encode(fixture.transfer.bundle_id.as_ref());
    materialize_replay_settlement(harness, fixture);
    let item = query_settlement_record_by_bundle(harness, &bundle_id_hex).await;
    assert_eq!(
        item["status"].as_str(),
        Some(expected_status),
        "unexpected final settlement item: {:?}",
        item
    );
    match expected_reject_code {
        Some(code) => {
            assert_eq!(
                item["rejection_reason"].as_str(),
                Some(code),
                "unexpected final settlement item: {:?}",
                item
            );
            assert_eq!(
                item["transfer"]["rejection_reason"].as_str(),
                Some(code),
                "unexpected final settlement item: {:?}",
                item
            );
        }
        None => {
            assert!(
                item["rejection_reason"].is_null(),
                "unexpected final settlement item: {:?}",
                item
            );
            assert!(
                item["transfer"]["rejection_reason"].is_null(),
                "unexpected final settlement item: {:?}",
                item
            );
        }
    }
}

fn world_from_fixtures(fixtures: &Fixtures) -> World {
    let domains = [Domain {
        id: fixtures
            .certificate
            .allowance
            .asset
            .definition()
            .domain()
            .clone(),
        logo: None,
        metadata: Metadata::default(),
        owned_by: fixtures.controller.clone(),
    }];

    let controller = Account {
        id: fixtures.controller.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let receiver = Account {
        id: fixtures.receiver.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let operator = Account {
        id: fixtures.certificate.operator.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };

    // `RegisterOfflineAllowance` seeding resolves the definition in order to evaluate
    // offline escrow requirements, so the test harness must include it.
    let mut asset_definition_metadata = Metadata::default();
    asset_definition_metadata.insert(
        OFFLINE_ASSET_ENABLED_METADATA_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("offline enabled metadata key"),
        true,
    );
    let asset_definition = AssetDefinition {
        id: fixtures.certificate.allowance.asset.definition().clone(),
        name: "OfflineAsset".to_owned(),
        description: None,
        alias: None,
        spec: NumericSpec::integer(),
        mintable: Default::default(),
        logo: None,
        metadata: asset_definition_metadata,
        balance_scope_policy: Default::default(),
        owned_by: fixtures.controller.clone(),
        total_quantity: Numeric::zero(),
        confidential_policy: Default::default(),
    };

    World::with(
        domains,
        [controller, receiver, operator],
        [asset_definition],
    )
}

fn build_fixtures() -> Fixtures {
    let chain_id = ChainId::from("test-chain");
    let operator_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let operator = AccountId::of(operator_keys.public_key().clone());
    let controller_keys = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let controller = AccountId::of(controller_keys.public_key().clone());
    let receiver_keys = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let receiver = AccountId::of(receiver_keys.public_key().clone());
    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let asset_definition = AssetDefinitionId::new(
        "merchants".parse().expect("domain id"),
        "xor".parse().expect("asset definition name"),
    );
    let allowance_asset = AssetId::new(asset_definition, controller.clone());
    let lineage_scope = "merchants-main-wallet";
    let ios_bundle_id = "com.example.merchants";

    let mut certificate = OfflineWalletCertificate {
        controller: controller.clone(),
        operator: operator.clone(),
        allowance: OfflineAllowanceCommitment {
            asset: allowance_asset.clone(),
            amount: Numeric::new(1_000, 0),
            commitment: vec![0xA1; 32],
        },
        spend_public_key: spend_keys.public_key().clone(),
        attestation_report: Vec::new(),
        issued_at_ms: 1_700_000_000,
        expires_at_ms: 1_800_000_000,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(1_000, 0),
            max_tx_value: Numeric::new(500, 0),
            expires_at_ms: 1_800_000_000,
        },
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata: Metadata::default(),
        verdict_id: None,
        attestation_nonce: None,
        refresh_at_ms: None,
    };
    certificate.metadata.insert(
        OFFLINE_LINEAGE_SCOPE_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("lineage scope metadata key"),
        lineage_scope,
    );
    certificate.metadata.insert(
        OFFLINE_LINEAGE_EPOCH_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("lineage epoch metadata key"),
        "1",
    );
    certificate.metadata.insert(
        OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("minimum build metadata key"),
        "1",
    );
    certificate.metadata.insert(
        IOS_TEAM_ID_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("ios team metadata key"),
        "TEAM123456",
    );
    certificate.metadata.insert(
        IOS_BUNDLE_ID_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("ios bundle metadata key"),
        ios_bundle_id,
    );
    certificate.metadata.insert(
        IOS_ENVIRONMENT_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("ios environment metadata key"),
        "production",
    );
    certificate.operator_signature = Signature::new(
        operator_keys.private_key(),
        &certificate
            .operator_signing_bytes()
            .expect("certificate signing bytes"),
    );

    let claimed_delta = Numeric::new(100, 0);
    let balance_proof = build_balance_proof_for_allowance(
        &chain_id,
        &certificate.allowance,
        &claimed_delta,
        scalar_bytes(1),
        scalar_bytes(2),
    );
    certificate
        .allowance
        .commitment
        .clone_from(&balance_proof.initial_commitment.commitment);

    let mut receipt = OfflineSpendReceipt {
        tx_id: Hash::new(b"receipt-1"),
        from: controller.clone(),
        to: receiver.clone(),
        asset: allowance_asset.clone(),
        amount: Numeric::new(100, 0),
        issued_at_ms: certificate.issued_at_ms + 100,
        invoice_id: "INV-001".into(),
        platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
            key_id: BASE64_STANDARD.encode(b"apple"),
            counter: 1,
            assertion: vec![],
            challenge_hash: Hash::new(b"challenge"),
        }),
        platform_snapshot: None,
        sender_certificate_id: certificate.certificate_id(),
        sender_signature: Signature::from_bytes(&[0; 64]),
        build_claim: None,
    };
    let mut build_claim = OfflineBuildClaim {
        claim_id: Hash::new(b"claim-1"),
        platform: OfflineBuildClaimPlatform::Apple,
        app_id: ios_bundle_id.to_owned(),
        build_number: 1,
        issued_at_ms: certificate.issued_at_ms,
        expires_at_ms: certificate.expires_at_ms,
        lineage_scope: lineage_scope.to_owned(),
        nonce: receipt.tx_id,
        operator_signature: Signature::from_bytes(&[0; 64]),
    };
    build_claim.operator_signature = Signature::new(
        operator_keys.private_key(),
        &build_claim
            .signing_bytes()
            .expect("build claim signing bytes"),
    );
    receipt.build_claim = Some(build_claim);
    receipt.sender_signature = Signature::new(
        spend_keys.private_key(),
        &receipt.signing_bytes().expect("receipt signing bytes"),
    );

    let transfer = OfflineToOnlineTransfer {
        bundle_id: Hash::new(b"bundle-1"),
        receiver: receiver.clone(),
        deposit_account: operator.clone(),
        receipts: vec![receipt.clone()],
        balance_proof,
        balance_proofs: None,
        aggregate_proof: None,
        attachments: None,
        platform_snapshot: None,
    };

    Fixtures {
        controller,
        controller_keys,
        receiver,
        receiver_keys,
        certificate,
        receipt,
        transfer,
    }
}

fn build_android_provisioned_submission(
    fixtures: &Fixtures,
) -> (OfflineWalletCertificate, OfflineToOnlineTransfer) {
    let chain_id = ChainId::from("test-chain");
    let app_id = "com.example.merchants.android";
    let inspector_keys = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    let inspector_public_key = inspector_keys.public_key().to_string();
    let mut certificate = fixtures.certificate.clone();
    certificate.metadata.insert(
        ANDROID_INTEGRITY_POLICY_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android policy metadata key"),
        "provisioned",
    );
    certificate.metadata.insert(
        ANDROID_PROVISIONED_APP_ID_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android app id metadata key"),
        app_id,
    );
    certificate.metadata.insert(
        ANDROID_PROVISIONED_INSPECTOR_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android inspector key metadata key"),
        inspector_public_key.as_str(),
    );
    certificate.metadata.insert(
        ANDROID_PROVISIONED_MANIFEST_SCHEMA_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android manifest schema metadata key"),
        "offline_provisioning_v1",
    );
    certificate.metadata.insert(
        ANDROID_PROVISIONED_MANIFEST_VERSION_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android manifest version metadata key"),
        1u64,
    );
    certificate.metadata.insert(
        ANDROID_PROVISIONED_MAX_AGE_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android max age metadata key"),
        60_000u64,
    );
    let operator_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    certificate.operator_signature = Signature::new(
        operator_keys.private_key(),
        &certificate
            .operator_signing_bytes()
            .expect("certificate signing bytes"),
    );

    let mut receipt = fixtures.receipt.clone();
    let mut device_manifest = Metadata::default();
    device_manifest.insert(
        ANDROID_PROVISIONED_DEVICE_ID_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("device id metadata key"),
        "device-android-01",
    );
    receipt.sender_certificate_id = certificate.certificate_id();
    receipt.platform_proof = OfflinePlatformProof::Provisioned(AndroidProvisionedProof {
        manifest_schema: "offline_provisioning_v1".to_owned(),
        manifest_version: Some(1),
        manifest_issued_at_ms: receipt.issued_at_ms.saturating_sub(1),
        challenge_hash: receipt
            .challenge_hash_with_chain_id(&chain_id)
            .expect("provisioned challenge hash"),
        counter: 1,
        device_manifest,
        inspector_signature: Signature::from_bytes(&[0; 64]),
    });

    let mut build_claim = OfflineBuildClaim {
        claim_id: Hash::new(b"claim-android-provisioned"),
        platform: OfflineBuildClaimPlatform::Android,
        app_id: app_id.to_owned(),
        build_number: 1,
        issued_at_ms: receipt.issued_at_ms.saturating_sub(1),
        expires_at_ms: receipt.issued_at_ms + 60_000,
        lineage_scope: "merchants-main-wallet".to_owned(),
        nonce: receipt.tx_id,
        operator_signature: Signature::from_bytes(&[0; 64]),
    };
    build_claim.operator_signature = Signature::new(
        operator_keys.private_key(),
        &build_claim
            .signing_bytes()
            .expect("build claim signing bytes"),
    );
    receipt.build_claim = Some(build_claim);

    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    receipt.sender_signature = Signature::new(
        spend_keys.private_key(),
        &receipt.signing_bytes().expect("receipt signing bytes"),
    );

    let mut transfer = fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-android-provisioned");
    transfer.receipts = vec![receipt];
    transfer.balance_proof.claimed_delta = Numeric::new(100, 0);

    (certificate, transfer)
}

fn build_android_marker_multi_package_submission(
    fixtures: &Fixtures,
) -> (OfflineWalletCertificate, OfflineToOnlineTransfer) {
    let package_a = "com.example.merchants.android.a";
    let package_b = "com.example.merchants.android.b";
    let mut certificate = fixtures.certificate.clone();
    certificate.metadata.insert(
        ANDROID_INTEGRITY_POLICY_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android policy metadata key"),
        "marker_key",
    );
    certificate.metadata.insert(
        ANDROID_PACKAGE_NAMES_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android package metadata key"),
        vec![package_a, package_b],
    );
    certificate.metadata.insert(
        ANDROID_SIGNATURE_DIGESTS_KEY
            .parse::<iroha_data_model::name::Name>()
            .expect("android digest metadata key"),
        vec!["11f9626752131a2dbd8f4f33f2f6f4d916f8a29414f303f89af142f8311bfca5"],
    );
    let operator_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    certificate.operator_signature = Signature::new(
        operator_keys.private_key(),
        &certificate
            .operator_signing_bytes()
            .expect("certificate signing bytes"),
    );

    let mut receipt = fixtures.receipt.clone();
    receipt.platform_proof = OfflinePlatformProof::AndroidMarkerKey(AndroidMarkerKeyProof {
        series: "marker-series-1".to_owned(),
        counter: 1,
        marker_public_key: vec![0x04; 65],
        marker_signature: None,
        attestation: Vec::new(),
    });
    receipt.sender_certificate_id = certificate.certificate_id();
    receipt.build_claim = None;

    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    receipt.sender_signature = Signature::new(
        spend_keys.private_key(),
        &receipt.signing_bytes().expect("receipt signing bytes"),
    );

    let mut transfer = fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-android-marker-multi-package");
    transfer.receipts = vec![receipt];
    transfer.balance_proof.claimed_delta = Numeric::new(100, 0);

    (certificate, transfer)
}

fn queued_settlement_transfer(harness: &Harness) -> OfflineToOnlineTransfer {
    let view = harness.state.view();
    let mut pending = harness.queue.all_transactions(&view);
    let tx = pending.next().expect("queued settlement transaction");
    assert!(
        pending.next().is_none(),
        "expected one queued settlement transaction"
    );

    let Executable::Instructions(instructions) = tx.as_ref().instructions() else {
        panic!("expected instruction-based settlement transaction");
    };
    assert_eq!(instructions.len(), 1, "expected one settlement instruction");
    let isi = instructions[0]
        .as_any()
        .downcast_ref::<SubmitOfflineToOnlineTransfer>()
        .expect("expected SubmitOfflineToOnlineTransfer instruction");
    isi.transfer.clone()
}

fn queued_settlement_transaction_hash_hex(harness: &Harness) -> String {
    let view = harness.state.view();
    let mut pending = harness.queue.all_transactions(&view);
    let tx = pending.next().expect("queued settlement transaction");
    assert!(
        pending.next().is_none(),
        "expected one queued settlement transaction"
    );
    tx.as_ref().hash().to_string()
}

async fn assert_replay_response_status(
    outcome: ReplaySubmissionOutcome,
    expected_ok: bool,
    expected_reject_code: Option<&str>,
    mode: &str,
) {
    let response = outcome.response;
    let status = response.status();
    let reject_header = response
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("replay response body")
        .to_bytes();
    let body_json: Option<Value> = json::from_slice(&body_bytes).ok();
    let body_text = String::from_utf8_lossy(&body_bytes).into_owned();

    if expected_ok {
        assert_eq!(
            status,
            StatusCode::OK,
            "{mode} replay expected success, got status={status}, reject_code={reject_header:?}, body={body_text}"
        );
        let body = body_json
            .as_ref()
            .expect("successful replay response must be JSON");
        let bundle_id_hex = body
            .get("bundle_id_hex")
            .and_then(Value::as_str)
            .expect("successful replay response must include bundle_id_hex");
        assert!(
            !bundle_id_hex.is_empty(),
            "{mode} replay bundle_id_hex must not be empty"
        );
        let transaction_hash_hex = body
            .get("transaction_hash_hex")
            .and_then(Value::as_str)
            .expect("successful replay response must include transaction_hash_hex");
        assert_eq!(
            transaction_hash_hex.len(),
            64,
            "{mode} replay transaction_hash_hex must be 32-byte hex"
        );
        assert!(
            transaction_hash_hex
                .chars()
                .all(|ch| ch.is_ascii_hexdigit()),
            "{mode} replay transaction_hash_hex must be hex"
        );
        let queued_tx_hash_hex = outcome
            .queued_tx_hash_hex
            .as_deref()
            .expect("successful replay must capture queued tx hash");
        assert_eq!(
            transaction_hash_hex, queued_tx_hash_hex,
            "{mode} replay transaction_hash_hex should match queued transaction hash"
        );
        let pipeline_status_response = outcome
            .pipeline_status_response
            .expect("successful replay should query pipeline status");
        assert_eq!(
            pipeline_status_response.status(),
            StatusCode::OK,
            "{mode} replay pipeline status request should succeed"
        );
        let pipeline_status_bytes = pipeline_status_response
            .into_body()
            .collect()
            .await
            .expect("pipeline status body")
            .to_bytes();
        let pipeline_status_json: Value =
            json::from_slice(&pipeline_status_bytes).expect("pipeline status json");
        assert_eq!(
            pipeline_status_json["kind"].as_str(),
            Some("Transaction"),
            "{mode} replay pipeline payload kind mismatch"
        );
        assert_eq!(
            pipeline_status_json["content"]["hash"].as_str(),
            Some(queued_tx_hash_hex),
            "{mode} replay pipeline hash mismatch"
        );
        assert_eq!(
            pipeline_status_json["content"]["status"]["kind"].as_str(),
            Some("Queued"),
            "{mode} replay pipeline status should be Queued"
        );
        return;
    }

    assert!(
        matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ),
        "{mode} replay expected rejection status, got status={status}, reject_code={reject_header:?}, body={body_text}"
    );
    if let Some(expected) = expected_reject_code {
        assert_eq!(
            reject_header.as_deref(),
            Some(expected),
            "{mode} replay rejection code mismatch, body={body_text}"
        );
    }
    assert!(
        outcome.queued_tx_hash_hex.is_none(),
        "{mode} replay rejection should not expose queued transaction hash"
    );
    assert!(
        outcome.pipeline_status_response.is_none(),
        "{mode} replay rejection should not query pipeline status"
    );

    if let Some(body) = body_json.as_ref() {
        let error_message = body
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| body.get("error").and_then(Value::as_str));
        assert!(
            error_message.is_some(),
            "{mode} replay rejection should expose an error message in body: {body_text}"
        );
    } else if let Ok(validation) =
        norito::decode_from_bytes::<iroha_data_model::ValidationFail>(&body_bytes)
    {
        let validation_text = validation.to_string();
        assert!(
            !validation_text.trim().is_empty(),
            "{mode} replay rejection should expose non-empty validation text"
        );
    } else {
        assert!(
            !body_text.trim().is_empty(),
            "{mode} replay rejection should expose non-empty response body"
        );
    }
}

#[tokio::test]
#[ignore = "requires IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE pointing to a redacted real-device settlement capture"]
async fn offline_settlements_submit_app_attest_real_device_fixture_replay_respects_strict_signature_policy()
 {
    let fixture_path = std::env::var("IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE")
        .expect("set IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE to a fixture JSON path");
    let fixture = parse_real_device_replay_fixture(&fixture_path);

    let non_strict = submit_replay_fixture(&fixture, false).await;
    assert_replay_response_status(
        non_strict,
        fixture.expect_non_strict_ok,
        fixture.expect_non_strict_reject_code.as_deref(),
        "non-strict",
    )
    .await;

    let strict = submit_replay_fixture(&fixture, true).await;
    assert_replay_response_status(
        strict,
        fixture.expect_strict_ok,
        fixture.expect_strict_reject_code.as_deref(),
        "strict",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE pointing to a redacted real-device settlement capture"]
async fn offline_settlements_submit_app_attest_real_device_fixture_replay_persists_final_status_and_reason()
 {
    let fixture_path = std::env::var("IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE")
        .expect("set IROHA_APPLE_ATTEST_REAL_DEVICE_SETTLEMENT_FIXTURE to a fixture JSON path");
    let fixture = parse_real_device_replay_fixture(&fixture_path);

    let (non_strict_harness, non_strict) =
        submit_replay_fixture_with_harness(&fixture, false).await;
    let non_strict_submit_status = non_strict.response.status();
    assert_replay_response_status(
        non_strict,
        fixture.expect_non_strict_ok,
        fixture.expect_non_strict_reject_code.as_deref(),
        "non-strict",
    )
    .await;
    if non_strict_submit_status == StatusCode::OK {
        if let Some(expected_status) = fixture.expect_non_strict_final_status.as_deref() {
            assert_replay_final_settlement_status(
                &non_strict_harness,
                &fixture,
                expected_status,
                fixture.expect_non_strict_final_reject_code.as_deref(),
            )
            .await;
        } else if fixture.expect_non_strict_ok {
            assert_replay_final_settlement_status(&non_strict_harness, &fixture, "settled", None)
                .await;
        }
    }

    let (strict_harness, strict) = submit_replay_fixture_with_harness(&fixture, true).await;
    let strict_submit_status = strict.response.status();
    assert_replay_response_status(
        strict,
        fixture.expect_strict_ok,
        fixture.expect_strict_reject_code.as_deref(),
        "strict",
    )
    .await;

    if strict_submit_status != StatusCode::OK {
        return;
    }
    if let Some(expected_status) = fixture.expect_strict_final_status.as_deref() {
        assert_replay_final_settlement_status(
            &strict_harness,
            &fixture,
            expected_status,
            fixture.expect_strict_final_reject_code.as_deref(),
        )
        .await;
    } else if fixture.expect_strict_ok {
        assert_replay_final_settlement_status(&strict_harness, &fixture, "settled", None).await;
    } else {
        assert_replay_final_settlement_status(
            &strict_harness,
            &fixture,
            "rejected",
            fixture.expect_strict_reject_code.as_deref(),
        )
        .await;
    }
}

#[tokio::test]
async fn offline_settlements_submit_app_attest_synthetic_fixture_replay_exercises_parser_and_submission_path()
 {
    let fixtures = build_fixtures();
    let private_key =
        iroha_crypto::ExposedPrivateKey(fixtures.receiver_keys.private_key().clone()).to_string();
    let mut fixture_json = json::Map::new();
    fixture_json.insert("chain_id".into(), Value::from("test-chain"));
    fixture_json.insert(
        "authority".into(),
        json::to_value(&fixtures.receiver).expect("fixture authority value"),
    );
    fixture_json.insert("private_key".into(), Value::from(private_key));
    fixture_json.insert(
        "certificate".into(),
        json::to_value(&fixtures.certificate).expect("fixture certificate value"),
    );
    fixture_json.insert(
        "transfer".into(),
        json::to_value(&fixtures.transfer).expect("fixture transfer value"),
    );
    fixture_json.insert("expect_non_strict_ok".into(), Value::from(true));
    fixture_json.insert("expect_non_strict_reject_code".into(), Value::Null);
    fixture_json.insert("expect_strict_ok".into(), Value::from(true));
    fixture_json.insert("expect_strict_reject_code".into(), Value::Null);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("wall clock after unix epoch")
        .as_nanos();
    let fixture_path = std::env::temp_dir().join(format!(
        "iroha-app-attest-synthetic-settlement-replay-{now}.json"
    ));
    fs::write(
        &fixture_path,
        json::to_vec(&Value::Object(fixture_json)).expect("serialize fixture JSON"),
    )
    .expect("write synthetic fixture JSON");

    let fixture = parse_real_device_replay_fixture(
        fixture_path
            .to_str()
            .expect("synthetic fixture path should be valid UTF-8"),
    );
    let non_strict = submit_replay_fixture(&fixture, false).await;
    assert_replay_response_status(
        non_strict,
        fixture.expect_non_strict_ok,
        fixture.expect_non_strict_reject_code.as_deref(),
        "synthetic non-strict",
    )
    .await;

    let strict = submit_replay_fixture(&fixture, true).await;
    assert_replay_response_status(
        strict,
        fixture.expect_strict_ok,
        fixture.expect_strict_reject_code.as_deref(),
        "synthetic strict",
    )
    .await;

    let _ignored = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn offline_settlements_submit_app_attest_synthetic_fixture_replay_persists_final_rejected_status()
 {
    let fixtures = build_fixtures();
    let private_key =
        iroha_crypto::ExposedPrivateKey(fixtures.receiver_keys.private_key().clone()).to_string();
    let mut fixture_json = json::Map::new();
    fixture_json.insert("chain_id".into(), Value::from("test-chain"));
    fixture_json.insert(
        "authority".into(),
        json::to_value(&fixtures.receiver).expect("fixture authority value"),
    );
    fixture_json.insert("private_key".into(), Value::from(private_key));
    fixture_json.insert(
        "certificate".into(),
        json::to_value(&fixtures.certificate).expect("fixture certificate value"),
    );
    fixture_json.insert(
        "transfer".into(),
        json::to_value(&fixtures.transfer).expect("fixture transfer value"),
    );
    fixture_json.insert("expect_non_strict_ok".into(), Value::from(true));
    fixture_json.insert("expect_non_strict_reject_code".into(), Value::Null);
    fixture_json.insert("expect_strict_ok".into(), Value::from(true));
    fixture_json.insert("expect_strict_reject_code".into(), Value::Null);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("wall clock after unix epoch")
        .as_nanos();
    let fixture_path = std::env::temp_dir().join(format!(
        "iroha-app-attest-synthetic-settlement-final-status-{now}.json"
    ));
    fs::write(
        &fixture_path,
        json::to_vec(&Value::Object(fixture_json)).expect("serialize fixture JSON"),
    )
    .expect("write synthetic fixture JSON");

    let fixture = parse_real_device_replay_fixture(
        fixture_path
            .to_str()
            .expect("synthetic fixture path should be valid UTF-8"),
    );

    let (non_strict_harness, non_strict) =
        submit_replay_fixture_with_harness(&fixture, false).await;
    assert_replay_response_status(non_strict, true, None, "synthetic non-strict").await;
    assert_replay_final_settlement_status(
        &non_strict_harness,
        &fixture,
        "rejected",
        Some("platform_attestation_missing"),
    )
    .await;

    let (strict_harness, strict) = submit_replay_fixture_with_harness(&fixture, true).await;
    assert_replay_response_status(strict, true, None, "synthetic strict").await;
    assert_replay_final_settlement_status(
        &strict_harness,
        &fixture,
        "rejected",
        Some("platform_attestation_missing"),
    )
    .await;

    let _ignored = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn offline_spend_receipts_submit_returns_poseidon_root() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());
    let mut map = json::Map::new();
    map.insert(
        "receipts".into(),
        json::to_value(&vec![harness.fixtures.receipt.clone()]).expect("receipts value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize receipts");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/spend-receipts")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let expected_root = compute_receipts_root(std::slice::from_ref(&harness.fixtures.receipt))
        .expect("compute root")
        .to_hex_upper()
        .to_lowercase();
    assert_eq!(
        json_body["receipts_root_hex"].as_str(),
        Some(expected_root.as_str())
    );
    assert_eq!(json_body["receipt_count"].as_u64(), Some(1));
}

#[tokio::test]
async fn offline_allowances_issue_returns_certificate_id() {
    let harness = build_harness();
    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.controller).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.controller_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "certificate".into(),
        json::to_value(&harness.fixtures.certificate.clone()).expect("certificate value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/allowances")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let expected_id = hex::encode(harness.fixtures.certificate.certificate_id().as_ref());
    assert_eq!(
        json_body["certificate_id_hex"].as_str(),
        Some(expected_id.as_str())
    );
}

#[tokio::test]
async fn offline_build_claims_issue_returns_signed_claim_accepted_by_settlement() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut claim_request = json::Map::new();
    claim_request.insert(
        "certificate_id_hex".into(),
        Value::from(hex::encode(
            harness.fixtures.certificate.certificate_id().as_ref(),
        )),
    );
    claim_request.insert(
        "tx_id_hex".into(),
        Value::from(hex::encode(harness.fixtures.receipt.tx_id.as_ref())),
    );
    claim_request.insert("platform".into(), Value::from("apple"));
    claim_request.insert("app_id".into(), Value::from("com.example.merchants"));
    let claim_body = json::to_vec(&Value::Object(claim_request)).expect("serialize claim request");

    let claim_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/build-claims/issue")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(claim_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(claim_resp.status(), StatusCode::OK);
    let claim_bytes = claim_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let claim_json: Value = json::from_slice(&claim_bytes).expect("json");
    assert!(claim_json["claim_id_hex"].as_str().is_some());

    let build_claim: OfflineBuildClaim =
        json::from_value(claim_json["build_claim"].clone()).expect("build claim");
    assert_eq!(build_claim.nonce, harness.fixtures.receipt.tx_id);
    assert_eq!(build_claim.platform, OfflineBuildClaimPlatform::Apple);
    let operator_key = harness
        .fixtures
        .certificate
        .operator
        .try_signatory()
        .expect("single-signature operator");
    build_claim
        .operator_signature
        .verify(
            operator_key,
            &build_claim.signing_bytes().expect("claim signing bytes"),
        )
        .expect("valid operator signature");

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.receipts[0].build_claim = Some(build_claim);

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let submit_body = json::to_vec(&Value::Object(submit)).expect("serialize settlement request");

    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(submit_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn offline_build_claims_issue_uses_legacy_operator_key_when_primary_rotated() {
    let harness = build_harness_with_issuer(0x12, &[0x11]);
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut claim_request = json::Map::new();
    claim_request.insert(
        "certificate_id_hex".into(),
        Value::from(hex::encode(
            harness.fixtures.certificate.certificate_id().as_ref(),
        )),
    );
    claim_request.insert(
        "tx_id_hex".into(),
        Value::from(hex::encode(harness.fixtures.receipt.tx_id.as_ref())),
    );
    claim_request.insert("platform".into(), Value::from("apple"));
    claim_request.insert("app_id".into(), Value::from("com.example.merchants"));
    let claim_body = json::to_vec(&Value::Object(claim_request)).expect("serialize claim request");

    let claim_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/build-claims/issue")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(claim_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(claim_resp.status(), StatusCode::OK);
    let claim_bytes = claim_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let claim_json: Value = json::from_slice(&claim_bytes).expect("json");

    let build_claim: OfflineBuildClaim =
        json::from_value(claim_json["build_claim"].clone()).expect("build claim");
    let operator_key = harness
        .fixtures
        .certificate
        .operator
        .try_signatory()
        .expect("single-signature operator");
    build_claim
        .operator_signature
        .verify(
            operator_key,
            &build_claim.signing_bytes().expect("claim signing bytes"),
        )
        .expect("valid operator signature");
}

#[tokio::test]
async fn offline_settlements_submit_auto_issues_missing_build_claims() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-auto-build-claim");
    transfer.receipts[0].build_claim = None;

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let submit_body = json::to_vec(&Value::Object(submit)).expect("serialize settlement request");

    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(submit_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);

    let queued_transfer = queued_settlement_transfer(&harness);
    assert_eq!(queued_transfer.bundle_id, transfer.bundle_id);
    let queued_receipt = queued_transfer
        .receipts
        .first()
        .expect("queued settlement receipt");
    let build_claim = queued_receipt
        .build_claim
        .as_ref()
        .expect("build claim auto-issued by settlement endpoint");

    assert_eq!(build_claim.nonce, queued_receipt.tx_id);
    assert_eq!(build_claim.platform, OfflineBuildClaimPlatform::Apple);
    assert_eq!(build_claim.app_id, "com.example.merchants");

    let operator_key = harness
        .fixtures
        .certificate
        .operator
        .try_signatory()
        .expect("single-signature operator");
    build_claim
        .operator_signature
        .verify(
            operator_key,
            &build_claim.signing_bytes().expect("claim signing bytes"),
        )
        .expect("valid operator signature");
}

#[tokio::test]
async fn offline_settlements_submit_rejects_missing_build_claim_when_issuer_disabled() {
    let harness = build_harness_without_offline_issuer();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-build-claim-missing-no-issuer");
    transfer.receipts[0].build_claim = None;

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("build_claim_missing"));
}

#[tokio::test]
async fn offline_settlements_submit_accepts_missing_build_claim_when_verification_skipped() {
    let harness = build_harness_without_offline_issuer_skip_build_claim_verification();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-build-claim-missing-skip-verification");
    transfer.receipts[0].build_claim = None;

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let queued_transfer = queued_settlement_transfer(&harness);
    assert!(
        queued_transfer.receipts[0].build_claim.is_none(),
        "build claim should remain absent when verification is skipped"
    );
}

#[tokio::test]
async fn offline_settlements_submit_rejects_android_multi_package_without_override() {
    let harness = build_harness();
    let (certificate, transfer) = build_android_marker_multi_package_submission(&harness.fixtures);
    seed_allowance(&harness.state, certificate);

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("build_claim_invalid"));
}

#[tokio::test]
async fn offline_settlements_submit_uses_override_for_android_multi_package() {
    let harness = build_harness();
    let (certificate, transfer) = build_android_marker_multi_package_submission(&harness.fixtures);
    let operator_key = certificate
        .operator
        .try_signatory()
        .expect("single-signature operator")
        .clone();
    seed_allowance(&harness.state, certificate);

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    submit.insert(
        "build_claim_overrides".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert(
                "tx_id_hex".into(),
                Value::from(hex::encode(transfer.receipts[0].tx_id.as_ref())),
            );
            map.insert(
                "app_id".into(),
                Value::from("com.example.merchants.android.b"),
            );
            map
        })]),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let queued_transfer = queued_settlement_transfer(&harness);
    let build_claim = queued_transfer.receipts[0]
        .build_claim
        .as_ref()
        .expect("build claim auto-issued with override");
    assert_eq!(build_claim.app_id, "com.example.merchants.android.b");
    build_claim
        .operator_signature
        .verify(
            &operator_key,
            &build_claim.signing_bytes().expect("claim signing bytes"),
        )
        .expect("valid operator signature");
}

#[tokio::test]
async fn offline_settlements_submit_rejects_unknown_build_claim_override_tx_id() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-unknown-override");
    transfer.receipts[0].build_claim = None;

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    submit.insert(
        "build_claim_overrides".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert(
                "tx_id_hex".into(),
                Value::from(hex::encode(Hash::new(b"missing-receipt-tx").as_ref())),
            );
            map.insert("app_id".into(), Value::from("com.example.merchants"));
            map
        })]),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("build_claim_invalid"));
}

#[tokio::test]
async fn offline_settlements_submit_rejects_duplicate_build_claim_override_tx_id() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-duplicate-override");
    transfer.receipts[0].build_claim = None;
    let tx_id_hex = hex::encode(transfer.receipts[0].tx_id.as_ref());

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    submit.insert(
        "build_claim_overrides".into(),
        Value::Array(vec![
            Value::Object({
                let mut map = json::Map::new();
                map.insert("tx_id_hex".into(), Value::from(tx_id_hex.clone()));
                map.insert("app_id".into(), Value::from("com.example.merchants"));
                map
            }),
            Value::Object({
                let mut map = json::Map::new();
                map.insert("tx_id_hex".into(), Value::from(tx_id_hex));
                map.insert("app_id".into(), Value::from("com.example.merchants"));
                map
            }),
        ]),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("build_claim_invalid"));
}

#[tokio::test]
async fn offline_settlements_submit_rejects_override_for_existing_claim_without_repair() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());
    let transfer = harness.fixtures.transfer.clone();
    let tx_id_hex = hex::encode(transfer.receipts[0].tx_id.as_ref());

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    submit.insert(
        "build_claim_overrides".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("tx_id_hex".into(), Value::from(tx_id_hex));
            map.insert("app_id".into(), Value::from("com.example.merchants"));
            map
        })]),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("build_claim_invalid"));
}

#[tokio::test]
async fn offline_settlements_submit_keeps_invalid_existing_claim_without_repair() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-existing-invalid-no-repair");
    transfer.receipts[0]
        .build_claim
        .as_mut()
        .expect("existing build claim")
        .app_id = "com.example.invalid".to_owned();

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let queued_transfer = queued_settlement_transfer(&harness);
    let build_claim = queued_transfer.receipts[0]
        .build_claim
        .as_ref()
        .expect("existing build claim preserved");
    assert_eq!(build_claim.app_id, "com.example.invalid");
}

#[tokio::test]
async fn offline_settlements_submit_repairs_existing_claim_when_requested() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());
    let operator_key = harness
        .fixtures
        .certificate
        .operator
        .try_signatory()
        .expect("single-signature operator");

    let mut transfer = harness.fixtures.transfer.clone();
    transfer.bundle_id = Hash::new(b"bundle-existing-invalid-with-repair");
    transfer.receipts[0]
        .build_claim
        .as_mut()
        .expect("existing build claim")
        .app_id = "com.example.invalid".to_owned();

    let mut submit = json::Map::new();
    submit.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    submit.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    submit.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    submit.insert("repair_existing_build_claims".into(), Value::from(true));
    let body = json::to_vec(&Value::Object(submit)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let queued_transfer = queued_settlement_transfer(&harness);
    let build_claim = queued_transfer.receipts[0]
        .build_claim
        .as_ref()
        .expect("repaired build claim");
    assert_eq!(build_claim.app_id, "com.example.merchants");
    build_claim
        .operator_signature
        .verify(
            operator_key,
            &build_claim.signing_bytes().expect("claim signing bytes"),
        )
        .expect("valid repaired claim signature");
}

#[tokio::test]
async fn offline_settlements_submit_returns_bundle_id_and_transaction_hash() {
    let harness = build_harness();
    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&harness.fixtures.transfer.clone()).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let expected_bundle = hex::encode(harness.fixtures.transfer.bundle_id.as_ref());
    assert_eq!(
        json_body["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );

    let tx_hash = json_body["transaction_hash_hex"]
        .as_str()
        .expect("transaction_hash_hex");
    assert_eq!(tx_hash.len(), 64, "expected 32-byte hash encoded as hex");
    assert!(
        tx_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "transaction_hash_hex must be lowercase/uppercase hex"
    );

    let pipeline_status_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri(format!("/v1/pipeline/transactions/status?hash={tx_hash}"))
                .body(Body::empty())
                .expect("pipeline status request"),
        )
        .await
        .expect("pipeline status response");
    assert_eq!(pipeline_status_resp.status(), StatusCode::OK);
    let pipeline_status_bytes = pipeline_status_resp
        .into_body()
        .collect()
        .await
        .expect("pipeline status body")
        .to_bytes();
    let pipeline_status_json: Value =
        json::from_slice(&pipeline_status_bytes).expect("pipeline status json");
    assert_eq!(
        pipeline_status_json["kind"].as_str(),
        Some("Transaction"),
        "pipeline status payload kind mismatch"
    );
    assert_eq!(
        pipeline_status_json["content"]["hash"].as_str(),
        Some(tx_hash),
        "pipeline status hash mismatch"
    );
    assert_eq!(
        pipeline_status_json["content"]["status"]["kind"].as_str(),
        Some("Queued"),
        "settlement tx should be queued immediately after submission"
    );
}

#[tokio::test]
async fn pipeline_transaction_status_unknown_hash_surfaces_not_found_error_payload() {
    let harness = build_harness();
    let unknown_hash = Hash::new(b"pipeline-status-missing").to_string();
    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::GET)
                .uri(format!(
                    "/v1/pipeline/transactions/status?hash={unknown_hash}"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert!(
        matches!(
            resp.status(),
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST
        ),
        "expected not-found style error status for unknown tx hash, got {}",
        resp.status()
    );
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let validation: iroha_data_model::ValidationFail =
        norito::decode_from_bytes(&bytes).expect("decode validation payload");
    use iroha_data_model::query::error::QueryExecutionFail;
    match &validation {
        iroha_data_model::ValidationFail::QueryFailed(QueryExecutionFail::NotFound)
        | iroha_data_model::ValidationFail::QueryFailed(QueryExecutionFail::Conversion(_)) => {}
        other => panic!("unexpected pipeline status error variant: {other:?}"),
    }
    let body_text = validation.to_string();
    assert!(
        !body_text.trim().is_empty(),
        "unknown tx hash response should include validation text"
    );
    assert!(
        body_text.to_ascii_lowercase().contains("query")
            || body_text.to_ascii_lowercase().contains("notfound")
            || body_text.to_ascii_lowercase().contains("not found")
            || body_text.to_ascii_lowercase().contains("conversion"),
        "unknown tx hash response should expose query/not-found style detail, got: {body_text}"
    );
}

#[tokio::test]
async fn offline_settlements_submit_persists_settled_record() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&harness.fixtures.transfer.clone()).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);

    let submit_bytes = submit_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let submit_json: Value = json::from_slice(&submit_bytes).expect("json");
    let expected_bundle = hex::encode(harness.fixtures.transfer.bundle_id.as_ref());
    assert_eq!(
        submit_json["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );

    // The integration harness does not run block production, so we materialize
    // the queued settlement by executing the instruction directly.
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 1_700_000_200, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: harness.fixtures.transfer.clone(),
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("materialize settled settlement row");
    tx.apply();
    block.commit().expect("commit settled settlement row");

    let mut query_envelope = json::Map::new();
    query_envelope.insert(
        "filter".into(),
        eq_filter("bundle_id_hex", Value::from(expected_bundle.clone())),
    );
    query_envelope.insert(
        "sort".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("key".into(), Value::from("recorded_at_ms"));
            map.insert("order".into(), Value::from("desc"));
            map
        })]),
    );
    query_envelope.insert(
        "pagination".into(),
        Value::Object({
            let mut map = json::Map::new();
            map.insert("limit".into(), Value::from(1u64));
            map.insert("offset".into(), Value::from(0u64));
            map
        }),
    );
    query_envelope.insert("fetch_size".into(), Value::from(32u64));
    let query_body = json::to_vec(&Value::Object(query_envelope)).expect("serialize envelope");

    let query_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(query_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(query_resp.status(), StatusCode::OK);

    let query_bytes = query_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let query_json: Value = json::from_slice(&query_bytes).expect("json");
    assert_eq!(query_json["total"].as_u64(), Some(1));
    let item = &query_json["items"][0];
    assert_eq!(
        item["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );
    assert_eq!(item["status"].as_str(), Some("settled"));
    assert!(item["rejection_reason"].is_null());
    assert!(item["transfer"]["rejection_reason"].is_null());
}

#[tokio::test]
async fn offline_settlements_submit_persists_settled_record_for_android_build_claim() {
    let harness = build_harness();
    let (certificate, transfer) = build_android_provisioned_submission(&harness.fixtures);
    seed_allowance(&harness.state, certificate);

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&transfer.clone()).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);

    let submit_bytes = submit_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let submit_json: Value = json::from_slice(&submit_bytes).expect("json");
    let expected_bundle = hex::encode(transfer.bundle_id.as_ref());
    assert_eq!(
        submit_json["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 1_700_000_200, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: transfer.clone(),
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("materialize settled settlement row");
    tx.apply();
    block.commit().expect("commit settled settlement row");

    let mut query_envelope = json::Map::new();
    query_envelope.insert(
        "filter".into(),
        eq_filter("bundle_id_hex", Value::from(expected_bundle.clone())),
    );
    query_envelope.insert(
        "sort".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("key".into(), Value::from("recorded_at_ms"));
            map.insert("order".into(), Value::from("desc"));
            map
        })]),
    );
    query_envelope.insert(
        "pagination".into(),
        Value::Object({
            let mut map = json::Map::new();
            map.insert("limit".into(), Value::from(1u64));
            map.insert("offset".into(), Value::from(0u64));
            map
        }),
    );
    query_envelope.insert("fetch_size".into(), Value::from(32u64));
    let query_body = json::to_vec(&Value::Object(query_envelope)).expect("serialize envelope");

    let query_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(query_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(query_resp.status(), StatusCode::OK);

    let query_bytes = query_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let query_json: Value = json::from_slice(&query_bytes).expect("json");
    assert_eq!(query_json["total"].as_u64(), Some(1));
    let item = &query_json["items"][0];
    assert_eq!(
        item["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );
    assert_eq!(item["status"].as_str(), Some("settled"));
    assert!(item["rejection_reason"].is_null());
    assert!(item["transfer"]["rejection_reason"].is_null());
}

#[tokio::test]
async fn offline_settlements_submit_persists_rejected_record_for_platform_signature_invalid() {
    let base = build_fixtures();
    let (certificate, transfer) = build_android_provisioned_submission(&base);
    let fixtures = Fixtures {
        controller: certificate.controller.clone(),
        controller_keys: KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519),
        receiver: base.receiver.clone(),
        receiver_keys: KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519),
        receipt: transfer
            .receipts
            .first()
            .cloned()
            .expect("provisioned transfer contains one receipt"),
        transfer: transfer.clone(),
        certificate,
    };
    let harness =
        build_harness_for_app_attest_fixture(ChainId::from("test-chain"), fixtures, false);
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");
    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);

    let block_ts = transfer
        .receipts
        .iter()
        .map(|receipt| receipt.issued_at_ms)
        .max()
        .expect("one receipt")
        .saturating_add(1_000);
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, block_ts, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: transfer.clone(),
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("materialize rejected settlement row");
    tx.apply();
    block.commit().expect("commit rejected settlement row");

    let bundle_id_hex = hex::encode(transfer.bundle_id.as_ref());
    let item = query_settlement_record_by_bundle(&harness, &bundle_id_hex).await;
    assert_eq!(item["status"].as_str(), Some("rejected"));
    assert_eq!(
        item["rejection_reason"].as_str(),
        Some("platform_signature_invalid")
    );
    assert_eq!(
        item["transfer"]["rejection_reason"].as_str(),
        Some("platform_signature_invalid")
    );
}

#[tokio::test]
async fn offline_settlements_submit_persists_rejected_record_for_offline_error() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut rejected_transfer = harness.fixtures.transfer.clone();
    rejected_transfer.bundle_id = Hash::new(b"bundle-submit-rejected");
    rejected_transfer.receipts.clear();
    let expected_bundle = hex::encode(rejected_transfer.bundle_id.as_ref());

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&rejected_transfer).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let submit_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(submit_resp.status(), StatusCode::OK);

    let submit_bytes = submit_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let submit_json: Value = json::from_slice(&submit_bytes).expect("json");
    assert_eq!(
        submit_json["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );

    // The integration harness does not run block production, so we materialize
    // the queued settlement by executing the instruction directly.
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 1_700_000_002, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: rejected_transfer,
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("materialize rejected settlement row");
    tx.apply();
    block.commit().expect("commit rejected settlement row");

    let mut query_envelope = json::Map::new();
    query_envelope.insert(
        "filter".into(),
        eq_filter("bundle_id_hex", Value::from(expected_bundle.clone())),
    );
    query_envelope.insert(
        "sort".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("key".into(), Value::from("recorded_at_ms"));
            map.insert("order".into(), Value::from("desc"));
            map
        })]),
    );
    query_envelope.insert(
        "pagination".into(),
        Value::Object({
            let mut map = json::Map::new();
            map.insert("limit".into(), Value::from(1u64));
            map.insert("offset".into(), Value::from(0u64));
            map
        }),
    );
    query_envelope.insert("fetch_size".into(), Value::from(32u64));
    let query_body = json::to_vec(&Value::Object(query_envelope)).expect("serialize envelope");

    let query_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(query_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(query_resp.status(), StatusCode::OK);

    let query_bytes = query_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let query_json: Value = json::from_slice(&query_bytes).expect("json");
    assert_eq!(query_json["total"].as_u64(), Some(1));
    let item = &query_json["items"][0];
    assert_eq!(
        item["bundle_id_hex"].as_str(),
        Some(expected_bundle.as_str())
    );
    assert_eq!(item["status"].as_str(), Some("rejected"));
    assert_eq!(item["rejection_reason"].as_str(), Some("empty_bundle"));
    assert_eq!(
        item["transfer"]["rejection_reason"].as_str(),
        Some("empty_bundle")
    );
}

#[tokio::test]
async fn offline_settlements_submit_rejects_duplicate_bundle_with_reject_code() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 1_700_000_002, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: harness.fixtures.transfer.clone(),
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("seed transfer record");
    tx.apply();
    block.commit().expect("commit seeded transfer record");

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.receiver).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.receiver_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "transfer".into(),
        json::to_value(&harness.fixtures.transfer.clone()).expect("transfer value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let reject_code = resp
        .headers()
        .get("x-iroha-reject-code")
        .and_then(|value| value.to_str().ok());
    assert_eq!(reject_code, Some("duplicate_bundle"));
    let body_bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let validation: iroha_data_model::ValidationFail =
        norito::decode_from_bytes(&body_bytes).expect("decode duplicate bundle payload");
    use iroha_data_model::{
        isi::error::InstructionExecutionError, query::error::QueryExecutionFail,
    };
    let duplicate_message = match &validation {
        iroha_data_model::ValidationFail::QueryFailed(QueryExecutionFail::Conversion(message))
        | iroha_data_model::ValidationFail::InstructionFailed(InstructionExecutionError::Query(
            QueryExecutionFail::Conversion(message),
        )) => message,
        other => panic!("unexpected duplicate-bundle validation variant: {other:?}"),
    };
    assert!(
        duplicate_message.contains("duplicate_bundle"),
        "duplicate bundle conversion message missing duplicate_bundle tag: {duplicate_message}"
    );
    let body_text = validation.to_string();
    assert!(
        !body_text.trim().is_empty(),
        "duplicate bundle rejection payload should include validation text"
    );
    assert!(
        body_text.to_ascii_lowercase().contains("query"),
        "duplicate bundle rejection payload should include query-style error text, got: {body_text}"
    );
}

#[tokio::test]
async fn offline_settlements_query_includes_rejected_record_with_reason() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let mut rejected_transfer = harness.fixtures.transfer.clone();
    rejected_transfer.bundle_id = Hash::new(b"bundle-rejected");
    rejected_transfer.receipts.clear();
    let rejected_bundle_hex = hex::encode(rejected_transfer.bundle_id.as_ref());
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 1_700_000_002, 0);
    let mut block = harness.state.block(header);
    let mut tx = block.transaction();
    SubmitOfflineToOnlineTransfer {
        transfer: rejected_transfer,
    }
    .execute(&harness.fixtures.receiver, &mut tx)
    .expect("execute settlement submit");
    tx.apply();
    block.commit().expect("commit rejected settlement record");

    let mut pagination = json::Map::new();
    pagination.insert("limit".into(), Value::from(1u64));
    pagination.insert("offset".into(), Value::from(0u64));
    let mut sort_entry = json::Map::new();
    sort_entry.insert("key".into(), Value::from("recorded_at_ms"));
    sort_entry.insert("order".into(), Value::from("desc"));
    let mut query_envelope = json::Map::new();
    query_envelope.insert(
        "filter".into(),
        eq_filter("bundle_id_hex", Value::from(rejected_bundle_hex.clone())),
    );
    query_envelope.insert("sort".into(), Value::Array(vec![Value::Object(sort_entry)]));
    query_envelope.insert("pagination".into(), Value::Object(pagination));
    query_envelope.insert("fetch_size".into(), Value::from(32u64));
    let query_body = json::to_vec(&Value::Object(query_envelope)).expect("serialize envelope");

    let query_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(query_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(query_resp.status(), StatusCode::OK);
    let query_bytes = query_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let query_json: Value = json::from_slice(&query_bytes).expect("json");
    assert_eq!(query_json["total"].as_u64(), Some(1));
    let items = query_json["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(
        item["bundle_id_hex"].as_str(),
        Some(rejected_bundle_hex.as_str())
    );
    assert_eq!(item["status"].as_str(), Some("rejected"));
    assert_eq!(item["rejection_reason"].as_str(), Some("empty_bundle"));
    assert_eq!(
        item["transfer"]["rejection_reason"].as_str(),
        Some("empty_bundle")
    );

    let mut status_envelope = json::Map::new();
    status_envelope.insert(
        "filter".into(),
        eq_filter("status", Value::from("rejected")),
    );
    status_envelope.insert(
        "sort".into(),
        Value::Array(vec![Value::Object({
            let mut map = json::Map::new();
            map.insert("key".into(), Value::from("recorded_at_ms"));
            map.insert("order".into(), Value::from("desc"));
            map
        })]),
    );
    status_envelope.insert(
        "pagination".into(),
        Value::Object({
            let mut map = json::Map::new();
            map.insert("limit".into(), Value::from(10u64));
            map.insert("offset".into(), Value::from(0u64));
            map
        }),
    );
    status_envelope.insert("fetch_size".into(), Value::from(32u64));
    let status_body = json::to_vec(&Value::Object(status_envelope)).expect("serialize envelope");

    let status_resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/settlements/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(status_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_bytes = status_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let status_json: Value = json::from_slice(&status_bytes).expect("json");
    assert_eq!(status_json["total"].as_u64(), Some(1));
    assert_eq!(
        status_json["items"][0]["bundle_id_hex"].as_str(),
        Some(rejected_bundle_hex.as_str())
    );
}

fn seed_allowance(state: &Arc<State>, certificate: OfflineWalletCertificate) {
    let controller = certificate.controller.clone();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_001, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    Mint::asset_numeric(
        certificate.allowance.amount.clone(),
        certificate.allowance.asset.clone(),
    )
    .execute(&controller, &mut tx)
    .expect("allowance prefund");
    RegisterOfflineAllowance { certificate }
        .execute(&controller, &mut tx)
        .expect("allowance registration");
    tx.apply();
    block.commit().expect("commit seeded allowance");
}

fn eq_filter(field: &str, value: Value) -> Value {
    Value::Object(
        [
            ("op".into(), Value::from("eq")),
            (
                "args".into(),
                Value::Array(vec![Value::from(field.to_owned()), value]),
            ),
        ]
        .into_iter()
        .collect(),
    )
}

#[tokio::test]
async fn offline_allowances_renew_returns_new_certificate_id() {
    let harness = build_harness();
    seed_allowance(&harness.state, harness.fixtures.certificate.clone());

    let old_id_hex = hex::encode(harness.fixtures.certificate.certificate_id().as_ref());
    let mut renewed = harness.fixtures.certificate.clone();
    renewed.issued_at_ms += 1;

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.controller).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.controller_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert(
        "certificate".into(),
        json::to_value(&renewed).expect("renewed certificate value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let uri = format!("/v1/offline/allowances/{old_id_hex}/renew");
    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri(uri)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let expected_new_id = hex::encode(renewed.certificate_id().as_ref());
    assert_eq!(
        json_body["certificate_id_hex"].as_str(),
        Some(expected_new_id.as_str())
    );
}

#[tokio::test]
async fn offline_certificates_revoke_returns_verdict_id() {
    let harness = build_harness();
    let verdict_id = Hash::new(b"verdict-seed");
    let mut certificate = harness.fixtures.certificate.clone();
    certificate.verdict_id = Some(verdict_id);
    seed_allowance(&harness.state, certificate.clone());

    let certificate_id_hex = hex::encode(certificate.certificate_id().as_ref());

    let mut map = json::Map::new();
    map.insert(
        "authority".into(),
        json::to_value(&harness.fixtures.controller).expect("authority value"),
    );
    map.insert(
        "private_key".into(),
        Value::from(
            iroha_crypto::ExposedPrivateKey(harness.fixtures.controller_keys.private_key().clone())
                .to_string(),
        ),
    );
    map.insert("certificate_id_hex".into(), Value::from(certificate_id_hex));
    map.insert("reason".into(), Value::from("unspecified"));
    let body = json::to_vec(&Value::Object(map)).expect("serialize request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/certificates/revoke")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let expected_hex = hex::encode(verdict_id.as_ref());
    assert_eq!(
        json_body["verdict_id_hex"].as_str(),
        Some(expected_hex.as_str())
    );
}
