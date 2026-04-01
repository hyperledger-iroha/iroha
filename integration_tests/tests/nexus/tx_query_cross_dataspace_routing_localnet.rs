#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Focused wrong-dataspace Torii ingress regression for transaction and query routing.

use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroU64},
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, UaidManifestQuery, UaidManifestStatus, UaidManifestStatusFilter},
    crypto::{Hash, HashOf},
    data_model::{
        Level, ValidationFail,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::SumeragiStatusWire,
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        isi::{
            Grant, InstructionBox, Log, Mint, Register, Revoke,
            space_directory::{PublishSpaceDirectoryManifest, RevokeSpaceDirectoryManifest},
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{
            Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceId,
            LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility, ManifestEffect,
            ManifestEntry, ManifestVersion, UniversalAccountId,
        },
        peer::PeerId,
        permission::Permission,
        prelude::{FindAssetById, FindPermissionsByAccountId, Numeric},
        transaction::{SignedTransaction, TransactionEntrypoint, TransactionSubmissionReceipt},
    },
    query::QueryError,
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    prelude::QueryBuilderExt,
    query::{
        CommittedTxFilters,
        dsl::CompoundPredicate,
        error::{FindError, QueryExecutionFail},
        parameters::{FetchSize, Pagination},
        transaction::prelude::FindTransactions,
    },
};
use iroha_executor_data_model::permission::{
    account::CanModifyAccountMetadata, nexus::CanPublishSpaceDirectoryManifest,
};
use iroha_test_network::{NetworkBuilder, genesis_factory_with_post_topology};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use norito::{decode_from_bytes, json::Value as JsonValue};
use reqwest::StatusCode as HttpStatusCode;
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
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const COMMITTED_TX_OUTCOME_TIMEOUT: Duration = Duration::from_secs(45);
const MANIFEST_QUERY_LIMIT: u32 = 16;
const ALICE_WRONG_INGRESS_INDEX: usize = VALIDATORS_PER_LANE * 2;
const BOB_WRONG_INGRESS_INDEX: usize = VALIDATORS_PER_LANE;

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        "nexus".parse().expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}

fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse()
        .expect("default nexus fee asset id")
}

