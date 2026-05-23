#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native AMX multidataspace routing integration coverage.

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::{Hash, HashOf},
    data_model::{
        Level,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::{
            ExternalExecutionRouteLeg, ExternalExecutionRouteRole, Header, SignedBlock,
            consensus::NativeAmxPhase,
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
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility},
        peer::PeerId,
        prelude::Numeric,
        query::block::prelude::FindBlocks,
        transaction::{SignedTransaction, TransactionEntrypoint},
    },
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
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
    let key_pair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    AccountId::new(key_pair.public_key().clone())
}

fn gas_account() -> AccountId {
    let key_pair = KeyPair::from_seed(
        b"integration_tests::native_amx_routing::gas".to_vec(),
        Algorithm::Ed25519,
    );
    AccountId::new(key_pair.public_key().clone())
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
        Mint::asset_numeric(
            VALIDATOR_FEE_SEED,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
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
            Mint::asset_numeric(
                stake_per_validator,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_numeric(
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
        .with_config_layer(move |layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
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

    let outcome = timeout(STATUS_WAIT_TIMEOUT, async {
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
    .map_err(|_| eyre!("timed out waiting for transaction approval event"))??;

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
) -> Result<()> {
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
        receipt.block_height == block.header().height().get(),
        "native AMX receipt block height differs from containing block"
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
    Ok(())
}

async fn wait_for_all_peers_to_observe_block(
    network: &sandbox::SerializedNetwork,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    expected_block_hash: HashOf<Header>,
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
    }
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
        assert_native_amx_execution_context(&committed_block, &transaction)?;
        wait_for_all_peers_to_observe_block(&network, entrypoint_hash, committed_block.hash())
            .await?;

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
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err("restart admitting peer")?;

        let restarted_client =
            admitting_peer.client_for(&ALICE_ID, PrivateKey::clone(ALICE_KEYPAIR.private_key()));
        let maybe_block = timeout(
            STATUS_WAIT_TIMEOUT,
            wait_for_block_with_entrypoint(
                &restarted_client,
                entrypoint_hash,
                "journal replay after restart",
            ),
        )
        .await;

        match maybe_block {
            Ok(Ok(block)) => {
                assert_native_amx_execution_context(&block, &transaction)?;
                wait_for_all_peers_to_observe_block(&network, entrypoint_hash, block.hash())
                    .await?;
            }
            Ok(Err(_)) | Err(_) => {
                let blocks = restarted_client.query(FindBlocks).execute_all()?;
                ensure!(
                    !blocks.iter().any(|block| block
                        .entrypoint_hashes()
                        .any(|hash| hash == entrypoint_hash)),
                    "stale native AMX transaction appeared after journal replay timeout"
                );
            }
        }

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}
