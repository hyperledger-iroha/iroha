//! Integration tests for the `/v1/offline/transfers/proof` endpoint.
#![cfg(feature = "app_api")]

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
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    name::Name,
    offline::{
        AndroidProvisionedProof, OfflineAllowanceCommitment, OfflineBalanceProof,
        OfflinePlatformProof, OfflineProofRequestCounter, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineWalletCertificate, OfflineWalletPolicy,
    },
};
use iroha_primitives::numeric::Numeric;
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use norito::json;
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

#[tokio::test]
async fn offline_transfer_proof_accepts_transfer_payload() {
    let app = build_harness();
    let transfer = build_transfer(42);
    let payload = norito::json!({
        "transfer": transfer,
        "kind": "counter",
        "counter_checkpoint": 41
    });
    let body = json::to_json_pretty(&payload).expect("proof request body");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/transfers/proof")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let proof: OfflineProofRequestCounter = json::from_slice(&bytes).expect("proof payload");
    assert_eq!(proof.header.bundle_id, transfer.bundle_id);
    assert_eq!(proof.counter_checkpoint, 41);
    assert_eq!(proof.counters, vec![42]);
}

#[tokio::test]
async fn offline_transfer_proof_rejects_missing_transfer() {
    let app = build_harness();
    let payload = norito::json!({
        "kind": "sum"
    });
    let body = json::to_json_pretty(&payload).expect("proof request body");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/offline/transfers/proof")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

fn build_harness() -> Router {
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

    torii.api_router_for_tests()
}

fn build_transfer(counter: u64) -> OfflineToOnlineTransfer {
    let domain = DomainId::from_str("offline").expect("domain id");
    let controller_keys = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let receiver_keys = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
    let spend_keys = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);

    let controller = AccountId::of(domain.clone(), controller_keys.public_key().clone());
    let receiver = AccountId::of(domain.clone(), receiver_keys.public_key().clone());

    let asset_definition =
        AssetDefinitionId::new(domain.clone(), Name::from_str("xor").expect("asset name"));
    let asset = AssetId::new(asset_definition, controller.clone());

    let certificate = OfflineWalletCertificate {
        controller: controller.clone(),
        operator: controller.clone(),
        allowance: OfflineAllowanceCommitment {
            asset: asset.clone(),
            amount: Numeric::new(100, 0),
            commitment: vec![0xAA; 32],
        },
        spend_public_key: spend_keys.public_key().clone(),
        attestation_report: Vec::new(),
        issued_at_ms: 1_700_000_000,
        expires_at_ms: 1_800_000_000,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(100, 0),
            max_tx_value: Numeric::new(100, 0),
            expires_at_ms: 1_800_000_000,
        },
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata: iroha_data_model::metadata::Metadata::default(),
        verdict_id: None,
        attestation_nonce: None,
        refresh_at_ms: None,
    };

    let mut device_manifest = iroha_data_model::metadata::Metadata::default();
    device_manifest.insert(
        "android.provisioned.device_id"
            .parse::<Name>()
            .expect("metadata key"),
        "device-1",
    );

    let receipt = OfflineSpendReceipt {
        tx_id: Hash::new(b"tx-1"),
        from: controller.clone(),
        to: receiver.clone(),
        asset: asset.clone(),
        amount: Numeric::new(10, 0),
        issued_at_ms: 1_700_000_100,
        invoice_id: "INV-001".to_owned(),
        platform_proof: OfflinePlatformProof::Provisioned(AndroidProvisionedProof {
            manifest_schema: "offline_provisioning_v1".to_owned(),
            manifest_version: None,
            manifest_issued_at_ms: 1_700_000_000,
            challenge_hash: Hash::new(b"challenge"),
            counter,
            device_manifest,
            inspector_signature: Signature::from_bytes(&[0; 64]),
        }),
        platform_snapshot: None,
        sender_certificate: certificate.clone(),
        sender_signature: Signature::from_bytes(&[0; 64]),
    };

    let balance_proof = OfflineBalanceProof {
        initial_commitment: OfflineAllowanceCommitment {
            asset: asset.clone(),
            amount: Numeric::new(100, 0),
            commitment: vec![0xBB; 32],
        },
        resulting_commitment: vec![0xCC; 32],
        claimed_delta: Numeric::new(10, 0),
        zk_proof: None,
    };

    OfflineToOnlineTransfer {
        bundle_id: Hash::new(b"bundle-1"),
        receiver,
        deposit_account: controller,
        receipts: vec![receipt],
        balance_proof,
        aggregate_proof: None,
        attachments: None,
        platform_snapshot: None,
    }
}
