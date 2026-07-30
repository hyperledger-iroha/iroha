#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native AMX multidataspace routing integration coverage.

use std::{
    collections::BTreeSet,
    fs,
    num::{NonZeroU32, NonZeroUsize},
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
                LaneBlockCommitment, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
            },
        },
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            InstructionBox, Log, Mint, Register, SetParameter,
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneRelayEnvelope,
            LaneVisibility, compute_settlement_hash,
        },
        parameter::{Parameter, system::SumeragiNposParameters},
        peer::PeerId,
        prelude::Quantity,
        query::block::prelude::FindBlocks,
        transaction::{FeePaymentIntent, SignedTransaction, TransactionEntrypoint},
    },
};
use iroha_config::{
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as ActualLaneConfig},
        defaults,
    },
};
use iroha_config_base::WithOrigin;
use iroha_core::{da::proof_policy_bundle, kura::Kura};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::prelude::QueryBuilderExt;
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, dataspace_setup_instruction,
    domain_setup_instruction_in_dataspace, genesis_factory_with_post_topology,
    init_instruction_registry,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use norito::json::{self, Value as JsonValue};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};

const PEERS: usize = 4;
const MULTILANE_RELEASE_MODE_ENV: &str = "IROHA_MULTILANE_RELEASE_MODE";
const RUN_IGNORED_ENV: &str = "IROHA_RUN_IGNORED";
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
const NATIVE_AMX_SOAK_ITERATIONS_ENV: &str = "IROHA_NATIVE_AMX_SOAK_ITERATIONS";
const NATIVE_AMX_SOAK_ITERATIONS_DEFAULT: usize = 10;
const NATIVE_AMX_SOAK_ITERATIONS_MAX: usize = 100;
const NATIVE_AMX_GROUP_SIZE: usize = 2;
const EVICTED_BLOCK_INDEX_START: u64 = u64::MAX;
const BLOCK_INDEX_ENTRY_BYTES: usize = core::mem::size_of::<u64>() * 2;
const NATIVE_AMX_GROUPED_PRUNING_MARKER: &str = "[multilane-release-native-evidence] \
grouped_sources=2 durable_manifest=passed body_eviction_recovery=passed \
authenticated_remote_recovery=passed exact_once=passed";
const NATIVE_AMX_MANIFEST_FILE_PREFIX: &str = "native_amx_manifest_v1_";
const NATIVE_AMX_RECEIPT_FILE_PREFIX: &str = "native_amx_receipt_v1_";
const NATIVE_AMX_EVIDENCE_FILE_SUFFIX: &str = ".norito";
const NATIVE_AMX_LATEST_POINTER_FILE: &str = "native_amx_participant_receipts.latest_v2.norito";

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

fn native_amx_lane_catalog() -> LaneCatalog {
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
    LaneCatalog::new(lane_count, lanes).expect("lane catalog")
}

fn da_proof_policy_bundle() -> DaProofPolicyBundle {
    let catalog = native_amx_lane_catalog();
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
                    Quantity::from(VALIDATOR_STAKE),
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
    let mut npos = SumeragiNposParameters::default();
    npos.max_validators = PEERS as u32;
    npos.epoch_length_blocks = std::num::NonZeroU64::new(3_600).unwrap();
    npos.vrf_commit_window_blocks = 100;
    npos.vrf_reveal_window_blocks = 40;

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
        .with_block_cadence(PIPELINE_TIME)
        .with_npos_consensus()
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )))
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
                .write(["nexus", "staking", "max_validators"], PEERS as i64);
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
            leg.prepare_qc.body.phase == NativeAmxPhase::Prepare,
            "prepare QC carried wrong phase"
        );
        ensure!(
            leg.commit_qc.body.phase == NativeAmxPhase::Commit,
            "commit QC carried wrong phase"
        );
        ensure!(
            leg.prepare_qc.body.plan_digest == context.routing_plan_digest
                && leg.commit_qc.body.plan_digest == context.routing_plan_digest,
            "participant QC plan digest differs from execution context"
        );
        ensure!(
            leg.prepare_qc.body.tx_entrypoint_hash == entrypoint_hash
                && leg.commit_qc.body.tx_entrypoint_hash == entrypoint_hash,
            "participant QC entrypoint hash differs from submitted tx"
        );
        ensure!(
            leg.prepare_qc.validator_set.len() == PEERS
                && leg.commit_qc.validator_set.len() == PEERS,
            "participant QCs should carry the 4-peer validator set"
        );
        ensure!(
            leg.prepare_qc.signers_bitmap.iter().any(|byte| *byte != 0)
                && leg.commit_qc.signers_bitmap.iter().any(|byte| *byte != 0),
            "participant QCs should include signer evidence"
        );
    }
    Ok(receipt.clone())
}

#[derive(Clone)]
struct GroupedNativeAmxEvidence {
    block: SignedBlock,
    transactions: Vec<SignedTransaction>,
    receipts: Vec<NativeAmxReceipt>,
    bank_leg: NativeAmxLegRecordV2,
    ordered_sources: Vec<[u8; Hash::LENGTH]>,
}

fn native_amx_source_id(transaction: &SignedTransaction) -> [u8; Hash::LENGTH] {
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction.hash().as_ref());
    source_id
}

fn bank_participant_leg(receipt: &NativeAmxReceipt) -> Result<&NativeAmxLegRecordV2> {
    receipt
        .legs
        .iter()
        .find(|leg| {
            leg.lane_id == LaneId::new(BANK_LANE)
                && leg.dataspace_id == DataSpaceId::new(BANK_DATASPACE)
        })
        .ok_or_else(|| eyre!("Native AMX receipt omitted the separate BANK participant leg"))
}

