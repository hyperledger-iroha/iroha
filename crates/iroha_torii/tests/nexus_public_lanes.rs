#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests for the Nexus public-lane REST endpoints.
#![cfg(feature = "app_api")]
use std::{net::SocketAddr, num::NonZeroU64, str::FromStr, sync::Arc};
use axum::{body::Body, http::Request};
use http::{Method, StatusCode, header};
use http_body_util::BodyExt;
use iroha_config::parameters::actual::Queue;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    consensus::{
        ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus, HsmBinding,
    },
    domain::{Domain, DomainId},
    isi::{
        Grant, RegisterPeerWithPop,
        consensus_keys::RegisterConsensusKey,
        staking::{BondPublicLaneStake, RegisterPublicLaneValidator},
    },
    metadata::Metadata,
    name::Name,
    nexus::LaneId,
    peer::PeerId,
    permission::Permission,
};
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_torii_shared::ErrorEnvelope;
use norito::json::{self, Value};
use tokio::sync::broadcast;
use tower::ServiceExt as _;
#[path = "fixtures.rs"]
mod fixtures;
fn with_loopback_connect_info(mut request: Request<Body>) -> Request<Body> {
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    request
}
fn enable_nexus(state: &mut State, escrow: &AccountId) {
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.staking.stake_escrow_account_id = escrow.to_string();
    nexus.staking.slash_sink_account_id = escrow.to_string();
    state
        .set_nexus(nexus)
        .expect("enable Nexus for public-lane fixture");
}
fn relax_consensus_key_activation_for_tests(state: &mut State) {
    let mut sumeragi_params = state.view().world().parameters().sumeragi.clone();
    sumeragi_params.key_activation_lead_blocks = 0;
    state.set_sumeragi_parameters(&sumeragi_params);
}
#[tokio::test]
async fn nexus_public_lane_endpoints_exist() {
    let (world, validator_keypair, validator, delegator, escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    enable_nexus(&mut state, &escrow);
    relax_consensus_key_activation_for_tests(&mut state);
    seed_public_lane_state(&state, &validator_keypair, &validator, &delegator);
    let local_peer_id = PeerId::from(validator_keypair.public_key().clone());
    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(
            &("/v1/nexus/public-lanes/0/validators"),
        )),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(&("/v1/nexus/public-lanes/0/stake"))),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
#[tokio::test]
async fn nexus_public_lane_endpoints_list_records() {
    let (world, validator_keypair, validator, delegator, escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    enable_nexus(&mut state, &escrow);
    relax_consensus_key_activation_for_tests(&mut state);
    seed_public_lane_state(&state, &validator_keypair, &validator, &delegator);
    let local_peer_id = PeerId::from(validator_keypair.public_key().clone());
    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(
            &("/v1/nexus/public-lanes/0/validators"),
        )),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = read_json(resp.into_body()).await;
    assert_eq!(json["total"], Value::from(1));
    assert_eq!(
        json["items"][0]["validator"],
        Value::from(validator.to_string())
    );
    assert_eq!(
        json["items"][0]["total_stake"],
        Value::from("1250".to_string())
    );
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(&("/v1/nexus/public-lanes/0/stake"))),
    )
    .await
    .unwrap();
    let shares = read_json(resp.into_body()).await;
    assert_eq!(shares["total"], Value::from(2));
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(
            &(format!("/v1/nexus/public-lanes/0/stake?validator={validator}")),
        )),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
