#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for offline app API write endpoints.
#![cfg(feature = "app_api")]

mod offline_balance_proof_utils;

use std::{str::FromStr, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::Queue as QueueConfig;
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
        AppleAppAttestProof, OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineAllowanceCommitment,
        OfflinePlatformProof, OfflineSpendReceipt, OfflineToOnlineTransfer,
        OfflineWalletCertificate, OfflineWalletPolicy, compute_receipts_root,
    },
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

fn build_harness() -> Harness {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let fixtures = build_fixtures();
    let world = world_from_fixtures(&fixtures);
    let chain_id = ChainId::from("test-chain");
    #[cfg(feature = "telemetry")]
    let mut state = State::new_with_chain(
        world,
        Arc::clone(&kura),
        query,
        chain_id.clone(),
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id.clone());
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
        queue,
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
    }
}

fn world_from_fixtures(fixtures: &Fixtures) -> World {
    let domain = Domain {
        id: fixtures.controller.domain().clone(),
        logo: None,
        metadata: Metadata::default(),
        owned_by: fixtures.controller.clone(),
    };

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
        spec: NumericSpec::integer(),
        mintable: Default::default(),
        logo: None,
        metadata: asset_definition_metadata,
        owned_by: fixtures.controller.clone(),
        total_quantity: Numeric::zero(),
        confidential_policy: Default::default(),
    };

    World::with(
        [domain],
        [controller, receiver, operator],
        [asset_definition],
    )
}

fn build_fixtures() -> Fixtures {
    let domain = iroha_data_model::domain::DomainId::from_str("merchants").expect("domain id");
    let chain_id = ChainId::from("test-chain");
    let operator_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let operator = AccountId::of(domain.clone(), operator_keys.public_key().clone());
    let controller_keys = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let controller = AccountId::of(domain.clone(), controller_keys.public_key().clone());
    let receiver_keys = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let receiver = AccountId::of(domain.clone(), receiver_keys.public_key().clone());
    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let asset_definition =
        AssetDefinitionId::from_str("xor#merchants").expect("asset definition id");
    let allowance_asset = AssetId::new(asset_definition, controller.clone());

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
    };
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