fn assert_grouped_native_amx_execution(
    block: &SignedBlock,
    transactions: &[SignedTransaction],
) -> Result<GroupedNativeAmxEvidence> {
    ensure!(
        transactions.len() == NATIVE_AMX_GROUP_SIZE,
        "grouped Native AMX release evidence requires exactly {NATIVE_AMX_GROUP_SIZE} sources"
    );
    let source_set = transactions
        .iter()
        .map(native_amx_source_id)
        .collect::<BTreeSet<_>>();
    ensure!(
        source_set.len() == NATIVE_AMX_GROUP_SIZE,
        "grouped Native AMX release evidence reused a source identity"
    );
    let mut ordered_sources = source_set.into_iter().collect::<Vec<_>>();
    ordered_sources.sort_unstable();

    let submitted_entrypoints = transactions
        .iter()
        .map(|transaction| Hash::from(transaction.hash_as_entrypoint()))
        .collect::<BTreeSet<_>>();
    ensure!(
        submitted_entrypoints.len() == NATIVE_AMX_GROUP_SIZE,
        "grouped Native AMX release evidence reused a transaction entrypoint"
    );
    let ordered_entrypoints = block
        .entrypoint_hashes()
        .map(Hash::from)
        .filter(|hash| submitted_entrypoints.contains(hash))
        .collect::<Vec<_>>();
    ensure!(
        ordered_entrypoints.len() == NATIVE_AMX_GROUP_SIZE
            && ordered_entrypoints.iter().copied().collect::<BTreeSet<_>>()
                == submitted_entrypoints,
        "the exact grouped Native AMX sources did not share one canonical application block"
    );

    let receipts = transactions
        .iter()
        .map(|transaction| assert_native_amx_execution_context(block, transaction))
        .collect::<Result<Vec<_>>>()?;
    let canonical_bank_leg = bank_participant_leg(
        receipts
            .first()
            .ok_or_else(|| eyre!("grouped Native AMX execution produced no receipt"))?,
    )?
    .clone();
    let descriptor = &canonical_bank_leg.participant_proposal.descriptor;
    ensure!(
        descriptor.accepted_transaction_hashes == ordered_entrypoints,
        "BANK participant proposal did not bind the exact ordered two-source entrypoint group"
    );
    ensure!(
        canonical_bank_leg.participant_settlement.tx_count == u64::try_from(NATIVE_AMX_GROUP_SIZE)?
            && canonical_bank_leg
                .participant_settlement
                .receipts
                .iter()
                .map(|receipt| receipt.source_id)
                .collect::<Vec<_>>()
                == ordered_sources,
        "BANK participant settlement did not bind the exact ordered two-source group"
    );
    ensure!(
        canonical_bank_leg
            .participant_settlement
            .receipts
            .iter()
            .all(|receipt| {
                receipt.local_amount == Quantity::zero()
                    && receipt.xor_due == Quantity::zero()
                    && receipt.xor_after_haircut == Quantity::zero()
                    && receipt.xor_variance == Quantity::zero()
                    && receipt.timestamp_ms == block.header().height().get()
            })
            && canonical_bank_leg
                .participant_settlement
                .nexus_fee_receipts
                .is_empty()
            && canonical_bank_leg
                .participant_settlement
                .native_amx_receipts
                .is_empty(),
        "BANK participant settlement must remain zero-effect and contain no nested receipts"
    );

    for (transaction, receipt) in transactions.iter().zip(&receipts) {
        let leg = bank_participant_leg(receipt)?;
        ensure!(
            leg.participant_proposal == canonical_bank_leg.participant_proposal
                && leg.participant_settlement == canonical_bank_leg.participant_settlement
                && leg.participant_settlement_hash
                    == canonical_bank_leg.participant_settlement_hash,
            "grouped Native AMX sources did not share one exact BANK proposal and settlement"
        );
        ensure!(
            receipt.source_id == native_amx_source_id(transaction)
                && ordered_sources.contains(&receipt.source_id),
            "grouped Native AMX receipt source is absent from the exact settlement membership"
        );
        for body in [&leg.prepare_qc.body, &leg.commit_qc.body] {
            ensure!(
                body.source_id == receipt.source_id
                    && body.tx_entrypoint_hash == transaction.hash_as_entrypoint()
                    && body.participant_proposal_hash
                        == canonical_bank_leg.participant_proposal.proposal_hash
                    && body.participant_settlement_commitment
                        == Hash::from(canonical_bank_leg.participant_settlement_hash)
                    && body.participant_previous_block_height
                        == descriptor.previous_lane_block_height
                    && body.participant_previous_block_descriptor_hash
                        == descriptor.previous_lane_block_descriptor_hash
                    && body.participant_lane_block_height == descriptor.lane_block_height
                    && body.participant_lane_block_view == descriptor.lane_block_view,
                "grouped Native AMX QC body drifted from its exact source/proposal/settlement/predecessor identity"
            );
        }
    }

    Ok(GroupedNativeAmxEvidence {
        block: block.clone(),
        transactions: transactions.to_vec(),
        receipts,
        bank_leg: canonical_bank_leg,
        ordered_sources,
    })
}

