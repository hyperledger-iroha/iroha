#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account faucet tests.
#![cfg(feature = "app_api")]

use std::{borrow::Cow, sync::Arc};

use axum::{body::to_bytes, http::Request, response::Response};
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
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    consensus::VrfEpochRecord,
    domain::DomainId,
    peer::PeerId,
    prelude::{Account, AssetDefinition, Domain, ExposedPrivateKey, InstructionBox, Mint},
};
use iroha_torii::{Torii, json_entry, json_object};
use mv::storage::StorageReadOnly;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
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
    other_user_id: AccountId,
    pow_difficulty_bits: u8,
    pow_scrypt_log_n: u8,
    pow_scrypt_r: u32,
    pow_scrypt_p: u32,
    pow_max_anchor_age_blocks: u64,
}

fn build_faucet_test_context(prefund_user: bool) -> FaucetTestContext {
    build_faucet_test_context_with_selector(prefund_user, None)
}

fn build_faucet_test_context_with_selector(
    prefund_user: bool,
    faucet_selector: Option<&str>,
) -> FaucetTestContext {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = "sora".parse().expect("domain id");
    let asset_definition_id =
        AssetDefinitionId::new(domain_id.clone(), "xor".parse().expect("asset name"));
    let canonical_selector = asset_definition_id.to_string();
    let authority_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let user_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let user_id = AccountId::new(user_kp.public_key().clone());
    let other_user_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let other_user_id = AccountId::new(other_user_kp.public_key().clone());

    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let user_account = Account::new(user_id.clone()).build(&authority_id);
    let other_user_account = Account::new(other_user_id.clone()).build(&authority_id);
    let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
        .with_name("XOR".to_owned())
        .build(&authority_id);

    let mut world = World::with(
        [domain],
        [authority_account, user_account, other_user_account],
        [asset_definition],
    );
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    {
        let mut block = world.block();
        block.vrf_epochs_mut_for_testing().insert(
            0,
            VrfEpochRecord {
                epoch: 0,
                seed: [0xAB; 32],
                epoch_length: 1,
                commit_deadline_offset: 0,
                reveal_deadline_offset: 0,
                roster_len: 1,
                finalized: true,
                updated_at_height: 0,
                participants: Vec::new(),
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: true,
                penalties_applied_at_height: Some(0),
                validator_election: None,
            },
        );
        block.commit();
    }
    if let Some(selector) = faucet_selector {
        if selector != canonical_selector {
            let alias: AssetDefinitionAlias = selector.parse().expect("asset alias");
            let mut block = world.block();
            let mut tx = block.transaction_without_telemetry(
                iroha_config::parameters::actual::LaneConfig::default(),
                0,
            );
            tx.bind_asset_definition_alias(&asset_definition_id, alias, None, None, 10_000)
                .expect("bind alias");
            tx.apply();
            block.commit();
        }
    }
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

    let pow_difficulty_bits = 5;
    let pow_scrypt_log_n = 4;
    let pow_scrypt_r = 1;
    let pow_scrypt_p = 1;
    let pow_max_anchor_age_blocks = 4;
    cfg.torii.faucet = Some(iroha_config::parameters::actual::ToriiFaucet {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        asset_definition_id: faucet_selector
            .unwrap_or(canonical_selector.as_str())
            .to_owned(),
        amount: 25_000_u32.into(),
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        pow_max_anchor_age_blocks: std::num::NonZeroU64::new(pow_max_anchor_age_blocks)
            .expect("non-zero faucet pow anchor age"),
        pow_adaptive_lookback_blocks: 8,
        pow_adaptive_claims_per_extra_bit: 1,
        pow_adaptive_max_extra_bits: 2,
        pow_vrf_seed_enabled: true,
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
        other_user_id,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        pow_max_anchor_age_blocks,
    }
}

const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v2";

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

fn faucet_vrf_seed_for_anchor(state: &State, anchor_height: u64) -> Option<[u8; 32]> {
    state
        .view()
        .world()
        .vrf_epochs()
        .iter()
        .filter_map(|(_, record)| {
            (record.finalized && record.updated_at_height <= anchor_height).then_some(record.seed)
        })
        .last()
}

fn faucet_pow_scrypt_params(log_n: u8, r: u32, p: u32) -> ScryptParams {
    ScryptParams::new(log_n, r, p, 32).expect("valid test scrypt params")
}

async fn expect_status(resp: Response, expected: StatusCode) -> Response {
    let status = resp.status();
    if status == expected {
        return resp;
    }

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    panic!(
        "expected status {}, got {} with body {}",
        expected,
        status,
        String::from_utf8_lossy(&body)
    );
}

fn faucet_post_request(body: String) -> Request<axum::body::Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/accounts/faucet")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::connect_info::ConnectInfo(
            std::net::SocketAddr::from(([127, 0, 0, 1], 8080)),
        ))
        .body(axum::body::Body::from(body))
        .expect("faucet request")
}

