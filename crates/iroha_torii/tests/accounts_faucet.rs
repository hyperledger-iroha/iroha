#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account faucet tests.
#![cfg(feature = "app_api")]
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
    domain::DomainId,
    level::Level,
    peer::PeerId,
    prelude::{Account, AssetDefinition, Domain, InstructionBox, Log, Mint, SignedTransaction},
};
use iroha_torii::{Torii, json_entry, json_object};
use iroha_version::codec::DecodeVersioned as _;
use mv::storage::StorageReadOnly;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest as _, Sha256};
use std::{borrow::Cow, num::NonZeroU8, sync::Arc};
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
    authority_key_pair: KeyPair,
    user_id: AccountId,
    other_user_id: AccountId,
    pow_difficulty_bits: u8,
    pow_scrypt_log_n: u8,
    pow_scrypt_r: u32,
    pow_scrypt_p: u32,
    pow_max_anchor_age_blocks: u64,
}
fn checked_faucet_account_key_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked faucet account fixture keypair")
}
fn checked_faucet_block_leader_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("generate checked faucet block leader fixture keypair")
}
fn signed_faucet_beacon_fixture(
    network_id: iroha_data_model::NetworkId,
) -> (
    iroha_core::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
) {
    static FIXTURE: std::sync::OnceLock<(
        iroha_core::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
        iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
    )> = std::sync::OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        iroha_core::beacon::signed_persisted_pulse_fixture_for_world(network_id, 5)
    });
    assert_eq!(fixture.1.network_id, network_id);
    (fixture.0.clone(), fixture.1)
}
#[test]
fn faucet_account_fixture_uses_checked_ed25519_key_generation() {
    let key_pair = checked_faucet_account_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture faucet account public key has a valid algorithm");
    assert_eq!(algorithm, Algorithm::Ed25519);
}
#[test]
fn faucet_block_leader_fixture_uses_checked_bls_key_generation() {
    let key_pair = checked_faucet_block_leader_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture faucet block leader public key has a valid algorithm");
    assert_eq!(algorithm, Algorithm::BlsNormal);
}
fn build_faucet_test_context(prefund_user: bool) -> FaucetTestContext {
    build_faucet_test_context_with_registration(prefund_user, None, true)
}
fn build_faucet_test_context_with_selector(
    prefund_user: bool,
    faucet_selector: Option<&str>,
) -> FaucetTestContext {
    build_faucet_test_context_with_registration(prefund_user, faucet_selector, true)
}
fn build_faucet_test_context_with_registration(
    prefund_user: bool,
    faucet_selector: Option<&str>,
    register_user: bool,
) -> FaucetTestContext {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let domain_id: DomainId = DomainId::try_new("sora", "universal").expect("domain id");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("asset name"),
    );
    let canonical_selector = asset_definition_id.to_string();
    let authority_kp = checked_faucet_account_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let user_kp = checked_faucet_account_key_fixture();
    let user_id = AccountId::new(user_kp.public_key().clone());
    let other_user_kp = checked_faucet_account_key_fixture();
    let other_user_id = AccountId::new(other_user_kp.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let other_user_account = Account::new(other_user_id.clone()).build(&authority_id);
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority_id);
    let mut accounts = vec![authority_account, other_user_account];
    if register_user {
        accounts.push(Account::new(user_id.clone()).build(&authority_id));
    }
    let chain_id = iroha_data_model::ChainId::from("test-chain");
    let network_id = iroha_torii::test_utils::signed_query_network_id();
    let mut world = World::with([domain], accounts, [asset_definition]);
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    {
        let mut block = world.block();
        let (key_record, pulse) = signed_faucet_beacon_fixture(network_id);
        block
            .install_global_beacon_fixture_for_testing(key_record, pulse)
            .expect("install proof-valid faucet beacon fixture");
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
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        world,
        kura.clone(),
        query,
        chain_id.clone(),
        network_id,
    ));
    {
        let mut seed_instructions: Vec<InstructionBox> = vec![
            Mint::asset_quantity(
                50_000_u32,
                AssetId::new(asset_definition_id.clone(), authority_id.clone()),
            )
            .into(),
        ];
        if prefund_user {
            seed_instructions.push(
                Mint::asset_quantity(
                    1_u32,
                    AssetId::new(asset_definition_id.clone(), user_id.clone()),
                )
                .into(),
            );
        }
        let seed_tx = TransactionBuilder::new(
            network_id,
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(seed_instructions)
        .sign(authority_kp.private_key());
        let leader = checked_faucet_block_leader_fixture();
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
    advance_faucet_state_chain(
        &state,
        &chain_id,
        &authority_id,
        &authority_kp,
        4,
        "reach faucet beacon pulse height",
    );
    let pow_difficulty_bits = 5;
    let pow_scrypt_log_n = 4;
    let pow_scrypt_r = 1;
    let pow_scrypt_p = 1;
    let pow_max_anchor_age_blocks = 4;
    cfg.torii.faucet = Some(iroha_config::parameters::actual::ToriiFaucet {
        authority: authority_id.clone(),
        private_key_file: "/runtime-only/faucet-signer.key".into(),
        signer: authority_kp.clone(),
        asset_definition_id: faucet_selector
            .unwrap_or(canonical_selector.as_str())
            .to_owned(),
        amount: 25_000_u32.into(),
        pow_difficulty_bits: NonZeroU8::new(pow_difficulty_bits)
            .expect("non-zero faucet pow difficulty"),
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        pow_max_anchor_age_blocks: std::num::NonZeroU64::new(pow_max_anchor_age_blocks)
            .expect("non-zero faucet pow anchor age"),
        pow_adaptive_lookback_blocks: 8,
        pow_adaptive_claims_per_extra_bit: 1,
        pow_adaptive_max_extra_bits: 2,
        pow_beacon_seed_enabled: true,
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
                network_id,
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
                network_id,
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
        authority_key_pair: authority_kp,
        user_id,
        other_user_id,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
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
fn faucet_beacon_seed_for_anchor(state: &State, anchor_height: u64) -> Option<[u8; 32]> {
    let view = state.view();
    iroha_core::beacon::verified_global_threshold_beacon_pulse_at_or_before_v1(
        view.world(),
        state.network_id_ref(),
        anchor_height,
    )
    .ok()
    .map(|pulse| pulse.seed)
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
fn faucet_post_request(path: &str, body: String) -> Request<axum::body::Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::connect_info::ConnectInfo(
            std::net::SocketAddr::from(([127, 0, 0, 1], 8080)),
        ))
        .body(axum::body::Body::from(body))
        .expect("faucet request")
}
fn faucet_mutation_binding() -> norito::json::Value {
    json_object(vec![
        json_entry("schema", "iroha.taira.public-reset.mutation-binding.v1"),
        json_entry("authorization_sha256", "11".repeat(32)),
        json_entry("authorization_nonce", "reset_nonce_00000000000000000000"),
        json_entry("kind", "faucet"),
        json_entry("phase", "prepare_faucet"),
        json_entry("idempotency_key", "22".repeat(32)),
        json_entry("execution_expires_at_unix_ms", u64::MAX),
    ])
}
async fn prepare_faucet_envelope(app: &axum::Router, claim_body: String) -> Response {
    let claim: norito::json::Value =
        norito::json::from_str(&claim_body).expect("decode faucet claim body");
    let request = json_object(vec![
        json_entry("schema", "iroha.accounts.faucet.prepare.v1"),
        json_entry("binding", faucet_mutation_binding()),
        json_entry("claim", claim),
    ]);
    let request = norito::json::to_json(&request).expect("encode faucet prepare request");
    app.clone()
        .oneshot(faucet_post_request("/v1/accounts/faucet/prepare", request))
        .await
        .expect("faucet prepare response")
}
async fn prepare_and_submit_faucet(app: &axum::Router, claim_body: String) -> Response {
    let prepared = expect_status(
        prepare_faucet_envelope(app, claim_body).await,
        StatusCode::OK,
    )
    .await;
    let body = to_bytes(prepared.into_body(), usize::MAX)
        .await
        .expect("prepared faucet body");
    app.clone()
        .oneshot(faucet_post_request(
            "/v1/accounts/faucet",
            String::from_utf8(body.to_vec()).expect("prepared faucet UTF-8 JSON"),
        ))
        .await
        .expect("faucet submit response")
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
    let challenge_salt = faucet_beacon_seed_for_anchor(state, anchor_height);
    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(state.network_id_ref().as_bytes());
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
fn advance_faucet_state_chain(
    state: &State,
    chain_id: &iroha_data_model::ChainId,
    authority_id: &AccountId,
    authority_key_pair: &KeyPair,
    blocks: u64,
    message: &str,
) {
    for index in 0..blocks {
        let tx = TransactionBuilder::new(
            *state.network_id_ref(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, format!("{message} {index}"))])
        .sign(authority_key_pair.private_key());
        let leader = checked_faucet_block_leader_fixture();
        let unverified =
            BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(tx))])
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader.private_key())
                .unpack(|_| {});
        let mut state_block = state.block(unverified.header());
        state_block.chain_id = chain_id.clone();
        let valid = unverified
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let committed = valid.commit_unchecked().unpack(|_| {});
        iroha_torii::test_utils::finalize_committed_block(state, state_block, committed);
    }
}
fn advance_faucet_chain(context: &FaucetTestContext, blocks: u64) {
    advance_faucet_state_chain(
        &context.state,
        &context.chain_id,
        &context.authority_id,
        &context.authority_key_pair,
        blocks,
        "age faucet anchor",
    );
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
    let resp = prepare_and_submit_faucet(&app, body).await;
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
async fn accounts_faucet_registers_missing_account_before_transfer() {
    let FaucetTestContext {
        app,
        state,
        queue,
        chain_id,
        asset_definition_id,
        user_id,
        pow_difficulty_bits,
        pow_scrypt_log_n,
        pow_scrypt_r,
        pow_scrypt_p,
        ..
    } = build_faucet_test_context_with_registration(false, None, false);
    let scrypt_params = faucet_pow_scrypt_params(pow_scrypt_log_n, pow_scrypt_r, pow_scrypt_p);
    let (pow_anchor_height, pow_nonce_hex) =
        solve_faucet_pow(&state, &user_id, pow_difficulty_bits, &scrypt_params);
    let body = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let prepared = expect_status(prepare_faucet_envelope(&app, body).await, StatusCode::OK).await;
    assert_eq!(queue.active_len(), 0, "faucet prepare must not enqueue");
    assert!(state.view().world().account(&user_id).is_err());
    let prepared_body = to_bytes(prepared.into_body(), usize::MAX)
        .await
        .expect("prepared faucet body");
    let prepared_json: norito::json::Value =
        norito::json::from_slice(&prepared_body).expect("prepared faucet JSON");
    let prepared_hash = prepared_json["transaction_hash_hex"]
        .as_str()
        .expect("prepared transaction hash")
        .to_owned();
    let prepared_wire = hex::decode(
        prepared_json["signed_transaction_wire_hex"]
            .as_str()
            .expect("prepared transaction wire"),
    )
    .expect("decode prepared transaction wire");
    let prepared_tx =
        SignedTransaction::decode_all_versioned(&prepared_wire).expect("decode prepared tx");
    let prepared_instructions: Vec<_> =
        prepared_tx.instructions().explicit_instructions().collect();
    assert_eq!(prepared_instructions.len(), 2);
    assert!(matches!(
        prepared_instructions[0]
            .as_any()
            .downcast_ref::<iroha_data_model::isi::RegisterBox>(),
        Some(iroha_data_model::isi::RegisterBox::Account(_))
    ));
    let prepared_body = String::from_utf8(prepared_body.to_vec()).expect("prepared UTF-8 JSON");
    let resp = app
        .clone()
        .oneshot(faucet_post_request("/v1/accounts/faucet", prepared_body))
        .await
        .expect("faucet submit response");
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
    assert!(
        view.world().account(&user_id).is_ok(),
        "user account should exist"
    );
    let user_asset_id = AssetId::new(asset_definition_id, user_id.clone());
    let user_asset = view
        .world()
        .asset(&user_asset_id)
        .expect("user faucet asset");
    assert_eq!(user_asset.value().as_ref().to_string(), "25000");
    drop(view);

    let (pow_anchor_height, pow_nonce_hex) = solve_faucet_pow(
        &state,
        &user_id,
        pow_difficulty_bits.saturating_add(1),
        &scrypt_params,
    );
    let post_onboarding_claim = json_object(vec![
        json_entry("account_id", user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let post_onboarding_claim =
        norito::json::to_json(&post_onboarding_claim).expect("serialize post-onboarding claim");
    let post_onboarding = expect_status(
        prepare_faucet_envelope(&app, post_onboarding_claim).await,
        StatusCode::OK,
    )
    .await;
    let post_body = to_bytes(post_onboarding.into_body(), usize::MAX)
        .await
        .expect("post-onboarding prepared body");
    let post_json: norito::json::Value =
        norito::json::from_slice(&post_body).expect("post-onboarding prepared JSON");
    assert_ne!(
        post_json["transaction_hash_hex"]
            .as_str()
            .expect("post-onboarding hash"),
        prepared_hash
    );
    let post_wire = hex::decode(
        post_json["signed_transaction_wire_hex"]
            .as_str()
            .expect("post-onboarding wire"),
    )
    .expect("decode post-onboarding wire");
    let post_tx =
        SignedTransaction::decode_all_versioned(&post_wire).expect("decode post-onboarding tx");
    assert_eq!(
        post_tx.instructions().explicit_instructions().count(),
        1,
        "post-onboarding faucet preparation must not be interchangeable with registration"
    );
}
#[tokio::test]
async fn accounts_faucet_adds_amount_to_prefunded_accounts() {
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
    let resp = prepare_and_submit_faucet(&app, body).await;
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
    assert_eq!(user_asset.value().as_ref().to_string(), "25001");
    let authority_asset_id = AssetId::new(asset_definition_id, authority_id);
    let authority_asset = view
        .world()
        .asset(&authority_asset_id)
        .expect("authority faucet asset");
    assert_eq!(authority_asset.value().as_ref().to_string(), "25000");
}
#[tokio::test]
async fn accounts_faucet_allows_repeated_claims_for_same_account() {
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
    for expected_extra_bits in [0_u8, 1] {
        let difficulty_bits = pow_difficulty_bits.saturating_add(expected_extra_bits);
        let (pow_anchor_height, pow_nonce_hex) =
            solve_faucet_pow(&state, &user_id, difficulty_bits, &scrypt_params);
        let body = json_object(vec![
            json_entry("account_id", user_id.to_string()),
            json_entry("pow_anchor_height", pow_anchor_height),
            json_entry("pow_nonce_hex", pow_nonce_hex),
        ]);
        let body = norito::json::to_json(&body).expect("serialize faucet request");
        let resp = prepare_and_submit_faucet(&app, body).await;
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
    }
    let view = state.view();
    let user_asset_id = AssetId::new(asset_definition_id.clone(), user_id.clone());
    let user_asset = view
        .world()
        .asset(&user_asset_id)
        .expect("user faucet asset");
    assert_eq!(user_asset.value().as_ref().to_string(), "50000");
    let authority_asset_id = AssetId::new(asset_definition_id, authority_id);
    let authority_balance = view
        .world()
        .asset(&authority_asset_id)
        .map(|asset| asset.value().as_ref().to_string())
        .unwrap_or_else(|_| "0".to_owned());
    assert_eq!(authority_balance, "0");
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
    let resp = prepare_and_submit_faucet(&app, body).await;
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
async fn faucet_prepared_envelope_survives_pow_anchor_aging() {
    let context = build_faucet_test_context(false);
    let scrypt_params = faucet_pow_scrypt_params(
        context.pow_scrypt_log_n,
        context.pow_scrypt_r,
        context.pow_scrypt_p,
    );
    let (pow_anchor_height, pow_nonce_hex) = solve_faucet_pow(
        &context.state,
        &context.user_id,
        context.pow_difficulty_bits,
        &scrypt_params,
    );
    let claim = json_object(vec![
        json_entry("account_id", context.user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let claim = norito::json::to_json(&claim).expect("serialize faucet claim");
    let prepared = expect_status(
        prepare_faucet_envelope(&context.app, claim).await,
        StatusCode::OK,
    )
    .await;
    let prepared_body = to_bytes(prepared.into_body(), usize::MAX)
        .await
        .expect("prepared faucet body");
    assert_eq!(context.queue.active_len(), 0);
    advance_faucet_chain(
        &context,
        context.pow_max_anchor_age_blocks.saturating_add(1),
    );
    let submitted = context
        .app
        .clone()
        .oneshot(faucet_post_request(
            "/v1/accounts/faucet",
            String::from_utf8(prepared_body.to_vec()).expect("prepared UTF-8 JSON"),
        ))
        .await
        .expect("aged faucet submit response");
    let _submitted = expect_status(submitted, StatusCode::ACCEPTED).await;
    assert_eq!(context.queue.active_len(), 1);
}

#[tokio::test]
async fn faucet_submit_rejects_old_and_tampered_shapes_and_deduplicates_exact_replay() {
    let context = build_faucet_test_context(false);
    let scrypt_params = faucet_pow_scrypt_params(
        context.pow_scrypt_log_n,
        context.pow_scrypt_r,
        context.pow_scrypt_p,
    );
    let (pow_anchor_height, pow_nonce_hex) = solve_faucet_pow(
        &context.state,
        &context.user_id,
        context.pow_difficulty_bits,
        &scrypt_params,
    );
    let claim = json_object(vec![
        json_entry("account_id", context.user_id.to_string()),
        json_entry("pow_anchor_height", pow_anchor_height),
        json_entry("pow_nonce_hex", pow_nonce_hex),
    ]);
    let claim_body = norito::json::to_json(&claim).expect("serialize faucet claim");
    let old = context
        .app
        .clone()
        .oneshot(faucet_post_request(
            "/v1/accounts/faucet",
            claim_body.clone(),
        ))
        .await
        .expect("old faucet request response");
    let _old = expect_status(old, StatusCode::BAD_REQUEST).await;

    let prepared = expect_status(
        prepare_faucet_envelope(&context.app, claim_body).await,
        StatusCode::OK,
    )
    .await;
    let prepared_body = to_bytes(prepared.into_body(), usize::MAX)
        .await
        .expect("prepared faucet body");
    let prepared_json: norito::json::Value =
        norito::json::from_slice(&prepared_body).expect("prepared faucet JSON");
    for field in [
        "transaction_hash_hex",
        "signed_transaction_wire_sha256",
        "signed_transaction_wire_hex",
    ] {
        let mut tampered = prepared_json.clone();
        tampered.as_object_mut().expect("prepared object").insert(
            field.to_owned(),
            norito::json::Value::String("00".repeat(32)),
        );
        let response = context
            .app
            .clone()
            .oneshot(faucet_post_request(
                "/v1/accounts/faucet",
                norito::json::to_json(&tampered).expect("tampered JSON"),
            ))
            .await
            .expect("tampered submit response");
        let _response = expect_status(response, StatusCode::BAD_REQUEST).await;
    }
    assert_eq!(context.queue.active_len(), 0);

    let exact_body = String::from_utf8(prepared_body.to_vec()).expect("prepared UTF-8 JSON");
    let submitted = context
        .app
        .clone()
        .oneshot(faucet_post_request(
            "/v1/accounts/faucet",
            exact_body.clone(),
        ))
        .await
        .expect("faucet submit response");
    let _submitted = expect_status(submitted, StatusCode::ACCEPTED).await;
    let response_loss_replay = context
        .app
        .clone()
        .oneshot(faucet_post_request(
            "/v1/accounts/faucet",
            exact_body.clone(),
        ))
        .await
        .expect("faucet replay response");
    let replay = expect_status(response_loss_replay, StatusCode::OK).await;
    let replay_body = to_bytes(replay.into_body(), usize::MAX)
        .await
        .expect("faucet replay body");
    let replay_json: norito::json::Value =
        norito::json::from_slice(&replay_body).expect("faucet replay JSON");
    assert_eq!(replay_json["outcome"].as_str(), Some("Pending"));
    assert_eq!(context.queue.active_len(), 1);

    let expected_height = u64::try_from(context.state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    assert_eq!(
        iroha_torii::test_utils::apply_queued_in_one_block(
            &context.state,
            &context.queue,
            &context.chain_id,
            expected_height,
        ),
        1
    );
    let applied_replay = context
        .app
        .clone()
        .oneshot(faucet_post_request("/v1/accounts/faucet", exact_body))
        .await
        .expect("applied faucet replay response");
    let applied_replay = expect_status(applied_replay, StatusCode::OK).await;
    let applied_body = to_bytes(applied_replay.into_body(), usize::MAX)
        .await
        .expect("applied replay body");
    let applied_json: norito::json::Value =
        norito::json::from_slice(&applied_body).expect("applied replay JSON");
    assert_eq!(applied_json["outcome"].as_str(), Some("Applied"));
    assert_eq!(context.queue.active_len(), 0);
    let destination = AssetId::new(context.asset_definition_id.clone(), context.user_id.clone());
    assert_eq!(
        context
            .state
            .view()
            .world()
            .asset(&destination)
            .expect("funded destination")
            .value()
            .as_ref()
            .to_string(),
        "25000",
        "exact replay must not charge or transfer twice"
    );
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
    let anchor_height = u64::try_from(state.committed_height()).expect("height fits");
    let expected_salt = hex::encode(
        faucet_beacon_seed_for_anchor(&state, anchor_height)
            .expect("proof-valid faucet beacon seed"),
    );
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
        Some(anchor_height)
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
        Some(expected_salt.as_str())
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
    let puzzle_network_id: iroha_data_model::NetworkId =
        norito::json::from_value(object.get("network_id").expect("puzzle network id").clone())
            .expect("canonical puzzle network id");
    assert_eq!(&puzzle_network_id, state.network_id_ref());
    assert!(!object.contains_key("chain_id"));
}
#[tokio::test]
async fn accounts_faucet_rejects_missing_pow_when_required() {
    let FaucetTestContext { app, user_id, .. } = build_faucet_test_context(false);
    let body = json_object(vec![json_entry("account_id", user_id.to_string())]);
    let body = norito::json::to_json(&body).expect("serialize faucet request");
    let resp = prepare_faucet_envelope(&app, body).await;
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
    let resp = prepare_and_submit_faucet(&app, initial_claim_body).await;
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
