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
    state::{State, World},
    sumeragi::{self, status},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    account::{AccountId, NewAccount},
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
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

    let domain_id: DomainId = "nexus".parse().expect("domain id");
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
    let mut bindings = UaidDataspaceBindings::default();
    bindings.bind_account(dataspace, account_id.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

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
        "nexus".parse().expect("domain id"),
        "xor".parse().expect("asset definition name"),
    );
    let mut block = state.block(block_header(1));
    let mut stx = block.transaction();

    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .expect("register domain");
    Register::account(
        NewAccount::new_in_domain(account_id.clone(), domain_id.clone()).with_uaid(Some(uaid)),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect("register account with uaid");
    Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()))
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