fn routing_probe_gas_account_id() -> AccountId {
    ALICE_ID.clone()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ExpectedLaneValidatorBinding {
    validator: String,
    peer_id: String,
}

#[derive(Debug)]
struct RoutedJsonResponse {
    status: HttpStatusCode,
    body: JsonValue,
    body_text: String,
    routed_by: Option<String>,
    route_lane_id: Option<String>,
    route_dataspace_id: Option<String>,
}

#[derive(Debug)]
struct RoutedTransactionSubmitResponse {
    status: HttpStatusCode,
    receipt: Option<TransactionSubmissionReceipt>,
    body_text: String,
    routed_by: Option<String>,
    route_lane_id: Option<String>,
    route_dataspace_id: Option<String>,
}

fn validator_authority_account_for_peer(index: usize) -> AccountId {
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xC1;
    seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
}

fn expected_lane_binding_for_peer(index: usize, peer_id: &PeerId) -> ExpectedLaneValidatorBinding {
    ExpectedLaneValidatorBinding {
        validator: validator_authority_account_for_peer(index).to_string(),
        peer_id: peer_id.to_string(),
    }
}

fn localnet_builder() -> NetworkBuilder {
    let gas_account_str = routing_probe_gas_account_id()
        .canonical_i105()
        .expect("canonical I105 gas account literal");
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
                .write(["norito", "allow_gpu_compression"], false)
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
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal(),
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
    let universal_domain: DomainId = "universal".parse().expect("universal domain");
    let ds1_domain: DomainId = "ds1".parse().expect("ds1 domain");
    let ds2_domain: DomainId = "ds2".parse().expect("ds2 domain");
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = nexus_fee_asset_definition_id();
    let ds1_asset_def = AssetDefinitionId::new(
        "nexus".parse().expect("asset definition domain"),
        "ds1coin".parse().expect("asset definition name"),
    );
    let ds2_asset_def = AssetDefinitionId::new(
        "nexus".parse().expect("asset definition domain"),
        "ds2coin".parse().expect("asset definition name"),
    );

    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::domain(Domain::new(universal_domain)).into(),
        Register::domain(Domain::new(ds1_domain)).into(),
        Register::domain(Domain::new(ds2_domain)).into(),
        Register::asset_definition({
            let __asset_definition_id = stake_asset_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = fee_asset_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds1_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds2_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Mint::asset_numeric(
            100_u32,
            AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(200_u32, AssetId::new(ds2_asset_def.clone(), BOB_ID.clone())).into(),
    ];

    for (index, peer) in topology.iter().enumerate() {
        let lane_index = if index < VALIDATORS_PER_LANE {
            NEXUS_LANE_INDEX
        } else if index < VALIDATORS_PER_LANE * 2 {
            DS1_LANE_INDEX
        } else {
            DS2_LANE_INDEX
        };
        let lane_id = LaneId::new(lane_index);
        let validator_id = validator_authority_account_for_peer(index);
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_numeric(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_numeric(
                NEXUS_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            RegisterPublicLaneValidator::new(
                lane_id,
                validator_id.clone(),
                peer.clone(),
                validator_id.clone(),
                Numeric::from(VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        bootstrap_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }

    vec![bootstrap_tx]
}

fn lane_validator_snapshot(
    snapshot: &JsonValue,
    context: &str,
) -> Result<(usize, BTreeSet<ExpectedLaneValidatorBinding>)> {
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
        let peer_id = entry
            .get("peer_id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing peer_id literal"))?;
        let status_type = entry
            .get("status")
            .and_then(JsonValue::as_object)
            .and_then(|status| status.get("type"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing status.type"))?;
        if status_type == "Active" {
            active.insert(ExpectedLaneValidatorBinding {
                validator: validator.to_owned(),
                peer_id: peer_id.to_owned(),
            });
        }
    }

    Ok((usize::try_from(total).unwrap_or(usize::MAX), active))
}

fn wait_for_active_lane_validators(
    client: &Client,
    lane_id: LaneId,
    expected_active: &BTreeSet<ExpectedLaneValidatorBinding>,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_total = 0usize;
    let mut last_active = BTreeSet::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let snapshot = client
            .get_public_lane_validators(lane_id)
            .map_err(|err| eyre!(err))?;
        let (total, active) = lane_validator_snapshot(&snapshot, context)?;
        last_total = total;
        last_active = active.clone();
        if total == expected_active.len() && active == *expected_active {
            return Ok(());
        }
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

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.get_sumeragi_status_wire() {
            Ok(status) => {
                last_height = status.commit_qc.height;
                if status.commit_qc.height >= target_height {
                    return Ok(status);
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}{suffix}"
    ))
}

fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => Ok(asset.value().clone()),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(Numeric::zero()),
        Err(err) => Err(eyre!(err)),
    }
}

fn query_account_permissions(client: &Client, account_id: &AccountId) -> Result<Vec<Permission>> {
    client
        .query(FindPermissionsByAccountId::new(account_id.clone()))
        .execute_all()
        .map_err(|err| eyre!(err))
}

fn wait_for_account_permissions(
    client: &Client,
    account_id: &AccountId,
    required_permissions: &[Permission],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match query_account_permissions(client, account_id) {
            Ok(permissions) => {
                last_observed = permissions.clone();
                if required_permissions
                    .iter()
                    .all(|required| permissions.iter().any(|permission| permission == required))
                {
                    return Ok(());
                }
                last_error = None;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last permission query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for permissions on {account_id}; required {required_permissions:?}; last observed {last_observed:?}{suffix}"
    ))
}

fn wait_for_account_permissions_absence(
    client: &Client,
    account_id: &AccountId,
    forbidden_permissions: &[Permission],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match query_account_permissions(client, account_id) {
            Ok(permissions) => {
                last_observed = permissions.clone();
                if forbidden_permissions
                    .iter()
                    .all(|forbidden| permissions.iter().all(|permission| permission != forbidden))
                {
                    return Ok(());
                }
                last_error = None;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last permission query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for permissions on {account_id} to exclude {forbidden_permissions:?}; last observed {last_observed:?}{suffix}"
    ))
}

fn wait_for_manifest_status(
    client: &Client,
    uaid_literal: &str,
    dataspace: DataSpaceId,
    expected_status: UaidManifestStatus,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error = String::new();
    let mut last_statuses = Vec::<String>::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let query = UaidManifestQuery {
            dataspace_id: Some(dataspace.as_u64()),
            status: Some(UaidManifestStatusFilter::All),
            limit: Some(MANIFEST_QUERY_LIMIT),
            offset: Some(0),
        };
        match client.get_uaid_manifests(uaid_literal, Some(query)) {
            Ok(response) => {
                last_statuses = response
                    .manifests
                    .iter()
                    .map(|record| format!("{}:{:?}", record.dataspace_id, record.status))
                    .collect();
                if response.manifests.iter().any(|record| {
                    record.dataspace_id == dataspace.as_u64() && record.status == expected_status
                }) {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    if last_error.is_empty() {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} status {:?}; last statuses {last_statuses:?}",
            dataspace.as_u64(),
            expected_status
        ))
    } else {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} status {:?}; last statuses {last_statuses:?}; last error: {last_error}",
            dataspace.as_u64(),
            expected_status
        ))
    }
}

fn wait_for_manifest_absence(
    client: &Client,
    uaid_literal: &str,
    dataspace: DataSpaceId,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error = String::new();
    let mut last_statuses = Vec::<String>::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let query = UaidManifestQuery {
            dataspace_id: Some(dataspace.as_u64()),
            status: Some(UaidManifestStatusFilter::All),
            limit: Some(MANIFEST_QUERY_LIMIT),
            offset: Some(0),
        };
        match client.get_uaid_manifests(uaid_literal, Some(query)) {
            Ok(response) => {
                last_statuses = response
                    .manifests
                    .iter()
                    .map(|record| format!("{}:{:?}", record.dataspace_id, record.status))
                    .collect();
                if response
                    .manifests
                    .iter()
                    .all(|record| record.dataspace_id != dataspace.as_u64())
                {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    if last_error.is_empty() {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} absence; last statuses {last_statuses:?}",
            dataspace.as_u64()
        ))
    } else {
        Err(eyre!(
            "{context}: timed out waiting for UAID {uaid_literal} dataspace {} absence; last statuses {last_statuses:?}; last error: {last_error}",
            dataspace.as_u64()
        ))
    }
}

fn routed_header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn add_client_headers(
    client: &Client,
    mut request: reqwest::RequestBuilder,
    include_content_type: bool,
) -> reqwest::RequestBuilder {
    for (name, value) in &client.headers {
        if !include_content_type && name.eq_ignore_ascii_case("content-type") {
            continue;
        }
        request = request.header(name, value);
    }
    request
}

fn encode_versioned_signed_transaction(transaction: &SignedTransaction) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1);
    bytes.push(1);
    bytes.extend(norito::codec::encode_adaptive(transaction));
    bytes
}

async fn torii_json_get(
    client: &Client,
    path_segments: &[String],
    query_pairs: &[(String, String)],
) -> Result<RoutedJsonResponse> {
    let mut url = client.torii_url.clone();
    let torii_url_literal = url.to_string();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre!("torii URL `{torii_url_literal}` cannot accept path segments"))?;
        segments.pop_if_empty();
        for segment in path_segments {
            segments.push(segment);
        }
    }
    if !query_pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_pairs {
            query.append_pair(key, value);
        }
    }

    let request = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    let response = add_client_headers(client, request, true).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let json_body = norito::json::from_slice(&body)
        .wrap_err_with(|| format!("decode JSON body: {body_text}"))?;

    Ok(RoutedJsonResponse {
        status,
        body: json_body,
        body_text,
        routed_by: routed_header_string(&headers, "x-iroha-routed-by"),
        route_lane_id: routed_header_string(&headers, "x-iroha-route-lane-id"),
        route_dataspace_id: routed_header_string(&headers, "x-iroha-route-dataspace-id"),
    })
}

