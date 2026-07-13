#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native AMX multidataspace routing integration coverage.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::nexus;
use iroha::{
    client::Client,
    crypto::{Hash, HashOf},
    data_model::{
        Level,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::{
            ExternalExecutionRouteLeg, ExternalExecutionRouteRole, Header, SignedBlock,
            consensus::{
                LaneBlockCommitment, NativeAmxAttestationQcV2, NativeAmxPhase, NativeAmxReceipt,
            },
            consensus_v2::SumeragiV2StatusResponse,
        },
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            InstructionBox, Log, Mint, Register,
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneRelayEnvelope,
            LaneVisibility, compute_settlement_hash,
        },
        peer::PeerId,
        prelude::Numeric,
        query::block::prelude::FindBlocks,
        transaction::{SignedTransaction, TransactionEntrypoint},
    },
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey};
use iroha_data_model::prelude::QueryBuilderExt;
use iroha_test_network::{
    NetworkBuilder, ensure_domain_registration_lease_for_network,
    genesis_factory_with_post_topology, init_instruction_registry,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};

const PEERS: usize = 4;
const UNIVERSAL_LANE: u32 = 0;
const ACME_LANE: u32 = 1;
const BANK_LANE: u32 = 2;
const UNIVERSAL_DATASPACE: u64 = 0;
const ACME_DATASPACE: u64 = 1;
const BANK_DATASPACE: u64 = 2;
const VALIDATOR_STAKE: u32 = 2_000;
const VALIDATOR_FEE_SEED: u32 = 1_000_000;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(90);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PIPELINE_TIME: Duration = Duration::from_secs(2);
const NATIVE_AMX_FAULT_SOAK_ITERATIONS: usize = 10;
const NATIVE_AMX_FAULT_SOAK_MAX_ITERATIONS: usize = 100;
const NATIVE_AMX_FAULT_SOAK_ITERATIONS_ENV: &str = "IROHA_NATIVE_AMX_SOAK_ITERATIONS";

fn parse_native_amx_fault_soak_iterations(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| (1..=NATIVE_AMX_FAULT_SOAK_MAX_ITERATIONS).contains(value))
        .unwrap_or(NATIVE_AMX_FAULT_SOAK_ITERATIONS)
}

