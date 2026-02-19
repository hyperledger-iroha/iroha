#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use std::{
    collections::BTreeSet,
    num::NonZeroU32,
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::SumeragiStatusWire,
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        isi::{
            Grant, InstructionBox, Log, Mint, Register,
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility},
        peer::PeerId,
        prelude::{FindAssets, Numeric, QueryBuilderExt},
    },
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_executor_data_model::permission::{
    asset::CanTransferAssetWithDefinition, asset_definition::CanRegisterAssetDefinition,
};
use iroha_test_network::{NetworkBuilder, genesis_factory_with_post_topology};
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use norito::json::Value as JsonValue;
use toml::{Table, Value as TomlValue};

const NEXUS_ALIAS: &str = "nexus";
const DS1_ALIAS: &str = "ds1";
const DS2_ALIAS: &str = "ds2";
const NEXUS_ID_U64: u64 = 0;
const DS1_ID_U64: u64 = 1;
const DS2_ID_U64: u64 = 2;
const NEXUS_LANE_INDEX: u32 = 0;
const DS1_LANE_INDEX: u32 = 1;
const DS2_LANE_INDEX: u32 = 2;
const TOTAL_PEERS: usize = 12;
const VALIDATORS_PER_LANE: usize = 4;
const VALIDATOR_STAKE: u64 = 2_000;
const STAKE_ASSET_ID: &str = "xor#nexus";
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);

fn localnet_builder() -> NetworkBuilder {
    let gas_account_str = format!("{}@ivm", SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key());
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology =
                npos_multilane_genesis_post_topology_transactions(topology.as_ref());
            let mut genesis = genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            );
            genesis
                .0
                .set_da_proof_policies(Some(multilane_da_proof_policy_bundle()));
            genesis
        })
        .with_config_layer(move |layer| {
            let mut lane_nexus = Table::new();
            lane_nexus.insert("index".into(), TomlValue::Integer(0));
            lane_nexus.insert("alias".into(), TomlValue::String("lane-nexus".to_owned()));
            lane_nexus.insert(
                "dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            lane_nexus.insert("visibility".into(), TomlValue::String("public".to_owned()));
            lane_nexus.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut lane_ds1 = Table::new();
            lane_ds1.insert("index".into(), TomlValue::Integer(1));
            lane_ds1.insert("alias".into(), TomlValue::String("lane-ds1".to_owned()));
            lane_ds1.insert("dataspace".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            lane_ds1.insert(
                "visibility".into(),
                TomlValue::String("restricted".to_owned()),
            );
            lane_ds1.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut lane_ds2 = Table::new();
            lane_ds2.insert("index".into(), TomlValue::Integer(2));
            lane_ds2.insert("alias".into(), TomlValue::String("lane-ds2".to_owned()));
            lane_ds2.insert("dataspace".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            lane_ds2.insert(
                "visibility".into(),
                TomlValue::String("restricted".to_owned()),
            );
            lane_ds2.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut ds_nexus = Table::new();
            ds_nexus.insert("alias".into(), TomlValue::String(NEXUS_ALIAS.to_owned()));
            ds_nexus.insert("id".into(), TomlValue::Integer(NEXUS_ID_U64 as i64));
            ds_nexus.insert(
                "description".into(),
                TomlValue::String("main nexus dataspace".to_owned()),
            );
            ds_nexus.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut ds1 = Table::new();
            ds1.insert("alias".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            ds1.insert("id".into(), TomlValue::Integer(DS1_ID_U64 as i64));
            ds1.insert(
                "description".into(),
                TomlValue::String("private dataspace one".to_owned()),
            );
            ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut ds2 = Table::new();
            ds2.insert("alias".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            ds2.insert("id".into(), TomlValue::Integer(DS2_ID_U64 as i64));
            ds2.insert(
                "description".into(),
                TomlValue::String("private dataspace two".to_owned()),
            );
            ds2.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut matcher_alice = Table::new();
            matcher_alice.insert("account".into(), TomlValue::String(ALICE_ID.to_string()));
            let mut rule_alice = Table::new();
            rule_alice.insert("lane".into(), TomlValue::Integer(1));
            rule_alice.insert("dataspace".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            rule_alice.insert("matcher".into(), TomlValue::Table(matcher_alice));

            let mut matcher_bob = Table::new();
            matcher_bob.insert("account".into(), TomlValue::String(BOB_ID.to_string()));
            let mut rule_bob = Table::new();
            rule_bob.insert("lane".into(), TomlValue::Integer(2));
            rule_bob.insert("dataspace".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            rule_bob.insert("matcher".into(), TomlValue::Table(matcher_bob));

            let mut policy = Table::new();
            policy.insert("default_lane".into(), TomlValue::Integer(0));
            policy.insert(
                "default_dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            policy.insert(
                "rules".into(),
                TomlValue::Array(vec![
                    TomlValue::Table(rule_alice),
                    TomlValue::Table(rule_bob),
                ]),
            );

            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 3_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(lane_nexus),
                        TomlValue::Table(lane_ds1),
                        TomlValue::Table(lane_ds2),
                    ]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(ds_nexus),
                        TomlValue::Table(ds1),
                        TomlValue::Table(ds2),
                    ]),
                )
                .write(["nexus", "routing_policy"], TomlValue::Table(policy))
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(["nexus", "staking", "stake_asset_id"], STAKE_ASSET_ID)
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "max_validators"],
                    VALIDATORS_PER_LANE as i64,
                )
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "npos", "election", "max_validators"],
                    VALIDATORS_PER_LANE as i64,
                )
                .write(["sumeragi", "npos", "epoch_length_blocks"], 3600_i64)
                .write(
                    ["sumeragi", "npos", "vrf", "commit_deadline_offset_blocks"],
                    100_i64,
                )
                .write(
                    ["sumeragi", "npos", "vrf", "reveal_deadline_offset_blocks"],
                    40_i64,
                );
        })
}

