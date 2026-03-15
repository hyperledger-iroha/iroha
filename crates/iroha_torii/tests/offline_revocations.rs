#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for the `/v2/offline/revocations{,/query}` endpoints.
#![cfg(feature = "app_api")]

use std::{collections::BTreeSet, str::FromStr, sync::Arc};

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
    account::{Account, AccountId},
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::Domain,
    isi::{
        Mint,
        offline::{RegisterOfflineAllowance, RegisterOfflineVerdictRevocation},
    },
    metadata::Metadata,
    name::Name,
    offline::{
        OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineAllowanceCommitment, OfflineVerdictRevocation,
        OfflineVerdictRevocationReason, OfflineWalletCertificate, OfflineWalletPolicy,
    },
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec},
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use nonzero_ext::nonzero;
use norito::json::{self, Map, Value};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;
use urlencoding::encode;

#[tokio::test]
async fn offline_revocations_list_supports_filtering_and_sorting() {
    let harness = build_revocation_harness();
    let note_fixture = harness
        .fixtures
        .iter()
        .find(|fixture| fixture.note.is_some())
        .expect("fixture with note");
    let filter_json = json::to_string(&exists_filter("note")).expect("serialize filter");
    let uri = format!(
        "/v2/offline/revocations?sort=issuer_id:asc&filter={}",
        encode(&filter_json)
    );

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

    assert_eq!(body["total"].as_u64(), Some(1));
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(
        item["verdict_id_hex"].as_str(),
        Some(note_fixture.verdict_hex.as_str())
    );
    assert_eq!(
        item["issuer_id"].as_str(),
        Some(note_fixture.issuer_literal.as_str())
    );
    assert_eq!(
        item["issuer_display"].as_str(),
        Some(note_fixture.issuer_literal.as_str()),
        "issuer display should match canonical i105 literal"
    );
    assert_eq!(
        item["note"].as_str(),
        note_fixture.note.as_deref(),
        "note should be propagated"
    );
    assert!(
        item["metadata"]["device_serial"].is_string(),
        "metadata should include device serial key"
    );
}

#[tokio::test]
async fn offline_revocations_query_respects_sorting() {
    let harness = build_revocation_harness();
    let mut fixtures = harness.fixtures.clone();
    fixtures.sort_by_key(|fixture| fixture.revoked_at_ms);
    let reason_values: Vec<Value> = fixtures
        .iter()
        .map(|fixture| Value::from(fixture.reason.as_str()))
        .collect();
    let mut sort_entry = Map::new();
    sort_entry.insert("key".into(), Value::from("revoked_at_ms"));
    sort_entry.insert("order".into(), Value::from("asc"));
    let mut pagination = Map::new();
    pagination.insert("limit".into(), Value::from(10u64));
    pagination.insert("offset".into(), Value::from(0u64));
    let mut envelope_map = Map::new();
    envelope_map.insert("filter".into(), in_filter("reason", reason_values));
    envelope_map.insert("sort".into(), Value::Array(vec![Value::Object(sort_entry)]));
    envelope_map.insert("pagination".into(), Value::Object(pagination));
    envelope_map.insert("fetch_size".into(), Value::from(32u64));
    let envelope = Value::Object(envelope_map);

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri("/v2/offline/revocations/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json::to_vec(&envelope).expect("serialize envelope"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body: Value = json::from_slice(&bytes).expect("json");

    assert_eq!(body["total"].as_u64(), Some(fixtures.len() as u64));
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), fixtures.len());
    for (item, fixture) in items.iter().zip(fixtures.iter()) {
        assert_eq!(item["revoked_at_ms"].as_u64(), Some(fixture.revoked_at_ms));
        assert_eq!(
            item["issuer_display"].as_str(),
            Some(fixture.issuer_i105.as_str()),
            "issuer display should emit canonical i105 literal"
        );
        let metadata_present = item
            .as_object()
            .and_then(|map| map.get("metadata"))
            .is_some_and(Value::is_object);
        assert_eq!(
            metadata_present,
            !fixture.metadata_keys.is_empty(),
            "top-level metadata should appear only when fixture metadata is non-empty"
        );
    }
    assert!(
        items[0]["revoked_at_ms"].as_u64() < items[1]["revoked_at_ms"].as_u64(),
        "ascending sort should place older revocation first"
    );
}

struct RevocationTestHarness {
    app: Router,
    fixtures: Vec<SeededRevocation>,
}

#[derive(Clone)]
struct SeededRevocation {
    verdict_hex: String,
    issuer_literal: String,
    issuer_i105: String,
    reason: OfflineVerdictRevocationReason,
    note: Option<String>,
    revoked_at_ms: u64,
    metadata_keys: Vec<String>,
}

