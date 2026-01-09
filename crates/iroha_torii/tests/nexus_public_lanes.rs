//! Tests for the Nexus public-lane REST endpoints.
#![cfg(feature = "app_api")]

use std::{collections::HashSet, num::NonZeroU64, str::FromStr, sync::Arc};

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
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_torii::Torii;
use iroha_torii_shared::ErrorEnvelope;
use norito::json::{self, Value};
use tokio::sync::{broadcast, watch};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

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
    let router = build_test_router(Arc::new(state), &kura);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/0/validators")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/0/stake")
                .body(axum::body::Body::empty())
                .unwrap(),
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
    let router = build_test_router(Arc::new(state), &kura);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/0/validators")
                .body(axum::body::Body::empty())
                .unwrap(),
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

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/0/stake")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let shares = read_json(resp.into_body()).await;
    assert_eq!(shares["total"], Value::from(2));

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/nexus/public_lanes/0/stake?validator={validator}"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn nexus_public_lane_endpoints_reject_when_nexus_disabled() {
    let (world, _validator_keypair, _validator, _delegator, _escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    let router = build_test_router(Arc::new(state), &kura);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nexus/public_lanes/0/validators")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let payload =
        norito::core::decode_from_bytes::<ErrorEnvelope>(&body).expect("decode error payload");
    assert_eq!(payload.code, "nexus_disabled");
    assert!(
        payload.message.contains("nexus.enabled=true"),
        "expected message to mention nexus.enabled: {}",
        payload.message
    );
}

#[tokio::test]
async fn da_commitments_reject_when_nexus_disabled() {
    let (world, _validator_keypair, _validator, _delegator, _escrow) = sample_world();
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    let router = build_test_router(Arc::new(state), &kura);

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
    let payload =
        norito::core::decode_from_bytes::<ErrorEnvelope>(&body).expect("decode error payload");
    assert_eq!(payload.code, "nexus_disabled");
    assert!(
        payload.message.contains("nexus.enabled=true"),
        "expected message to mention nexus.enabled: {}",
        payload.message
    );
}

fn sample_world() -> (World, KeyPair, AccountId, AccountId, AccountId) {
    let domain_id: DomainId = "nexus".parse().expect("domain id");
    let validator_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::BlsNormal);
    let validator_id = AccountId::new(domain_id.clone(), validator_keypair.public_key().clone());
    let validator = Account::new(validator_id.clone()).build(&validator_id);

    let delegator_keypair = KeyPair::from_seed(vec![0x02; 32], Algorithm::Ed25519);
    let delegator_id = AccountId::new(domain_id.clone(), delegator_keypair.public_key().clone());
    let delegator = Account::new(delegator_id.clone()).build(&delegator_id);

    let escrow_keypair = KeyPair::from_seed(vec![0x04; 32], Algorithm::Ed25519);
    let escrow_id = AccountId::new(domain_id.clone(), escrow_keypair.public_key().clone());
    let escrow = Account::new(escrow_id.clone()).build(&escrow_id);

    let domain = Domain::new(domain_id.clone()).build(&validator_id);
    let asset_definition_id: AssetDefinitionId = "xor#nexus".parse().expect("asset definition");
    let asset_definition =
        AssetDefinition::numeric(asset_definition_id.clone()).build(&validator_id);

    let validator_asset_id = AssetId::new(asset_definition_id.clone(), validator_id.clone());
    let delegator_asset_id = AssetId::new(asset_definition_id.clone(), delegator_id.clone());
    let validator_asset = Asset::new(validator_asset_id, Numeric::new(10_000, 0));
    let delegator_asset = Asset::new(delegator_asset_id, Numeric::new(10_000, 0));

    let world = World::with_assets(
        [domain],
        [validator, delegator, escrow],
        [asset_definition],
        [validator_asset, delegator_asset],
        [],
    );
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
    RegisterPeerWithPop::new(peer_id.clone(), pop)
        .execute(validator, &mut tx)
        .expect("peer registration");

    let consensus_keypair = KeyPair::from_seed(vec![0x03; 32], Algorithm::BlsNormal);
    let consensus_pop = iroha_crypto::bls_normal_pop_prove(consensus_keypair.private_key())
        .expect("PoP prove for consensus keypair");
    let consensus_id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "main");
    let consensus_record = ConsensusKeyRecord {
        id: consensus_id.clone(),
        public_key: consensus_keypair.public_key().clone(),
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
        stake_account: validator.clone(),
        initial_stake: Numeric::new(1000, 0),
        metadata,
    }
    .execute(validator, &mut tx)
    .expect("validator registration");

    BondPublicLaneStake {
        lane_id: LaneId::SINGLE,
        validator: validator.clone(),
        staker: delegator.clone(),
        amount: Numeric::new(250, 0),
        metadata: Metadata::default(),
    }
    .execute(delegator, &mut tx)
    .expect("bond stake");

    tx.apply();
    block.commit().expect("commit block");
}

fn build_test_router(state: Arc<State>, kura: &Arc<Kura>) -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = iroha_core::kiso::KisoHandle::start(cfg.clone());
    let queue_cfg = Queue::default();
    let (events_tx, _events_rx) = broadcast::channel(1);
    let queue = Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events_tx));
    let (_peers_tx, peers_rx) = watch::channel(HashSet::default());

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