async fn wait_for_grouped_native_amx_durable_application(
    client: &Client,
    evidence: &GroupedNativeAmxEvidence,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    let descriptor = &evidence.bank_leg.participant_proposal.descriptor;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let client = client.clone();
        match spawn_blocking(move || client.get_sumeragi_diagnostics()).await {
            Ok(Ok(diagnostics)) => {
                let application_rows = diagnostics
                    .native_amx_participant_applications
                    .iter()
                    .filter(|row| row.application_block_hash == Some(evidence.block.hash()))
                    .collect::<Vec<_>>();
                let exact = application_rows.len() == 1
                    && application_rows[0].lane_id == LaneId::new(BANK_LANE)
                    && application_rows[0].dataspace_id == DataSpaceId::new(BANK_DATASPACE)
                    && application_rows[0].lane_incarnation == descriptor.lane_incarnation
                    && application_rows[0].participant_height == descriptor.lane_block_height
                    && application_rows[0].participant_view == descriptor.lane_block_view
                    && application_rows[0].predecessor_height
                        == descriptor.previous_lane_block_height
                    && application_rows[0].predecessor_descriptor_hash
                        == descriptor.previous_lane_block_descriptor_hash
                    && application_rows[0].descriptor_hash == descriptor.descriptor_hash
                    && application_rows[0].proposal_hash
                        == evidence.bank_leg.participant_proposal.proposal_hash
                    && application_rows[0].settlement_hash
                        == evidence.bank_leg.participant_settlement_hash
                    && application_rows[0].source_count
                        == u64::try_from(evidence.ordered_sources.len())?
                    && application_rows[0].source_count >= 2
                    && application_rows[0].application_block_height
                        == Some(evidence.block.header().height().get())
                    && application_rows[0].state.as_str() == "durably_applied";
                if exact {
                    return Ok(());
                }
                last_error = Some(format!(
                    "typed diagnostics did not expose the exact two-source BANK durable application: {application_rows:?}"
                ));
            }
            Ok(Err(error)) => last_error = Some(error.to_string()),
            Err(error) => last_error = Some(format!("diagnostics task join error: {error}")),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|error| format!("; last diagnostics error: {error}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for exact grouped Native AMX durable application{suffix}"
    ))
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
        expected_receipt.legs.first().is_some_and(|leg| {
            !leg.prepare_qc.signers_bitmap.is_empty()
                && !leg.prepare_qc.bls_aggregate_signature.is_empty()
        }),
        "tamper matrix requires a receipt with non-empty QC bitmap and signature evidence"
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

async fn wait_for_diagnostics_native_amx_evidence(
    client: &Client,
    receipt: &NativeAmxReceipt,
    context: &str,
) -> Result<(LaneBlockCommitment, LaneRelayEnvelope)> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let client = client.clone();
        match spawn_blocking(move || client.get_sumeragi_diagnostics()).await {
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
                last_error = Some(
                    "typed diagnostics omitted the exact commitment or relay receipt".to_owned(),
                );
            }
            Ok(Err(err)) => last_error = Some(err.to_string()),
            Err(err) => last_error = Some(format!("diagnostics task join error: {err}")),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last diagnostics error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for exact native AMX commitment and relay diagnostics{suffix}"
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
        let observed = wait_for_diagnostics_native_amx_evidence(
            &peer.client(),
            expected_receipt,
            &format!("{context}: peer {index} diagnostics"),
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

async fn fetch_sumeragi_diagnostics_json(client: &Client) -> Result<JsonValue> {
    let diagnostics_url = client.torii_url.join("v1/sumeragi/diagnostics")?;
    let response = reqwest::Client::new()
        .get(diagnostics_url)
        .send()
        .await
        .wrap_err("fetch Sumeragi diagnostics")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .wrap_err("read Sumeragi diagnostics body")?;
    ensure!(
        status.is_success(),
        "Sumeragi diagnostics request failed with status {status}: {body}"
    );
    json::from_str(&body).wrap_err("parse Sumeragi diagnostics JSON")
}

fn diagnostics_contain_native_amx_receipt(
    diagnostics: &JsonValue,
    receipt: &NativeAmxReceipt,
) -> bool {
    let Some(commitments) = diagnostics
        .get("lane_settlement_commitments")
        .and_then(JsonValue::as_array)
    else {
        return false;
    };
    commitments.iter().any(|commitment| {
        let Some(commitment) = commitment.as_object() else {
            return false;
        };
        if commitment.get("block_height").and_then(JsonValue::as_u64)
            != Some(receipt.authority_context_height)
            || commitment.get("lane_id").and_then(JsonValue::as_u64)
                != Some(u64::from(receipt.lane_id))
            || commitment.get("dataspace_id").and_then(JsonValue::as_u64)
                != Some(u64::from(receipt.dataspace_id))
        {
            return false;
        }
        let Some(native_receipts) = commitment
            .get("native_amx_receipts")
            .and_then(JsonValue::as_array)
        else {
            return false;
        };
        native_receipts.iter().any(|native| {
            let Some(native) = native.as_object() else {
                return false;
            };
            let source_id_is_hex = native
                .get("source_id")
                .and_then(JsonValue::as_str)
                .is_some_and(|value| {
                    value.len() == Hash::LENGTH * 2
                        && value.chars().all(|char| char.is_ascii_hexdigit())
                });
            let plan_digest_is_present = native
                .get("plan_digest")
                .and_then(JsonValue::as_str)
                .is_some_and(|value| !value.is_empty());
            if !source_id_is_hex
                || !plan_digest_is_present
                || native
                    .get("authority_context_height")
                    .and_then(JsonValue::as_u64)
                    != Some(receipt.authority_context_height)
                || native.get("lane_block_height").and_then(JsonValue::as_u64)
                    != Some(receipt.lane_block_height)
                || native.get("lane_id").and_then(JsonValue::as_u64)
                    != Some(u64::from(receipt.lane_id))
                || native.get("dataspace_id").and_then(JsonValue::as_u64)
                    != Some(u64::from(receipt.dataspace_id))
            {
                return false;
            }
            let Some(legs) = native.get("legs").and_then(JsonValue::as_array) else {
                return false;
            };
            receipt.legs.iter().all(|expected_leg| {
                legs.iter().any(|leg| {
                    let Some(leg) = leg.as_object() else {
                        return false;
                    };
                    leg.get("lane_id").and_then(JsonValue::as_u64)
                        == Some(u64::from(expected_leg.lane_id))
                        && leg.get("dataspace_id").and_then(JsonValue::as_u64)
                            == Some(u64::from(expected_leg.dataspace_id))
                        && leg
                            .get("prepare_qc")
                            .and_then(JsonValue::as_object)
                            .and_then(|qc| qc.get("body"))
                            .and_then(JsonValue::as_object)
                            .and_then(|body| body.get("phase"))
                            .and_then(JsonValue::as_str)
                            == Some("prepare")
                        && leg
                            .get("commit_qc")
                            .and_then(JsonValue::as_object)
                            .and_then(|qc| qc.get("body"))
                            .and_then(JsonValue::as_object)
                            .and_then(|body| body.get("phase"))
                            .and_then(JsonValue::as_str)
                            == Some("commit")
                })
            })
        })
    })
}

fn native_amx_diagnostics_summary(diagnostics: &JsonValue) -> String {
    let Some(commitments) = diagnostics
        .get("lane_settlement_commitments")
        .and_then(JsonValue::as_array)
    else {
        return "lane_settlement_commitments missing".to_owned();
    };
    let native_total = commitments
        .iter()
        .filter_map(|commitment| {
            commitment
                .get("native_amx_receipts")
                .and_then(JsonValue::as_array)
        })
        .map(|receipts| receipts.len())
        .sum::<usize>();
    let first = commitments
        .first()
        .map(|commitment| {
            let native = commitment
                .get("native_amx_receipts")
                .and_then(JsonValue::as_array)
                .and_then(|receipts| receipts.first());
            let native_summary = native
                .map(|receipt| {
                    format!(
                        " native_source={:?} native_plan={:?} authority_height={:?} lane_height={:?} legs={}",
                        receipt.get("source_id").and_then(JsonValue::as_str),
                        receipt.get("plan_digest").and_then(JsonValue::as_str),
                        receipt
                            .get("authority_context_height")
                            .and_then(JsonValue::as_u64),
                        receipt
                            .get("lane_block_height")
                            .and_then(JsonValue::as_u64),
                        receipt
                            .get("legs")
                            .and_then(JsonValue::as_array)
                            .map(|legs| legs.len())
                            .unwrap_or(0)
                    )
                })
                .unwrap_or_else(|| " native_receipts_empty".to_owned());
            format!(
                " first_block={:?} first_lane={:?} first_dataspace={:?}{native_summary}",
                commitment.get("block_height").and_then(JsonValue::as_u64),
                commitment.get("lane_id").and_then(JsonValue::as_u64),
                commitment.get("dataspace_id").and_then(JsonValue::as_u64)
            )
        })
        .unwrap_or_else(|| " no_first_commitment".to_owned());
    format!(
        "commitments={} native_receipts_total={}{}",
        commitments.len(),
        native_total,
        first
    )
}

async fn wait_for_diagnostics_native_amx_receipt(
    client: &Client,
    receipt: &NativeAmxReceipt,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match fetch_sumeragi_diagnostics_json(client).await {
            Ok(diagnostics) if diagnostics_contain_native_amx_receipt(&diagnostics, receipt) => {
                return Ok(());
            }
            Ok(diagnostics) => {
                last_error = Some(format!(
                    "diagnostics did not include expected native AMX receipt ({})",
                    native_amx_diagnostics_summary(&diagnostics)
                ));
            }
            Err(error) => last_error = Some(error.to_string()),
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|error| format!("; last diagnostics error: {error}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for native AMX receipt in Sumeragi diagnostics{suffix}"
    ))
}

fn native_amx_soak_iterations() -> Result<usize> {
    let raw = std::env::var(NATIVE_AMX_SOAK_ITERATIONS_ENV)
        .unwrap_or_else(|_| NATIVE_AMX_SOAK_ITERATIONS_DEFAULT.to_string());
    let iterations = raw.parse::<usize>().wrap_err_with(|| {
        format!("{NATIVE_AMX_SOAK_ITERATIONS_ENV} must be an integer in 1..={NATIVE_AMX_SOAK_ITERATIONS_MAX}")
    })?;
    ensure!(
        (1..=NATIVE_AMX_SOAK_ITERATIONS_MAX).contains(&iterations),
        "{NATIVE_AMX_SOAK_ITERATIONS_ENV} must be in 1..={NATIVE_AMX_SOAK_ITERATIONS_MAX}, got {iterations}"
    );
    Ok(iterations)
}

fn native_amx_bootstrap_transaction(submitter: &Client) -> Result<SignedTransaction> {
    let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
    let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
    let instructions = vec![
        dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
        dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
        domain_setup_instruction_in_dataspace(
            &DomainId::try_new("soakbootstrapmerchant", "acme")?,
            acme_dataspace,
            &submitter.account,
        )?,
        domain_setup_instruction_in_dataspace(
            &DomainId::try_new("soakbootstrapvault", "bank")?,
            bank_dataspace,
            &submitter.account,
        )?,
    ];
    Ok(submitter.build_transaction(
        instructions,
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))
}

fn native_amx_soak_transactions(
    submitter: &Client,
    iteration: usize,
) -> Result<Vec<SignedTransaction>> {
    let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
    let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
    let mut transactions = (0..NATIVE_AMX_GROUP_SIZE)
        .map(|member| {
            let merchant_domain =
                DomainId::try_new(format!("soakmerchant{iteration:03}{member}"), "acme")
                    .wrap_err("construct grouped soak merchant domain")?;
            let treasury_domain =
                DomainId::try_new(format!("soakbankvault{iteration:03}{member}"), "bank")
                    .wrap_err("construct grouped soak bank domain")?;
            let instructions = vec![
                domain_setup_instruction_in_dataspace(
                    &merchant_domain,
                    acme_dataspace,
                    &submitter.account,
                )?,
                domain_setup_instruction_in_dataspace(
                    &treasury_domain,
                    bank_dataspace,
                    &submitter.account,
                )?,
            ];
            Ok(submitter.build_transaction(
                instructions,
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    transactions.sort_by_key(native_amx_source_id);
    Ok(transactions)
}

async fn submit_grouped_native_amx_transactions(
    submitter: &Client,
    transactions: Vec<SignedTransaction>,
    context: &str,
) -> Result<GroupedNativeAmxEvidence> {
    ensure!(
        transactions.len() == NATIVE_AMX_GROUP_SIZE,
        "{context}: expected exactly {NATIVE_AMX_GROUP_SIZE} grouped transactions"
    );
    let payloads = transactions
        .iter()
        .map(|transaction| submitter.prepare_transaction_payload(transaction))
        .collect::<Vec<_>>();
    submitter
        .submit_prepared_transaction_payload_batch_async(&payloads)
        .await
        .wrap_err_with(|| format!("{context}: submit exact two-source Torii batch"))?;

    let first_entrypoint = transactions[0].hash_as_entrypoint();
    let block = wait_for_block_with_entrypoint(submitter, first_entrypoint, context).await?;
    for transaction in &transactions {
        ensure!(
            block
                .entrypoint_hashes()
                .any(|hash| hash == transaction.hash_as_entrypoint()),
            "{context}: Torii accepted the two-source batch but the sources landed in separate canonical blocks"
        );
    }
    assert_grouped_native_amx_execution(&block, &transactions)
        .wrap_err_with(|| format!("{context}: validate grouped Native AMX carrier evidence"))
}

async fn advance_past_native_amx_eviction_tail(
    submitter: &Client,
    target_height: u64,
    context: &str,
) -> Result<(HashOf<TransactionEntrypoint>, SignedBlock)> {
    let mut last_height = target_height;
    let mut final_barrier = None;
    for offset in 0..3 {
        let transaction = submitter.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!("{context}: post-carrier eviction-tail barrier {offset}"),
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();
        submit_and_wait_for_approval(submitter, transaction).await?;
        let block = wait_for_block_with_entrypoint(
            submitter,
            entrypoint_hash,
            &format!("{context}: eviction-tail barrier {offset}"),
        )
        .await?;
        last_height = block.header().height().get();
        final_barrier = Some((entrypoint_hash, block));
    }
    ensure!(
        last_height > target_height.saturating_add(2),
        "{context}: carrier height {target_height} remained inside the two-block Kura eviction tail at height {last_height}"
    );
    final_barrier.ok_or_else(|| eyre!("{context}: no eviction-tail barrier was committed"))
}

fn offline_kura_config(store_dir: std::path::PathBuf) -> KuraConfig {
    KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(store_dir),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: NonZeroUsize::new(2).expect("two is non-zero"),
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
        eviction_required_replicas: defaults::kura::EVICTION_REQUIRED_REPLICAS,
    }
}

fn decode_block_index_entry(bytes: &[u8], height: u64) -> Result<(u64, u64)> {
    ensure!(height > 0, "block index height must be positive");
    let index = usize::try_from(height.saturating_sub(1))?;
    let start = index
        .checked_mul(BLOCK_INDEX_ENTRY_BYTES)
        .ok_or_else(|| eyre!("block index byte offset overflow"))?;
    let end = start
        .checked_add(BLOCK_INDEX_ENTRY_BYTES)
        .ok_or_else(|| eyre!("block index byte range overflow"))?;
    let entry = bytes
        .get(start..end)
        .ok_or_else(|| eyre!("block index omits height {height}"))?;
    let offset = u64::from_le_bytes(entry[..8].try_into().expect("index offset is eight bytes"));
    let length = u64::from_le_bytes(entry[8..].try_into().expect("index length is eight bytes"));
    Ok((offset, length))
}

fn native_amx_primary_blocks_dir(peer: &NetworkPeer) -> std::path::PathBuf {
    ActualLaneConfig::from_catalog(&native_amx_lane_catalog())
        .primary()
        .blocks_dir(peer.kura_store_dir())
}

fn native_amx_block_index_entry(peer: &NetworkPeer, height: u64) -> Result<(u64, u64)> {
    decode_block_index_entry(
        &fs::read(native_amx_primary_blocks_dir(peer).join("blocks.index"))?,
        height,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeAmxArtifactSelection {
    All,
    Receipts,
    Manifests,
}

fn canonical_native_amx_height_artifact(name: &str) -> Option<(NativeAmxArtifactSelection, u64)> {
    for (prefix, selection) in [
        (
            NATIVE_AMX_MANIFEST_FILE_PREFIX,
            NativeAmxArtifactSelection::Manifests,
        ),
        (
            NATIVE_AMX_RECEIPT_FILE_PREFIX,
            NativeAmxArtifactSelection::Receipts,
        ),
    ] {
        let Some(height) = name
            .strip_prefix(prefix)
            .and_then(|height| height.strip_suffix(NATIVE_AMX_EVIDENCE_FILE_SUFFIX))
        else {
            continue;
        };
        if height.len() != 20 || !height.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let height = height.parse::<u64>().ok()?;
        if height == 0 || format!("{prefix}{height:020}{NATIVE_AMX_EVIDENCE_FILE_SUFFIX}") != name {
            return None;
        }
        return Some((selection, height));
    }
    None
}

fn native_amx_artifact_snapshot(
    peer: &NetworkPeer,
    selection: NativeAmxArtifactSelection,
) -> Result<Vec<(String, Hash)>> {
    let lane_config = ActualLaneConfig::from_catalog(&native_amx_lane_catalog());
    let bank_entry = lane_config
        .entry(LaneId::new(BANK_LANE))
        .ok_or_else(|| eyre!("Native AMX lane catalog omitted BANK storage"))?;
    let artifact_dir = bank_entry
        .blocks_dir(peer.kura_store_dir())
        .join("lane_artifacts");
    let mut snapshot = Vec::new();
    for entry in fs::read_dir(&artifact_dir)
        .wrap_err_with(|| format!("scan Native AMX evidence {}", artifact_dir.display()))?
    {
        let entry = entry.wrap_err_with(|| {
            format!("read Native AMX evidence entry {}", artifact_dir.display())
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| eyre!("Native AMX evidence file name is not UTF-8"))?;
        let artifact_selection = if name == NATIVE_AMX_LATEST_POINTER_FILE {
            Some(NativeAmxArtifactSelection::Receipts)
        } else {
            canonical_native_amx_height_artifact(&name).map(|(selection, _)| selection)
        };
        let Some(artifact_selection) = artifact_selection else {
            ensure!(
                !name.starts_with("native_amx_"),
                "unexpected, temporary, or legacy Native AMX evidence file: {}",
                artifact_dir.join(&name).display()
            );
            continue;
        };
        if !matches!(selection, NativeAmxArtifactSelection::All) && selection != artifact_selection
        {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .wrap_err_with(|| format!("inspect Native AMX evidence {}", path.display()))?;
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "Native AMX evidence must be a regular non-symlink file: {}",
            path.display()
        );
        let bytes = fs::read(&path)
            .wrap_err_with(|| format!("read Native AMX evidence {}", path.display()))?;
        ensure!(
            !bytes.is_empty(),
            "Native AMX evidence file is empty: {}",
            path.display()
        );
        snapshot.push((name, Hash::new(&bytes)));
    }
    snapshot.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(snapshot)
}

fn native_amx_evidence_artifact_snapshot(peer: &NetworkPeer) -> Result<Vec<(String, Hash)>> {
    let snapshot = native_amx_artifact_snapshot(peer, NativeAmxArtifactSelection::All)?;
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name.starts_with(NATIVE_AMX_MANIFEST_FILE_PREFIX)),
        "Native AMX evidence snapshot omitted standalone manifests"
    );
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name.starts_with(NATIVE_AMX_RECEIPT_FILE_PREFIX)),
        "Native AMX evidence snapshot omitted standalone receipts"
    );
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name == NATIVE_AMX_LATEST_POINTER_FILE),
        "Native AMX evidence snapshot omitted the latest pointer"
    );
    Ok(snapshot)
}

fn evict_native_amx_carrier_body_offline(peer: &NetworkPeer, height: u64) -> Result<u64> {
    let catalog = native_amx_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    let config = offline_kura_config(peer.kura_store_dir());
    let (kura, block_count) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)?;
    ensure!(
        u64::try_from(block_count.0)?.saturating_sub(2) > height,
        "Native AMX carrier height {height} is still inside the two-block eviction tail at durable height {}",
        block_count.0
    );
    let height =
        NonZeroUsize::new(usize::try_from(height)?).ok_or_else(|| eyre!("zero carrier height"))?;
    let payload_len = kura
        .advertise_required_replicas_for_bench(height)
        .ok_or_else(|| eyre!("Native AMX carrier has no inline body to evict"))?;
    let freed = kura.evict_block_bodies_for_bench(payload_len)?;
    ensure!(
        freed >= payload_len,
        "Native AMX carrier eviction freed {freed} bytes, below selected body length {payload_len}"
    );
    kura.remove_evicted_block_sidecar_for_testing(height)?;
    drop(kura);

    let height_u64 = u64::try_from(height.get())?;
    let (offset, retained_len) = native_amx_block_index_entry(peer, height_u64)?;
    ensure!(
        offset == EVICTED_BLOCK_INDEX_START && retained_len == payload_len,
        "Native AMX carrier index was not durably marked evicted: offset={offset}, length={retained_len}, expected={payload_len}"
    );
    ensure!(
        !native_amx_primary_blocks_dir(peer)
            .join("da_blocks")
            .join(format!("{height_u64:020}.norito"))
            .exists(),
        "Native AMX remote-recovery fixture retained a local DA body"
    );
    Ok(payload_len)
}

fn remove_latest_native_amx_manifest_offline(
    peer: &NetworkPeer,
    evidence: &GroupedNativeAmxEvidence,
) -> Result<()> {
    let catalog = native_amx_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    let config = offline_kura_config(peer.kura_store_dir());
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)?;
    let descriptor = &evidence.bank_leg.participant_proposal.descriptor;
    kura.remove_latest_native_amx_participant_manifest_for_testing(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        evidence.block.hash(),
    )?;
    drop(kura);
    Ok(())
}

