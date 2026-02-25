#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for the `/v1/offline/receipts{,/query}` endpoints.
#![cfg(feature = "app_api")]

mod offline_balance_proof_utils;

use std::{str::FromStr, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
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
    account::AccountId,
    asset::{
        AssetDefinitionId, AssetId, Mintable, NewAssetDefinition,
        definition::AssetConfidentialPolicy,
    },
    block::BlockHeader,
    domain::Domain,
    isi::{
        Mint, Register,
        offline::{RegisterOfflineAllowance, SubmitOfflineToOnlineTransfer},
    },
    metadata::Metadata,
    name::Name,
    offline::{
        AndroidProvisionedProof, OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineAllowanceCommitment,
        OfflinePlatformProof, OfflineSpendReceipt, OfflineToOnlineTransfer,
        OfflineWalletCertificate, OfflineWalletPolicy,
    },
};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use nonzero_ext::nonzero;
use norito::json::{self, Map, Value};
use offline_balance_proof_utils::{build_balance_proof_for_allowance, scalar_bytes};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

#[tokio::test]
async fn offline_receipts_list_returns_flattened_receipts() {
    let harness = build_receipt_harness();
    let uri = "/v1/offline/receipts?address_format=canonical";

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body: Value = json::from_slice(&bytes).expect("json");

    assert_eq!(body["total"].as_u64(), Some(2));
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    let first = &items[0];
    assert_eq!(
        first["bundle_id_hex"].as_str(),
        Some(harness.fixtures.bundle_hex.as_str())
    );
    assert_eq!(
        first["certificate_id_hex"].as_str(),
        Some(harness.fixtures.certificate_hex.as_str())
    );
    assert_eq!(
        first["controller_id"].as_str(),
        Some(harness.fixtures.controller.to_string().as_str())
    );
    assert_eq!(
        first["receiver_id"].as_str(),
        Some(harness.fixtures.receiver.to_string().as_str())
    );
}

#[tokio::test]
async fn offline_receipts_query_filters_by_invoice_id() {
    let harness = build_receipt_harness();

    let mut pagination = Map::new();
    pagination.insert("limit".into(), Value::from(10u64));
    pagination.insert("offset".into(), Value::from(0u64));

    let mut sort_entry = Map::new();
    sort_entry.insert("key".into(), Value::from("recorded_at_ms"));
    sort_entry.insert("order".into(), Value::from("desc"));

    let mut envelope_map = Map::new();
    envelope_map.insert(
        "filter".into(),
        eq_filter("invoice_id", Value::from("INV-002")),
    );
    envelope_map.insert("sort".into(), Value::Array(vec![Value::Object(sort_entry)]));
    envelope_map.insert("pagination".into(), Value::Object(pagination));
    envelope_map.insert("fetch_size".into(), Value::from(32u64));
    let body = json::to_vec(&Value::Object(envelope_map)).expect("serialize envelope");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/receipts/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body: Value = json::from_slice(&bytes).expect("json");

    assert_eq!(body["total"].as_u64(), Some(1));
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["invoice_id"].as_str(), Some("INV-002"));
}

struct ReceiptHarness {
    app: Router,
    fixtures: ReceiptFixtures,
}

struct ReceiptFixtures {
    operator: AccountId,
    controller: AccountId,
    receiver: AccountId,
    certificate: OfflineWalletCertificate,
    transfer: OfflineToOnlineTransfer,
    bundle_hex: String,
    certificate_hex: String,
}

fn build_receipt_harness() -> ReceiptHarness {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let chain_id = cfg.common.chain.clone();
    #[cfg(feature = "telemetry")]
    let state = Arc::new(State::new_with_chain(
        World::default(),
        Arc::clone(&kura),
        query,
        chain_id.clone(),
        iroha_core::telemetry::StateTelemetry::default(),
    ));
    #[cfg(not(feature = "telemetry"))]
    let state = Arc::new(State::new_with_chain(
        World::default(),
        Arc::clone(&kura),
        query,
        chain_id.clone(),
    ));

    let fixtures = build_receipt_fixtures(&chain_id);
    seed_offline_receipts(&state, &fixtures);

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
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    ReceiptHarness {
        app: torii.api_router_for_tests(),
        fixtures,
    }
}

