#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for offline certificate renewal and revocation endpoints.
#![cfg(feature = "app_api")]

use std::{str::FromStr, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
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
    isi::{Mint, offline::RegisterOfflineAllowance},
    metadata::Metadata,
    name::Name,
    offline::{
        OFFLINE_ASSET_ENABLED_METADATA_KEY, OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY,
        OFFLINE_LINEAGE_EPOCH_KEY, OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY,
        OFFLINE_LINEAGE_SCOPE_KEY, OfflineAllowanceCommitment, OfflineVerdictRevocationReason,
        OfflineWalletCertificate, OfflineWalletPolicy,
    },
};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_torii::{
    MaybeTelemetry, OnlinePeersProvider, Torii,
    filter::{Pagination, QueryEnvelope},
    test_utils,
};
use nonzero_ext::nonzero;
use norito::json::{self, Map, Value};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

#[tokio::test]
async fn offline_certificates_revoke_returns_verdict_id() {
    let harness = build_cert_harness();
    let mut map = Map::new();
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
        "certificate_id_hex".into(),
        Value::from(harness.fixtures.certificate_hex.clone()),
    );
    map.insert(
        "reason".into(),
        Value::from(OfflineVerdictRevocationReason::IssuerRequest.as_str()),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize revoke request");

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

    assert_eq!(
        json_body["verdict_id_hex"].as_str(),
        Some(harness.fixtures.verdict_hex.as_str())
    );
}

#[tokio::test]
async fn offline_allowances_renew_returns_new_certificate_id() {
    let harness = build_cert_harness();
    let mut map = Map::new();
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
        json::to_value(&harness.fixtures.renewed_certificate).expect("certificate value"),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize renew request");

    let uri = format!(
        "/v1/offline/allowances/{}/renew",
        harness.fixtures.certificate_hex
    );
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

    assert_eq!(
        json_body["certificate_id_hex"].as_str(),
        Some(harness.fixtures.renewed_hex.as_str())
    );
}

