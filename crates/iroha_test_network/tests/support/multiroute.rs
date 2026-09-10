//! Shared four-validator, three-public-lane fixture using production NPoS and DA policies.
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::{Account, AccountId},
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    da::commitment::DaProofPolicyBundle,
    domain::{Domain, DomainId},
    isi::{
        InstructionBox, Mint, Register, SetParameter,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    metadata::Metadata,
    nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility},
    parameter::{Parameter, system::SumeragiNposParameters},
    peer::PeerId,
    prelude::Quantity,
};
use iroha_test_network::{NetworkBuilder, unexecuted_genesis_factory_with_post_topology};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use std::time::Duration;
use toml::{Table, Value as TomlValue};

const SMOKE_PIPELINE_TIME: Duration = Duration::from_secs(2);
const ROUTE_VALIDATOR_STAKE: u32 = 2_000;
pub(super) const ROUTE_VALIDATOR_FEE_SEED_AMOUNT: u32 = 1_000_000;
const ROUTE_STAKE_ASSET_NAME: &str = "Route Stake";
const ROUTE_FEE_ASSET_NAME: &str = "Route Fee";

fn route_lane_validator_account(index: usize) -> AccountId {
    let key_pair = checked_localnet_smoke_keypair(
        format!("integration_tests::sumeragi_localnet_smoke::route-validator::{index}")
            .into_bytes(),
        Algorithm::Ed25519,
    );
    AccountId::new(key_pair.public_key().clone())
}
fn route_bootstrap_gas_account_id() -> AccountId {
    let key_pair = checked_localnet_smoke_keypair(
        b"integration_tests::sumeragi_localnet_smoke::route-bootstrap-gas".to_vec(),
        Algorithm::Ed25519,
    );
    AccountId::new(key_pair.public_key().clone())
}
fn checked_localnet_smoke_keypair(seed: Vec<u8>, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(seed, algorithm).expect("derive localnet smoke fixture key")
}
fn route_stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}
pub(super) fn route_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
}
fn route_multilane_da_proof_policy_bundle() -> DaProofPolicyBundle {
    let lane_count = std::num::NonZeroU32::new(3).expect("lane count");
    let lanes = vec![
        ModelLaneConfig {
            id: LaneId::new(0),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "lane-universal".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            alias: "lane-alice".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(2),
            alias: "lane-bob".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
    ];
    let catalog = LaneCatalog::new(lane_count, lanes).expect("route lane catalog");
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    proof_policy_bundle(&lane_config)
}
fn route_multilane_genesis_post_topology_transactions(
    topology: &[PeerId],
) -> Vec<Vec<InstructionBox>> {
    let stake_asset_id = route_stake_asset_definition_id();
    let fee_asset_id = route_fee_asset_definition_id();
    let gas_account_id = route_bootstrap_gas_account_id();
    let lane_ids = [LaneId::new(0), LaneId::new(1), LaneId::new(2)];
    let mint_amount = ROUTE_VALIDATOR_STAKE
        .saturating_mul(u32::try_from(lane_ids.len()).expect("lane count fits into u32"));
    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(
            DomainId::try_new("nexus", "universal").expect("nexus domain"),
        ))
        .into(),
        Register::domain(Domain::new(
            DomainId::try_new("universal", "universal").expect("universal domain"),
        ))
        .into(),
        Register::account(Account::new(gas_account_id.clone())).into(),
        Register::asset_definition(
            AssetDefinition::new(
                stake_asset_id.clone(),
                ROUTE_STAKE_ASSET_NAME.to_owned(),
                Default::default(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .with_metadata(Metadata::default()),
        )
        .into(),
        Register::asset_definition(
            AssetDefinition::new(
                fee_asset_id.clone(),
                ROUTE_FEE_ASSET_NAME.to_owned(),
                Default::default(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .with_metadata(Metadata::default()),
        )
        .into(),
        Mint::asset_quantity(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), gas_account_id),
        )
        .into(),
    ];
    let mut validator_tx = Vec::with_capacity(topology.len() * 2);
    for (index, peer_id) in topology.iter().enumerate() {
        let validator_id = route_lane_validator_account(index);
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_quantity(
                mint_amount,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
                ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        for lane_id in lane_ids {
            validator_tx.push(
                RegisterPublicLaneValidator::new(
                    lane_id,
                    validator_id.clone(),
                    peer_id.clone(),
                    validator_id.clone(),
                    Quantity::from(ROUTE_VALIDATOR_STAKE),
                    Metadata::default(),
                )
                .into(),
            );
            validator_tx
                .push(ActivatePublicLaneValidator::new(lane_id, validator_id.clone()).into());
        }
    }
    vec![bootstrap_tx, validator_tx]
}
pub(super) fn network_builder() -> NetworkBuilder {
    let mut lane_universal = Table::new();
    lane_universal.insert("index".into(), TomlValue::Integer(0));
    lane_universal.insert(
        "alias".into(),
        TomlValue::String("lane-universal".to_owned()),
    );
    lane_universal.insert(
        "dataspace".into(),
        TomlValue::String("universal".to_owned()),
    );
    lane_universal.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_universal.insert("metadata".into(), TomlValue::Table(Table::new()));
    let mut lane_alice = Table::new();
    lane_alice.insert("index".into(), TomlValue::Integer(1));
    lane_alice.insert("alias".into(), TomlValue::String("lane-alice".to_owned()));
    lane_alice.insert("dataspace".into(), TomlValue::String("ds1".to_owned()));
    lane_alice.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_alice.insert("metadata".into(), TomlValue::Table(Table::new()));
    let mut lane_bob = Table::new();
    lane_bob.insert("index".into(), TomlValue::Integer(2));
    lane_bob.insert("alias".into(), TomlValue::String("lane-bob".to_owned()));
    lane_bob.insert("dataspace".into(), TomlValue::String("ds2".to_owned()));
    lane_bob.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_bob.insert("metadata".into(), TomlValue::Table(Table::new()));
    let mut ds_universal = Table::new();
    ds_universal.insert("alias".into(), TomlValue::String("universal".to_owned()));
    ds_universal.insert("id".into(), TomlValue::Integer(0));
    ds_universal.insert(
        "description".into(),
        TomlValue::String("default dataspace".to_owned()),
    );
    ds_universal.insert("fault_tolerance".into(), TomlValue::Integer(1));
    let mut ds1 = Table::new();
    ds1.insert("alias".into(), TomlValue::String("ds1".to_owned()));
    ds1.insert("id".into(), TomlValue::Integer(1));
    ds1.insert(
        "manifest_hash".into(),
        TomlValue::String(
            "0100000000000000000000000000000000000000000000000000000000000000".to_owned(),
        ),
    );
    ds1.insert(
        "description".into(),
        TomlValue::String("alice route dataspace".to_owned()),
    );
    ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));
    let mut ds2 = Table::new();
    ds2.insert("alias".into(), TomlValue::String("ds2".to_owned()));
    ds2.insert("id".into(), TomlValue::Integer(2));
    ds2.insert(
        "manifest_hash".into(),
        TomlValue::String(
            "0200000000000000000000000000000000000000000000000000000000000000".to_owned(),
        ),
    );
    ds2.insert(
        "description".into(),
        TomlValue::String("bob route dataspace".to_owned()),
    );
    ds2.insert("fault_tolerance".into(), TomlValue::Integer(1));
    let mut matcher_alice = Table::new();
    matcher_alice.insert("account".into(), TomlValue::String(ALICE_ID.to_string()));
    let mut rule_alice = Table::new();
    rule_alice.insert("lane".into(), TomlValue::Integer(1));
    rule_alice.insert("dataspace".into(), TomlValue::String("ds1".to_owned()));
    rule_alice.insert("matcher".into(), TomlValue::Table(matcher_alice));
    let mut matcher_bob = Table::new();
    matcher_bob.insert("account".into(), TomlValue::String(BOB_ID.to_string()));
    let mut rule_bob = Table::new();
    rule_bob.insert("lane".into(), TomlValue::Integer(2));
    rule_bob.insert("dataspace".into(), TomlValue::String("ds2".to_owned()));
    rule_bob.insert("matcher".into(), TomlValue::Table(matcher_bob));
    let mut policy = Table::new();
    policy.insert("default_lane".into(), TomlValue::Integer(0));
    policy.insert(
        "default_dataspace".into(),
        TomlValue::String("universal".to_owned()),
    );
    policy.insert(
        "rules".into(),
        TomlValue::Array(vec![
            TomlValue::Table(rule_alice),
            TomlValue::Table(rule_bob),
        ]),
    );
    let gas_account_str = route_bootstrap_gas_account_id()
        .canonical_i105()
        .expect("canonical I105 bootstrap gas account literal");
    let stake_asset_id_literal = route_stake_asset_definition_id().to_string();
    let fee_asset_id_literal = route_fee_asset_definition_id().to_string();
    let mut npos = SumeragiNposParameters::default();
    npos.max_validators = 4;
    npos.epoch_length_blocks = std::num::NonZeroU64::new(3_600).unwrap();
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology =
                route_multilane_genesis_post_topology_transactions(topology.as_ref());
            let mut genesis = unexecuted_genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            );
            genesis
                .0
                .set_da_proof_policies(Some(route_multilane_da_proof_policy_bundle()));
            genesis
        })
        .with_block_cadence(SMOKE_PIPELINE_TIME)
        .with_npos_consensus()
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )))
        .with_config_layer(move |layer| {
            layer
                .write(["nexus", "lane_count"], 3_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(lane_universal.clone()),
                        TomlValue::Table(lane_alice.clone()),
                        TomlValue::Table(lane_bob.clone()),
                    ]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(ds_universal.clone()),
                        TomlValue::Table(ds1.clone()),
                        TomlValue::Table(ds2.clone()),
                    ]),
                )
                .write(
                    ["nexus", "routing_policy"],
                    TomlValue::Table(policy.clone()),
                )
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    fee_asset_id_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(["nexus", "staking", "max_validators"], 4_i64);
        })
}