#[derive(Clone)]
struct RevocationSeed {
    certificate: OfflineWalletCertificate,
    revocation: OfflineVerdictRevocation,
    metadata_keys: Vec<String>,
}

fn build_revocation_harness() -> RevocationTestHarness {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let seeds = build_revocation_seeds();
    let world = world_from_revocation_seeds(&seeds);
    let state = Arc::new(State::new_for_testing(world, Arc::clone(&kura), query));

    let fixtures = seed_offline_revocations(&state, &seeds);

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

    RevocationTestHarness {
        app: torii.api_router_for_tests(),
        fixtures,
    }
}

fn seed_offline_revocations(state: &Arc<State>, seeds: &[RevocationSeed]) -> Vec<SeededRevocation> {
    // Register all allowances and the first revocation.
    let header_one = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_700_000_321, 0);
    {
        let mut block = state.block(header_one);
        let mut tx = block.transaction();
        for seed in seeds {
            Mint::asset_numeric(
                seed.certificate.allowance.amount.clone(),
                seed.certificate.allowance.asset.clone(),
            )
            .execute(&seed.certificate.controller, &mut tx)
            .expect("allowance prefund");
            RegisterOfflineAllowance {
                certificate: seed.certificate.clone(),
            }
            .execute(&seed.certificate.controller, &mut tx)
            .expect("allowance registration");
        }
        RegisterOfflineVerdictRevocation {
            revocation: seeds[0].revocation.clone(),
        }
        .execute(&seeds[0].certificate.controller, &mut tx)
        .expect("revocation registration");
        tx.apply();
        block.commit().expect("commit block one");
    }

    let mut fixtures = Vec::with_capacity(seeds.len());
    fixtures.push(make_seeded(&seeds[0], 1_700_000_321));

    // Second revocation in a newer block for deterministic ordering.
    let prev_hash = state.view().latest_block_hash();
    let header_two = BlockHeader::new(nonzero!(2_u64), prev_hash, None, None, 1_700_000_654, 0);
    {
        let mut block = state.block(header_two);
        let mut tx = block.transaction();
        RegisterOfflineVerdictRevocation {
            revocation: seeds[1].revocation.clone(),
        }
        .execute(&seeds[1].certificate.controller, &mut tx)
        .expect("revocation registration");
        tx.apply();
        block.commit().expect("commit block two");
    }
    fixtures.push(make_seeded(&seeds[1], 1_700_000_654));
    fixtures
}

fn world_from_revocation_seeds(seeds: &[RevocationSeed]) -> World {
    let first = seeds.first().expect("revocation seeds");
    let domain_id = first
        .certificate
        .allowance
        .asset
        .definition()
        .domain()
        .clone();

    let domain = Domain {
        id: domain_id,
        logo: None,
        metadata: Metadata::default(),
        owned_by: first.certificate.controller.clone(),
    };

    let mut accounts: Vec<Account> = Vec::new();
    for seed in seeds {
        accounts.push(Account {
            id: seed.certificate.controller.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
            linked_domains: BTreeSet::new(),
        });
        accounts.push(Account {
            id: seed.certificate.operator.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
            linked_domains: BTreeSet::new(),
        });
    }
    accounts.sort_by(|a, b| a.id.cmp(&b.id));
    accounts.dedup_by(|a, b| a.id == b.id);

    let mut asset_definition_metadata = Metadata::default();
    asset_definition_metadata.insert(
        Name::from_str(OFFLINE_ASSET_ENABLED_METADATA_KEY).expect("offline enabled metadata key"),
        true,
    );
    let asset_definition = AssetDefinition {
        id: first.certificate.allowance.asset.definition().clone(),
        name: "OfflineAsset".to_owned(),
        description: None,
        alias: None,
        spec: NumericSpec::integer(),
        mintable: Default::default(),
        logo: None,
        metadata: asset_definition_metadata,
        balance_scope_policy: Default::default(),
        owned_by: first.certificate.controller.clone(),
        total_quantity: Numeric::zero(),
        confidential_policy: Default::default(),
    };

    // `RegisterOfflineAllowance` seeding resolves the definition in order to evaluate
    // offline escrow requirements, so the harness must include it.
    World::with([domain], accounts, [asset_definition])
}