#[tokio::test]
async fn offline_certificates_issue_returns_signed_certificate() {
    let harness = build_cert_harness();
    let mut map = Map::new();
    map.insert(
        "certificate".into(),
        certificate_draft_json(&harness.fixtures.certificate),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize issue request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/certificates/issue")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let certificate_value = json_body["certificate"].clone();
    let certificate: OfflineWalletCertificate =
        json::from_value(certificate_value).expect("certificate");
    let certificate_id_hex = json_body["certificate_id_hex"]
        .as_str()
        .expect("certificate_id_hex");
    assert_eq!(
        certificate_id_hex,
        hex::encode(certificate.certificate_id().as_ref())
    );
    assert_eq!(certificate.operator, harness.fixtures.controller);

    let payload = certificate.operator_signing_bytes().expect("payload");
    certificate
        .operator_signature
        .verify(harness.fixtures.controller_keys.public_key(), &payload)
        .expect("operator signature");
}

#[tokio::test]
async fn offline_certificates_issue_ignores_client_supplied_operator() {
    let harness = build_cert_harness();
    let mut draft_value = certificate_draft_json(&harness.fixtures.certificate);
    let wrong_operator_keys = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let wrong_operator = AccountId::of(wrong_operator_keys.public_key().clone());
    match &mut draft_value {
        Value::Object(map) => {
            map.insert(
                "operator".into(),
                json::to_value(&wrong_operator).expect("operator value"),
            );
        }
        _ => panic!("certificate json must be object"),
    }

    let mut map = Map::new();
    map.insert("certificate".into(), draft_value);
    let body = json::to_vec(&Value::Object(map)).expect("serialize issue request");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/certificates/issue")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let certificate: OfflineWalletCertificate =
        json::from_value(json_body["certificate"].clone()).expect("certificate");
    assert_eq!(certificate.operator, harness.fixtures.controller);
}

#[tokio::test]
async fn offline_certificates_renew_issue_ignores_client_supplied_operator() {
    let harness = build_cert_harness();
    let mut draft_value = certificate_draft_json(&harness.fixtures.renewed_certificate);
    let wrong_operator_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let wrong_operator = AccountId::of(wrong_operator_keys.public_key().clone());
    match &mut draft_value {
        Value::Object(map) => {
            map.insert(
                "operator".into(),
                json::to_value(&wrong_operator).expect("operator value"),
            );
        }
        _ => panic!("certificate json must be object"),
    }

    let mut map = Map::new();
    map.insert("certificate".into(), draft_value);
    let body = json::to_vec(&Value::Object(map)).expect("serialize renew issue request");

    let uri = format!(
        "/v1/offline/certificates/{}/renew/issue",
        harness.fixtures.certificate_hex
    );
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

    let certificate: OfflineWalletCertificate =
        json::from_value(json_body["certificate"].clone()).expect("certificate");
    assert_eq!(certificate.operator, harness.fixtures.controller);
}

#[tokio::test]
async fn offline_certificates_renew_issue_returns_signed_certificate() {
    let harness = build_cert_harness();
    let mut map = Map::new();
    map.insert(
        "certificate".into(),
        certificate_draft_json(&harness.fixtures.renewed_certificate),
    );
    let body = json::to_vec(&Value::Object(map)).expect("serialize renew issue request");

    let uri = format!(
        "/v1/offline/certificates/{}/renew/issue",
        harness.fixtures.certificate_hex
    );
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

    let certificate_value = json_body["certificate"].clone();
    let certificate: OfflineWalletCertificate =
        json::from_value(certificate_value).expect("certificate");
    let certificate_id_hex = json_body["certificate_id_hex"]
        .as_str()
        .expect("certificate_id_hex");
    assert_eq!(
        certificate_id_hex,
        hex::encode(certificate.certificate_id().as_ref())
    );
    assert_eq!(certificate.operator, harness.fixtures.controller);

    let payload = certificate.operator_signing_bytes().expect("payload");
    certificate
        .operator_signature
        .verify(harness.fixtures.controller_keys.public_key(), &payload)
        .expect("operator signature");
}

#[tokio::test]
async fn offline_certificates_query_lists_allowances() {
    let harness = build_cert_harness();
    let envelope = QueryEnvelope {
        pagination: Pagination {
            limit: Some(25),
            ..Pagination::default()
        },
        ..QueryEnvelope::default()
    };
    let body = json::to_vec(&envelope).expect("serialize envelope");

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v1/offline/certificates/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let json_body: Value = json::from_slice(&bytes).expect("json");

    let items = json_body["items"].as_array().expect("items array");
    assert!(
        items.iter().any(|item| {
            item["certificate_id_hex"].as_str() == Some(harness.fixtures.certificate_hex.as_str())
        }),
        "query results contain seeded allowance"
    );
}

struct CertHarness {
    app: Router,
    fixtures: CertFixtures,
}

struct CertFixtures {
    controller: AccountId,
    controller_keys: KeyPair,
    certificate: OfflineWalletCertificate,
    renewed_certificate: OfflineWalletCertificate,
    certificate_hex: String,
    renewed_hex: String,
    verdict_hex: String,
}

fn build_cert_harness() -> CertHarness {
    let fixtures = build_cert_fixtures();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.offline_issuer = Some(ToriiOfflineIssuer {
        operator_private_key: iroha_crypto::ExposedPrivateKey(
            fixtures.controller_keys.private_key().clone(),
        ),
        legacy_operator_private_keys: vec![],
        allowed_controllers: vec![fixtures.controller.clone()],
    });
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = world_from_cert_fixtures(&fixtures);
    let state = Arc::new(State::new_for_testing(world, Arc::clone(&kura), query));

    seed_allowance(&state, &fixtures.certificate);

    let queue_cfg = QueueConfig::default();
    let (events_sender, _) = broadcast::channel(64);
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        ChainId::from("test-chain"),
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

    CertHarness {
        app: torii.api_router_for_tests(),
        fixtures,
    }
}

fn build_cert_fixtures() -> CertFixtures {
    let domain = iroha_data_model::domain::DomainId::from_str("merchants").expect("domain id");
    let controller_keys = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let controller = AccountId::of(controller_keys.public_key().clone());
    let spend_keys = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let asset_definition =
        AssetDefinitionId::new(domain.clone(), Name::from_str("xor").expect("asset name"));
    let allowance_asset = AssetId::new(asset_definition, controller.clone());

    let verdict_id = Hash::new(b"verdict-1");

    let mut certificate = OfflineWalletCertificate {
        controller: controller.clone(),
        operator: controller.clone(),
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
        verdict_id: Some(verdict_id),
        attestation_nonce: None,
        refresh_at_ms: None,
    };
    set_lineage_metadata(&mut certificate, "cert-app-api", 1, None);
    let certificate_id = certificate.certificate_id();
    let mut renewed_certificate = certificate.clone();
    renewed_certificate.issued_at_ms = 1_750_000_000;
    renewed_certificate.expires_at_ms = 1_850_000_000;
    set_lineage_metadata(
        &mut renewed_certificate,
        "cert-app-api",
        2,
        Some(certificate_id),
    );

    let certificate_hex = hex::encode(certificate_id.as_ref());
    let renewed_hex = hex::encode(renewed_certificate.certificate_id().as_ref());
    let verdict_hex = hex::encode(verdict_id.as_ref());

    CertFixtures {
        controller,
        controller_keys,
        certificate,
        renewed_certificate,
        certificate_hex,
        renewed_hex,
        verdict_hex,
    }
}

fn set_lineage_metadata(
    certificate: &mut OfflineWalletCertificate,
    scope: &str,
    epoch: u64,
    prev_certificate_id: Option<Hash>,
) {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str(OFFLINE_LINEAGE_SCOPE_KEY).expect("lineage scope key"),
        scope,
    );
    metadata.insert(
        Name::from_str(OFFLINE_LINEAGE_EPOCH_KEY).expect("lineage epoch key"),
        epoch,
    );
    metadata.insert(
        Name::from_str(OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY).expect("min build key"),
        1_u64,
    );
    if let Some(prev_certificate_id) = prev_certificate_id {
        let prev_hex = hex::encode(prev_certificate_id.as_ref());
        metadata.insert(
            Name::from_str(OFFLINE_LINEAGE_PREV_CERTIFICATE_ID_HEX_KEY)
                .expect("lineage previous key"),
            prev_hex.as_str(),
        );
    }
    certificate.metadata = metadata;
}