fn native_amx_fault_soak_iterations() -> usize {
    parse_native_amx_fault_soak_iterations(
        std::env::var(NATIVE_AMX_FAULT_SOAK_ITERATIONS_ENV)
            .ok()
            .as_deref(),
    )
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

fn validator_account(index: usize) -> AccountId {
    let mut seed = b"integration_tests::native_amx_routing::validator".to_vec();
    seed.extend_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    let key_pair =
        KeyPair::try_from_seed(seed, Algorithm::Ed25519).expect("fixture Native AMX validator key");
    AccountId::new(key_pair.public_key().clone())
}

fn gas_account() -> AccountId {
    let key_pair = KeyPair::try_from_seed(
        b"integration_tests::native_amx_routing::gas".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture Native AMX gas key");
    AccountId::new(key_pair.public_key().clone())
}

#[test]
fn native_amx_account_fixtures_use_checked_seed_derivation() {
    let mut validator_seed = b"integration_tests::native_amx_routing::validator".to_vec();
    validator_seed.extend_from_slice(&0_u64.to_le_bytes());
    let expected_validator = KeyPair::try_from_seed(validator_seed, Algorithm::Ed25519)
        .expect("fixture Native AMX validator key");
    assert_eq!(
        validator_account(0),
        AccountId::new(expected_validator.public_key().clone()),
    );

    let expected_gas = KeyPair::try_from_seed(
        b"integration_tests::native_amx_routing::gas".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture Native AMX gas key");
    assert_eq!(
        gas_account(),
        AccountId::new(expected_gas.public_key().clone())
    );
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
}

fn da_proof_policy_bundle() -> DaProofPolicyBundle {
    let lane_count = NonZeroU32::new(3).expect("lane count");
    let lanes = vec![
        ModelLaneConfig {
            id: LaneId::new(UNIVERSAL_LANE),
            dataspace_id: DataSpaceId::new(UNIVERSAL_DATASPACE),
            alias: "lane-universal".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(ACME_LANE),
            dataspace_id: DataSpaceId::new(ACME_DATASPACE),
            alias: "lane-acme".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(BANK_LANE),
            dataspace_id: DataSpaceId::new(BANK_DATASPACE),
            alias: "lane-bank".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
    ];
    let catalog = LaneCatalog::new(lane_count, lanes).expect("lane catalog");
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    proof_policy_bundle(&lane_config)
}

fn genesis_post_topology_transactions(topology: &[PeerId]) -> Vec<Vec<InstructionBox>> {
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = fee_asset_definition_id();
    let gas_account_id = gas_account();
    let lane_ids = [
        LaneId::new(UNIVERSAL_LANE),
        LaneId::new(ACME_LANE),
        LaneId::new(BANK_LANE),
    ];
    let stake_per_validator =
        VALIDATOR_STAKE.saturating_mul(u32::try_from(lane_ids.len()).expect("lane count fits"));

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
        Register::asset_definition({
            let asset_definition_id = stake_asset_id.clone();
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let asset_definition_id = fee_asset_id.clone();
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string())
        })
        .into(),
        Mint::asset_quantity(
            VALIDATOR_FEE_SEED,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            VALIDATOR_FEE_SEED,
            AssetId::new(fee_asset_id.clone(), gas_account_id),
        )
        .into(),
    ];
    let mut validator_tx = Vec::with_capacity(topology.len() * lane_ids.len() * 2);
    for (index, peer_id) in topology.iter().enumerate() {
        let validator_id = validator_account(index);
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_quantity(
                stake_per_validator,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
                VALIDATOR_FEE_SEED,
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
                    Numeric::from(VALIDATOR_STAKE),
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

fn lane_descriptor(index: u32, alias: &str, dataspace: &str) -> Table {
    let mut lane = Table::new();
    lane.insert("index".into(), TomlValue::Integer(i64::from(index)));
    lane.insert("alias".into(), TomlValue::String(alias.to_owned()));
    lane.insert("dataspace".into(), TomlValue::String(dataspace.to_owned()));
    lane.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane.insert("metadata".into(), TomlValue::Table(Table::new()));
    lane
}

fn dataspace_descriptor(alias: &str, id: u64) -> Table {
    let mut dataspace = Table::new();
    dataspace.insert("alias".into(), TomlValue::String(alias.to_owned()));
    dataspace.insert(
        "id".into(),
        TomlValue::Integer(i64::try_from(id).expect("dataspace id fits i64")),
    );
    if id != DataSpaceId::UNIVERSAL.as_u64() {
        let mut bytes = [0_u8; 32];
        bytes[..8].copy_from_slice(&id.to_le_bytes());
        let manifest_hash = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        dataspace.insert("manifest_hash".into(), TomlValue::String(manifest_hash));
    }
    dataspace.insert(
        "description".into(),
        TomlValue::String(format!("{alias} dataspace")),
    );
    dataspace.insert("fault_tolerance".into(), TomlValue::Integer(1));
    dataspace
}

fn routing_policy() -> Table {
    let mut policy = Table::new();
    policy.insert("default_lane".into(), TomlValue::Integer(0));
    policy.insert(
        "default_dataspace".into(),
        TomlValue::String("universal".to_owned()),
    );
    policy.insert("rules".into(), TomlValue::Array(Vec::new()));
    policy
}

fn localnet_builder() -> NetworkBuilder {
    let gas_account_literal = gas_account()
        .canonical_i105()
        .expect("canonical gas account literal");
    let stake_asset_literal = stake_asset_definition_id().to_string();
    let fee_asset_literal = fee_asset_definition_id().to_string();

    NetworkBuilder::new()
        .with_peers(PEERS)
        .with_auto_populated_trusted_peers()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let mut genesis = genesis_factory_with_post_topology(
                Vec::new(),
                genesis_post_topology_transactions(topology.as_ref()),
                topology,
                topology_entries,
            );
            genesis
                .0
                .set_da_proof_policies(Some(da_proof_policy_bundle()));
            genesis
        })
        .with_pipeline_time(PIPELINE_TIME)
        .with_npos_consensus()
        .with_config_layer(move |layer| {
            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 3_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(lane_descriptor(
                            UNIVERSAL_LANE,
                            "lane-universal",
                            "universal",
                        )),
                        TomlValue::Table(lane_descriptor(ACME_LANE, "lane-acme", "acme")),
                        TomlValue::Table(lane_descriptor(BANK_LANE, "lane-bank", "bank")),
                    ]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(dataspace_descriptor("universal", UNIVERSAL_DATASPACE)),
                        TomlValue::Table(dataspace_descriptor("acme", ACME_DATASPACE)),
                        TomlValue::Table(dataspace_descriptor("bank", BANK_DATASPACE)),
                    ]),
                )
                .write(
                    ["nexus", "routing_policy"],
                    TomlValue::Table(routing_policy()),
                )
                .write(["nexus", "fees", "fee_asset_id"], fee_asset_literal.clone())
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(["nexus", "staking", "max_validators"], PEERS as i64)
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "npos", "election", "max_validators"],
                    PEERS as i64,
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

async fn submit_and_wait_for_approval(
    submitter: &Client,
    transaction: SignedTransaction,
) -> Result<Option<(LaneId, DataSpaceId)>> {
    let tx_hash = transaction.hash();
    let mut events = timeout(
        STATUS_WAIT_TIMEOUT,
        submitter.listen_for_events_async([TransactionEventFilter::default().for_hash(tx_hash)]),
    )
    .await
    .map_err(|_| eyre!("timed out opening transaction event stream"))??;

    let submitter_for_submit = submitter.clone();
    let transaction_for_submit = transaction.clone();
    spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction_for_submit))
        .await
        .map_err(|err| eyre!("submit task join error: {err}"))?
        .map_err(|err| eyre!("failed to submit native AMX transaction: {err}"))?;

    let outcome = match timeout(STATUS_WAIT_TIMEOUT, async {
        while let Some(next) = events.next().await {
            let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = next? else {
                continue;
            };
            match event.status() {
                TransactionStatus::Approved => {
                    return Ok(Some((event.lane_id(), event.dataspace_id())));
                }
                TransactionStatus::Rejected(reason) => {
                    return Err(eyre!("native AMX transaction rejected: {reason:?}"));
                }
                TransactionStatus::Expired => {
                    return Err(eyre!("native AMX transaction expired"));
                }
                TransactionStatus::Queued => {}
            }
        }
        Ok(None)
    })
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            events.close().await;
            return Ok(None);
        }
    };

    events.close().await;
    Ok(outcome)
}