async fn torii_json_get_as_account(
    client: &Client,
    account: &AccountId,
    path_segments: &[String],
    query_pairs: &[(String, String)],
) -> Result<RoutedJsonResponse> {
    let mut url = client.torii_url.clone();
    let torii_url_literal = url.to_string();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre!("torii URL `{torii_url_literal}` cannot accept path segments"))?;
        segments.pop_if_empty();
        for segment in path_segments {
            segments.push(segment);
        }
    }
    if !query_pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_pairs {
            query.append_pair(key, value);
        }
    }

    let request = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            "X-Iroha-Account",
            account
                .canonical_i105()
                .expect("account header should encode as canonical I105"),
        );
    let response = add_client_headers(client, request, true).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let json_body = norito::json::from_slice(&body)
        .wrap_err_with(|| format!("decode JSON body: {body_text}"))?;

    Ok(RoutedJsonResponse {
        status,
        body: json_body,
        body_text,
        routed_by: routed_header_string(&headers, "x-iroha-routed-by"),
        route_lane_id: routed_header_string(&headers, "x-iroha-route-lane-id"),
        route_dataspace_id: routed_header_string(&headers, "x-iroha-route-dataspace-id"),
    })
}

async fn submit_transaction_raw(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<RoutedTransactionSubmitResponse> {
    let request = reqwest::Client::new()
        .post(
            client
                .torii_url
                .join("transaction")
                .wrap_err("compose /transaction URL")?,
        )
        .header(reqwest::header::CONTENT_TYPE, "application/x-norito")
        .body(encode_versioned_signed_transaction(transaction));
    let response = add_client_headers(client, request, false).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let receipt = if status == HttpStatusCode::ACCEPTED {
        Some(decode_from_bytes::<TransactionSubmissionReceipt>(&body)?)
    } else {
        None
    };

    Ok(RoutedTransactionSubmitResponse {
        status,
        receipt,
        body_text,
        routed_by: routed_header_string(&headers, "x-iroha-routed-by"),
        route_lane_id: routed_header_string(&headers, "x-iroha-route-lane-id"),
        route_dataspace_id: routed_header_string(&headers, "x-iroha-route-dataspace-id"),
    })
}

fn render_rejection_reason(
    reason: &iroha::data_model::transaction::error::TransactionRejectionReason,
) -> String {
    let display = reason.to_string();
    let debug = format!("{reason:?}");
    if display == debug {
        display
    } else {
        format!("{display}; details: {debug}")
    }
}

enum CommittedTxOutcome {
    Applied,
    Rejected(String),
}

fn wait_for_committed_tx_outcome(
    client: &Client,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<CommittedTxOutcome> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    let one = NonZeroU64::new(1).expect("nonzero");
    while started.elapsed() <= timeout_duration {
        let filters = CommittedTxFilters {
            entry_eq: Some(entry_hash.clone()),
            ..Default::default()
        };
        match client
            .query(FindTransactions::new())
            .filter(CompoundPredicate::from_filters(filters))
            .with_pagination(Pagination::new(Some(one), 0))
            .with_fetch_size(FetchSize::new(Some(one)))
            .execute_all()
        {
            Ok(snapshot) => {
                if let Some(tx) = snapshot.first() {
                    return match &tx.result().0 {
                        Ok(_) => Ok(CommittedTxOutcome::Applied),
                        Err(reason) => Ok(CommittedTxOutcome::Rejected(render_rejection_reason(
                            reason,
                        ))),
                    };
                }
                last_error = None;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last tx history query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for committed transaction outcome for transaction {entry_hash}{suffix}"
    ))
}

async fn submit_transaction_and_expect_route(
    submitter: &Client,
    confirmation_client: &Client,
    transaction: &SignedTransaction,
    expected_lane_id: LaneId,
    expected_dataspace_id: DataSpaceId,
    context: &str,
) -> Result<TransactionSubmissionReceipt> {
    let response = submit_transaction_raw(submitter, transaction).await?;
    ensure!(
        response.status == HttpStatusCode::ACCEPTED,
        "{context}: expected 202 Accepted, observed {} body `{}`",
        response.status,
        response.body_text
    );
    ensure!(
        response.routed_by.as_deref() == Some("proxy"),
        "{context}: expected proxy routing, observed {:?}",
        response.routed_by
    );
    ensure!(
        response.route_lane_id.as_deref() == Some(expected_lane_id.as_u32().to_string().as_str()),
        "{context}: expected routed lane {}, observed {:?}",
        expected_lane_id.as_u32(),
        response.route_lane_id
    );
    ensure!(
        response.route_dataspace_id.as_deref()
            == Some(expected_dataspace_id.as_u64().to_string().as_str()),
        "{context}: expected routed dataspace {}, observed {:?}",
        expected_dataspace_id.as_u64(),
        response.route_dataspace_id
    );

    let receipt = response
        .receipt
        .ok_or_else(|| eyre!("{context}: missing transaction submission receipt"))?;
    let entry_hash = transaction.hash_as_entrypoint();
    match wait_for_committed_tx_outcome(
        confirmation_client,
        entry_hash.clone(),
        context,
        COMMITTED_TX_OUTCOME_TIMEOUT,
    )? {
        CommittedTxOutcome::Applied => {}
        CommittedTxOutcome::Rejected(reason) => {
            return Err(eyre!(
                "{context}: transaction {entry_hash} rejected unexpectedly: {reason}"
            ));
        }
    }

    Ok(receipt)
}

fn expect_proxy_route_headers(
    response: &RoutedJsonResponse,
    expected_lane_id: LaneId,
    expected_dataspace_id: DataSpaceId,
    context: &str,
) -> Result<()> {
    ensure!(
        response.routed_by.as_deref() == Some("proxy"),
        "{context}: expected proxied read, observed {:?}",
        response.routed_by
    );
    ensure!(
        response.route_lane_id.as_deref() == Some(expected_lane_id.as_u32().to_string().as_str()),
        "{context}: expected routed lane {}, observed {:?}",
        expected_lane_id.as_u32(),
        response.route_lane_id
    );
    ensure!(
        response.route_dataspace_id.as_deref()
            == Some(expected_dataspace_id.as_u64().to_string().as_str()),
        "{context}: expected routed dataspace {}, observed {:?}",
        expected_dataspace_id.as_u64(),
        response.route_dataspace_id
    );
    Ok(())
}

fn expect_proxy_fanout_headers(response: &RoutedJsonResponse, context: &str) -> Result<()> {
    ensure!(
        response.routed_by.as_deref() == Some("proxy"),
        "{context}: expected proxied fanout read, observed {:?}",
        response.routed_by
    );
    ensure!(
        response.route_lane_id.is_none(),
        "{context}: fanout response should not expose singular route lane {:?}",
        response.route_lane_id
    );
    ensure!(
        response.route_dataspace_id.is_none(),
        "{context}: fanout response should not expose singular route dataspace {:?}",
        response.route_dataspace_id
    );
    Ok(())
}

fn pipeline_status_kind<'a>(body: &'a JsonValue, context: &'a str) -> Result<&'a str> {
    body.get("content")
        .and_then(|content| content.get("status"))
        .and_then(|status| status.get("kind"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("{context}: pipeline status response missing content.status.kind"))
}

async fn wait_for_pipeline_status_via_ingress(
    client: &Client,
    transaction: &SignedTransaction,
    expected_kind: &str,
    context: &str,
) -> Result<RoutedJsonResponse> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;

    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match torii_json_get(
            client,
            &[
                "v1".to_owned(),
                "pipeline".to_owned(),
                "transactions".to_owned(),
                "status".to_owned(),
            ],
            &[
                ("hash".to_owned(), transaction.hash().to_string()),
                ("scope".to_owned(), "auto".to_owned()),
            ],
        )
        .await
        {
            Ok(response) => {
                if response.status == HttpStatusCode::OK {
                    let kind = pipeline_status_kind(&response.body, context)?;
                    if kind == expected_kind {
                        return Ok(response);
                    }
                    last_error = Some(format!(
                        "observed pipeline status `{kind}` body `{}`",
                        response.body_text
                    ));
                } else {
                    last_error = Some(format!(
                        "unexpected pipeline status HTTP {} body `{}`",
                        response.status, response.body_text
                    ));
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        tokio::time::sleep(STATUS_POLL_INTERVAL).await;
    }

    let suffix = last_error
        .map(|err| format!("; last pipeline status error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for pipeline status `{expected_kind}`{suffix}"
    ))
}

async fn submit_transaction_and_expect_rejection_route(
    submitter: &Client,
    confirmation_client: &Client,
    transaction: &SignedTransaction,
    expected_lane_id: LaneId,
    expected_dataspace_id: DataSpaceId,
    rejection_contains: &str,
    context: &str,
) -> Result<TransactionSubmissionReceipt> {
    let response = submit_transaction_raw(submitter, transaction).await?;
    ensure!(
        response.status == HttpStatusCode::ACCEPTED,
        "{context}: expected 202 Accepted, observed {} body `{}`",
        response.status,
        response.body_text
    );
    ensure!(
        response.routed_by.as_deref() == Some("proxy"),
        "{context}: expected proxy routing, observed {:?}",
        response.routed_by
    );
    ensure!(
        response.route_lane_id.as_deref() == Some(expected_lane_id.as_u32().to_string().as_str()),
        "{context}: expected routed lane {}, observed {:?}",
        expected_lane_id.as_u32(),
        response.route_lane_id
    );
    ensure!(
        response.route_dataspace_id.as_deref()
            == Some(expected_dataspace_id.as_u64().to_string().as_str()),
        "{context}: expected routed dataspace {}, observed {:?}",
        expected_dataspace_id.as_u64(),
        response.route_dataspace_id
    );

    let receipt = response
        .receipt
        .ok_or_else(|| eyre!("{context}: missing transaction submission receipt"))?;
    let status =
        wait_for_pipeline_status_via_ingress(submitter, transaction, "Rejected", context).await?;
    ensure!(
        status.routed_by.as_deref() == Some("proxy"),
        "{context}: same-ingress status lookup should stay proxied, observed {:?}",
        status.routed_by
    );
    if let Some(lane_id) = status.route_lane_id.as_deref() {
        ensure!(
            lane_id == expected_lane_id.as_u32().to_string(),
            "{context}: pipeline status reported unexpected lane {lane_id}"
        );
    }
    if let Some(dataspace_id) = status.route_dataspace_id.as_deref() {
        ensure!(
            dataspace_id == expected_dataspace_id.as_u64().to_string(),
            "{context}: pipeline status reported unexpected dataspace {dataspace_id}"
        );
    }

    let entry_hash = transaction.hash_as_entrypoint();
    match wait_for_committed_tx_outcome(
        confirmation_client,
        entry_hash.clone(),
        context,
        COMMITTED_TX_OUTCOME_TIMEOUT,
    )? {
        CommittedTxOutcome::Applied => {
            return Err(eyre!(
                "{context}: transaction {entry_hash} applied unexpectedly"
            ));
        }
        CommittedTxOutcome::Rejected(reason) => {
            ensure!(
                reason.contains(rejection_contains),
                "{context}: expected rejection containing `{rejection_contains}`, observed `{reason}`"
            );
        }
    }

    Ok(receipt)
}

fn permission_response_contains(
    body: &JsonValue,
    permission_name: &str,
    payload_matches: impl Fn(&JsonValue) -> bool,
    context: &str,
) -> Result<bool> {
    let items = body
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: permission response missing items array"))?;
    Ok(items.iter().any(|item| {
        item.get("name").and_then(JsonValue::as_str) == Some(permission_name)
            && item.get("payload").is_some_and(&payload_matches)
    }))
}

async fn wait_for_permissions_api_state<F>(
    client: &Client,
    account_id: &AccountId,
    permission_name: &str,
    payload_matches: F,
    expected_present: bool,
    expected_route: Option<(LaneId, DataSpaceId)>,
    context: &str,
) -> Result<()>
where
    F: Fn(&JsonValue) -> bool,
{
    let started = Instant::now();
    let mut last_body = String::new();
    let mut last_error: Option<String> = None;

    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match torii_json_get_as_account(
            client,
            &client.account,
            &[
                "v1".to_owned(),
                "accounts".to_owned(),
                account_id.to_string(),
                "permissions".to_owned(),
            ],
            &[],
        )
        .await
        {
            Ok(response) => {
                last_body = response.body_text.clone();
                if response.status == HttpStatusCode::OK {
                    if let Some((lane_id, dataspace_id)) = expected_route {
                        expect_proxy_route_headers(&response, lane_id, dataspace_id, context)?;
                    } else {
                        expect_proxy_fanout_headers(&response, context)?;
                    }
                    let observed = permission_response_contains(
                        &response.body,
                        permission_name,
                        &payload_matches,
                        context,
                    )?;
                    if observed == expected_present {
                        return Ok(());
                    }
                    last_error = None;
                } else {
                    last_error = Some(format!(
                        "unexpected status {} body `{}`",
                        response.status, response.body_text
                    ));
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        tokio::time::sleep(STATUS_POLL_INTERVAL).await;
    }

    let suffix = last_error
        .map(|err| format!("; last permissions API error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for app API permission state on {account_id}; expected_present={expected_present}; last body `{last_body}`{suffix}"
    ))
}

fn manifest_response_contains_status(
    body: &JsonValue,
    dataspace_id: DataSpaceId,
    expected_status: &str,
    context: &str,
) -> Result<bool> {
    let manifests = body
        .get("manifests")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: manifest response missing manifests array"))?;
    Ok(manifests.iter().any(|record| {
        record.get("dataspace_id").and_then(JsonValue::as_u64) == Some(dataspace_id.as_u64())
            && record.get("status").and_then(JsonValue::as_str) == Some(expected_status)
    }))
}

async fn wait_for_manifest_api_status(
    client: &Client,
    uaid_literal: &str,
    dataspace_id: DataSpaceId,
    expected_status: &str,
    expected_lane_id: LaneId,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_body = String::new();
    let mut last_error: Option<String> = None;

    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match torii_json_get(
            client,
            &[
                "v1".to_owned(),
                "space-directory".to_owned(),
                "uaids".to_owned(),
                uaid_literal.to_owned(),
                "manifests".to_owned(),
            ],
            &[("dataspace".to_owned(), dataspace_id.as_u64().to_string())],
        )
        .await
        {
            Ok(response) => {
                last_body = response.body_text.clone();
                if response.status == HttpStatusCode::OK {
                    expect_proxy_route_headers(&response, expected_lane_id, dataspace_id, context)?;
                    if manifest_response_contains_status(
                        &response.body,
                        dataspace_id,
                        expected_status,
                        context,
                    )? {
                        return Ok(());
                    }
                    last_error = None;
                } else {
                    last_error = Some(format!(
                        "unexpected status {} body `{}`",
                        response.status, response.body_text
                    ));
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        tokio::time::sleep(STATUS_POLL_INTERVAL).await;
    }

    let suffix = last_error
        .map(|err| format!("; last manifests API error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for manifest status `{expected_status}` on UAID {uaid_literal}; last body `{last_body}`{suffix}"
    ))
}

fn account_assets_response_contains(
    body: &JsonValue,
    asset_definition_id: &AssetDefinitionId,
    context: &str,
) -> Result<bool> {
    let expected = asset_definition_id.to_string();
    let items = body
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: account assets response missing items array"))?;
    Ok(items
        .iter()
        .any(|item| item.get("asset").and_then(JsonValue::as_str) == Some(expected.as_str())))
}

#[test]
fn wrong_dataspace_ingress_routes_transactions_and_queries_across_permission_models() -> Result<()>
{
    let context = stringify!(
        wrong_dataspace_ingress_routes_transactions_and_queries_across_permission_models
    );
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(localnet_builder(), context)?
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

    let expected_nexus_validators: BTreeSet<_> = peers
        .iter()
        .enumerate()
        .take(VALIDATORS_PER_LANE)
        .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
        .collect();
    let expected_ds1_validators: BTreeSet<_> = peers
        .iter()
        .enumerate()
        .skip(VALIDATORS_PER_LANE)
        .take(VALIDATORS_PER_LANE)
        .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
        .collect();
    let expected_ds2_validators: BTreeSet<_> = peers
        .iter()
        .enumerate()
        .skip(VALIDATORS_PER_LANE * 2)
        .take(VALIDATORS_PER_LANE)
        .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
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
    wait_for_height(
        &bob,
        lane_sync_height,
        "lane validator activation propagation on bob",
    )?;

    ensure!(
        (VALIDATORS_PER_LANE * 2..TOTAL_PEERS).contains(&ALICE_WRONG_INGRESS_INDEX),
        "alice wrong-dataspace ingress index must point into the ds2 lane"
    );
    ensure!(
        (VALIDATORS_PER_LANE..VALIDATORS_PER_LANE * 2).contains(&BOB_WRONG_INGRESS_INDEX),
        "bob wrong-dataspace ingress index must point into the ds1 lane"
    );

    let alice_via_ds2 =
        peers[ALICE_WRONG_INGRESS_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let alice_via_ds1 =
        peers[BOB_WRONG_INGRESS_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let alice_on_ds1 =
        peers[BOB_WRONG_INGRESS_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let alice_on_ds2 =
        peers[ALICE_WRONG_INGRESS_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let bob_via_ds1 =
        peers[BOB_WRONG_INGRESS_INDEX].client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
    let bob_on_ds2 =
        peers[ALICE_WRONG_INGRESS_INDEX].client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
    wait_for_height(
        &alice_via_ds2,
        lane_sync_height,
        "lane validator activation propagation on alice ds2 ingress",
    )?;
    wait_for_height(
        &alice_on_ds1,
        lane_sync_height,
        "lane validator activation propagation on alice ds1 authoritative client",
    )?;
    wait_for_height(
        &alice_on_ds2,
        lane_sync_height,
        "lane validator activation propagation on alice ds2 authoritative client",
    )?;
    wait_for_height(
        &bob_via_ds1,
        lane_sync_height,
        "lane validator activation propagation on bob ds1 ingress",
    )?;
    wait_for_height(
        &bob_on_ds2,
        lane_sync_height,
        "lane validator activation propagation on bob ds2 authoritative client",
    )?;

    let ds1_lane_id = LaneId::new(DS1_LANE_INDEX);
    let ds2_lane_id = LaneId::new(DS2_LANE_INDEX);
    let ds1_dataspace_id = DataSpaceId::new(DS1_ID_U64);
    let ds2_dataspace_id = DataSpaceId::new(DS2_ID_U64);

    let ds1_asset_definition_id = AssetDefinitionId::new(
        "nexus".parse().expect("asset definition domain"),
        "ds1coin".parse().expect("asset definition name"),
    );
    let ds2_asset_definition_id = AssetDefinitionId::new(
        "nexus".parse().expect("asset definition domain"),
        "ds2coin".parse().expect("asset definition name"),
    );
    let alice_ds1_asset = AssetId::new(ds1_asset_definition_id.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_definition_id.clone(), BOB_ID.clone());

    let bob_modify_alice_metadata_permission: Permission = CanModifyAccountMetadata {
        account: ALICE_ID.clone(),
    }
    .into();
    let _alice_publish_ds1_manifest_permission: Permission = CanPublishSpaceDirectoryManifest {
        dataspace: ds1_dataspace_id,
    }
    .into();
    let bob_publish_ds2_manifest_permission: Permission = CanPublishSpaceDirectoryManifest {
        dataspace: ds2_dataspace_id,
    }
    .into();

    rt.block_on(async {
        let alice_probe = alice_via_ds2.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                "wrong ingress route probe ds1".to_owned(),
            ))],
            Metadata::default(),
        );
        submit_transaction_and_expect_route(
            &alice_via_ds2,
            &alice_on_ds1,
            &alice_probe,
            ds1_lane_id,
            ds1_dataspace_id,
            "alice tx via ds2 should route to ds1",
        )
        .await?;

        let bob_probe = bob_via_ds1.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                "wrong ingress route probe ds2".to_owned(),
            ))],
            Metadata::default(),
        );
        submit_transaction_and_expect_route(
            &bob_via_ds1,
            &bob_on_ds2,
            &bob_probe,
            ds2_lane_id,
            ds2_dataspace_id,
            "bob tx via ds1 should route to ds2",
        )
        .await?;

        Ok::<(), eyre::Report>(())
    })?;

    ensure!(
        asset_balance(&alice_via_ds2, &alice_ds1_asset)? == Numeric::from(100_u32),
        "alice signed query through ds2 ingress did not route to ds1"
    );
    ensure!(
        asset_balance(&bob_via_ds1, &bob_ds2_asset)? == Numeric::from(200_u32),
        "bob signed query through ds1 ingress did not route to ds2"
    );

    let alice_account = rt.block_on(torii_json_get(
        &alice_via_ds2,
        &["v1".to_owned(), "accounts".to_owned(), ALICE_ID.to_string()],
        &[],
    ))?;
    ensure!(
        alice_account.status == HttpStatusCode::OK,
        "alice account GET through ds2 ingress failed with {} body `{}`",
        alice_account.status,
        alice_account.body_text
    );
    expect_proxy_fanout_headers(
        &alice_account,
        "alice account GET through ds2 ingress should fan out globally",
    )?;
    ensure!(
        alice_account
            .body
            .get("account_id")
            .and_then(JsonValue::as_str)
            == Some(ALICE_ID.to_string().as_str()),
        "alice account GET through ds2 ingress did not return alice's canonical account id"
    );

    let alice_assets = rt.block_on(torii_json_get_as_account(
        &alice_via_ds2,
        &ALICE_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            ALICE_ID.to_string(),
            "assets".to_owned(),
        ],
        &[],
    ))?;
    ensure!(
        alice_assets.status == HttpStatusCode::OK,
        "alice assets query through ds2 ingress failed with {} body `{}`",
        alice_assets.status,
        alice_assets.body_text
    );
    expect_proxy_fanout_headers(&alice_assets, "alice assets query through ds2 ingress")?;
    ensure!(
        account_assets_response_contains(
            &alice_assets.body,
            &ds1_asset_definition_id,
            "alice assets query",
        )?,
        "alice assets query through ds2 ingress did not include ds1 asset definition"
    );

    let bob_assets = rt.block_on(torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            BOB_ID.to_string(),
            "assets".to_owned(),
        ],
        &[],
    ))?;
    ensure!(
        bob_assets.status == HttpStatusCode::OK,
        "bob assets query through ds1 ingress failed with {} body `{}`",
        bob_assets.status,
        bob_assets.body_text
    );
    expect_proxy_fanout_headers(&bob_assets, "bob assets query through ds1 ingress")?;
    ensure!(
        account_assets_response_contains(
            &bob_assets.body,
            &ds2_asset_definition_id,
            "bob assets query",
        )?,
        "bob assets query through ds1 ingress did not include ds2 asset definition"
    );

    let alice_assets_hidden_from_bob = rt.block_on(torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            ALICE_ID.to_string(),
            "assets".to_owned(),
        ],
        &[],
    ))?;
    ensure!(
        alice_assets_hidden_from_bob.status == HttpStatusCode::OK,
        "alice assets query as bob through ds1 ingress failed with {} body `{}`",
        alice_assets_hidden_from_bob.status,
        alice_assets_hidden_from_bob.body_text
    );
    expect_proxy_fanout_headers(
        &alice_assets_hidden_from_bob,
        "alice assets query as bob through ds1 ingress",
    )?;
    ensure!(
        !account_assets_response_contains(
            &alice_assets_hidden_from_bob.body,
            &ds1_asset_definition_id,
            "alice assets query as bob",
        )?,
        "alice private ds1 asset should be omitted when bob lacks ds1 visibility"
    );

    let bob_permissions_before = query_account_permissions(&bob_via_ds1, &BOB_ID)?;
    ensure!(
        !bob_permissions_before
            .iter()
            .any(|permission| permission == &bob_modify_alice_metadata_permission),
        "bob should not start with CanModifyAccountMetadata for alice"
    );

    let bob_permissions_api_before = rt.block_on(torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            BOB_ID.to_string(),
            "permissions".to_owned(),
        ],
        &[],
    ))?;
    ensure!(
        bob_permissions_api_before.status == HttpStatusCode::OK,
        "bob permissions query through ds1 ingress failed with {} body `{}`",
        bob_permissions_api_before.status,
        bob_permissions_api_before.body_text
    );
    expect_proxy_fanout_headers(
        &bob_permissions_api_before,
        "bob permissions query through ds1 ingress before grant",
    )?;
    ensure!(
        !permission_response_contains(
            &bob_permissions_api_before.body,
            "CanModifyAccountMetadata",
            |payload| {
                payload.get("account").and_then(JsonValue::as_str)
                    == Some(ALICE_ID.to_string().as_str())
            },
            "bob permissions app api before account metadata grant",
        )?,
        "bob app API permissions should not expose alice account metadata permission before grant"
    );
    let grant_account_metadata_permission_tx = alice_via_ds2.build_transaction(
        [InstructionBox::from(Grant::account_permission(
            CanModifyAccountMetadata {
                account: ALICE_ID.clone(),
            },
            BOB_ID.clone(),
        ))],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &alice_via_ds2,
        &alice_on_ds1,
        &grant_account_metadata_permission_tx,
        ds1_lane_id,
        ds1_dataspace_id,
        "alice grant account metadata permission to bob via ds2 ingress",
    ))?;
    wait_for_account_permissions(
        &bob_via_ds1,
        &BOB_ID,
        &[bob_modify_alice_metadata_permission.clone()],
        "bob account metadata permission propagation after routed grant",
    )?;
    rt.block_on(wait_for_permissions_api_state(
        &bob_via_ds1,
        &BOB_ID,
        "CanModifyAccountMetadata",
        |payload| {
            payload.get("account").and_then(JsonValue::as_str)
                == Some(ALICE_ID.to_string().as_str())
        },
        true,
        None,
        "bob permissions app api after account metadata grant",
    ))?;

    let revoke_account_metadata_permission_tx = alice_via_ds2.build_transaction(
        [InstructionBox::from(Revoke::account_permission(
            bob_modify_alice_metadata_permission.clone(),
            BOB_ID.clone(),
        ))],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &alice_via_ds2,
        &alice_on_ds1,
        &revoke_account_metadata_permission_tx,
        ds1_lane_id,
        ds1_dataspace_id,
        "alice revoke account metadata permission from bob via ds2 ingress",
    ))?;
    wait_for_account_permissions_absence(
        &bob_via_ds1,
        &BOB_ID,
        &[bob_modify_alice_metadata_permission.clone()],
        "bob account metadata permission propagation after routed revoke",
    )?;
    rt.block_on(wait_for_permissions_api_state(
        &bob_via_ds1,
        &BOB_ID,
        "CanModifyAccountMetadata",
        |payload| {
            payload.get("account").and_then(JsonValue::as_str)
                == Some(ALICE_ID.to_string().as_str())
        },
        false,
        None,
        "bob permissions app api after account metadata revoke",
    ))?;

    let manifest_uaid =
        UniversalAccountId::from_hash(Hash::new(b"wrong-ingress-ds2-manifest-routing"));
    let manifest_uaid_literal = manifest_uaid.to_string();
    let ds2_manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid: manifest_uaid,
        dataspace: ds2_dataspace_id,
        issued_ms: 1,
        activation_epoch: 1,
        expiry_epoch: None,
        entries: vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(ds2_dataspace_id),
                program: None,
                method: None,
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::from(1_u32)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("wrong ingress manifest routing regression".to_owned()),
        }],
    };

    let manifests_before = rt.block_on(torii_json_get(
        &bob_via_ds1,
        &[
            "v1".to_owned(),
            "space-directory".to_owned(),
            "uaids".to_owned(),
            manifest_uaid_literal.clone(),
            "manifests".to_owned(),
        ],
        &[(
            "dataspace".to_owned(),
            ds2_dataspace_id.as_u64().to_string(),
        )],
    ))?;
    ensure!(
        manifests_before.status == HttpStatusCode::OK,
        "initial manifests read through ds1 ingress failed with {} body `{}`",
        manifests_before.status,
        manifests_before.body_text
    );
    expect_proxy_route_headers(
        &manifests_before,
        ds2_lane_id,
        ds2_dataspace_id,
        "initial ds2 manifest read through ds1 ingress",
    )?;
    ensure!(
        !manifest_response_contains_status(
            &manifests_before.body,
            ds2_dataspace_id,
            "Active",
            "initial ds2 manifest read",
        )?,
        "manifest should not exist before publish"
    );

    let bob_manifest_permissions_api_before = rt.block_on(torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            BOB_ID.to_string(),
            "permissions".to_owned(),
        ],
        &[],
    ))?;
    ensure!(
        bob_manifest_permissions_api_before.status == HttpStatusCode::OK,
        "bob manifest permissions query through ds1 ingress failed with {} body `{}`",
        bob_manifest_permissions_api_before.status,
        bob_manifest_permissions_api_before.body_text
    );
    expect_proxy_fanout_headers(
        &bob_manifest_permissions_api_before,
        "bob manifest permissions query before grant",
    )?;
    ensure!(
        !permission_response_contains(
            &bob_manifest_permissions_api_before.body,
            "CanPublishSpaceDirectoryManifest",
            |payload| {
                payload.get("dataspace").and_then(JsonValue::as_u64)
                    == Some(ds2_dataspace_id.as_u64())
            },
            "bob manifest permissions app api before grant",
        )?,
        "bob should not expose ds2 manifest publish permission before grant"
    );

    let unauthorized_publish_tx = bob_via_ds1.build_transaction(
        [InstructionBox::from(PublishSpaceDirectoryManifest {
            manifest: ds2_manifest.clone(),
        })],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_rejection_route(
        &bob_via_ds1,
        &bob_on_ds2,
        &unauthorized_publish_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "CanPublishSpaceDirectoryManifest",
        "bob unauthorized manifest publish via ds1 ingress should reject on ds2",
    ))?;

    let grant_manifest_permission_tx = alice_via_ds1.build_transaction(
        [InstructionBox::from(Grant::account_permission(
            CanPublishSpaceDirectoryManifest {
                dataspace: ds2_dataspace_id,
            },
            BOB_ID.clone(),
        ))],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &alice_via_ds1,
        &alice_on_ds2,
        &grant_manifest_permission_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "alice grant ds2 manifest permission to bob via ds1 ingress",
    ))?;
    wait_for_account_permissions(
        &bob_via_ds1,
        &BOB_ID,
        &[bob_publish_ds2_manifest_permission.clone()],
        "bob manifest permission propagation after routed grant",
    )?;
    rt.block_on(wait_for_permissions_api_state(
        &bob_via_ds1,
        &BOB_ID,
        "CanPublishSpaceDirectoryManifest",
        |payload| {
            payload.get("dataspace").and_then(JsonValue::as_u64) == Some(ds2_dataspace_id.as_u64())
        },
        true,
        None,
        "bob permissions app api after manifest grant",
    ))?;

    let authorized_publish_tx = bob_via_ds1.build_transaction(
        [InstructionBox::from(PublishSpaceDirectoryManifest {
            manifest: ds2_manifest.clone(),
        })],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &bob_via_ds1,
        &bob_on_ds2,
        &authorized_publish_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "bob authorized manifest publish via ds1 ingress should route to ds2 and approve",
    ))?;
    rt.block_on(wait_for_manifest_api_status(
        &bob_via_ds1,
        &manifest_uaid_literal,
        ds2_dataspace_id,
        "Active",
        ds2_lane_id,
        "manifest becomes active through wrong ingress after approved publish",
    ))?;

    let bindings_after_publish = rt.block_on(torii_json_get(
        &bob_via_ds1,
        &[
            "v1".to_owned(),
            "space-directory".to_owned(),
            "uaids".to_owned(),
            manifest_uaid_literal.clone(),
        ],
        &[],
    ))?;
    ensure!(
        bindings_after_publish.status == HttpStatusCode::OK,
        "space-directory bindings fanout through ds1 ingress failed with {} body `{}`",
        bindings_after_publish.status,
        bindings_after_publish.body_text
    );
    expect_proxy_fanout_headers(
        &bindings_after_publish,
        "space-directory bindings fanout through ds1 ingress",
    )?;

    let revoke_manifest_tx = bob_via_ds1.build_transaction(
        [InstructionBox::from(RevokeSpaceDirectoryManifest {
            uaid: ds2_manifest.uaid,
            dataspace: ds2_manifest.dataspace,
            revoked_epoch: ds2_manifest.activation_epoch.saturating_add(1),
            reason: Some("route regression cleanup".to_owned()),
        })],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &bob_via_ds1,
        &bob_on_ds2,
        &revoke_manifest_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "bob manifest revoke via ds1 ingress should route to ds2 and approve",
    ))?;
    rt.block_on(wait_for_manifest_api_status(
        &bob_via_ds1,
        &manifest_uaid_literal,
        ds2_dataspace_id,
        "Revoked",
        ds2_lane_id,
        "manifest becomes revoked through wrong ingress after approved revoke",
    ))?;

    let revoke_manifest_permission_tx = alice_via_ds1.build_transaction(
        [InstructionBox::from(Revoke::account_permission(
            bob_publish_ds2_manifest_permission.clone(),
            BOB_ID.clone(),
        ))],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_route(
        &alice_via_ds1,
        &alice_on_ds2,
        &revoke_manifest_permission_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "alice revoke ds2 manifest permission from bob via ds1 ingress",
    ))?;
    wait_for_account_permissions_absence(
        &bob_via_ds1,
        &BOB_ID,
        &[bob_publish_ds2_manifest_permission.clone()],
        "bob manifest permission propagation after routed revoke",
    )?;
    rt.block_on(wait_for_permissions_api_state(
        &bob_via_ds1,
        &BOB_ID,
        "CanPublishSpaceDirectoryManifest",
        |payload| {
            payload.get("dataspace").and_then(JsonValue::as_u64) == Some(ds2_dataspace_id.as_u64())
        },
        false,
        None,
        "bob permissions app api after manifest permission revoke",
    ))?;

    let publish_after_revoke_tx = bob_via_ds1.build_transaction(
        [InstructionBox::from(PublishSpaceDirectoryManifest {
            manifest: AssetPermissionManifest {
                issued_ms: ds2_manifest.issued_ms.saturating_add(1),
                ..ds2_manifest.clone()
            },
        })],
        Metadata::default(),
    );
    rt.block_on(submit_transaction_and_expect_rejection_route(
        &bob_via_ds1,
        &bob_on_ds2,
        &publish_after_revoke_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "CanPublishSpaceDirectoryManifest",
        "bob manifest publish after permission revoke should reject via ds1 ingress",
    ))?;

    Ok(())
}
