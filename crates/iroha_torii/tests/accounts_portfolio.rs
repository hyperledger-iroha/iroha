#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii UAID portfolio endpoint tests.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, http::Request};
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
use iroha_data_model::{
    account::NewAccount,
    asset::{AssetDefinition, AssetId},
    block::BlockHeader,
    isi::{Mint, Register},
    metadata::Metadata,
    nexus::UniversalAccountId,
    peer::PeerId,
    prelude::*,
    sns::{NameControllerV1, NameRecordV1},
};
use iroha_test_samples::ALICE_ID;
use iroha_torii::Torii;
use nonzero_ext::nonzero;
use norito::json::{self, Value};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

const ACCOUNT_SIGNATORY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

#[tokio::test]
async fn accounts_portfolio_endpoint_returns_snapshot() {
    let (app, uaid) = setup_portfolio_app(|state| {
        let (uaid, _accounts) = seed_portfolio_accounts(state);
        uaid
    });
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/uaid:{uaid_hex}/portfolio"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    assert_eq!(value["totals"]["accounts"], Value::from(1));
    assert_eq!(value["totals"]["positions"], Value::from(2));
    let mut observed_quantities: Vec<String> = value["dataspaces"][0]["accounts"]
        .as_array()
        .expect("accounts array")
        .iter()
        .filter_map(|account| account["assets"].as_array())
        .flat_map(|assets| assets.iter())
        .filter_map(|asset| asset["quantity"].as_str())
        .map(ToOwned::to_owned)
        .collect();
    observed_quantities.sort();
    assert_eq!(
        observed_quantities,
        vec!["250".to_string(), "500".to_string()]
    );
}

#[tokio::test]
async fn accounts_portfolio_snapshot_matches_fixture() {
    let (app, uaid) = setup_portfolio_app(seed_fixture_portfolio_accounts);
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/uaid:{uaid_hex}/portfolio"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    let fixture: Value = json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/nexus/uaid_portfolio/global_default_portfolio.json"
    )))
    .expect("portfolio fixture JSON");
    assert_eq!(value, fixture);
}

#[tokio::test]
async fn accounts_portfolio_filters_by_asset_id() {
    let (app, uaid) = setup_portfolio_app(|state| {
        let (uaid, _accounts) = seed_portfolio_accounts(state);
        uaid
    });
    let uaid_hex = hex::encode(uaid.as_hash().as_ref());
    let baseline = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/accounts/uaid:{uaid_hex}/portfolio"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(baseline.status(), StatusCode::OK);
    let baseline_body = baseline.into_body().collect().await.unwrap().to_bytes();
    let baseline_value: Value = json::from_slice(&baseline_body).expect("valid portfolio payload");
    let asset_id = baseline_value["dataspaces"][0]["accounts"][0]["assets"][0]["asset_id"]
        .as_str()
        .expect("baseline asset id")
        .to_owned();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/accounts/uaid:{uaid_hex}/portfolio?asset_id={}",
                    urlencoding::encode(&asset_id)
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = json::from_slice(&body).expect("valid portfolio payload");
    assert_eq!(value["totals"]["accounts"], Value::from(1));
    assert_eq!(value["totals"]["positions"], Value::from(1));
    let assets = value["dataspaces"][0]["accounts"][0]["assets"]
        .as_array()
        .expect("assets array");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["asset_id"], Value::from(asset_id));
}

fn setup_portfolio_app<SeedFn>(seed_fn: SeedFn) -> (Router, UniversalAccountId)
where
    SeedFn: FnOnce(&Arc<State>) -> UniversalAccountId,
{
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let uaid = seed_fn(&state);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
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
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
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
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    let app = torii.api_router_for_tests();
    (app, uaid)
}

fn seed_portfolio_accounts(state: &Arc<State>) -> (UniversalAccountId, Vec<AccountId>) {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii_portfolio"));
    let domain_id: DomainId = "portfolio".parse().unwrap();
    let cash_id = AssetDefinitionId::new(domain_id.clone(), "cash".parse().unwrap());
    let points_id = AssetDefinitionId::new(domain_id.clone(), "points".parse().unwrap());
    let first_account = account_id_from_signatory(domain_id.clone(), ACCOUNT_SIGNATORY);
    let register: [InstructionBox; 6] = [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(
            NewAccount::new_in_domain(first_account.clone(), domain_id.clone())
                .with_uaid(Some(uaid)),
        )
        .into(),
        Register::asset_definition(AssetDefinition::numeric(cash_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(points_id.clone())).into(),
        Mint::asset_numeric(500u64, AssetId::new(cash_id, first_account.clone())).into(),
        Mint::asset_numeric(250u64, AssetId::new(points_id, first_account.clone())).into(),
    ];

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let selector =
        iroha_core::sns::selector_for_domain(&domain_id).expect("domain selector for fixtures");
    let address = iroha_data_model::account::AccountAddress::from_account_id(&ALICE_ID)
        .expect("fixture authority address");
    let record = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        4_000_000_000_000,
        4_100_000_000_000,
        4_200_000_000_000,
        Metadata::default(),
    );
    tx.world_mut_for_testing()
        .smart_contract_state_mut_for_testing()
        .insert(
            iroha_core::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    for instr in register {
        instr
            .execute(&ALICE_ID, &mut tx)
            .expect("registering portfolio fixtures");
    }
    tx.apply();
    block.commit().unwrap();

    (uaid, vec![first_account])
}

fn seed_fixture_portfolio_accounts(state: &Arc<State>) -> UniversalAccountId {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::torii_fixture"));
    let domain_id: DomainId = "portfolio_fixture".parse().unwrap();
    let cash_id = AssetDefinitionId::new(domain_id.clone(), "cash".parse().unwrap());
    let points_id = AssetDefinitionId::new(domain_id.clone(), "points".parse().unwrap());
    let first_account = account_id_from_seed(&domain_id, 0x11);
    let register: [InstructionBox; 6] = [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(
            NewAccount::new_in_domain(first_account.clone(), domain_id.clone())
                .with_uaid(Some(uaid)),
        )
        .into(),
        Register::asset_definition(AssetDefinition::numeric(cash_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(points_id.clone())).into(),
        Mint::asset_numeric(875u64, AssetId::new(cash_id, first_account.clone())).into(),
        Mint::asset_numeric(320u64, AssetId::new(points_id, first_account.clone())).into(),
    ];

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let selector =
        iroha_core::sns::selector_for_domain(&domain_id).expect("domain selector for fixtures");
    let address = iroha_data_model::account::AccountAddress::from_account_id(&ALICE_ID)
        .expect("fixture authority address");
    let record = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        4_000_000_000_000,
        4_100_000_000_000,
        4_200_000_000_000,
        Metadata::default(),
    );
    tx.world_mut_for_testing()
        .smart_contract_state_mut_for_testing()
        .insert(
            iroha_core::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    for instr in register {
        instr
            .execute(&ALICE_ID, &mut tx)
            .expect("registering portfolio fixture");
    }
    tx.apply();
    block.commit().unwrap();

    uaid
}

fn account_id_from_signatory(_domain_id: DomainId, signatory_hex: &str) -> AccountId {
    let normalized = signatory_hex
        .strip_prefix("ed0120")
        .unwrap_or(signatory_hex);
    let public_key = PublicKey::from_hex(Algorithm::Ed25519, normalized)
        .expect("signatory literal parses into a public key");
    AccountId::new(public_key)
}

fn account_id_from_seed(_domain_id: &DomainId, seed_byte: u8) -> AccountId {
    let seed = vec![seed_byte; 32];
    let (public, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
    AccountId::new(public)
}