async fn wait_for_block_with_entrypoint(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    context: &str,
) -> Result<SignedBlock> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.query(FindBlocks).execute_all() {
            Ok(blocks) => {
                if let Some(block) = blocks.into_iter().find(|block| {
                    block
                        .entrypoint_hashes()
                        .any(|hash| hash == entrypoint_hash)
                }) {
                    return Ok(block);
                }
            }
            Err(err) => last_error = Some(err.to_string()),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for committed entrypoint {entrypoint_hash}{suffix}"
    ))
}

fn native_amx_qc_signer_count(qc: &NativeAmxAttestationQcV2) -> usize {
    qc.signers_bitmap
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum()
}

fn assert_native_amx_qc_is_independently_verifiable(
    qc: &NativeAmxAttestationQcV2,
    phase: NativeAmxPhase,
    receipt: &NativeAmxReceipt,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
) -> Result<()> {
    let body = &qc.body;
    ensure!(body.phase == phase, "participant QC carried wrong phase");
    ensure!(
        body.source_id == receipt.source_id
            && body.chain_id_hash == receipt.chain_id_hash
            && body.tx_entrypoint_hash == entrypoint_hash
            && body.plan_digest == receipt.plan_digest,
        "participant QC changed the receipt source, chain, entrypoint, or plan binding"
    );
    ensure!(
        body.coordinator_lane_id == receipt.lane_id
            && body.coordinator_dataspace_id == receipt.dataspace_id
            && body.coordinator_lane_incarnation == receipt.lane_incarnation
            && body.authority_context_height == receipt.authority_context_height
            && body.planned_coordinator_block_height == receipt.lane_block_height
            && body.coordinator_lane_block_view == receipt.lane_block_view
            && body.coordinator_proposal_hash == receipt.coordinator_proposal_hash,
        "participant QC changed the coordinator finality binding"
    );
    ensure!(
        body.round.height == receipt.authority_context_height,
        "participant QC round height differs from the receipt authority context"
    );
    ensure!(
        body.participant_lane_id == participant_lane_id
            && body.participant_dataspace_id == participant_dataspace_id,
        "participant QC is not bound to its own participant lane/dataspace"
    );
    ensure!(
        body.participant_lane_incarnation
            .as_ref()
            .iter()
            .any(|byte| *byte != 0),
        "participant QC used a zero lane incarnation"
    );

    let min_quorum = usize::try_from(body.participant_min_quorum)
        .map_err(|_| eyre!("participant minimum quorum does not fit usize"))?;
    ensure!(
        native_amx_qc_signer_count(qc) >= min_quorum,
        "participant QC signer bitmap does not meet its advertised quorum"
    );
    ensure!(
        qc.validator_set.len() == qc.validator_set_pops.len(),
        "participant QC validator and proof-of-possession vectors are misaligned"
    );
    let pops = qc
        .validator_set
        .iter()
        .zip(&qc.validator_set_pops)
        .map(|(validator, pop)| (validator.public_key().clone(), pop.clone()))
        .collect::<BTreeMap<PublicKey, Vec<u8>>>();
    ensure!(
        pops.len() == qc.validator_set.len(),
        "participant QC validator roster contains duplicate public keys"
    );
    iroha_core::native_amx::validate_native_amx_qc(qc, body, &qc.validator_set, min_quorum, &pops)
        .wrap_err("participant QC failed independent aggregate-signature verification")?;
    Ok(())
}

fn native_amx_qc_signer_bit(qc: &NativeAmxAttestationQcV2, validator: &PeerId) -> Result<bool> {
    let validator_index = qc
        .validator_set
        .iter()
        .position(|candidate| candidate == validator)
        .ok_or_else(|| eyre!("faulted validator is missing from the frozen participant roster"))?;
    let byte = qc
        .signers_bitmap
        .get(validator_index / 8)
        .ok_or_else(|| eyre!("participant QC signer bitmap is shorter than its roster"))?;
    Ok(byte & (1_u8 << (validator_index % 8)) != 0)
}

fn assert_faulted_validator_excluded_from_native_amx_qcs(
    receipt: &NativeAmxReceipt,
    faulted_validator: &PeerId,
) -> Result<()> {
    for leg in &receipt.legs {
        for (phase, qc) in [("prepare", &leg.prepare_qc), ("commit", &leg.commit_qc)] {
            ensure!(
                qc.validator_set.len() == PEERS,
                "{phase} QC replaced the frozen four-validator participant roster after a fault"
            );
            ensure!(
                !native_amx_qc_signer_bit(qc, faulted_validator)?,
                "{phase} QC claims a signature from the validator held offline for the full AMX round"
            );
            ensure!(
                native_amx_qc_signer_count(qc) == PEERS - 1,
                "{phase} QC should contain exactly the three live validator signatures"
            );
        }
    }
    Ok(())
}