fn ensure_entrypoint_committed_once(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    context: &str,
) -> Result<()> {
    let occurrences = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("{context}: query canonical blocks"))?
        .iter()
        .map(|block| {
            block
                .entrypoint_hashes()
                .filter(|hash| *hash == entrypoint_hash)
                .count()
        })
        .sum::<usize>();
    ensure!(
        occurrences == 1,
        "{context}: expected one canonical application for {entrypoint_hash}, observed {occurrences}"
    );
    Ok(())
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
        let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
        let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
        let transaction = submitter.build_transaction(
            [
                dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
                dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
                domain_setup_instruction_in_dataspace(
                    &merchant_domain,
                    acme_dataspace,
                    &submitter.account,
                )?,
                domain_setup_instruction_in_dataspace(
                    &treasury_domain,
                    bank_dataspace,
                    &submitter.account,
                )?,
            ],
            FeePaymentIntent::authority(Vec::new(), None),
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
        wait_for_diagnostics_native_amx_receipt(&submitter, &receipt, context).await?;

        submitter.submit::<InstructionBox>(
            Log::new(
                Level::INFO,
                "native AMX routing receipt convergence tick".to_owned(),
            )
            .into(),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;

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
        let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
        let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
        let transaction = submitter.build_transaction(
            [
                dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
                dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
                domain_setup_instruction_in_dataspace(
                    &merchant_domain,
                    acme_dataspace,
                    &submitter.account,
                )?,
                domain_setup_instruction_in_dataspace(
                    &treasury_domain,
                    bank_dataspace,
                    &submitter.account,
                )?,
            ],
            FeePaymentIntent::authority(Vec::new(), None),
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
            .start_checked(config_layers.iter().cloned(), None)
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
        wait_for_diagnostics_native_amx_receipt(&restarted_client, &receipt, context).await?;

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

fn multilane_release_gate_requested(context: &str) -> Result<bool> {
    let release = std::env::var(MULTILANE_RELEASE_MODE_ENV).ok();
    let developer = std::env::var(RUN_IGNORED_ENV).ok();
    if release.as_deref().is_some_and(|value| value != "1") {
        return Err(eyre!(
            "{context}: {MULTILANE_RELEASE_MODE_ENV} must be exactly 1 when present"
        ));
    }
    if release.as_deref() == Some("1") {
        return Ok(true);
    }
    if developer.as_deref().is_some_and(|value| value != "1") {
        return Err(eyre!(
            "{context}: {RUN_IGNORED_ENV} must be exactly 1 when present"
        ));
    }
    let requested = developer.as_deref() == Some("1");
    if !requested {
        eprintln!(
            "{context}: developer opt-out; set {RUN_IGNORED_ENV}=1 to run the rotating-validator gate"
        );
    }
    Ok(requested)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs()
-> Result<()> {
    init_instruction_registry();
    let context =
        stringify!(native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs);
    if !multilane_release_gate_requested(context)? {
        return Ok(());
    }
    eprintln!("[multilane-release-gate] started: {context}");
    let iterations = native_amx_soak_iterations()?;
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let bootstrap_submitter = network
            .peers()
            .first()
            .ok_or_else(|| eyre!("Native AMX release network has no bootstrap peer"))?
            .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let bootstrap_transaction = native_amx_bootstrap_transaction(&bootstrap_submitter)?;
        let bootstrap_entrypoint = bootstrap_transaction.hash_as_entrypoint();
        submit_and_wait_for_approval(&bootstrap_submitter, bootstrap_transaction.clone()).await?;
        let bootstrap_block = wait_for_block_with_entrypoint(
            &bootstrap_submitter,
            bootstrap_entrypoint,
            "Native AMX dataspace bootstrap",
        )
        .await?;
        let bootstrap_receipt =
            assert_native_amx_execution_context(&bootstrap_block, &bootstrap_transaction)?;
        wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &bootstrap_transaction,
            bootstrap_block.hash(),
            &bootstrap_receipt,
            "Native AMX dataspace bootstrap convergence",
        )
        .await?;

        let mut observed_sources = BTreeSet::new();
        let mut pruning_evidence: Option<GroupedNativeAmxEvidence> = None;

        for iteration in 0..iterations {
            let offline_index = iteration % PEERS;
            let submit_index = (offline_index + 1) % PEERS;
            let offline_peer = network
                .peers()
                .get(offline_index)
                .cloned()
                .ok_or_else(|| eyre!("iteration {iteration}: missing offline peer"))?;
            let submit_peer = network
                .peers()
                .get(submit_index)
                .ok_or_else(|| eyre!("iteration {iteration}: missing submit peer"))?;
            let submitter =
                submit_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
            let transactions = native_amx_soak_transactions(&submitter, iteration)?;

            offline_peer.shutdown().await;

            // Always restart the rotated validator, even if the three-live-peer
            // commit attempt fails. This keeps the failure diagnostic local to
            // the iteration and lets network teardown remain deterministic.
            let outage_result: Result<GroupedNativeAmxEvidence> = async {
                let evidence = submit_grouped_native_amx_transactions(
                    &submitter,
                    transactions,
                    &format!("iteration {iteration}: three-live-validator commit"),
                )
                .await?;
                for receipt in &evidence.receipts {
                    ensure!(
                        observed_sources.insert(receipt.source_id),
                        "iteration {iteration}: a grouped source identity was reused"
                    );
                    let [first, second] = receipt.legs.as_slice() else {
                        return Err(eyre!(
                            "iteration {iteration}: expected exactly two participant legs"
                        ));
                    };
                    ensure!(
                        first.prepare_qc.body != second.prepare_qc.body
                            && first.commit_qc.body != second.commit_qc.body,
                        "iteration {iteration}: participant routes did not retain independent phase-QC bodies"
                    );
                }
                Ok(evidence)
            }
            .await;

            let restart_result = offline_peer
                .start_checked(config_layers.iter().cloned(), None)
                .await
                .wrap_err_with(|| {
                    format!("iteration {iteration}: restart validator {offline_index}")
                });
            restart_result?;
            let evidence = outage_result?;

            let mut canonical_group_relay: Option<LaneRelayEnvelope> = None;
            for (member, (transaction, receipt)) in evidence
                .transactions
                .iter()
                .zip(&evidence.receipts)
                .enumerate()
            {
                let relay = wait_for_all_peers_to_observe_native_amx_evidence(
                    &network,
                    transaction,
                    evidence.block.hash(),
                    receipt,
                    &format!(
                        "iteration {iteration}: grouped member {member} post-restart convergence"
                    ),
                )
                .await?;
                assert_native_amx_relay_tamper_matrix(&relay, receipt)?;
                if let Some(canonical) = canonical_group_relay.as_ref() {
                    ensure!(
                        relay.settlement_commitment == canonical.settlement_commitment,
                        "iteration {iteration}: grouped sources exposed different coordinator settlements"
                    );
                } else {
                    canonical_group_relay = Some(relay);
                }
            }
            let relay_sources = canonical_group_relay
                .as_ref()
                .ok_or_else(|| eyre!("iteration {iteration}: grouped relay was not published"))?
                .settlement_commitment
                .native_amx_receipts
                .iter()
                .map(|receipt| receipt.source_id)
                .collect::<BTreeSet<_>>();
            ensure!(
                relay_sources
                    == evidence
                        .ordered_sources
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>(),
                "iteration {iteration}: coordinator relay did not bind the exact grouped source membership"
            );

            for (peer_index, peer) in network.peers().iter().enumerate() {
                let client = peer.client();
                for transaction in &evidence.transactions {
                    ensure_entrypoint_committed_once(
                        &client,
                        transaction.hash_as_entrypoint(),
                        &format!("iteration {iteration}: peer {peer_index}"),
                    )?;
                }
                wait_for_grouped_native_amx_durable_application(
                    &client,
                    &evidence,
                    &format!("iteration {iteration}: peer {peer_index}"),
                )
                .await?;
                let diagnostics = client.get_sumeragi_diagnostics().wrap_err_with(|| {
                    format!("iteration {iteration}: peer {peer_index} diagnostics")
                })?;
                let same_route_rows = diagnostics
                    .native_amx_participant_applications
                    .iter()
                    .filter(|row| {
                        row.application_block_hash == Some(evidence.block.hash())
                            && row.lane_id == LaneId::new(ACME_LANE)
                            && row.dataspace_id == DataSpaceId::new(ACME_DATASPACE)
                    })
                    .count();
                ensure!(
                    same_route_rows == 0,
                    "iteration {iteration}: peer {peer_index} published a forbidden separate same-route coordinator marker"
                );
            }
            pruning_evidence = Some(evidence);
        }

        ensure!(
            observed_sources.len() == iterations.saturating_mul(NATIVE_AMX_GROUP_SIZE),
            "fault soak lost or duplicated Native AMX source identities"
        );

        let pruning_evidence =
            pruning_evidence.ok_or_else(|| eyre!("fault soak produced no grouped evidence"))?;
        let pruning_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("missing Native AMX pruning peer"))?;
        let pruning_submitter = network
            .peers()
            .get(1)
            .ok_or_else(|| eyre!("missing pruning-tail submit peer"))?
            .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let (barrier_entrypoint, barrier_block) = advance_past_native_amx_eviction_tail(
            &pruning_submitter,
            pruning_evidence.block.header().height().get(),
            context,
        )
        .await?;
        timeout(
            STATUS_WAIT_TIMEOUT,
            pruning_peer.once_block(barrier_block.header().height().get()),
        )
        .await
        .wrap_err("pruning peer did not durably cross the carrier eviction tail")?;
        let pruning_barrier = wait_for_block_with_entrypoint(
            &pruning_peer.client(),
            barrier_entrypoint,
            "pruning peer exact eviction-tail barrier",
        )
        .await?;
        ensure!(
            pruning_barrier.hash() == barrier_block.hash(),
            "pruning peer observed a different eviction-tail barrier identity"
        );
        pruning_peer.shutdown().await;
        let evidence_artifacts = native_amx_evidence_artifact_snapshot(&pruning_peer)?;
        let receipt_artifacts = native_amx_artifact_snapshot(
            &pruning_peer,
            NativeAmxArtifactSelection::Receipts,
        )?;
        let manifest_artifacts = native_amx_artifact_snapshot(
            &pruning_peer,
            NativeAmxArtifactSelection::Manifests,
        )?;
        let eviction_height = pruning_evidence.block.header().height().get();
        let evicted_payload_len =
            evict_native_amx_carrier_body_offline(&pruning_peer, eviction_height)?;
        ensure!(
            native_amx_evidence_artifact_snapshot(&pruning_peer)? == evidence_artifacts,
            "Native AMX body eviction changed durable receipt/manifest/index evidence"
        );
        remove_latest_native_amx_manifest_offline(&pruning_peer, &pruning_evidence)?;
        ensure!(
            native_amx_artifact_snapshot(
                &pruning_peer,
                NativeAmxArtifactSelection::Receipts,
            )? == receipt_artifacts,
            "Native AMX remote-recovery fixture changed receipt/latest-index evidence"
        );
        ensure!(
            native_amx_artifact_snapshot(
                &pruning_peer,
                NativeAmxArtifactSelection::Manifests,
            )? != manifest_artifacts,
            "Native AMX remote-recovery fixture failed to create an exact manifest gap"
        );
        pruning_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err("restart Native AMX peer after authenticated carrier eviction")?;
        ensure!(
            native_amx_block_index_entry(&pruning_peer, eviction_height)?
                == (EVICTED_BLOCK_INDEX_START, evicted_payload_len),
            "Native AMX restart reinserted the evicted carrier body into inline Kura storage"
        );

        let recovered_block = wait_for_block_with_entrypoint(
            &pruning_peer.client(),
            pruning_evidence.transactions[0].hash_as_entrypoint(),
            "post-pruning Native AMX carrier recovery",
        )
        .await?;
        ensure!(
            recovered_block.hash() == pruning_evidence.block.hash(),
            "authenticated recovery returned a different Native AMX carrier identity"
        );
        ensure!(
            native_amx_primary_blocks_dir(&pruning_peer)
                .join("da_blocks")
                .join(format!("{eviction_height:020}.norito"))
                .is_file(),
            "authenticated CommitQC-signer recovery did not restore the local DA body"
        );
        let recovered_evidence =
            assert_grouped_native_amx_execution(&recovered_block, &pruning_evidence.transactions)?;
        ensure!(
            recovered_evidence.receipts == pruning_evidence.receipts
                && recovered_evidence.bank_leg == pruning_evidence.bank_leg
                && recovered_evidence.ordered_sources == pruning_evidence.ordered_sources,
            "authenticated recovery changed the exact Native AMX manifest-backed group evidence"
        );
        ensure!(
            native_amx_evidence_artifact_snapshot(&pruning_peer)? == evidence_artifacts,
            "Native AMX startup recovery changed exact durable manifest/receipt/index artifacts"
        );
        for (peer_index, peer) in network.peers().iter().enumerate() {
            let client = peer.client();
            wait_for_grouped_native_amx_durable_application(
                &client,
                &pruning_evidence,
                &format!("post-pruning peer {peer_index} durable evidence"),
            )
            .await?;
            for transaction in &pruning_evidence.transactions {
                ensure_entrypoint_committed_once(
                    &client,
                    transaction.hash_as_entrypoint(),
                    &format!("post-pruning peer {peer_index} exact-once"),
                )?;
            }
        }
        ensure!(
            native_amx_block_index_entry(&pruning_peer, eviction_height)?
                == (EVICTED_BLOCK_INDEX_START, evicted_payload_len),
            "Native AMX proof recovery repopulated the inline carrier body"
        );
        eprintln!("{NATIVE_AMX_GROUPED_PRUNING_MARKER}");
        Ok(())
    }
    .await;

    network.shutdown().await;
    result?;
    eprintln!("[multilane-release-gate] completed: {context}");
    Ok(())
}