fn multilane_da_proof_policy_bundle() -> DaProofPolicyBundle {
    let lane_count = NonZeroU32::new(3).expect("lane count");
    let lanes = vec![
        ModelLaneConfig {
            id: LaneId::new(NEXUS_LANE_INDEX),
            dataspace_id: DataSpaceId::new(NEXUS_ID_U64),
            alias: "lane-nexus".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(DS1_LANE_INDEX),
            dataspace_id: DataSpaceId::new(DS1_ID_U64),
            alias: "lane-ds1".to_owned(),
            visibility: LaneVisibility::Restricted,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(DS2_LANE_INDEX),
            dataspace_id: DataSpaceId::new(DS2_ID_U64),
            alias: "lane-ds2".to_owned(),
            visibility: LaneVisibility::Restricted,
            ..ModelLaneConfig::default()
        },
    ];
    let catalog = LaneCatalog::new(lane_count, lanes).expect("lane catalog");
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    proof_policy_bundle(&lane_config)
}

fn npos_multilane_genesis_post_topology_transactions(
    topology: &[PeerId],
) -> Vec<Vec<InstructionBox>> {
    assert_eq!(
        topology.len(),
        TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers in genesis topology, got {}",
        topology.len()
    );
    let nexus_domain: DomainId = "nexus".parse().expect("nexus domain");
    let ivm_domain: DomainId = "ivm".parse().expect("ivm domain");
    let stake_asset_id: AssetDefinitionId = STAKE_ASSET_ID.parse().expect("stake asset definition");
    let gas_account_id = AccountId::new(
        ivm_domain.clone(),
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
    );

    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::domain(Domain::new(ivm_domain)).into(),
        Register::account(Account::new(gas_account_id)).into(),
        Register::asset_definition(AssetDefinition::numeric(stake_asset_id.clone())).into(),
    ];

    let mut validator_tx = Vec::with_capacity(TOTAL_PEERS * 2);
    for (index, peer) in topology.iter().enumerate() {
        let lane_index = if index < VALIDATORS_PER_LANE {
            NEXUS_LANE_INDEX
        } else if index < VALIDATORS_PER_LANE * 2 {
            DS1_LANE_INDEX
        } else {
            DS2_LANE_INDEX
        };
        let lane_id = LaneId::new(lane_index);
        let validator_id = AccountId::new(nexus_domain.clone(), peer.public_key().clone());
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_numeric(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        validator_tx.push(
            RegisterPublicLaneValidator::new(
                lane_id,
                validator_id.clone(),
                validator_id.clone(),
                Numeric::from(VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        validator_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }

    vec![bootstrap_tx, validator_tx]
}

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = client
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        if status.commit_qc.height >= target_height {
            return Ok(status);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}"
    ))
}

fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    let assets = client
        .query(FindAssets::new())
        .execute_all()
        .map_err(|err| eyre!(err))?;
    Ok(assets
        .into_iter()
        .find(|asset| &asset.id == asset_id)
        .map_or_else(Numeric::zero, |asset| asset.value().clone()))
}

fn has_dataspace_commitment(status: &SumeragiStatusWire, dataspace_id: u64) -> bool {
    let expected = DataSpaceId::new(dataspace_id);
    status
        .dataspace_commitments
        .iter()
        .any(|entry| entry.dataspace_id == expected && entry.tx_count > 0)
}

fn wait_for_dataspace_commitment(
    observer: &Client,
    tick_submitter: &Client,
    min_height: u64,
    dataspace_id: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_commitments = String::new();
    let mut tick = 0_u64;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = observer
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        last_commitments = format!("{:?}", status.dataspace_commitments);
        if status.commit_qc.height >= min_height && has_dataspace_commitment(&status, dataspace_id)
        {
            return Ok(status);
        }
        tick = tick.saturating_add(1);
        let _ = tick_submitter.submit(Log::new(Level::INFO, format!("{context} tick {tick}")));
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for dataspace commitment {dataspace_id} at or above height {min_height}; last height {last_height}, last commitments {last_commitments}"
    ))
}

fn wait_for_expected_balances(
    client: &Client,
    expectations: &[(&AssetId, Numeric)],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        last_observed.clear();
        let mut all_match = true;
        for (asset_id, expected) in expectations {
            let observed = asset_balance(client, asset_id)?;
            if observed != *expected {
                all_match = false;
            }
            last_observed.push(((*asset_id).clone(), observed));
        }
        if all_match {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for expected balances; last observed {last_observed:?}"
    ))
}

fn lane_validator_snapshot(
    snapshot: &JsonValue,
    context: &str,
) -> Result<(usize, BTreeSet<String>)> {
    let root = snapshot
        .as_object()
        .ok_or_else(|| eyre!("{context}: lane validator response is not an object"))?;
    let total = root
        .get("total")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| eyre!("{context}: lane validator response is missing total"))?;
    let items = root
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: lane validator response is missing items"))?;

    let mut active = BTreeSet::new();
    for item in items {
        let entry = item
            .as_object()
            .ok_or_else(|| eyre!("{context}: validator entry is not an object"))?;
        let validator = entry
            .get("validator")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing validator literal"))?;
        let status_type = entry
            .get("status")
            .and_then(JsonValue::as_object)
            .and_then(|status| status.get("type"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing status.type"))?;
        if status_type == "Active" {
            active.insert(validator.to_owned());
        }
    }

    Ok((usize::try_from(total).unwrap_or(usize::MAX), active))
}

fn wait_for_active_lane_validators(
    client: &Client,
    lane_id: LaneId,
    expected_active: &BTreeSet<String>,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_total = 0usize;
    let mut last_active = BTreeSet::new();
    let mut tick = 0_u64;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let snapshot = client
            .get_public_lane_validators(lane_id, None)
            .map_err(|err| eyre!(err))?;
        let (total, active) = lane_validator_snapshot(&snapshot, context)?;
        last_total = total;
        last_active = active.clone();
        if total == expected_active.len() && active == *expected_active {
            return Ok(());
        }
        tick = tick.saturating_add(1);
        let _ = client.submit(Log::new(
            Level::INFO,
            format!("{context} activation tick {tick}"),
        ));
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for active validators on lane {lane_id}; expected total {} active {:?}, observed total {} active {:?}",
        expected_active.len(),
        expected_active,
        last_total,
        last_active
    ))
}