fn assert_native_amx_execution_context(
    block: &SignedBlock,
    transaction: &SignedTransaction,
) -> Result<NativeAmxReceipt> {
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let context_bundle = block
        .execution_context()
        .ok_or_else(|| eyre!("native AMX block is missing durable execution context"))?;
    let context = context_bundle
        .external
        .iter()
        .find(|context| context.entrypoint_hash == entrypoint_hash)
        .ok_or_else(|| eyre!("native AMX block missing execution context for submitted tx"))?;

    ensure!(
        context.lane_id == LaneId::new(ACME_LANE)
            && context.dataspace_id == DataSpaceId::new(ACME_DATASPACE),
        "expected ACME coordinator route, got lane {} dataspace {}",
        context.lane_id.as_u32(),
        context.dataspace_id.as_u64()
    );
    ensure!(
        context.routing_plan_legs
            == vec![
                ExternalExecutionRouteLeg::new(
                    LaneId::new(ACME_LANE),
                    DataSpaceId::new(ACME_DATASPACE),
                    ExternalExecutionRouteRole::Coordinator,
                ),
                ExternalExecutionRouteLeg::new(
                    LaneId::new(ACME_LANE),
                    DataSpaceId::new(ACME_DATASPACE),
                    ExternalExecutionRouteRole::Participant,
                ),
                ExternalExecutionRouteLeg::new(
                    LaneId::new(BANK_LANE),
                    DataSpaceId::new(BANK_DATASPACE),
                    ExternalExecutionRouteRole::Participant,
                ),
            ],
        "native AMX execution context did not preserve coordinator-first full plan legs: {:?}",
        context.routing_plan_legs
    );

    let receipt = context
        .native_amx_receipt
        .as_ref()
        .ok_or_else(|| eyre!("native AMX execution context is missing receipt"))?;
    ensure!(
        receipt.plan_digest == context.routing_plan_digest,
        "native AMX receipt plan digest differs from execution context"
    );
    ensure!(
        receipt.authority_context_height == block.header().height().get(),
        "native AMX receipt authority context height differs from containing block"
    );
    ensure!(
        receipt.lane_block_height > 0,
        "native AMX receipt lane-local height must be positive"
    );
    let mut expected_source_id = [0_u8; Hash::LENGTH];
    expected_source_id.copy_from_slice(transaction.hash().as_ref());
    ensure!(
        receipt.source_id == expected_source_id,
        "native AMX receipt source transaction hash mismatch"
    );
    ensure!(
        receipt.legs.len() == 2,
        "expected AMX receipt for two participant legs, got {}",
        receipt.legs.len()
    );

    let expected_legs = [
        (LaneId::new(ACME_LANE), DataSpaceId::new(ACME_DATASPACE)),
        (LaneId::new(BANK_LANE), DataSpaceId::new(BANK_DATASPACE)),
    ];
    let mut participant_bindings = BTreeSet::new();
    let mut prepare_signatures = BTreeSet::new();
    let mut commit_signatures = BTreeSet::new();
    for (expected_lane, expected_dataspace) in expected_legs {
        let leg = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == expected_lane && leg.dataspace_id == expected_dataspace)
            .ok_or_else(|| {
                eyre!(
                    "missing native AMX receipt leg lane {} dataspace {}",
                    expected_lane.as_u32(),
                    expected_dataspace.as_u64()
                )
            })?;
        ensure!(
            participant_bindings.insert((
                leg.prepare_qc.body.participant_lane_id,
                leg.prepare_qc.body.participant_dataspace_id,
                leg.prepare_qc.body.participant_lane_incarnation,
            )),
            "native AMX receipt duplicated a participant-lane finality binding"
        );
        ensure!(
            prepare_signatures.insert(leg.prepare_qc.bls_aggregate_signature.clone()),
            "distinct participant lanes reused the same prepare aggregate signature"
        );
        ensure!(
            commit_signatures.insert(leg.commit_qc.bls_aggregate_signature.clone()),
            "distinct participant lanes reused the same commit aggregate signature"
        );
        ensure!(
            leg.prepare_qc.validator_set.len() == PEERS
                && leg.commit_qc.validator_set.len() == PEERS,
            "participant QCs should carry the 4-peer validator set"
        );
        let mut expected_commit_body = leg.prepare_qc.body;
        expected_commit_body.phase = NativeAmxPhase::Commit;
        ensure!(
            leg.commit_qc.body == expected_commit_body,
            "participant prepare and commit QCs differ by more than phase"
        );
        ensure!(
            leg.prepare_qc.validator_set == leg.commit_qc.validator_set
                && leg.prepare_qc.validator_set_pops == leg.commit_qc.validator_set_pops,
            "participant prepare and commit QCs changed the frozen validator authority"
        );
        assert_native_amx_qc_is_independently_verifiable(
            &leg.prepare_qc,
            NativeAmxPhase::Prepare,
            receipt,
            entrypoint_hash,
            expected_lane,
            expected_dataspace,
        )?;
        assert_native_amx_qc_is_independently_verifiable(
            &leg.commit_qc,
            NativeAmxPhase::Commit,
            receipt,
            entrypoint_hash,
            expected_lane,
            expected_dataspace,
        )?;
    }
    ensure!(
        participant_bindings.len() == receipt.legs.len()
            && prepare_signatures.len() == receipt.legs.len()
            && commit_signatures.len() == receipt.legs.len(),
        "native AMX participant legs did not carry independent lane-bound attestation certificates"
    );
    Ok(receipt.clone())
}