#[allow(clippy::too_many_lines)]
fn build_revocation_seeds() -> Vec<RevocationSeed> {
    let _domain = iroha_data_model::domain::DomainId::from_str("merchants").expect("domain id");
    let operator_keypair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let operator_account = AccountId::of(operator_keypair.public_key().clone());
    let controller_one = AccountId::of(
        KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let controller_two = AccountId::of(
        KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let spend_pair = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let asset_definition = AssetDefinitionId::new(
        "merchants".parse().expect("domain id"),
        "xor".parse().expect("asset definition name"),
    );

    let mut cert_one = OfflineWalletCertificate {
        controller: controller_one.clone(),
        operator: operator_account.clone(),
        allowance: OfflineAllowanceCommitment {
            asset: AssetId::new(asset_definition.clone(), controller_one.clone()),
            amount: Numeric::new(1_000, 0),
            commitment: vec![0xA1; 32],
        },
        spend_public_key: spend_pair.public_key().clone(),
        attestation_report: Vec::new(),
        issued_at_ms: 1_700_000_000,
        expires_at_ms: 1_700_100_000,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(1_000, 0),
            max_tx_value: Numeric::new(300, 0),
            expires_at_ms: 1_700_100_000,
        },
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata: Metadata::default(),
        verdict_id: Some(Hash::new(b"rev-seed-one")),
        attestation_nonce: None,
        refresh_at_ms: None,
    };
    cert_one.operator_signature = Signature::new(
        operator_keypair.private_key(),
        &cert_one.operator_signing_bytes().expect("payload"),
    );
    let mut metadata_one = Metadata::default();
    metadata_one.insert(
        Name::from_str("device_serial").expect("name"),
        Json::from("POS-1234"),
    );
    let rev_one = OfflineVerdictRevocation {
        verdict_id: cert_one.verdict_id.expect("verdict id"),
        issuer: controller_one.clone(),
        revoked_at_ms: 0,
        reason: OfflineVerdictRevocationReason::DeviceCompromised,
        note: Some(String::from("lost terminal")),
        metadata: metadata_one.clone(),
    };

    let mut cert_two = OfflineWalletCertificate {
        controller: controller_two.clone(),
        operator: operator_account.clone(),
        allowance: OfflineAllowanceCommitment {
            asset: AssetId::new(asset_definition, controller_two.clone()),
            amount: Numeric::new(2_000, 0),
            commitment: vec![0xB2; 32],
        },
        spend_public_key: spend_pair.public_key().clone(),
        attestation_report: Vec::new(),
        issued_at_ms: 1_700_000_100,
        expires_at_ms: 1_700_200_000,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(2_000, 0),
            max_tx_value: Numeric::new(500, 0),
            expires_at_ms: 1_700_200_000,
        },
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata: Metadata::default(),
        verdict_id: Some(Hash::new(b"rev-seed-two")),
        attestation_nonce: None,
        refresh_at_ms: None,
    };
    cert_two.operator_signature = Signature::new(
        operator_keypair.private_key(),
        &cert_two.operator_signing_bytes().expect("payload"),
    );
    let mut metadata_two = Metadata::default();
    metadata_two.insert(
        Name::from_str("ticket").expect("name"),
        Json::from(1001_u64),
    );
    let rev_two = OfflineVerdictRevocation {
        verdict_id: cert_two.verdict_id.expect("verdict id"),
        issuer: controller_two.clone(),
        revoked_at_ms: 0,
        reason: OfflineVerdictRevocationReason::IssuerRequest,
        note: None,
        metadata: metadata_two.clone(),
    };

    vec![
        RevocationSeed {
            certificate: cert_one,
            revocation: rev_one,
            metadata_keys: vec!["device_serial".into()],
        },
        RevocationSeed {
            certificate: cert_two,
            revocation: rev_two,
            metadata_keys: vec!["ticket".into()],
        },
    ]
}

fn make_seeded(seed: &RevocationSeed, revoked_at_ms: u64) -> SeededRevocation {
    let issuer_literal = seed.certificate.controller.to_string();
    SeededRevocation {
        verdict_hex: hex::encode(seed.revocation.verdict_id.as_ref()),
        issuer_literal: issuer_literal.clone(),
        issuer_i105: i105_literal(&seed.certificate.controller),
        reason: seed.revocation.reason,
        note: seed.revocation.note.clone(),
        revoked_at_ms,
        metadata_keys: seed.metadata_keys.clone(),
    }
}

fn i105_literal(account_id: &AccountId) -> String {
    account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .unwrap_or_else(|_| account_id.to_string())
}

fn exists_filter(field: &str) -> Value {
    let mut map = Map::new();
    map.insert("op".into(), Value::from("exists"));
    map.insert("args".into(), Value::from(field.to_string()));
    Value::Object(map)
}

fn in_filter(field: &str, values: Vec<Value>) -> Value {
    let mut map = Map::new();
    map.insert("op".into(), Value::from("in"));
    map.insert(
        "args".into(),
        Value::Array(vec![Value::from(field.to_string()), Value::Array(values)]),
    );
    Value::Object(map)
}
