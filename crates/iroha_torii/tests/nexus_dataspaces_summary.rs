#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii Nexus dataspaces account summary endpoint tests.
#![cfg(feature = "app_api")]

use std::{
    collections::HashSet,
    num::NonZeroU64,
    sync::{Arc, LazyLock, Mutex, MutexGuard},
};

use axum::{body::Body, http::Request};
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::Queue;
use iroha_core::{
    kura::Kura,
    nexus::space_directory::{
        SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet, UaidDataspaceBindings,
    },
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    sumeragi::{self, status},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    account::{AccountId, NewAccount},
    asset::{AssetDefinitionId, AssetId, NewAssetDefinition},
    block::BlockHeader,
    domain::{Domain, DomainId},
    isi::{Mint, Register},
    nexus::{
        Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceCatalog,
        DataSpaceId, DataSpaceMetadata, ManifestEffect, ManifestEntry, ManifestVersion,
        UniversalAccountId,
    },
    peer::PeerId,
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::ALICE_ID;
use iroha_torii::Torii;
use mv::storage::StorageReadOnly;
use norito::json::{self, Value};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

static CONSENSUS_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_returns_joined_snapshot() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = DomainId::try_new("nexus", "universal").expect("domain id");
    let account_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let account_literal = account_id.to_string();
    let i105_literal = account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("i105 account literal");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii::dataspaces::summary"));
    let dataspace = DataSpaceId::new(42);

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_710_000_000_000,
        activation_epoch: 100,
        expiry_epoch: Some(200),
        entries: vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: None,
                method: None,
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::new(1_000, 0)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("daily limit".to_owned()),
        }],
    };
    let mut record = SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(101);
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);

    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "retail".to_owned(),
            description: Some("Retail payments lane".to_owned()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    state.set_nexus(nexus).expect("set nexus config");

    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("domain id"),
        "xor".parse().expect("asset definition name"),
    );
    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();

    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");
    Register::account(NewAccount::new(account_id.clone()).with_uaid(Some(uaid)))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account with uaid");
    Register::asset_definition(NewAssetDefinition {
        id: asset_definition_id.clone(),
        name: "xor".to_owned(),
        description: None,
        alias: None,
        spec: Default::default(),
        mintable: Default::default(),
        logo: None,
        metadata: Default::default(),
        balance_scope_policy: Default::default(),
        confidential_policy: Default::default(),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");
    Mint::asset_numeric(
        500u64,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("mint asset");

    stx.apply();
    block.commit().expect("commit seeded state");

    sumeragi::status::set_lane_commitments(
        Vec::new(),
        vec![status::DataspaceCommitmentSnapshot {
            block_height: 123,
            lane_id: 7,
            dataspace_id: dataspace.as_u64(),
            tx_count: 2,
            total_chunks: 4,
            rbc_bytes_total: 640,
            teu_total: 320,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0xAB; Hash::LENGTH],
            )),
        }],
    );

    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/dataspaces/accounts/{}/summary",
                    urlencoding::encode(&account_literal)
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let payload: Value = json::from_slice(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(account_literal.as_str()));
    assert_eq!(payload["account"], Value::from(i105_literal.as_str()));
    assert_eq!(payload["uaid"], Value::from(uaid.to_string()));
    assert_eq!(payload["totals"]["dataspaces"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(1));
    assert_eq!(payload["totals"]["consensus_tx_count"], Value::from(2));

    let dataspaces = payload["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 1);
    let row = &dataspaces[0];
    assert_eq!(row["dataspace_id"], Value::from(dataspace.as_u64()));
    assert_eq!(row["dataspace_alias"], Value::from("retail"));
    assert_eq!(row["accounts"].as_array().expect("accounts").len(), 1);
    assert_eq!(
        row["accounts"][0],
        Value::from(i105_literal.as_str()),
        "dataspace row should render canonical I105 account literal"
    );
    assert_eq!(row["manifest"]["status"], Value::from("Active"));
    assert_eq!(row["portfolio"]["positions"], Value::from(1));
    assert_eq!(row["consensus"]["entries"], Value::from(1));
    assert_eq!(row["consensus"]["lane_ids"][0], Value::from(7));
    assert_eq!(row["consensus"]["last_block_height"], Value::from(123));

    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_returns_zeroed_snapshot_for_account_without_uaid() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(true);
    let account_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let account_literal = account_id.to_string();
    let i105_literal = account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("i105 account literal");

    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();
    Register::account(NewAccount::new(account_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account");
    stx.apply();
    block.commit().expect("commit account");

    let router = build_test_router(state, &kura, local_peer_id);
    let spaced_literal = format!("  {account_literal}  ");
    let literal = urlencoding::encode(&spaced_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
    let payload: Value = json::from_str(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(account_literal.as_str()));
    assert_eq!(payload["account"], Value::from(i105_literal.as_str()));
    assert!(payload["uaid"].is_null(), "uaid should be null: {body}");
    assert_eq!(payload["totals"]["dataspaces"], Value::from(0));
    assert_eq!(payload["totals"]["accounts_bound"], Value::from(0));
    assert_eq!(payload["totals"]["portfolio_accounts"], Value::from(0));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(0));
    assert_eq!(payload["totals"]["manifests_total"], Value::from(0));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_entries"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_tx_count"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_chunks_total"], Value::from(0));
    assert_eq!(
        payload["totals"]["consensus_rbc_bytes_total"],
        Value::from(0)
    );
    assert_eq!(payload["totals"]["consensus_teu_total"], Value::from(0));
    assert_eq!(
        payload["dataspaces"].as_array().expect("dataspaces"),
        &Vec::<Value>::new()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_reports_portfolio_only_default_dataspace() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(true);
    let account_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let account_literal = account_id.to_string();
    let i105_literal = account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("i105 account literal");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii::portfolio_only"));
    let domain_id: DomainId = DomainId::try_new("portfolio-only", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::new(
        domain_id.clone(),
        "rose".parse().expect("asset definition name"),
    );

    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");
    Register::account(NewAccount::new(account_id.clone()).with_uaid(Some(uaid)))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account with uaid");
    Register::asset_definition(NewAssetDefinition {
        id: definition_id.clone(),
        name: "rose".to_owned(),
        description: None,
        alias: None,
        spec: Default::default(),
        mintable: Default::default(),
        logo: None,
        metadata: Default::default(),
        balance_scope_policy: Default::default(),
        confidential_policy: Default::default(),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");
    Mint::asset_numeric(25u64, AssetId::new(definition_id, account_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint asset");
    stx.apply();
    block.commit().expect("commit seeded state");

    let router = build_test_router(state, &kura, local_peer_id);
    let literal = urlencoding::encode(&account_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
    let payload: Value = json::from_str(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(account_literal.as_str()));
    assert_eq!(payload["account"], Value::from(i105_literal.as_str()));
    assert_eq!(payload["uaid"], Value::from(uaid.to_string()));
    assert_eq!(payload["totals"]["dataspaces"], Value::from(1));
    assert_eq!(payload["totals"]["accounts_bound"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_accounts"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_total"], Value::from(0));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_entries"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_tx_count"], Value::from(0));

    let dataspaces = payload["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 1);
    let row = &dataspaces[0];
    assert_eq!(
        row["dataspace_id"],
        Value::from(DataSpaceId::UNIVERSAL.as_u64())
    );
    assert_eq!(row["dataspace_alias"], Value::from("universal"));
    assert_eq!(
        row["accounts"].as_array().expect("accounts"),
        &vec![Value::from(i105_literal.as_str())]
    );
    assert_eq!(row["portfolio"]["accounts"], Value::from(1));
    assert_eq!(row["portfolio"]["positions"], Value::from(1));
    assert_eq!(row["portfolio"]["asset_definitions"], Value::from(1));
    assert_eq!(row["manifest"]["status"], Value::from("Missing"));
    assert_eq!(row["consensus"]["entries"], Value::from(0));
    assert!(
        row["consensus"]["last_block_height"].is_null(),
        "expected null last block height: {body}"
    );
    assert_eq!(
        row["consensus"]["details"].as_array().expect("details"),
        &Vec::<Value>::new()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_reports_pending_expired_and_revoked_manifests() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let pending_dataspace = DataSpaceId::new(7);
    let expired_dataspace = DataSpaceId::new(8);
    let revoked_dataspace = DataSpaceId::new(9);
    let account_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let account_literal = account_id.to_string();
    let i105_literal = account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("i105 account literal");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii::manifest_states"));

    let manifest_for = |dataspace: DataSpaceId, issued_ms: u64| AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms,
        activation_epoch: 10,
        expiry_epoch: Some(30),
        entries: Vec::new(),
    };

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());

    let mut bindings = UaidDataspaceBindings::default();
    for dataspace in [pending_dataspace, expired_dataspace, revoked_dataspace] {
        bindings.bind_account(dataspace, account_id.clone());
    }
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let pending_record =
        SpaceDirectoryManifestRecord::new(manifest_for(pending_dataspace, 1_710_000_000_000));
    let mut expired_record =
        SpaceDirectoryManifestRecord::new(manifest_for(expired_dataspace, 1_710_000_000_100));
    expired_record.lifecycle.mark_activated(11);
    expired_record.lifecycle.mark_expired(22);
    let mut revoked_record =
        SpaceDirectoryManifestRecord::new(manifest_for(revoked_dataspace, 1_710_000_000_200));
    revoked_record.lifecycle.mark_activated(12);
    revoked_record
        .lifecycle
        .mark_revoked(23, Some("operator request".to_owned()));
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(pending_record);
    set.upsert(expired_record);
    set.upsert(revoked_record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: pending_dataspace,
            alias: "pending".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: expired_dataspace,
            alias: "expired".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: revoked_dataspace,
            alias: "revoked".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    state.set_nexus(nexus).expect("set nexus config");

    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();
    Register::account(NewAccount::new(account_id.clone()).with_uaid(Some(uaid)))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account with uaid");
    stx.apply();
    block.commit().expect("commit account");

    sumeragi::status::set_lane_commitments(
        Vec::new(),
        vec![status::DataspaceCommitmentSnapshot {
            block_height: 77,
            lane_id: 5,
            dataspace_id: DataSpaceId::new(99).as_u64(),
            tx_count: 9,
            total_chunks: 18,
            rbc_bytes_total: 900,
            teu_total: 450,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0xCD; Hash::LENGTH],
            )),
        }],
    );

    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let literal = urlencoding::encode(&account_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
    let payload: Value = json::from_str(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(account_literal.as_str()));
    assert_eq!(payload["account"], Value::from(i105_literal.as_str()));
    assert_eq!(payload["uaid"], Value::from(uaid.to_string()));
    assert_eq!(payload["totals"]["dataspaces"], Value::from(4));
    assert_eq!(payload["totals"]["accounts_bound"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_accounts"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(0));
    assert_eq!(payload["totals"]["manifests_total"], Value::from(3));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_entries"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_tx_count"], Value::from(0));
    assert_eq!(payload["totals"]["consensus_chunks_total"], Value::from(0));
    assert_eq!(
        payload["totals"]["consensus_rbc_bytes_total"],
        Value::from(0)
    );
    assert_eq!(payload["totals"]["consensus_teu_total"], Value::from(0));

    let dataspaces = payload["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 4);

    let universal = &dataspaces[0];
    assert_eq!(
        universal["dataspace_id"],
        Value::from(DataSpaceId::UNIVERSAL.as_u64())
    );
    assert_eq!(universal["dataspace_alias"], Value::from("universal"));
    assert_eq!(
        universal["accounts"].as_array().expect("accounts"),
        &vec![Value::from(i105_literal.as_str())]
    );
    assert_eq!(universal["manifest"]["status"], Value::from("Missing"));
    assert_eq!(universal["portfolio"]["accounts"], Value::from(1));
    assert_eq!(universal["portfolio"]["positions"], Value::from(0));
    assert_eq!(universal["consensus"]["entries"], Value::from(0));

    let pending = &dataspaces[1];
    assert_eq!(
        pending["dataspace_id"],
        Value::from(pending_dataspace.as_u64())
    );
    assert_eq!(pending["dataspace_alias"], Value::from("pending"));
    assert_eq!(pending["accounts"].as_array().expect("accounts").len(), 0);
    assert_eq!(pending["manifest"]["status"], Value::from("Pending"));
    assert!(pending["manifest"]["activated_epoch"].is_null());
    assert!(pending["manifest"]["expired_epoch"].is_null());
    assert!(pending["manifest"]["revoked_epoch"].is_null());
    assert_eq!(pending["portfolio"]["accounts"], Value::from(0));
    assert_eq!(pending["portfolio"]["positions"], Value::from(0));
    assert_eq!(pending["consensus"]["entries"], Value::from(0));

    let expired = &dataspaces[2];
    assert_eq!(
        expired["dataspace_id"],
        Value::from(expired_dataspace.as_u64())
    );
    assert_eq!(expired["dataspace_alias"], Value::from("expired"));
    assert_eq!(expired["accounts"].as_array().expect("accounts").len(), 0);
    assert_eq!(expired["manifest"]["status"], Value::from("Expired"));
    assert_eq!(expired["manifest"]["activated_epoch"], Value::from(11));
    assert_eq!(expired["manifest"]["expired_epoch"], Value::from(22));
    assert!(expired["manifest"]["revoked_epoch"].is_null());
    assert_eq!(expired["portfolio"]["accounts"], Value::from(0));
    assert_eq!(expired["consensus"]["entries"], Value::from(0));

    let revoked = &dataspaces[3];
    assert_eq!(
        revoked["dataspace_id"],
        Value::from(revoked_dataspace.as_u64())
    );
    assert_eq!(revoked["dataspace_alias"], Value::from("revoked"));
    assert_eq!(revoked["accounts"].as_array().expect("accounts").len(), 0);
    assert_eq!(revoked["manifest"]["status"], Value::from("Revoked"));
    assert_eq!(revoked["manifest"]["activated_epoch"], Value::from(12));
    assert_eq!(revoked["manifest"]["revoked_epoch"], Value::from(23));
    assert_eq!(
        revoked["manifest"]["revoked_reason"],
        Value::from("operator request")
    );
    assert_eq!(revoked["portfolio"]["accounts"], Value::from(0));
    assert_eq!(revoked["consensus"]["entries"], Value::from(0));

    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_reports_null_alias_for_uncataloged_dataspace() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let dataspace = DataSpaceId::new(404);
    let account_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let account_literal = account_id.to_string();
    let i105_literal = account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("i105 account literal");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii::uncataloged_alias"));
    let domain_id: DomainId = DomainId::try_new("uncataloged", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::new(
        domain_id.clone(),
        "lotus".parse().expect("asset definition name"),
    );

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let mut bindings = UaidDataspaceBindings::default();
    bindings.bind_account(dataspace, account_id.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    state.set_nexus(nexus).expect("set nexus config");

    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");
    Register::account(NewAccount::new(account_id.clone()).with_uaid(Some(uaid)))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account with uaid");
    Register::asset_definition(NewAssetDefinition {
        id: definition_id.clone(),
        name: "lotus".to_owned(),
        description: None,
        alias: None,
        spec: Default::default(),
        mintable: Default::default(),
        logo: None,
        metadata: Default::default(),
        balance_scope_policy: Default::default(),
        confidential_policy: Default::default(),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");
    Mint::asset_numeric(9u64, AssetId::new(definition_id, account_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("mint asset");
    stx.apply();
    block.commit().expect("commit seeded state");

    let mut bindings = UaidDataspaceBindings::default();
    bindings.bind_account(dataspace, account_id.clone());
    state
        .world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_710_000_222_000,
        activation_epoch: 70,
        expiry_epoch: Some(170),
        entries: Vec::new(),
    };
    let mut record = SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(71);
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    state
        .world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let literal = urlencoding::encode(&account_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
    let payload: Value = json::from_str(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(account_literal.as_str()));
    assert_eq!(payload["account"], Value::from(i105_literal.as_str()));
    assert_eq!(payload["uaid"], Value::from(uaid.to_string()));
    assert_eq!(payload["totals"]["dataspaces"], Value::from(1));
    assert_eq!(payload["totals"]["accounts_bound"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_accounts"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_total"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(1));
    assert_eq!(payload["totals"]["consensus_entries"], Value::from(0));

    let dataspaces = payload["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 1);
    let row = &dataspaces[0];
    assert_eq!(row["dataspace_id"], Value::from(dataspace.as_u64()));
    assert!(
        row["dataspace_alias"].is_null(),
        "expected null alias for uncataloged dataspace: {body}"
    );
    assert_eq!(
        row["accounts"].as_array().expect("accounts"),
        &vec![Value::from(i105_literal.as_str())]
    );
    assert_eq!(row["portfolio"]["accounts"], Value::from(1));
    assert_eq!(row["portfolio"]["positions"], Value::from(1));
    assert_eq!(row["portfolio"]["asset_definitions"], Value::from(1));
    assert_eq!(row["manifest"]["status"], Value::from("Active"));
    assert_eq!(row["consensus"]["entries"], Value::from(0));
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_merges_bound_accounts_and_consensus_totals() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let dataspace = DataSpaceId::new(52);
    let primary_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let primary_account_id = AccountId::new(primary_keypair.public_key().clone());
    let primary_literal = primary_account_id.to_string();
    let primary_i105_literal = primary_account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("primary i105 account literal");
    let secondary_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let secondary_account_id = AccountId::new(secondary_keypair.public_key().clone());
    let secondary_i105_literal = secondary_account_id
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("secondary i105 account literal");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii::binding_consensus_merge"));
    let domain_id: DomainId = DomainId::try_new("multi-bindings", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::new(
        domain_id.clone(),
        "cedar".parse().expect("asset definition name"),
    );

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_710_000_111_000,
        activation_epoch: 50,
        expiry_epoch: Some(150),
        entries: Vec::new(),
    };
    let mut record = SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(51);
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "retail".to_owned(),
            description: Some("Retail routed dataspace".to_owned()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    state.set_nexus(nexus).expect("set nexus config");

    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");
    Register::account(NewAccount::new(primary_account_id.clone()).with_uaid(Some(uaid)))
        .execute(&ALICE_ID, &mut stx)
        .expect("register primary account with uaid");
    Register::account(NewAccount::new(secondary_account_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register secondary account");
    Register::asset_definition(NewAssetDefinition {
        id: definition_id.clone(),
        name: "cedar".to_owned(),
        description: None,
        alias: None,
        spec: Default::default(),
        mintable: Default::default(),
        logo: None,
        metadata: Default::default(),
        balance_scope_policy: Default::default(),
        confidential_policy: Default::default(),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect("register asset definition");
    Mint::asset_numeric(
        13u64,
        AssetId::new(definition_id, primary_account_id.clone()),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("mint asset");
    stx.apply();
    block.commit().expect("commit seeded state");

    let mut bindings = state
        .view()
        .world()
        .uaid_dataspaces()
        .get(&uaid)
        .cloned()
        .expect("bindings should exist after active manifest registration");
    bindings.bind_account(dataspace, secondary_account_id.clone());
    state
        .world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    sumeragi::status::set_lane_commitments(
        Vec::new(),
        vec![
            status::DataspaceCommitmentSnapshot {
                block_height: 41,
                lane_id: 9,
                dataspace_id: dataspace.as_u64(),
                tx_count: 3,
                total_chunks: 5,
                rbc_bytes_total: 500,
                teu_total: 250,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xD1; Hash::LENGTH],
                )),
            },
            status::DataspaceCommitmentSnapshot {
                block_height: 42,
                lane_id: 3,
                dataspace_id: dataspace.as_u64(),
                tx_count: 4,
                total_chunks: 6,
                rbc_bytes_total: 600,
                teu_total: 300,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xD2; Hash::LENGTH],
                )),
            },
        ],
    );

    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let literal = urlencoding::encode(&primary_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::OK, "unexpected body: {body}");
    let payload: Value = json::from_str(&body).expect("json payload");

    assert_eq!(payload["account_id"], Value::from(primary_literal.as_str()));
    assert_eq!(
        payload["account"],
        Value::from(primary_i105_literal.as_str())
    );
    assert_eq!(payload["uaid"], Value::from(uaid.to_string()));
    assert_eq!(payload["totals"]["dataspaces"], Value::from(1));
    assert_eq!(payload["totals"]["accounts_bound"], Value::from(2));
    assert_eq!(payload["totals"]["portfolio_accounts"], Value::from(1));
    assert_eq!(payload["totals"]["portfolio_positions"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_total"], Value::from(1));
    assert_eq!(payload["totals"]["manifests_active"], Value::from(1));
    assert_eq!(payload["totals"]["consensus_entries"], Value::from(2));
    assert_eq!(payload["totals"]["consensus_tx_count"], Value::from(7));
    assert_eq!(payload["totals"]["consensus_chunks_total"], Value::from(11));
    assert_eq!(
        payload["totals"]["consensus_rbc_bytes_total"],
        Value::from(1_100)
    );
    assert_eq!(payload["totals"]["consensus_teu_total"], Value::from(550));

    let dataspaces = payload["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 1);
    let row = &dataspaces[0];
    assert_eq!(row["dataspace_id"], Value::from(dataspace.as_u64()));
    assert_eq!(row["dataspace_alias"], Value::from("retail"));
    let accounts: HashSet<_> = row["accounts"]
        .as_array()
        .expect("accounts")
        .iter()
        .map(|value| value.as_str().expect("account string").to_owned())
        .collect();
    assert_eq!(
        accounts,
        HashSet::from([primary_i105_literal.clone(), secondary_i105_literal.clone(),])
    );
    assert_eq!(row["portfolio"]["accounts"], Value::from(1));
    assert_eq!(row["portfolio"]["positions"], Value::from(1));
    assert_eq!(row["portfolio"]["asset_definitions"], Value::from(1));
    assert_eq!(row["manifest"]["status"], Value::from("Active"));
    assert_eq!(row["consensus"]["entries"], Value::from(2));
    assert_eq!(row["consensus"]["tx_count"], Value::from(7));
    assert_eq!(row["consensus"]["total_chunks"], Value::from(11));
    assert_eq!(row["consensus"]["last_block_height"], Value::from(42));
    assert_eq!(
        row["consensus"]["lane_ids"].as_array().expect("lane ids"),
        &vec![Value::from(3_u64), Value::from(9_u64)]
    );
    assert_eq!(
        row["consensus"]["details"].as_array().expect("details")[0]["lane_id"],
        Value::from(3_u64)
    );

    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_rejects_invalid_account_literal() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(true);
    let router = build_test_router(state, &kura, local_peer_id);
    let (status, body) = request_summary(
        router,
        "/v1/nexus/dataspaces/accounts/not-a-valid-literal/summary",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("invalid account literal"),
        "expected invalid account literal message, got: {body}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_rejects_empty_account_literal() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(true);
    let router = build_test_router(state, &kura, local_peer_id);
    let (status, body) =
        request_summary(router, "/v1/nexus/dataspaces/accounts/%20%20/summary").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("account literal must not be empty"),
        "expected empty account literal error, got: {body}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_returns_not_found_for_missing_account() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(true);
    let router = build_test_router(state, &kura, local_peer_id);
    let account_literal = valid_missing_account_literal();
    let literal = urlencoding::encode(&account_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, _body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "current_thread")]
async fn nexus_dataspaces_summary_endpoint_rejects_when_nexus_disabled() {
    let _guard = consensus_guard();
    sumeragi::status::set_lane_commitments(Vec::new(), Vec::new());

    let (state, kura, local_peer_id) = minimal_state(false);
    let router = build_test_router(state, &kura, local_peer_id);
    let account_literal = valid_missing_account_literal();
    let literal = urlencoding::encode(&account_literal);
    let uri = format!("/v1/nexus/dataspaces/accounts/{literal}/summary");
    let (status, body) = request_summary(router, &uri).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("nexus_disabled"),
        "expected nexus_disabled code, got: {body}"
    );
}

fn minimal_state(nexus_enabled: bool) -> (Arc<State>, Arc<Kura>, PeerId) {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = nexus_enabled;
    state.set_nexus(nexus).expect("set nexus config");

    (Arc::new(state), kura, local_peer_id)
}

async fn request_summary(router: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = String::from_utf8_lossy(&bytes).to_string();
    (status, body)
}

fn consensus_guard() -> MutexGuard<'static, ()> {
    CONSENSUS_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn valid_missing_account_literal() -> String {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    AccountId::new(key_pair.public_key().clone()).to_string()
}

fn build_test_router(state: Arc<State>, kura: &Arc<Kura>, local_peer_id: PeerId) -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = iroha_core::kiso::KisoHandle::start(cfg.clone());
    let queue_cfg = Queue::default();
    let (events_tx, _events_rx) = broadcast::channel(1);
    let queue = Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events_tx));
    let (_peers_tx, peers_rx) = watch::channel(HashSet::default());
    #[cfg(not(feature = "telemetry"))]
    let _ = local_peer_id;

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry;
        use iroha_primitives::time::TimeSource;

        let metrics = fixtures::shared_metrics();
        let (_guard, mock_time) = TimeSource::new_mock(core::time::Duration::default());
        telemetry::start(
            metrics,
            Arc::clone(&state),
            Arc::clone(kura),
            Arc::clone(&queue),
            peers_rx.clone(),
            local_peer_id,
            mock_time,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                Arc::clone(kura),
                state,
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                Arc::clone(kura),
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    torii.api_router_for_tests()
}

fn block_header(height: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("height must be non-zero"),
        None,
        None,
        None,
        height,
        0,
    )
}