async fn wait_for_all_peers_to_observe_block(
    network: &sandbox::SerializedNetwork,
    transaction: &SignedTransaction,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    expected_block_hash: HashOf<Header>,
    expected_receipt: &NativeAmxReceipt,
) -> Result<()> {
    for (index, peer) in network.peers().iter().enumerate() {
        let peer_block = wait_for_block_with_entrypoint(
            &peer.client(),
            entrypoint_hash,
            &format!("peer {index} convergence"),
        )
        .await?;
        ensure!(
            peer_block.hash() == expected_block_hash,
            "peer {index} committed a different block for native AMX tx"
        );
        let peer_receipt = assert_native_amx_execution_context(&peer_block, transaction)?;
        ensure!(
            peer_receipt == *expected_receipt,
            "peer {index} committed different native AMX receipt identity/QCs/legs"
        );
    }
    Ok(())
}

fn audit_native_amx_relay(
    relay: &LaneRelayEnvelope,
    expected_commitment: &LaneBlockCommitment,
    expected_receipt: &NativeAmxReceipt,
) -> Result<()> {
    relay
        .verify()
        .wrap_err("downstream lane relay verification rejected envelope")?;
    nexus::verify_lane_relay_envelopes(std::slice::from_ref(relay))
        .wrap_err("downstream lane relay audit rejected envelope")?;
    ensure!(
        relay.settlement_commitment == *expected_commitment,
        "relay settlement commitment differs from the finalized lane commitment"
    );
    ensure!(
        relay
            .settlement_commitment
            .native_amx_receipts
            .iter()
            .filter(|receipt| receipt.source_id == expected_receipt.source_id)
            .count()
            == 1,
        "relay must contain exactly one receipt for the finalized native AMX source"
    );
    ensure!(
        relay
            .settlement_commitment
            .native_amx_receipts
            .iter()
            .any(|receipt| receipt == expected_receipt),
        "relay changed native AMX receipt identity, phases, QCs, legs, bitmap, or signature"
    );
    Ok(())
}

fn assert_native_amx_relay_tamper_rejected<F>(
    label: &str,
    baseline: &LaneRelayEnvelope,
    expected_commitment: &LaneBlockCommitment,
    expected_receipt: &NativeAmxReceipt,
    mutate: F,
) -> Result<()>
where
    F: FnOnce(&mut NativeAmxReceipt),
{
    let mut tampered = baseline.clone();
    let receipt = tampered
        .settlement_commitment
        .native_amx_receipts
        .iter_mut()
        .find(|receipt| receipt.source_id == expected_receipt.source_id)
        .ok_or_else(|| eyre!("{label}: baseline relay omitted expected receipt"))?;
    mutate(receipt);
    tampered.settlement_hash = compute_settlement_hash(&tampered.settlement_commitment)?;
    ensure!(
        audit_native_amx_relay(&tampered, expected_commitment, expected_receipt).is_err(),
        "{label}: downstream audit accepted a recomputed tampered relay"
    );
    Ok(())
}

fn assert_native_amx_relay_tamper_matrix(
    relay: &LaneRelayEnvelope,
    expected_receipt: &NativeAmxReceipt,
) -> Result<()> {
    ensure!(
        expected_receipt.legs.len() >= 2
            && expected_receipt.legs.iter().all(|leg| {
                !leg.prepare_qc.signers_bitmap.is_empty()
                    && !leg.prepare_qc.bls_aggregate_signature.is_empty()
            }),
        "tamper matrix requires two participant receipts with non-empty QC evidence"
    );
    let expected_commitment = &relay.settlement_commitment;
    assert_native_amx_relay_tamper_rejected(
        "source identity tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.source_id[0] ^= 0x01,
    )?;
    assert_native_amx_relay_tamper_rejected(
        "plan digest tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.plan_digest = Hash::new(b"tampered native AMX plan"),
    )?;
    assert_native_amx_relay_tamper_rejected(
        "authority context height tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.authority_context_height = receipt.authority_context_height.saturating_add(1);
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "coordinator lane tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.lane_id = LaneId::new(receipt.lane_id.as_u32().saturating_add(1)),
    )?;
    assert_native_amx_relay_tamper_rejected(
        "coordinator dataspace tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.dataspace_id =
                DataSpaceId::new(receipt.dataspace_id.as_u64().saturating_add(1));
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "participant leg tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[0].lane_id =
                LaneId::new(receipt.legs[0].lane_id.as_u32().saturating_add(1));
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "participant lane incarnation tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[0].prepare_qc.body.participant_lane_incarnation =
                Hash::new(b"tampered participant incarnation");
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "cross-lane QC substitution",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[1].prepare_qc = receipt.legs[0].prepare_qc.clone();
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC phase tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.legs[0].prepare_qc.body.phase = NativeAmxPhase::Commit,
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC plan digest tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[0].prepare_qc.body.plan_digest = Hash::new(b"tampered native AMX QC plan");
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC entrypoint hash tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[0].prepare_qc.body.tx_entrypoint_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"tampered native AMX entrypoint"));
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC validator-set digest tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| {
            receipt.legs[0].prepare_qc.validator_set_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"tampered native AMX validators"));
        },
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC bitmap tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.legs[0].prepare_qc.signers_bitmap[0] ^= 0x01,
    )?;
    assert_native_amx_relay_tamper_rejected(
        "QC signature tamper",
        relay,
        expected_commitment,
        expected_receipt,
        |receipt| receipt.legs[0].prepare_qc.bls_aggregate_signature[0] ^= 0x01,
    )?;
    Ok(())
}

