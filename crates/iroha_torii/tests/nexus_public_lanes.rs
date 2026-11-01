//! Tests for the Nexus public-lane REST endpoints.
#![cfg(feature = "app_api")]
use crate::fixtures;
use axum::{body::Body, http::Request};
use http::StatusCode;
use http_body_util::BodyExt;
use iroha_config::parameters::actual::Queue;
use iroha_core::{
    kura::Kura,
    smartcontracts::Execute,
    state::{State, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::Domain,
    isi::staking::{ActivatePublicLaneValidator, BondPublicLaneStake, RegisterPublicLaneValidator},
    nexus::{
        PublicLaneMonetaryBondV1, PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1,
        PublicLaneMonetaryScopeV1,
    },
    parameter::{
        Parameter,
        system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
    },
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_primitives::{
    json::Json,
    numeric::{NumericSpec, Quantity},
};
use norito::json::{self, Value};
use std::{net::SocketAddr, num::NonZeroU64, str::FromStr, sync::Arc};
use tokio::sync::broadcast;
fn with_loopback_connect_info(mut request: Request<Body>) -> Request<Body> {
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    request
}
#[tokio::test]
async fn nexus_public_lane_endpoints_exist() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let fixture = public_lane_fixture();
    let router = build_test_router(
        fixture.chain.state(),
        fixture.chain.kura(),
        &fixture.local_peer,
    );
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request("/v1/nexus/public-lanes/0/validators")),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    fixtures::response_body(resp, "public lane endpoint response").await;
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request("/v1/nexus/public-lanes/0/stake")),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    fixtures::response_body(resp, "public lane endpoint response").await;
    router.shutdown().await;
}
#[tokio::test]
async fn nexus_public_lane_endpoints_list_records() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let fixture = public_lane_fixture();
    let validator = fixture.validator.clone();
    let router = build_test_router(
        fixture.chain.state(),
        fixture.chain.kura(),
        &fixture.local_peer,
    );
    let resp = fixtures::request(
        &router,
        with_loopback_connect_info(fixtures::get_request("/v1/nexus/public-lanes/0/validators")),
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
        with_loopback_connect_info(fixtures::get_request("/v1/nexus/public-lanes/0/stake")),
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
    router.shutdown().await;
}
/// A signed `NPoS` genesis whose committee member registers on the public lane, plus one bond.
struct PublicLaneFixture {
    chain: CertifiedTestChain,
    validator: AccountId,
    local_peer: PeerId,
}
fn public_lane_fixture() -> PublicLaneFixture {
    let domain_id: DomainId = DomainId::try_new("nexus", "universal").expect("domain id");
    let validator_keys: Vec<KeyPair> = [0x01_u8, 0x05, 0x06, 0x07]
        .into_iter()
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive validator key")
        })
        .collect();
    let validator_id = AccountId::new(validator_keys[0].public_key().clone());
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
    let npos = SumeragiNposParameters::default();
    npos.validate()
        .expect("valid committed NPoS fixture parameters");
    let asset_definition_id = npos.xor_asset_definition_id.clone();
    let asset_definition = AssetDefinition::new(
        asset_definition_id.clone(),
        "xor".to_owned(),
        NumericSpec::fractional(9),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&validator_id);
    let validator_asset_id = AssetId::new(asset_definition_id.clone(), validator_id.clone());
    let delegator_asset_id = AssetId::new(asset_definition_id.clone(), delegator_id.clone());
    let validator_asset = Asset::new(validator_asset_id.clone(), Quantity::from(10_000_u64));
    let delegator_asset = Asset::new(delegator_asset_id.clone(), Quantity::from(10_000_u64));
    let world = World::with_assets(
        [domain],
        [validator, delegator, escrow],
        [asset_definition],
        [validator_asset, delegator_asset],
        [],
    );
    let profile =
        iroha_config::profile::Profile::compiled(iroha_config::profile::ProfileId::IrohaDevV1)
            .unwrap();
    let mut staking = profile.genesis_recipe().public_lane_staking.clone();
    staking.stake_asset_id = asset_definition_id.clone();
    staking.stake_escrow_account_id = escrow_id.clone();
    staking.slash_sink_account_id = escrow_id.clone();
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("alias").expect("alias key"),
        Json::from("validator-01"),
    );
    let escrow_asset = AssetId::new(asset_definition_id, escrow_id);
    let local_peer = PeerId::from(validator_keys[0].public_key().clone());
    let mut config = TestChainConfig::new(world, 1_000);
    config.consensus_mode = SumeragiConsensusMode::Npos;
    config.genesis_parameters = vec![
        Parameter::Custom(staking.into_custom_parameter().unwrap()),
        Parameter::Sumeragi(SumeragiParameter::EpochLengthBlocks(
            npos.epoch_length_blocks,
        )),
        Parameter::Custom(npos.into_custom_parameter()),
    ];
    config.validator_keys = Some(validator_keys);
    config.genesis_instructions = vec![
        RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
            peer_id: local_peer.clone(),
            stake_account: validator_id.clone(),
            initial_stake: Quantity::from(1000_u32),
            metadata,
            monetary_plan: PublicLaneMonetaryPlanV1::genesis_registration(
                validator_asset_id,
                escrow_asset.clone(),
                Quantity::from(1000_u32),
            ),
        }
        .into(),
        ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
        }
        .into(),
    ];
    let chain = CertifiedTestChain::start(config).expect("original public-lane genesis");
    bond_delegator_stake(
        chain.state(),
        &validator_id,
        &delegator_id,
        delegator_asset_id,
        escrow_asset,
        local_peer.clone(),
    );
    PublicLaneFixture {
        chain,
        validator: validator_id,
        local_peer,
    }
}
fn bond_delegator_stake(
    state: &Arc<State>,
    validator: &AccountId,
    delegator: &AccountId,
    source_asset: AssetId,
    destination_asset: AssetId,
    peer_id: PeerId,
) {
    let mut block = state.block(block_header(2));
    let mut tx = block.transaction();
    BondPublicLaneStake {
        lane_id: LaneId::SINGLE,
        validator: validator.clone(),
        staker: delegator.clone(),
        amount: Quantity::from(250_u32),
        metadata: Metadata::default(),
        monetary_plan: PublicLaneMonetaryPlanV1 {
            network_scope: PublicLaneMonetaryScopeV1::Network(*state.network_id_ref()),
            valid_until_height: 2,
            source_asset,
            destination_asset,
            amount: Quantity::from(250_u32),
            precondition: PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
                activation_height: 2,
                peer_id,
            }),
        },
    }
    .execute(delegator, &mut tx)
    .expect("bond stake");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit block");
}
fn build_test_router(
    state: &Arc<State>,
    kura: &Arc<Kura>,
    local_peer_id: &PeerId,
) -> iroha_torii::TestApiRouterRuntime {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let queue_cfg = Queue::default();
    let (events_tx, _events_rx) = broadcast::channel(1);
    let queue = Arc::new(iroha_core::queue::Queue::from_config(queue_cfg, events_tx));
    let torii = fixtures::ToriiHarness::new(
        &cfg,
        kura,
        state,
        &queue,
        local_peer_id,
        broadcast::channel(1).0,
        iroha_config::parameters::actual::TelemetryProfile::Operator,
    );
    torii.router()
}
fn block_header(height: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("height must be non-zero"),
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
