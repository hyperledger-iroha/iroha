#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account faucet tests.
#![cfg(feature = "app_api")]

use std::{borrow::Cow, sync::Arc};

use axum::{body::to_bytes, http::Request};
use http::StatusCode;
use iroha_core::{
    block::BlockBuilder,
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::{AcceptedTransaction, TransactionBuilder},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    peer::PeerId,
    prelude::{Account, AssetDefinition, Domain, ExposedPrivateKey, InstructionBox, Mint},
};
use iroha_torii::{Torii, json_entry, json_object};
use sha2::{Digest as _, Sha256};
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
    pow_difficulty_bits: u8,
    pow_max_anchor_age_blocks: u64,
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
    let chain_id = iroha_data_model::ChainId::from("test-chain");

    {
        let mut seed_instructions: Vec<InstructionBox> = vec![
            Mint::asset_numeric(
                50_000_u32,
                AssetId::new(asset_definition_id.clone(), authority_id.clone()),
            )
            .into(),
        ];
        if prefund_user {
            seed_instructions.push(
                Mint::asset_numeric(
                    1_u32,
                    AssetId::new(asset_definition_id.clone(), user_id.clone()),
                )
                .into(),
            );
        }

        let seed_tx = TransactionBuilder::new(chain_id.clone(), authority_id.clone())
            .with_instructions(seed_instructions)
            .sign(authority_kp.private_key());
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let unverified = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
            seed_tx,
        ))])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
        let mut state_block = state.block(unverified.header());
        state_block.chain_id = chain_id.clone();
        let valid = unverified
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let committed = valid.commit_unchecked().unpack(|_| {});
        iroha_torii::test_utils::finalize_committed_block(&state, state_block, committed);
    }

    let pow_difficulty_bits = 10;
    let pow_max_anchor_age_blocks = 4;
    cfg.torii.faucet = Some(iroha_config::parameters::actual::ToriiFaucet {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        asset_definition_id: asset_definition_id.clone(),
        amount: 25_000_u32.into(),
        pow_difficulty_bits,
        pow_max_anchor_age_blocks: std::num::NonZeroU64::new(pow_max_anchor_age_blocks)
            .expect("non-zero faucet pow anchor age"),
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
        pow_difficulty_bits,
        pow_max_anchor_age_blocks,
    }
}

const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v1";

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0u32;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
            continue;
        }
        total += byte.leading_zeros();
        break;
    }
    total
}

fn solve_faucet_pow(state: &State, account_id: &AccountId, difficulty_bits: u8) -> (u64, String) {
    let anchor_height = u64::try_from(state.committed_height()).expect("height fits");
    let anchor_block = state
        .block_by_height(
            usize::try_from(anchor_height)
                .ok()
                .and_then(std::num::NonZeroUsize::new)
                .expect("non-zero height"),
        )
        .expect("anchor block");
    let anchor_hash = anchor_block.hash();

    for nonce in 0u64.. {
        let nonce_bytes = nonce.to_be_bytes();
        let mut hasher = Sha256::new();
        hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
        hasher.update(account_id.to_string().as_bytes());
        hasher.update(anchor_height.to_be_bytes());
        hasher.update(anchor_hash.as_ref());
        hasher.update(nonce_bytes);
        let digest: [u8; 32] = hasher.finalize().into();
        if leading_zero_bits(&digest) >= u32::from(difficulty_bits) {
            return (anchor_height, hex::encode(nonce_bytes));
        }
    }

    unreachable!("u64 nonce space exhausted");
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
        pow_difficulty_bits,
        ..
    } = build_faucet_test_context(false);

    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
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
    let user_asset_id = AssetId::new(asset_definition_id.clone(), user_id.clone());
    let user_asset = view
        .world()
        .asset(&user_asset_id)
        .expect("user faucet asset");
    assert_eq!(user_asset.value().as_ref().to_string(), "25000");
    let authority_asset_id = AssetId::new(asset_definition_id, authority_id);
    let authority_asset = view
        .world()
        .asset(&authority_asset_id)
        .expect("authority faucet asset");
    assert_eq!(authority_asset.value().as_ref().to_string(), "25000");
}

#[tokio::test]
async fn accounts_faucet_rejects_prefunded_accounts() {
    let FaucetTestContext {
        app,
        state,
        user_id,
        pow_difficulty_bits,
        ..
    } = build_faucet_test_context(true);

    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
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

#[tokio::test]
async fn accounts_faucet_puzzle_exposes_current_anchor() {
    let FaucetTestContext {
        app,
        state,
        pow_difficulty_bits,
        pow_max_anchor_age_blocks,
        ..
    } = build_faucet_test_context(false);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/accounts/faucet/puzzle")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("faucet puzzle response");
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("puzzle body bytes");
    let payload =
        norito::json::from_slice::<norito::json::Value>(body.as_ref()).expect("parse puzzle json");
    let object = payload.as_object().expect("puzzle object");
    assert_eq!(
        object
            .get("difficulty_bits")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(pow_difficulty_bits))
    );
    assert_eq!(
        object
            .get("anchor_height")
            .and_then(norito::json::Value::as_u64),
        Some(u64::try_from(state.committed_height()).expect("height fits"))
    );
    assert_eq!(
        object
            .get("max_anchor_age_blocks")
            .and_then(norito::json::Value::as_u64),
        Some(pow_max_anchor_age_blocks)
    );
    assert_eq!(
        object
            .get("algorithm")
            .and_then(norito::json::Value::as_str),
        Some("sha256-leading-zero-bits-v1")
    );
}

#[tokio::test]
async fn accounts_faucet_rejects_missing_pow_when_required() {
    let FaucetTestContext { app, user_id, .. } = build_faucet_test_context(false);

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