async fn wait_for_status_native_amx_evidence(
    client: &Client,
    receipt: &NativeAmxReceipt,
    context: &str,
) -> Result<(LaneBlockCommitment, LaneRelayEnvelope)> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let client = client.clone();
        match spawn_blocking(move || client.get_sumeragi_v2_status()).await {
            Ok(Ok(status)) => {
                let commitment = status
                    .lane_settlement_commitments
                    .iter()
                    .find(|commitment| {
                        commitment
                            .native_amx_receipts
                            .iter()
                            .any(|candidate| candidate == receipt)
                    })
                    .cloned();
                let relay = status
                    .lane_relay_envelopes
                    .iter()
                    .find(|relay| {
                        relay
                            .settlement_commitment
                            .native_amx_receipts
                            .iter()
                            .any(|candidate| candidate == receipt)
                    })
                    .cloned();
                if let (Some(commitment), Some(relay)) = (commitment, relay) {
                    audit_native_amx_relay(&relay, &commitment, receipt)?;
                    return Ok((commitment, relay));
                }
                last_error =
                    Some("typed status omitted the exact commitment or relay receipt".to_owned());
            }
            Ok(Err(err)) => last_error = Some(err.to_string()),
            Err(err) => last_error = Some(format!("status task join error: {err}")),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last status error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for exact native AMX commitment and relay evidence{suffix}"
    ))
}

async fn wait_for_all_peers_to_observe_native_amx_evidence(
    network: &sandbox::SerializedNetwork,
    transaction: &SignedTransaction,
    expected_block_hash: HashOf<Header>,
    expected_receipt: &NativeAmxReceipt,
    context: &str,
) -> Result<LaneRelayEnvelope> {
    wait_for_all_peers_to_observe_block(
        network,
        transaction,
        transaction.hash_as_entrypoint(),
        expected_block_hash,
        expected_receipt,
    )
    .await?;

    let mut canonical: Option<(LaneBlockCommitment, LaneRelayEnvelope)> = None;
    for (index, peer) in network.peers().iter().enumerate() {
        let observed = wait_for_status_native_amx_evidence(
            &peer.client(),
            expected_receipt,
            &format!("{context}: peer {index} status"),
        )
        .await?;
        if let Some((canonical_commitment, canonical_relay)) = canonical.as_ref() {
            ensure!(
                observed.0 == *canonical_commitment,
                "peer {index} exposed a different settlement commitment or native AMX QCs"
            );
            ensure!(
                observed.1.settlement_commitment == canonical_relay.settlement_commitment,
                "peer {index} relay changed the exact native AMX settlement evidence"
            );
        } else {
            canonical = Some(observed);
        }
    }
    canonical
        .map(|(_, relay)| relay)
        .ok_or_else(|| eyre!("{context}: four-peer network returned no native AMX relay"))
}

async fn fetch_sumeragi_v2_status(client: &Client) -> Result<SumeragiV2StatusResponse> {
    let client = client.clone();
    spawn_blocking(move || client.get_sumeragi_v2_status())
        .await
        .wrap_err("join authoritative Sumeragi v2 status request")?
}

fn status_contains_native_amx_receipt(
    status: &SumeragiV2StatusResponse,
    receipt: &NativeAmxReceipt,
) -> bool {
    status.lane_settlement_commitments.iter().any(|commitment| {
        commitment.block_height == receipt.authority_context_height
            && commitment.lane_id == receipt.lane_id
            && commitment.dataspace_id == receipt.dataspace_id
            && commitment
                .native_amx_receipts
                .iter()
                .any(|candidate| candidate == receipt)
    })
}

fn native_amx_status_summary(status: &SumeragiV2StatusResponse) -> String {
    let native_total = status
        .lane_settlement_commitments
        .iter()
        .map(|commitment| commitment.native_amx_receipts.len())
        .sum::<usize>();
    let first = status
        .lane_settlement_commitments
        .first()
        .map(|commitment| {
            let native_summary = commitment
                .native_amx_receipts
                .first()
                .map(|receipt| {
                    format!(
                        " native_source={:?} native_plan={:?} authority_height={:?} lane_height={:?} legs={}",
                        receipt.source_id,
                        receipt.plan_digest,
                        receipt.authority_context_height,
                        receipt.lane_block_height,
                        receipt.legs.len(),
                    )
                })
                .unwrap_or_else(|| " native_receipts_empty".to_owned());
            format!(
                " first_block={:?} first_lane={:?} first_dataspace={:?}{native_summary}",
                commitment.block_height, commitment.lane_id, commitment.dataspace_id,
            )
        })
        .unwrap_or_else(|| " no_first_commitment".to_owned());
    format!(
        "commitments={} native_receipts_total={}{}",
        status.lane_settlement_commitments.len(),
        native_total,
        first
    )
}