#[tokio::test]
async fn nexus_public_lane_endpoints_reject_when_nexus_disabled() {
    let (world, validator_keypair, _validator, _delegator, _escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    let local_peer_id = PeerId::from(validator_keypair.public_key().clone());
    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request(
            &("/v1/nexus/public-lanes/0/validators"),
        )),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&body).expect("decode error payload");
    assert_eq!(payload.code, "nexus_disabled");
    assert!(
        payload.message.contains("nexus.enabled=true"),
        "expected message to mention nexus.enabled: {}",
        payload.message
    );
}
#[tokio::test]
async fn da_commitments_reject_when_nexus_disabled() {
    let (world, validator_keypair, _validator, _delegator, _escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    let local_peer_id = PeerId::from(validator_keypair.public_key().clone());
    let router = build_test_router(Arc::new(state), &kura, local_peer_id);
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/da/commitments")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&body).expect("decode error payload");
    assert_eq!(payload.code, "nexus_disabled");
    assert!(
        payload.message.contains("nexus.enabled=true"),
        "expected message to mention nexus.enabled: {}",
        payload.message
    );
}
fn sample_world() -> (World, KeyPair, AccountId, AccountId, AccountId) {
    let domain_id: DomainId = DomainId::try_new("nexus", "universal").expect("domain id");
    let validator_keypair =
        KeyPair::try_from_seed(vec![0x01; 32], Algorithm::BlsNormal).expect("derive validator key");
    let validator_id = AccountId::new(validator_keypair.public_key().clone());
    let validator = Account::new(validator_id.clone()).build(&validator_id);
    let delegator_keypair =
        KeyPair::try_from_seed(vec![0x02; 32], Algorithm::Ed25519).expect("derive delegator key");
    let delegator_id = AccountId::new(delegator_keypair.public_key().clone());
    let delegator = Account::new(delegator_id.clone()).build(&delegator_id);
    let escrow_keypair =
        KeyPair::try_from_seed(vec![0x04; 32], Algorithm::Ed25519).expect("derive escrow key");
    let escrow_id = AccountId::new(escrow_keypair.public_key().clone());
    let escrow = Account::new(escrow_id.clone()).build(&escrow_id);
    let domain = Domain::new(domain_id.clone()).build(&validator_id);
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("domain id"),
        "xor".parse().expect("asset definition name"),
    );
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&validator_id);
    let validator_asset_id = AssetId::new(asset_definition_id.clone(), validator_id.clone());
    let delegator_asset_id = AssetId::new(asset_definition_id.clone(), delegator_id.clone());
    let validator_asset = Asset::new(validator_asset_id, Quantity::from(10_000_u64));
    let delegator_asset = Asset::new(delegator_asset_id, Quantity::from(10_000_u64));
    let local_peer_id = PeerId::from(validator_keypair.public_key().clone());
    let mut world = World::with_assets(
        [domain],
        [validator, delegator, escrow],
        [asset_definition],
        [validator_asset, delegator_asset],
        [],
    );
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    (
        world,
        validator_keypair,
        validator_id,
        delegator_id,
        escrow_id,
    )
}
fn seed_public_lane_state(
    state: &State,
    validator_keypair: &KeyPair,
    validator: &AccountId,
    delegator: &AccountId,
) {
    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    let manage_consensus_keys = Permission::new(
        "CanManageConsensusKeys"
            .parse()
            .expect("CanManageConsensusKeys permission token"),
        Json::new(()),
    );
    Grant::account_permission(manage_consensus_keys, validator.clone())
        .execute(validator, &mut tx)
        .expect("grant manage consensus keys");
    let peer_id = PeerId::from(validator_keypair.public_key().clone());
    let pop = iroha_crypto::bls_normal_pop_prove(validator_keypair.private_key())
        .expect("PoP prove for validator keypair");
    let consensus_pop = pop.clone();
    RegisterPeerWithPop::new(peer_id.clone(), pop)
        .execute(validator, &mut tx)
        .expect("peer registration");
    let consensus_id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "main");
    let consensus_record = ConsensusKeyRecord {
        id: consensus_id.clone(),
        public_key: validator_keypair.public_key().clone(),
        pop: Some(consensus_pop),
        activation_height: 1,
        expiry_height: None,
        hsm: Some(HsmBinding {
            provider: "softkey".to_string(),
            key_label: "consensus-main".to_string(),
            slot: None,
        }),
        replaces: None,
        status: ConsensusKeyStatus::Active,
    };
    RegisterConsensusKey {
        id: consensus_id,
        record: consensus_record,
    }
    .execute(validator, &mut tx)
    .expect("consensus key registration");
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("alias").expect("alias key"),
        Json::from("validator-01"),
    );
    RegisterPublicLaneValidator {
        lane_id: LaneId::SINGLE,
        validator: validator.clone(),
        peer_id: PeerId::from(validator.expect_single_signatory().clone()),
        stake_account: validator.clone(),
        initial_stake: iroha_primitives::numeric::Quantity::from(1000_u32),
        metadata,
    }
    .execute(validator, &mut tx)
    .expect("validator registration");
    BondPublicLaneStake {
        lane_id: LaneId::SINGLE,
        validator: validator.clone(),
        staker: delegator.clone(),
        amount: iroha_primitives::numeric::Quantity::from(250_u32),
        metadata: Metadata::default(),
    }
    .execute(delegator, &mut tx)
    .expect("bond stake");
    tx.apply();
    block.commit().expect("commit block");
}
fn build_test_router(state: Arc<State>, kura: &Arc<Kura>, local_peer_id: PeerId) -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let queue_cfg = Queue::default();
    let (events_tx, _events_rx) = broadcast::channel(1);
    let queue = Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events_tx));
    let torii = fixtures::ToriiHarness::new(
        &cfg,
        iroha_data_model::ChainId::from("test-chain"),
        iroha_torii::test_utils::signed_query_network_id(),
        kura,
        &state,
        &queue,
        &local_peer_id,
        broadcast::channel(1).0,
        true,
        false,
    );
    torii.router()
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
async fn read_json(body: Body) -> Value {
    json::from_slice(&collect_body(body).await).expect("valid JSON payload")
}
async fn collect_body(body: Body) -> Vec<u8> {
    body.collect()
        .await
        .expect("body collection")
        .to_bytes()
        .to_vec()
}