#[allow(clippy::too_many_lines)]
fn build_receipt_fixtures(chain_id: &ChainId) -> ReceiptFixtures {
    let domain = iroha_data_model::domain::DomainId::from_str("merchants").expect("domain id");
    let operator_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let operator = AccountId::of(domain.clone(), operator_keys.public_key().clone());
    let controller_keys = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let controller = AccountId::of(domain.clone(), controller_keys.public_key().clone());
    let receiver_keys = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let receiver = AccountId::of(domain.clone(), receiver_keys.public_key().clone());
    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);

    let asset_definition =
        AssetDefinitionId::new(domain.clone(), Name::from_str("xor").expect("asset name"));
    let allowance_asset = AssetId::new(asset_definition.clone(), controller.clone());

    let inspector_keys = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    let inspector_pk_str = inspector_keys.public_key().to_string();
    let mut cert_metadata = Metadata::default();
    cert_metadata.insert(
        "android.integrity.policy"
            .parse::<Name>()
            .expect("metadata key"),
        "provisioned",
    );
    cert_metadata.insert(
        "android.provisioned.inspector_public_key"
            .parse::<Name>()
            .expect("metadata key"),
        inspector_pk_str.as_str(),
    );
    cert_metadata.insert(
        "android.provisioned.manifest_schema"
            .parse::<Name>()
            .expect("metadata key"),
        "offline_provisioning_v1",
    );

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
        metadata: cert_metadata,
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

    let claimed_delta = Numeric::new(150, 0);
    let balance_proof = build_balance_proof_for_allowance(
        chain_id,
        &certificate.allowance,
        &claimed_delta,
        scalar_bytes(1),
        scalar_bytes(2),
    );
    certificate
        .allowance
        .commitment
        .clone_from(&balance_proof.initial_commitment.commitment);

    let receipt_one = build_receipt(
        chain_id,
        b"receipt-1",
        "INV-001",
        1,
        Numeric::new(100, 0),
        &certificate,
        &controller,
        &receiver,
        &spend_keys,
        &inspector_keys,
        "offline_provisioning_v1",
    );
    let receipt_two = build_receipt(
        chain_id,
        b"receipt-2",
        "INV-002",
        2,
        Numeric::new(50, 0),
        &certificate,
        &controller,
        &receiver,
        &spend_keys,
        &inspector_keys,
        "offline_provisioning_v1",
    );

    let transfer = OfflineToOnlineTransfer {
        bundle_id: Hash::new(b"bundle-1"),
        receiver: receiver.clone(),
        deposit_account: operator.clone(),
        receipts: vec![receipt_one, receipt_two],
        balance_proof,
        balance_proofs: None,
        aggregate_proof: None,
        attachments: None,
        platform_snapshot: None,
    };

    ReceiptFixtures {
        operator,
        controller,
        receiver,
        certificate_hex: hex::encode(certificate.certificate_id().as_ref()),
        bundle_hex: hex::encode(transfer.bundle_id.as_ref()),
        certificate,
        transfer,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    chain_id: &ChainId,
    tag: &'static [u8],
    invoice_id: &str,
    counter: u64,
    amount: Numeric,
    certificate: &OfflineWalletCertificate,
    controller: &AccountId,
    receiver: &AccountId,
    spend_keys: &KeyPair,
    inspector_keys: &KeyPair,
    manifest_schema: &str,
) -> OfflineSpendReceipt {
    let mut device_manifest = Metadata::default();
    device_manifest.insert(
        "android.provisioned.device_id"
            .parse::<Name>()
            .expect("metadata key"),
        "device-1",
    );
    let mut receipt = OfflineSpendReceipt {
        tx_id: Hash::new(tag),
        from: controller.clone(),
        to: receiver.clone(),
        asset: certificate.allowance.asset.clone(),
        amount,
        issued_at_ms: certificate.issued_at_ms + 100,
        invoice_id: invoice_id.to_owned(),
        platform_proof: OfflinePlatformProof::Provisioned(AndroidProvisionedProof {
            manifest_schema: manifest_schema.to_owned(),
            manifest_version: None,
            manifest_issued_at_ms: 1_700_000_000,
            challenge_hash: Hash::new(b"challenge"),
            counter,
            device_manifest: device_manifest.clone(),
            inspector_signature: Signature::from_bytes(&[0; 64]),
        }),
        platform_snapshot: None,
        sender_certificate_id: certificate.certificate_id(),
        sender_signature: Signature::from_bytes(&[0; 64]),
    };
    let challenge_hash = receipt
        .challenge_hash_with_chain_id(chain_id)
        .expect("challenge hash");
    let mut proof = AndroidProvisionedProof {
        manifest_schema: manifest_schema.to_owned(),
        manifest_version: None,
        manifest_issued_at_ms: 1_700_000_000,
        challenge_hash,
        counter,
        device_manifest,
        inspector_signature: Signature::from_bytes(&[0; 64]),
    };
    let payload = proof.signing_bytes().expect("provisioned signing bytes");
    proof.inspector_signature = Signature::new(inspector_keys.private_key(), &payload);
    receipt.platform_proof = OfflinePlatformProof::Provisioned(proof);
    receipt.sender_signature = Signature::new(
        spend_keys.private_key(),
        &receipt.signing_bytes().expect("receipt signing bytes"),
    );
    receipt
}

