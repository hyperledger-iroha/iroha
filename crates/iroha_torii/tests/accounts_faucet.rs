#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account faucet tests.
#![cfg(feature = "app_api")]

use std::{num::NonZeroU64, sync::Arc};

use axum::http::Request;
use http::StatusCode;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute as _,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    peer::PeerId,
    prelude::{Account, AssetDefinition, Domain, ExposedPrivateKey, Mint},
};
use iroha_torii::{Torii, json_entry, json_object};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

struct FaucetTestContext {
    app: axum::Router,
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: iroha_data_model::ChainId,
    asset_definition_id: AssetDefinitionId,
    authority_id: AccountId,
    user_id: AccountId,
}

fn build_faucet_test_context(prefund_user: bool) -> FaucetTestContext {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = "sora".parse().expect("domain id");
    let asset_definition_id =
        AssetDefinitionId::new(domain_id.clone(), "xor".parse().expect("asset name"));
    let authority_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let user_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let user_id = AccountId::new(user_kp.public_key().clone());

    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account =
        Account::new(authority_id.clone().to_account_id(domain_id.clone())).build(&authority_id);
    let user_account =
        Account::new(user_id.clone().to_account_id(domain_id.clone())).build(&authority_id);
    let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
        .with_name("XOR".to_owned())
        .build(&authority_id);

    let mut world = World::with(
        [domain],
        [authority_account, user_account],
        [asset_definition],
    );
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));

    {
        let height_u64 = u64::try_from(state.view().height())
            .unwrap_or(0)
            .saturating_add(1);
        let header = BlockHeader::new(
            NonZeroU64::new(height_u64).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();
        Mint::asset_numeric(
            50_000_u32,
            AssetId::new(asset_definition_id.clone(), authority_id.clone()),
        )
        .execute(&authority_id, &mut stx)
        .expect("mint faucet balance");
        if prefund_user {
            Mint::asset_numeric(
                1_u32,
                AssetId::new(asset_definition_id.clone(), user_id.clone()),
            )
            .execute(&authority_id, &mut stx)
            .expect("mint user balance");
        }
        stx.apply();
        block.commit().expect("commit seeded balances");
    }

    cfg.torii.faucet = Some(iroha_config::parameters::actual::ToriiFaucet {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        asset_definition_id: asset_definition_id.clone(),
        amount: 25_000_u32.into(),
    });

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
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

    let chain_id = iroha_data_model::ChainId::from("test-chain");
    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                chain_id.clone(),
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

    FaucetTestContext {
        app: torii.api_router_for_tests(),
        state,
        queue,
        chain_id,
        asset_definition_id,
        authority_id,
        user_id,
    }
}

#[tokio::test]
async fn accounts_faucet_transfers_starter_balance_to_empty_account() {
    let FaucetTestContext {
        app,
        state,
        queue,
        chain_id,
        asset_definition_id,
        authority_id,
        user_id,
    } = build_faucet_test_context(false);

    let body = json_object(vec![json_entry("account_id", user_id.to_string())]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts/faucet")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .expect("faucet response");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let expected_height = u64::try_from(state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    let applied = iroha_torii::test_utils::apply_queued_in_one_block(
        &state,
        &queue,
        &chain_id,
        expected_height,
    );
    assert!(applied > 0);

    let view = state.view();
    let user_asset = view
        .world()
        .asset(&AssetId::new(asset_definition_id.clone(), user_id.clone()))
        .expect("user faucet asset");
    assert_eq!(user_asset.value().as_ref().to_string(), "25000");
    let authority_asset = view
        .world()
        .asset(&AssetId::new(asset_definition_id, authority_id))
        .expect("authority faucet asset");
    assert_eq!(authority_asset.value().as_ref().to_string(), "25000");
}

#[tokio::test]
async fn accounts_faucet_rejects_prefunded_accounts() {
    let FaucetTestContext { app, user_id, .. } = build_faucet_test_context(true);

    let body = json_object(vec![json_entry("account_id", user_id.to_string())]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts/faucet")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .expect("faucet response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