fn certificate_draft_json(certificate: &OfflineWalletCertificate) -> Value {
    let mut value = json::to_value(certificate).expect("certificate value");
    match &mut value {
        Value::Object(map) => {
            map.remove("operator_signature");
            map.remove("operator");
        }
        _ => panic!("certificate json must be object"),
    }
    value
}

fn world_from_cert_fixtures(fixtures: &CertFixtures) -> World {
    let domain = Domain {
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
    };
    let controller = Account {
        id: fixtures.controller.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let mut asset_definition_metadata = Metadata::default();
    asset_definition_metadata.insert(
        Name::from_str(OFFLINE_ASSET_ENABLED_METADATA_KEY).expect("offline enabled metadata key"),
        true,
    );
    let asset_definition = AssetDefinition {
        id: fixtures.certificate.allowance.asset.definition().clone(),
        spec: NumericSpec::integer(),
        mintable: Default::default(),
        logo: None,
        metadata: asset_definition_metadata,
        balance_scope_policy: Default::default(),
        owned_by: fixtures.controller.clone(),
        total_quantity: Numeric::zero(),
        confidential_policy: Default::default(),
    };

    // `RegisterOfflineAllowance` seeding resolves the definition in order to evaluate
    // offline escrow requirements, so the harness must include it.
    World::with([domain], [controller], [asset_definition])
}

fn seed_allowance(state: &Arc<State>, certificate: &OfflineWalletCertificate) {
    let header_one = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_321, 0);
    let mut block = state.block(header_one);
    let mut tx = block.transaction();
    Mint::asset_numeric(
        certificate.allowance.amount.clone(),
        certificate.allowance.asset.clone(),
    )
    .execute(&certificate.controller, &mut tx)
    .expect("allowance prefund");
    RegisterOfflineAllowance {
        certificate: certificate.clone(),
    }
    .execute(&certificate.controller, &mut tx)
    .expect("allowance registration");
    tx.apply();
    block.commit().expect("commit block");
}