async fn wait_for_status_native_amx_receipt(
    client: &Client,
    receipt: &NativeAmxReceipt,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match fetch_sumeragi_v2_status(client).await {
            Ok(status) if status_contains_native_amx_receipt(&status, receipt) => return Ok(()),
            Ok(status) => {
                last_error = Some(format!(
                    "status did not include expected native AMX receipt ({})",
                    native_amx_status_summary(&status)
                ));
            }
            Err(err) => last_error = Some(err.to_string()),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last status error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for native AMX receipt in Sumeragi status{suffix}"
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_dataspace_native_amx_routes_and_commits_with_receipts() -> Result<()> {
    init_instruction_registry();
    let context = stringify!(mixed_dataspace_native_amx_routes_and_commits_with_receipts);
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let submit_peer = network
            .peers()
            .get(PEERS - 1)
            .ok_or_else(|| eyre!("expected {PEERS} peers"))?;
        let submitter = submit_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let merchant_domain =
            DomainId::try_new("merchant", "acme").expect("merchant domain");
        let treasury_domain =
            DomainId::try_new("bankvault", "bank").expect("bank vault domain");
        ensure_domain_registration_lease_for_network(&network, &merchant_domain)?;
        ensure_domain_registration_lease_for_network(&network, &treasury_domain)?;
        let transaction = submitter.build_transaction(
            [
                InstructionBox::from(Register::domain(Domain::new(merchant_domain))),
                InstructionBox::from(Register::domain(Domain::new(treasury_domain))),
            ],
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();

        let approved_route =
            submit_and_wait_for_approval(&submitter, transaction.clone()).await?;
        if let Some((lane_id, dataspace_id)) = approved_route {
            ensure!(
                (lane_id == LaneId::new(ACME_LANE)
                    && dataspace_id == DataSpaceId::new(ACME_DATASPACE))
                    || (lane_id == LaneId::new(UNIVERSAL_LANE)
                        && dataspace_id == DataSpaceId::UNIVERSAL),
                "approved route should be deterministic coordinator metadata; got lane {}, dataspace {}",
                lane_id.as_u32(),
                dataspace_id.as_u64()
            );
        }

        let committed_block =
            wait_for_block_with_entrypoint(&submitter, entrypoint_hash, context).await?;
        let receipt = assert_native_amx_execution_context(&committed_block, &transaction)?;
        let relay = wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &transaction,
            committed_block.hash(),
            &receipt,
            context,
        )
        .await?;
        assert_native_amx_relay_tamper_matrix(&relay, &receipt)?;
        wait_for_status_native_amx_receipt(&submitter, &receipt, context).await?;

        submitter.submit::<InstructionBox>(
            Log::new(
                Level::INFO,
                "native AMX routing receipt convergence tick".to_owned(),
            )
            .into(),
        )?;

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rotating-validator Native AMX fault soak is reserved for nightly/release validation"]
async fn native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs()
-> Result<()> {
    init_instruction_registry();
    let context =
        stringify!(native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs);
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let iterations = native_amx_fault_soak_iterations();
        let mut seen_sources = BTreeSet::new();
        let mut previous_authority_height = 0_u64;
        let mut previous_lane_heights = BTreeMap::new();

        for iteration in 0..iterations {
            let faulted_index = iteration % PEERS;
            let submitter_index = (faulted_index + 1) % PEERS;
            let faulted_peer = network
                .peers()
                .get(faulted_index)
                .cloned()
                .ok_or_else(|| eyre!("expected fault target {faulted_index} of {PEERS} peers"))?;
            let submitter_peer = network.peers().get(submitter_index).ok_or_else(|| {
                eyre!("expected submitter {submitter_index} of {PEERS} peers")
            })?;
            let submitter = submitter_peer
                .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
            let merchant_domain = DomainId::try_new(
                format!("faultmerchant{iteration}"),
                "acme",
            )
            .expect("fault-soak merchant domain");
            let treasury_domain = DomainId::try_new(
                format!("faultbankvault{iteration}"),
                "bank",
            )
            .expect("fault-soak bank vault domain");
            ensure_domain_registration_lease_for_network(&network, &merchant_domain)?;
            ensure_domain_registration_lease_for_network(&network, &treasury_domain)?;

            let faulted_validator = faulted_peer.id();
            faulted_peer.shutdown().await;
            ensure!(
                !faulted_peer.is_running(),
                "fault injection did not hold validator {faulted_index} offline"
            );

            let transaction = submitter.build_transaction(
                [
                    InstructionBox::from(Register::domain(Domain::new(merchant_domain))),
                    InstructionBox::from(Register::domain(Domain::new(treasury_domain))),
                ],
                Metadata::default(),
            );
            let entrypoint_hash = transaction.hash_as_entrypoint();
            let approved_route =
                submit_and_wait_for_approval(&submitter, transaction.clone()).await?;
            if let Some((lane_id, dataspace_id)) = approved_route {
                ensure!(
                    (lane_id == LaneId::new(ACME_LANE)
                        && dataspace_id == DataSpaceId::new(ACME_DATASPACE))
                        || (lane_id == LaneId::new(UNIVERSAL_LANE)
                            && dataspace_id == DataSpaceId::UNIVERSAL),
                    "fault-soak iteration {iteration} exposed a non-deterministic coordinator route"
                );
            }

            let committed_block = wait_for_block_with_entrypoint(
                &submitter,
                entrypoint_hash,
                &format!("{context}: iteration {iteration}"),
            )
            .await?;
            let receipt = assert_native_amx_execution_context(&committed_block, &transaction)?;
            if let Some((approved_lane_id, approved_dataspace_id)) = approved_route {
                ensure!(
                    receipt.lane_id == approved_lane_id
                        && receipt.dataspace_id == approved_dataspace_id,
                    "fault-soak iteration {iteration} finalized coordinator route ({}, {}) differs from approved route ({}, {})",
                    receipt.lane_id.as_u32(),
                    receipt.dataspace_id.as_u64(),
                    approved_lane_id.as_u32(),
                    approved_dataspace_id.as_u64(),
                );
            }
            assert_faulted_validator_excluded_from_native_amx_qcs(
                &receipt,
                &faulted_validator,
            )?;
            ensure!(
                seen_sources.insert(receipt.source_id),
                "fault-soak iteration {iteration} replayed an earlier Native AMX source"
            );
            ensure!(
                receipt.authority_context_height > previous_authority_height,
                "fault-soak authority height failed to advance monotonically"
            );
            let coordinator_lane_session = (receipt.lane_id, receipt.lane_incarnation);
            if let Some(previous_lane_height) = previous_lane_heights
                .insert(coordinator_lane_session, receipt.lane_block_height)
            {
                ensure!(
                    receipt.lane_block_height > previous_lane_height,
                    "fault-soak coordinator lane-local height failed to advance within one lane incarnation"
                );
            }
            previous_authority_height = receipt.authority_context_height;

            faulted_peer
                .start_checked(config_layers.iter(), None)
                .await
                .wrap_err_with(|| {
                    format!("restart faulted validator after AMX iteration {iteration}")
                })?;
            let relay = wait_for_all_peers_to_observe_native_amx_evidence(
                &network,
                &transaction,
                committed_block.hash(),
                &receipt,
                &format!("{context}: iteration {iteration} recovery"),
            )
            .await?;
            assert_native_amx_relay_tamper_matrix(&relay, &receipt)?;
            wait_for_status_native_amx_receipt(
                &faulted_peer.client(),
                &receipt,
                &format!("{context}: iteration {iteration} restarted status"),
            )
            .await?;
            eprintln!(
                "[native-amx-fault-soak] iteration {}/{} passed; faulted_peer={} authority_height={} lane_height={}",
                iteration + 1,
                iterations,
                faulted_index,
                receipt.authority_context_height,
                receipt.lane_block_height,
            );
        }

        ensure!(
            seen_sources.len() == iterations,
            "Native AMX fault soak did not finalize every unique source"
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_amx_queue_journal_replays_plan_after_restart() -> Result<()> {
    init_instruction_registry();
    let context = stringify!(native_amx_queue_journal_replays_plan_after_restart);
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let admitting_peer = network
            .peers()
            .get(PEERS - 1)
            .cloned()
            .ok_or_else(|| eyre!("expected {PEERS} peers"))?;
        let submitter = admitting_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let merchant_domain =
            DomainId::try_new("journalmerchant", "acme").expect("merchant domain");
        let treasury_domain =
            DomainId::try_new("journalbankvault", "bank").expect("bank vault domain");
        ensure_domain_registration_lease_for_network(&network, &merchant_domain)?;
        ensure_domain_registration_lease_for_network(&network, &treasury_domain)?;
        let transaction = submitter.build_transaction(
            [
                InstructionBox::from(Register::domain(Domain::new(merchant_domain))),
                InstructionBox::from(Register::domain(Domain::new(treasury_domain))),
            ],
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();

        let submitter_for_submit = submitter.clone();
        let transaction_for_submit = transaction.clone();
        spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction_for_submit))
            .await
            .map_err(|err| eyre!("submit task join error: {err}"))?
            .map_err(|err| eyre!("failed to submit journaled native AMX transaction: {err}"))?;

        admitting_peer.shutdown().await;
        admitting_peer
            .start_checked(config_layers.iter(), None)
            .await
            .wrap_err("restart admitting peer")?;

        let restarted_client =
            admitting_peer.client_for(&ALICE_ID, PrivateKey::clone(ALICE_KEYPAIR.private_key()));
        let block = timeout(
            STATUS_WAIT_TIMEOUT,
            wait_for_block_with_entrypoint(
                &restarted_client,
                entrypoint_hash,
                "journal replay after restart",
            ),
        )
        .await
        .map_err(|_| {
            eyre!("timed out waiting for journaled native AMX transaction after restart")
        })??;
        let receipt = assert_native_amx_execution_context(&block, &transaction)?;
        let relay = wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &transaction,
            block.hash(),
            &receipt,
            context,
        )
        .await?;
        assert_native_amx_relay_tamper_matrix(&relay, &receipt)?;
        wait_for_status_native_amx_receipt(&restarted_client, &receipt, context).await?;

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[test]
fn native_amx_fault_soak_iteration_override_is_positive_and_bounded() {
    assert_eq!(
        parse_native_amx_fault_soak_iterations(None),
        NATIVE_AMX_FAULT_SOAK_ITERATIONS
    );
    assert_eq!(parse_native_amx_fault_soak_iterations(Some(" 17 ")), 17);
    for rejected in ["", "0", "-1", "101", "not-a-number"] {
        assert_eq!(
            parse_native_amx_fault_soak_iterations(Some(rejected)),
            NATIVE_AMX_FAULT_SOAK_ITERATIONS,
            "override {rejected:?} should fail closed to the bounded default"
        );
    }
    assert_eq!(
        parse_native_amx_fault_soak_iterations(Some("100")),
        NATIVE_AMX_FAULT_SOAK_MAX_ITERATIONS
    );
}