fn seed_offline_receipts(state: &Arc<State>, fixtures: &ReceiptFixtures) {
    let domain_id = fixtures.operator.domain().clone();
    let asset_definition_id = fixtures.certificate.allowance.asset.definition().clone();

    let header_one = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_321, 0);
    {
        let mut block = state.block(header_one);
        let mut tx = block.transaction();
        let mut asset_definition_metadata = Metadata::default();
        asset_definition_metadata.insert(
            Name::from_str(OFFLINE_ASSET_ENABLED_METADATA_KEY)
                .expect("offline enabled metadata key"),
            true,
        );
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&fixtures.operator, &mut tx)
            .expect("domain registration");
        for account_id in [&fixtures.operator, &fixtures.controller, &fixtures.receiver] {
            Register::account(iroha_data_model::account::Account::new(account_id.clone()))
                .execute(&fixtures.operator, &mut tx)
                .expect("account registration");
        }
        Register::asset_definition(NewAssetDefinition {
            id: asset_definition_id.clone(),
            spec: NumericSpec::default(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: asset_definition_metadata,
            confidential_policy: AssetConfidentialPolicy::default(),
        })
        .execute(&fixtures.operator, &mut tx)
        .expect("asset definition registration");
        Mint::asset_numeric(
            fixtures.certificate.allowance.amount.clone(),
            fixtures.certificate.allowance.asset.clone(),
        )
        .execute(&fixtures.controller, &mut tx)
        .expect("allowance prefund");
        RegisterOfflineAllowance {
            certificate: fixtures.certificate.clone(),
        }
        .execute(&fixtures.controller, &mut tx)
        .expect("allowance registration");
        tx.apply();
        block.commit().expect("commit block one");
    }

    let prev_hash = state.view().latest_block_hash();
    let header_two = BlockHeader::new(nonzero!(2_u64), prev_hash, None, None, 1_700_000_654, 0);
    {
        let mut block = state.block(header_two);
        let mut tx = block.transaction();
        SubmitOfflineToOnlineTransfer {
            transfer: fixtures.transfer.clone(),
        }
        .execute(&fixtures.receiver, &mut tx)
        .expect("transfer submission");
        tx.apply();
        block.commit().expect("commit block two");
    }
}

fn eq_filter(field: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert("op".into(), Value::from("eq"));
    map.insert(
        "args".into(),
        Value::Array(vec![Value::from(field.to_string()), value]),
    );
    Value::Object(map)
}