fn faucet_pow_challenge(state: &State, account_id: &AccountId, anchor_height: u64) -> [u8; 32] {
    let anchor_block = state
        .block_by_height(
            usize::try_from(anchor_height)
                .ok()
                .and_then(std::num::NonZeroUsize::new)
                .expect("non-zero height"),
        )
        .expect("anchor block");
    let anchor_hash = anchor_block.hash();
    let challenge_salt = faucet_vrf_seed_for_anchor(state, anchor_height);

    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(account_id.to_string().as_bytes());
    hasher.update(anchor_height.to_be_bytes());
    hasher.update(anchor_hash.as_ref());
    if let Some(challenge_salt) = challenge_salt.as_ref() {
        hasher.update(challenge_salt);
    }
    hasher.finalize().into()
}

fn solve_faucet_pow(
    state: &State,
    account_id: &AccountId,
    difficulty_bits: u8,
    scrypt_params: &ScryptParams,
) -> (u64, String) {
    let anchor_height = u64::try_from(state.committed_height()).expect("height fits");
    let challenge = faucet_pow_challenge(state, account_id, anchor_height);

    for nonce in 0u64.. {
        let nonce_bytes = nonce.to_be_bytes();
        let mut digest = [0u8; 32];
        derive_scrypt(&nonce_bytes, &challenge, scrypt_params, &mut digest)
            .expect("test scrypt digest");
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
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        ..
    } = build_faucet_test_context(false);

    let scrypt_params = faucet_pow_scrypt_params(pow_scrypt_log_n, pow_scrypt_r, pow_scrypt_p);
    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits, &scrypt_params);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(faucet_post_request(body))
        .await
        .expect("faucet response");
    let _resp = expect_status(resp, StatusCode::ACCEPTED).await;

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
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        ..
    } = build_faucet_test_context(true);

    let scrypt_params = faucet_pow_scrypt_params(pow_scrypt_log_n, pow_scrypt_r, pow_scrypt_p);
    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits, &scrypt_params);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(faucet_post_request(body))
        .await
        .expect("faucet response");
    let _resp = expect_status(resp, StatusCode::BAD_REQUEST).await;
}

#[tokio::test]
async fn accounts_faucet_accepts_alias_selector_config() {
    let FaucetTestContext {
        app,
        state,
        queue,
        chain_id,
        asset_definition_id,
        authority_id,
        user_id,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        ..
    } = build_faucet_test_context_with_selector(false, Some("xor#universal"));

    let scrypt_params = faucet_pow_scrypt_params(pow_scrypt_log_n, pow_scrypt_r, pow_scrypt_p);
    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits, &scrypt_params);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(faucet_post_request(body))
        .await
        .expect("faucet response");
    let _resp = expect_status(resp, StatusCode::ACCEPTED).await;

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
async fn accounts_faucet_puzzle_exposes_current_anchor() {
    let FaucetTestContext {
        app,
        state,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
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
    let resp = expect_status(resp, StatusCode::OK).await;

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
            .get("challenge_salt_hex")
            .and_then(norito::json::Value::as_str),
        Some("abababababababababababababababababababababababababababababababab")
    );
    assert_eq!(
        object
            .get("scrypt_log_n")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(pow_scrypt_log_n))
    );
    assert_eq!(
        object.get("scrypt_r").and_then(norito::json::Value::as_u64),
        Some(u64::from(pow_scrypt_r))
    );
    assert_eq!(
        object.get("scrypt_p").and_then(norito::json::Value::as_u64),
        Some(u64::from(pow_scrypt_p))
    );
    assert_eq!(
        object
            .get("algorithm")
            .and_then(norito::json::Value::as_str),
        Some("scrypt-leading-zero-bits-v1")
    );
}

#[tokio::test]
async fn accounts_faucet_rejects_missing_pow_when_required() {
    let FaucetTestContext { app, user_id, .. } = build_faucet_test_context(false);

    let body = json_object(vec![json_entry("account_id", user_id.to_string())]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = app
        .clone()
        .oneshot(faucet_post_request(body))
        .await
        .expect("faucet response");
    let _resp = expect_status(resp, StatusCode::BAD_REQUEST).await;
}

#[tokio::test]
async fn accounts_faucet_puzzle_raises_difficulty_after_recent_claim() {
    let FaucetTestContext {
        app,
        state,
        queue,
        other_user_id,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        ..
    } = build_faucet_test_context(false);

    let scrypt_params = faucet_pow_scrypt_params(pow_scrypt_log_n, pow_scrypt_r, pow_scrypt_p);
    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &other_user_id, pow_difficulty_bits, &scrypt_params);
    let initial_claim_body = json_object(vec![
        json_entry("account_id", other_user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let initial_claim_body =
        norito::json::to_json(&initial_claim_body).expect("serialize initial faucet request");
    let resp = app
        .clone()
        .oneshot(faucet_post_request(initial_claim_body))
        .await
        .expect("initial faucet response");
    let _resp = expect_status(resp, StatusCode::ACCEPTED).await;

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
    let resp = expect_status(resp, StatusCode::OK).await;

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
        Some(u64::from(pow_difficulty_bits.saturating_add(1)))
    );
    assert_eq!(
        object
            .get("anchor_height")
            .and_then(norito::json::Value::as_u64),
        Some(u64::try_from(state.committed_height()).expect("height fits"))
    );
    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).count()
    };
    assert!(queued > 0);
}