#[test]
fn cross_dataspace_atomic_swap_is_all_or_nothing() -> Result<()> {
    let context = stringify!(cross_dataspace_atomic_swap_is_all_or_nothing);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(localnet_builder(), context)?
    else {
        return Ok(());
    };

    let alice = network.client();
    let bob = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());

    let peers = network.peers();
    ensure!(
        peers.len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers for cross-dataspace topology, got {}",
        peers.len()
    );
    let validator_domain: DomainId = "nexus".parse().expect("validator domain");
    let nexus_lane_validators: Vec<AccountId> = peers
        .iter()
        .take(VALIDATORS_PER_LANE)
        .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
        .collect();
    let ds1_lane_validators: Vec<AccountId> = peers
        .iter()
        .skip(VALIDATORS_PER_LANE)
        .take(VALIDATORS_PER_LANE)
        .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
        .collect();
    let ds2_lane_validators: Vec<AccountId> = peers
        .iter()
        .skip(VALIDATORS_PER_LANE * 2)
        .take(VALIDATORS_PER_LANE)
        .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
        .collect();
    let mut all_validators = Vec::with_capacity(TOTAL_PEERS);
    all_validators.extend(nexus_lane_validators.iter().cloned());
    all_validators.extend(ds1_lane_validators.iter().cloned());
    all_validators.extend(ds2_lane_validators.iter().cloned());
    let unique_validators: BTreeSet<_> = all_validators.into_iter().collect();
    ensure!(
        unique_validators.len() == TOTAL_PEERS,
        "validator groups must be disjoint and total {}",
        TOTAL_PEERS
    );
    let expected_nexus_validators: BTreeSet<_> = nexus_lane_validators
        .iter()
        .map(ToString::to_string)
        .collect();
    let expected_ds1_validators: BTreeSet<_> = ds1_lane_validators
        .iter()
        .map(ToString::to_string)
        .collect();
    let expected_ds2_validators: BTreeSet<_> = ds2_lane_validators
        .iter()
        .map(ToString::to_string)
        .collect();
    wait_for_active_lane_validators(
        &alice,
        LaneId::new(NEXUS_LANE_INDEX),
        &expected_nexus_validators,
        "nexus lane validator activation",
    )?;
    wait_for_active_lane_validators(
        &alice,
        LaneId::new(DS1_LANE_INDEX),
        &expected_ds1_validators,
        "ds1 lane validator activation",
    )?;
    wait_for_active_lane_validators(
        &alice,
        LaneId::new(DS2_LANE_INDEX),
        &expected_ds2_validators,
        "ds2 lane validator activation",
    )?;
    let lane_sync_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;
    let _lane_sync_on_bob = wait_for_height(
        &bob,
        lane_sync_height,
        "lane validator activation propagation",
    )?;

    let initial_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;

    alice.submit_blocking(Log::new(Level::INFO, "route probe ds1".to_string()))?;
    let ds1_status = wait_for_dataspace_commitment(
        &alice,
        &alice,
        initial_height + 1,
        DS1_ID_U64,
        "ds1 route probe",
    )?;
    let _ds1_status_bob = wait_for_height(&bob, ds1_status.commit_qc.height, "ds1 probe on bob")?;

    bob.submit_blocking(Log::new(Level::INFO, "route probe ds2".to_string()))?;
    let _ds2_status = wait_for_dataspace_commitment(
        &alice,
        &bob,
        ds1_status.commit_qc.height + 1,
        DS2_ID_U64,
        "ds2 route probe",
    )?;

    let ds1_asset_def: AssetDefinitionId = "ds1coin#wonderland".parse().expect("asset definition");
    let ds2_asset_def: AssetDefinitionId = "ds2coin#wonderland".parse().expect("asset definition");
    let wonderland_domain = "wonderland".parse().expect("domain id");

    alice.submit_blocking(Grant::account_permission(
        CanRegisterAssetDefinition {
            domain: wonderland_domain,
        },
        BOB_ID.clone(),
    ))?;

    alice.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
        ds1_asset_def.clone(),
    )))?;
    bob.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
        ds2_asset_def.clone(),
    )))?;

    let alice_ds1_asset = AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone());
    let bob_ds1_asset = AssetId::new(ds1_asset_def.clone(), BOB_ID.clone());
    let alice_ds2_asset = AssetId::new(ds2_asset_def.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_def.clone(), BOB_ID.clone());

    alice.submit_blocking(Grant::account_permission(
        CanTransferAssetWithDefinition {
            asset_definition: ds1_asset_def.clone(),
        },
        BOB_ID.clone(),
    ))?;

    alice.submit_blocking(Mint::asset_numeric(100_u32, alice_ds1_asset.clone()))?;
    bob.submit_blocking(Mint::asset_numeric(200_u32, bob_ds2_asset.clone()))?;

    let successful_swap = DvpIsi::new(
        "ds1ds2swapok".parse().expect("settlement id"),
        SettlementLeg::new(
            ds1_asset_def.clone(),
            Numeric::from(30_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementLeg::new(
            ds2_asset_def.clone(),
            Numeric::from(45_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    alice.submit_blocking(InstructionBox::from(successful_swap))?;
    let synced_after_success = alice.get_sumeragi_status_wire().map_err(|err| eyre!(err))?;
    let _synced_after_success_bob = wait_for_height(
        &bob,
        synced_after_success.commit_qc.height,
        "successful swap propagation to bob",
    )?;
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(70_u32)),
            (&bob_ds1_asset, Numeric::from(30_u32)),
            (&alice_ds2_asset, Numeric::from(45_u32)),
            (&bob_ds2_asset, Numeric::from(155_u32)),
        ],
        "successful swap balances",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(70_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(30_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(45_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(155_u32)
    );

    let reverse_successful_swap = DvpIsi::new(
        "ds2ds1swapok".parse().expect("settlement id"),
        SettlementLeg::new(
            ds2_asset_def.clone(),
            Numeric::from(20_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementLeg::new(
            ds1_asset_def.clone(),
            Numeric::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    bob.submit_blocking(InstructionBox::from(reverse_successful_swap))?;
    let synced_after_reverse = bob.get_sumeragi_status_wire().map_err(|err| eyre!(err))?;
    let _synced_after_reverse_bob = wait_for_height(
        &alice,
        synced_after_reverse.commit_qc.height,
        "reverse swap propagation to alice",
    )?;
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(60_u32)),
            (&bob_ds1_asset, Numeric::from(40_u32)),
            (&alice_ds2_asset, Numeric::from(65_u32)),
            (&bob_ds2_asset, Numeric::from(135_u32)),
        ],
        "reverse swap balances",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(60_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(40_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(65_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(135_u32)
    );

    let failing_swap = DvpIsi::new(
        "ds1ds2swapfail".parse().expect("settlement id"),
        SettlementLeg::new(
            ds1_asset_def,
            Numeric::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementLeg::new(
            ds2_asset_def,
            Numeric::from(10_000_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    let failure = alice
        .submit_blocking(InstructionBox::from(failing_swap))
        .expect_err("underfunded counter-leg must reject all-or-nothing settlement");
    let failure_text = failure.to_string();
    assert!(
        failure_text.contains("settlement leg requires 10000")
            || failure_text.contains("requires 10000"),
        "unexpected failure message: {failure_text}"
    );
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(60_u32)),
            (&bob_ds1_asset, Numeric::from(40_u32)),
            (&alice_ds2_asset, Numeric::from(65_u32)),
            (&bob_ds2_asset, Numeric::from(135_u32)),
        ],
        "rollback balances after failing swap",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(60_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(40_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(65_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(135_u32)
    );

    Ok(())
}
