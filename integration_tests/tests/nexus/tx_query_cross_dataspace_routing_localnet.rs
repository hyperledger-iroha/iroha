//! Focused wrong-dataspace Torii ingress regression for transaction and query routing.
use super::localnet_npos::npos_override_instruction;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::query::{AsyncQueryBuilderExt as _, QueryError};
use iroha::{
    blocking::Client,
    crypto::Hash,
    data_model::{
        Level, NetworkId,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        da::commitment::DaProofPolicyBundle,
        domain::Domain,
        isi::{
            Grant, InstructionBox, Log, Mint, Register,
            consensus_keys::RegisterConsensusKey,
            register::RegisterPeerWithPop,
            space_directory::PublishSpaceDirectoryManifest,
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        nexus::{
            Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, LaneCatalog,
            LaneConfig as ModelLaneConfig, LaneVisibility, ManifestEffect, ManifestEntry,
            ManifestVersion, UniversalAccountId,
        },
        prelude::{FindAssetById, Identifiable, Quantity},
        sumeragi::SumeragiStatus,
        transaction::{SignedTransaction, TransactionSubmissionReceipt},
    },
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::{da::proof_policy_bundle, state::derive_committee_key_id};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    ValidationFail,
    query::{
        error::{FindError, QueryExecutionFail},
        parameters::FetchSize,
        peer::prelude::FindPeers,
    },
    sumeragi_lanes::{
        SumeragiFixedLane, SumeragiLaneMember, SumeragiLanePolicy, SumeragiLaneRoute,
        SumeragiLaneStatus,
    },
};
use iroha_executor_data_model::permission::peer::CanManagePeers;
use iroha_genesis::GenesisTopologyEntry;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::numeric::NumericSpec;
use iroha_test_network::{
    Network, NetworkBuilder, ObserverP2pBootstrap, genesis_participant_committee_key_instructions,
    unexecuted_genesis_factory_with_post_topology,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use iroha_torii::{
    HEADER_ACCOUNT, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP_MS, Method, Uri,
    canonical_network_request_signature_message, signature_header_value,
};
use norito::{decode_from_bytes, json::Value as JsonValue};
use reqwest::StatusCode as HttpStatusCode;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU32, NonZeroU64},
    thread,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use toml::{Table, Value as TomlValue};
const NEXUS_ALIAS: &str = "universal";
const DS1_ALIAS: &str = "ds1";
const DS2_ALIAS: &str = "ds2";
const NEXUS_ID_U64: u64 = 0;
const DS1_ID_U64: u64 = 1;
const DS2_ID_U64: u64 = 2;
const DS1_MANIFEST_HASH: &str = "0100000000000000000000000000000000000000000000000000000000000000";
const DS2_MANIFEST_HASH: &str = "0200000000000000000000000000000000000000000000000000000000000000";
const NEXUS_LANE_INDEX: u32 = 0;
const DS1_LANE_INDEX: u32 = 1;
const DS2_LANE_INDEX: u32 = 2;
const VALIDATORS_PER_LANE: usize = 4;
const LANE_VALIDATOR_COUNT: usize = VALIDATORS_PER_LANE * 3;
// Keep the signed global committee at exact 3f+1 geometry while assigning only
// the first twelve peers to the three disjoint lane committees. This routing
// fixture runs within the initial 3,600-block epoch: lane-zero's four candidates
// are not a replacement thirteen-validator global election pool.
const TOTAL_PEERS: usize = 13;
const VALIDATOR_STAKE: u64 = 2_000;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const ASSET_PAGE_LIMIT: usize = 16;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const DS1_VALIDATOR_INDEX: usize = VALIDATORS_PER_LANE;
const DS2_VALIDATOR_INDEX: usize = VALIDATORS_PER_LANE * 2;
const INGRESS_OBSERVER_COUNT: usize = 2;
const DS1_INGRESS_OBSERVER_INDEX: usize = 0;
const DS2_INGRESS_OBSERVER_INDEX: usize = 1;
fn partitioned_asset_domain() -> DomainId {
    DomainId::try_new("nexus", NEXUS_ALIAS).expect("public asset definition domain")
}

fn partitioned_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        partitioned_asset_domain(),
        "reserve".parse().expect("private asset name"),
    )
}

fn restricted_asset_partition() -> AssetId {
    AssetId::with_scope(
        partitioned_asset_definition_id(),
        ALICE_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(DS1_ID_U64)),
    )
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    nexus_fee_asset_definition_id()
}
fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}
fn genesis_staking_policy() -> iroha_data_model::block::consensus::PublicLaneStakingPolicy {
    iroha_genesis::default_staking_policy(
        stake_asset_definition_id(),
        routing_probe_gas_account_id(),
        routing_probe_gas_account_id(),
    )
}
fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
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
    permission_semantics: Option<String>,
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
    let keypair = KeyPair::try_from_seed(validator_authority_seed(index), Algorithm::Ed25519)
        .expect("fixture tx/query routing validator authority key");
    AccountId::new(keypair.public_key().clone())
}
fn validator_authority_seed(index: usize) -> Vec<u8> {
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xC1;
    seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    seed
}
fn expected_lane_binding_for_peer(index: usize, peer_id: &PeerId) -> ExpectedLaneValidatorBinding {
    ExpectedLaneValidatorBinding {
        validator: validator_authority_account_for_peer(index).to_string(),
        peer_id: peer_id.to_string(),
    }
}
// Runtime registration adds a proof-bound registry candidate, not a voter. The
// observers remain outside the original global and fixed lane committees.
fn ingress_observer_registration(peer: PeerId, pop: &[u8]) -> Result<RegisterPeerWithPop> {
    iroha_crypto::bls_normal_pop_verify(peer.public_key(), pop)
        .wrap_err("ingress observer must prove possession of its exact peer key")?;
    Ok(RegisterPeerWithPop::new(peer, pop.to_vec()))
}
fn ingress_registry_matches(actual: &[PeerId], expected: &BTreeSet<PeerId>) -> bool {
    actual.len() == expected.len() && actual.iter().cloned().collect::<BTreeSet<_>>() == *expected
}
fn localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        // Every global validator is authoritative for these globally executed
        // routes. Non-voting ingress peers exercise actual proxy dispatch while
        // retaining the exact global and native lane committees below.
        .with_observer_p2p_bootstrap(
            ObserverP2pBootstrap::new(INGRESS_OBSERVER_COUNT).expect("two ingress observers"),
        )
        .expect("thirteen validators and two ingress observers fit the P2P capacity")
        .with_npos_consensus()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology = npos_multilane_genesis_post_topology_transactions(
                topology.as_ref(),
                &topology_entries,
            );
            // The builder binds configured DA policies before re-signing the
            // normalized carrier. Mutating this signed source invalidates it.
            unexecuted_genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
                genesis_staking_policy(),
            )
        })
        // Bind NPoS after selecting the custom source, which resets extra
        // instructions. Its source factory starts with a permissioned carrier;
        // normalization signs this exact thirteen-validator election policy.
        .with_genesis_instruction(npos_override_instruction(TOTAL_PEERS))
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
            ds_nexus.insert(
                "id".into(),
                TomlValue::Integer(
                    i64::try_from(NEXUS_ID_U64).expect("dataspace id fits TOML integer"),
                ),
            );
            ds_nexus.insert(
                "description".into(),
                TomlValue::String("main nexus dataspace".to_owned()),
            );
            ds_nexus.insert("fault_tolerance".into(), TomlValue::Integer(1));
            let mut ds1 = Table::new();
            ds1.insert("alias".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            ds1.insert(
                "id".into(),
                TomlValue::Integer(
                    i64::try_from(DS1_ID_U64).expect("dataspace id fits TOML integer"),
                ),
            );
            ds1.insert(
                "manifest_hash".into(),
                TomlValue::String(DS1_MANIFEST_HASH.to_owned()),
            );
            ds1.insert(
                "description".into(),
                TomlValue::String("private dataspace one".to_owned()),
            );
            ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));
            let mut ds2 = Table::new();
            ds2.insert("alias".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            ds2.insert(
                "id".into(),
                TomlValue::Integer(
                    i64::try_from(DS2_ID_U64).expect("dataspace id fits TOML integer"),
                ),
            );
            ds2.insert(
                "manifest_hash".into(),
                TomlValue::String(DS2_MANIFEST_HASH.to_owned()),
            );
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
                    ["nexus", "staking", "max_validators"],
                    i64::try_from(VALIDATORS_PER_LANE).expect("validator count fits TOML integer"),
                );
        })
}
fn wrong_ingress_clients(network: &Network) -> Result<(Client, Client)> {
    let observers = network.observers();
    ensure!(
        observers.len() == INGRESS_OBSERVER_COUNT,
        "expected two non-voting ingress peers, got {}",
        observers.len()
    );
    Ok((
        observers[DS2_INGRESS_OBSERVER_INDEX]
            .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone()),
        observers[DS1_INGRESS_OBSERVER_INDEX]
            .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone()),
    ))
}
fn ingress_status_retry_delay(error: &iroha::Error) -> Option<Duration> {
    match error {
        iroha::Error::StatusUnavailable {
            reason: Some(iroha::StatusFailureReason::StateBusy),
            retry_after,
        } => Some(STATUS_POLL_INTERVAL.max(retry_after.unwrap_or_default())),
        _ => None,
    }
}
fn ingress_readiness_before_deadline(
    latest: &[u64; INGRESS_OBSERVER_COUNT],
    target: u64,
    now: tokio::time::Instant,
    deadline: tokio::time::Instant,
) -> Result<bool> {
    ensure!(
        now < deadline,
        "ingress applied readiness did not reach {target} before its deadline: {latest:?}"
    );
    Ok(latest.iter().all(|height| *height >= target))
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
fn restricted_lane_policy(
    topology: &[PeerId],
    topology_entries: &[GenesisTopologyEntry],
) -> SumeragiLanePolicy {
    assert_eq!(topology.len(), TOTAL_PEERS);
    let policy = SumeragiLanePolicy {
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        anchor_freshness: 64,
        max_merge_blocks: 16,
        stall_window: 10_000,
        lane_params: iroha_data_model::parameter::system::SumeragiParameters::default(),
        fixed: [(DS1_LANE_INDEX, DS1_ID_U64), (DS2_LANE_INDEX, DS2_ID_U64)]
            .into_iter()
            .enumerate()
            .map(|(offset, (lane, dataspace))| {
                let start = (offset + 1) * VALIDATORS_PER_LANE;
                let mut committee = topology[start..start + VALIDATORS_PER_LANE]
                    .iter()
                    .map(|peer| {
                        let entry = topology_entries
                            .iter()
                            .find(|entry| &entry.peer == peer)
                            .expect("signed topology contains each lane member");
                        SumeragiLaneMember {
                            peer: peer.clone(),
                            pop: entry
                                .pop_bytes()
                                .expect("well-formed PoP")
                                .expect("lane PoP"),
                        }
                    })
                    .collect::<Vec<_>>();
                // The signed policy pins the native record's canonical peer order,
                // independently of each validator's position in the global topology.
                committee.sort_by(|left, right| left.peer.cmp(&right.peer));
                SumeragiFixedLane {
                    lane: LaneId::new(lane),
                    dataspace: DataSpaceId::new(dataspace),
                    committee,
                }
            })
            .collect(),
        routes: [(DS1_LANE_INDEX, &*ALICE_ID), (DS2_LANE_INDEX, &*BOB_ID)]
            .into_iter()
            .map(|(lane, account)| SumeragiLaneRoute {
                lane: LaneId::new(lane),
                account: Some(account.to_string()),
                instruction: None,
            })
            .collect(),
        autoscale: None,
    };
    policy.validate().expect("native restricted lane policy");
    policy
}
fn npos_multilane_genesis_post_topology_transactions(
    topology: &[PeerId],
    topology_entries: &[GenesisTopologyEntry],
) -> Vec<Vec<InstructionBox>> {
    assert_eq!(
        topology.len(),
        TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers in genesis topology, got {}",
        topology.len()
    );
    let nexus_domain: DomainId = DomainId::try_new("nexus", "universal").expect("nexus domain");
    let universal_domain: DomainId =
        DomainId::try_new("universal", "universal").expect("universal domain");
    let ds1_domain: DomainId = DomainId::try_new("ds1", "universal").expect("ds1 domain");
    let ds2_domain: DomainId = DomainId::try_new("ds2", "universal").expect("ds2 domain");
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = nexus_fee_asset_definition_id();
    let ds1_asset_def = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds1coin".parse().expect("asset definition name"),
    );
    let ds2_asset_def = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds2coin".parse().expect("asset definition name"),
    );
    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::domain(Domain::new(universal_domain.clone())).into(),
        Register::domain(Domain::new(ds1_domain)).into(),
        Register::domain(Domain::new(ds2_domain)).into(),
        Register::asset_definition({
            let __asset_definition_id = stake_asset_id.clone();
            AssetDefinition::new(
                __asset_definition_id.clone(),
                "XOR".to_owned(),
                NumericSpec::fractional(9),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(universal_domain.clone()),
            )
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds1_asset_def.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "ds1coin".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(nexus_domain.clone()),
            )
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds2_asset_def.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "ds2coin".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(nexus_domain.clone()),
            )
        })
        .into(),
        Mint::asset_quantity(
            100_u32,
            AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(200_u32, AssetId::new(ds2_asset_def.clone(), BOB_ID.clone())).into(),
    ];
    bootstrap_tx.extend(genesis_participant_committee_key_instructions(
        topology_entries,
        &topology[VALIDATORS_PER_LANE..LANE_VALIDATOR_COUNT],
    ));
    for (index, peer) in topology.iter().take(LANE_VALIDATOR_COUNT).enumerate() {
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
            Mint::asset_quantity(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
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
                Quantity::from(VALIDATOR_STAKE),
                Metadata::default(),
                iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                    AssetId::new(stake_asset_id.clone(), validator_id.clone()),
                    AssetId::new(stake_asset_id.clone(), routing_probe_gas_account_id()),
                    Quantity::from(VALIDATOR_STAKE),
                ),
            )
            .into(),
        );
        bootstrap_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }
    // The funded global-route authority registers the two observer ingress
    // identities after genesis. P2P trust alone grants no delegated-read authority.
    bootstrap_tx.push(
        Grant::account_permission(CanManagePeers, validator_authority_account_for_peer(0)).into(),
    );
    // A node-local catalog cannot create native committees. Commit the exact
    // two restricted committees and authority routes in the signed genesis.
    bootstrap_tx.push(
        iroha_data_model::isi::SetParameter::new(iroha_data_model::parameter::Parameter::Custom(
            restricted_lane_policy(topology, topology_entries).into_custom_parameter(),
        ))
        .into(),
    );
    // Register the restricted-balance definition in the existing universal
    // namespace, then mint into the explicit DS1 partition in its own transaction.
    let public_definition_tx = vec![
        Register::asset_definition(AssetDefinition::numeric(
            partitioned_asset_definition_id(),
            "reserve".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            Some(partitioned_asset_domain()),
        ))
        .into(),
    ];
    let restricted_balance_tx =
        vec![Mint::asset_quantity(7_u32, restricted_asset_partition()).into()];
    vec![bootstrap_tx, public_definition_tx, restricted_balance_tx]
}
fn restricted_lane_records_match(
    statuses: &[SumeragiLaneStatus],
    policy: &SumeragiLanePolicy,
    network_id: &NetworkId,
) -> bool {
    statuses.len() == policy.fixed.len()
        && policy.fixed.iter().enumerate().all(|(ordinal, fixed)| {
            statuses
                .iter()
                .find(|status| status.record.lane == fixed.lane)
                .is_some_and(|status| {
                    let record = &status.record;
                    record.dataspace == fixed.dataspace
                        && record.incarnation
                            == iroha_core::sumeragi::lanes::step::incarnation(
                                network_id,
                                fixed.lane,
                                fixed.dataspace,
                                1,
                                ordinal as u64,
                            )
                        && record.committee == fixed.committee
                        && record.params == policy.lane_params
                        && record.da_layout == policy.da_layout
                        && record.anchor_freshness == policy.anchor_freshness
                        && record.created_at == 1
                        && record.active_from == 3
                        && record.closing.is_none()
                })
        })
}
fn wait_for_restricted_lane_instances(client: &Client, policy: &SumeragiLanePolicy) -> Result<()> {
    let started = Instant::now();
    let mut observed = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.client().get_sumeragi_lanes() {
            Ok(statuses)
                if restricted_lane_records_match(
                    &statuses,
                    policy,
                    client.client().network_id(),
                ) && statuses.iter().all(|status| {
                    status
                        .instance
                        .as_ref()
                        .is_some_and(|instance| instance.halted.is_none())
                }) =>
            {
                return Ok(());
            }
            Ok(statuses) => observed = format!("{statuses:?}"),
            Err(error) => observed = error.to_string(),
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "native restricted lane membership/readiness timed out: {observed}"
    ))
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
fn genesis_active_lane_bindings<'a>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
    lane_id: LaneId,
) -> Result<BTreeSet<ExpectedLaneValidatorBinding>> {
    let mut registered = BTreeMap::new();
    let mut activated = BTreeSet::new();
    for instruction in instructions {
        if let Some(registration) = instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
            && registration.lane_id == lane_id
        {
            ensure!(
                registered
                    .insert(
                        registration.validator.to_string(),
                        registration.peer_id.to_string(),
                    )
                    .is_none(),
                "duplicate signed genesis validator registration on lane {lane_id}"
            );
        }
        if let Some(activation) = instruction
            .as_any()
            .downcast_ref::<ActivatePublicLaneValidator>()
            && activation.lane_id == lane_id
        {
            ensure!(
                registered.contains_key(&activation.validator.to_string()),
                "signed genesis activation precedes registration on lane {lane_id}"
            );
            ensure!(
                activated.insert(activation.validator.to_string()),
                "duplicate signed genesis validator activation on lane {lane_id}"
            );
        }
    }
    ensure!(
        !registered.is_empty() && activated == registered.keys().cloned().collect(),
        "signed genesis must activate every registered validator on lane {lane_id}"
    );
    Ok(registered
        .into_iter()
        .map(|(validator, peer_id)| ExpectedLaneValidatorBinding { validator, peer_id })
        .collect())
}
fn expect_restricted_lane_hidden(response: &RoutedJsonResponse) -> Result<()> {
    ensure!(
        response.status == HttpStatusCode::NOT_FOUND,
        "restricted lane validators must be hidden by the public endpoint"
    );
    let body = response
        .body
        .as_object()
        .ok_or_else(|| eyre!("restricted lane response must contain the query error envelope"))?;
    ensure!(
        body.len() == 2
            && body.get("code").and_then(JsonValue::as_str) == Some("query_validation_failed")
            && body.get("message").and_then(JsonValue::as_str)
                == Some("Query not found in the live query store."),
        "restricted lane response must contain only the canonical not-found error"
    );
    Ok(())
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
            .client()
            .get_public_lane_validators(lane_id)
            .map_err(|err| eyre!(err))?;
        let (total, active) = lane_validator_snapshot(&snapshot, context)?;
        last_total = total;
        last_active.clone_from(&active);
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
fn wait_for_height(client: &Client, target_height: u64, context: &str) -> Result<SumeragiStatus> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.client().get_sumeragi_status() {
            Ok(status) => {
                last_height = status.committed_height;
                if status.committed_height >= target_height {
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
fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Quantity> {
    match client
        .client()
        .query_single(FindAssetById::new(asset_id.clone()))
    {
        Ok(asset) if asset.id() == asset_id => Ok(asset.value().clone()),
        Ok(_) => Err(eyre!("exact asset query returned a different asset")),
        Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Asset(missing),
        )))) if missing.as_ref() == asset_id => Ok(Quantity::zero()),
        Err(error) => Err(eyre!(error)),
    }
}
fn routed_header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}
fn routed_response_context(
    status: HttpStatusCode,
    headers: &reqwest::header::HeaderMap,
    path_segments: &[String],
    query_pairs: &[(String, String)],
) -> String {
    let path = if path_segments.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", path_segments.join("/"))
    };
    let query = if query_pairs.is_empty() {
        String::new()
    } else {
        let encoded = query_pairs
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{encoded}")
    };
    format!(
        "status={status}, path={path}{query}, routed_by={:?}, route_lane_id={:?}, route_dataspace_id={:?}",
        routed_header_string(headers, "x-iroha-routed-by"),
        routed_header_string(headers, "x-iroha-route-lane-id"),
        routed_header_string(headers, "x-iroha-route-dataspace-id"),
    )
}
fn routed_json_empty_body_is_transient(status: HttpStatusCode, body: &[u8]) -> bool {
    body.is_empty()
        && matches!(
            status,
            HttpStatusCode::REQUEST_TIMEOUT
                | HttpStatusCode::BAD_GATEWAY
                | HttpStatusCode::SERVICE_UNAVAILABLE
                | HttpStatusCode::GATEWAY_TIMEOUT
        )
}
fn routed_json_response_is_transient(response: &RoutedJsonResponse) -> bool {
    response.body.is_null()
        && routed_json_empty_body_is_transient(response.status, response.body_text.as_bytes())
}
fn routed_submit_response_is_transient(response: &RoutedTransactionSubmitResponse) -> bool {
    if routed_json_empty_body_is_transient(response.status, response.body_text.as_bytes()) {
        return true;
    }
    if response.status == HttpStatusCode::NOT_FOUND && response.body_text.is_empty() {
        return true;
    }
    matches!(
        response.status,
        HttpStatusCode::NOT_FOUND | HttpStatusCode::SERVICE_UNAVAILABLE
    ) && response.body_text.contains("route_unavailable")
}
fn add_client_headers(
    client: &Client,
    mut request: reqwest::RequestBuilder,
    include_content_type: bool,
    include_account_header: bool,
) -> reqwest::RequestBuilder {
    for (name, value) in client.client().headers() {
        if !include_content_type && name.eq_ignore_ascii_case("content-type") {
            continue;
        }
        if !include_account_header && name.eq_ignore_ascii_case("x-iroha-account") {
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
fn routed_http_client() -> reqwest::Client {
    integration_tests::http::client_with_timeout(iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT)
}
async fn torii_json_get(
    client: &Client,
    path_segments: &[String],
    query_pairs: &[(String, String)],
) -> Result<RoutedJsonResponse> {
    let mut url = client.client().endpoint().clone();
    let torii_url_literal = url.to_string();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| eyre!("torii URL `{torii_url_literal}` cannot accept path segments"))?;
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
    let request = routed_http_client()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    let response = add_client_headers(client, request, true, true)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let response_context = routed_response_context(status, &headers, path_segments, query_pairs);
    let json_body = if routed_json_empty_body_is_transient(status, &body) {
        JsonValue::Null
    } else {
        norito::json::from_slice(&body)
            .wrap_err_with(|| format!("decode JSON body ({response_context}): {body_text}"))?
    };
    Ok(RoutedJsonResponse {
        permission_semantics: routed_header_string(
            &headers,
            "x-iroha-account-permission-semantics",
        ),
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
    let mut url = client.client().endpoint().clone();
    let torii_url_literal = url.to_string();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| eyre!("torii URL `{torii_url_literal}` cannot accept path segments"))?;
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
    let uri: Uri = url
        .as_str()
        .parse()
        .wrap_err("parse canonical app-api URI")?;
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .wrap_err("derive canonical auth timestamp")?
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let nonce = format!("nexus-app-api-{timestamp_ms}-{}", Hash::new(url.as_str()));
    let message = canonical_network_request_signature_message(
        client.client().network_id(),
        &Method::GET,
        &uri,
        &[],
        timestamp_ms,
        &nonce,
    )
    .wrap_err("construct canonical app-api request")?;
    let signature = Signature::try_new(client.client().key_pair().private_key(), &message)
        .wrap_err("sign canonical app-api request")?;
    let response = routed_http_client()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(HEADER_ACCOUNT, account.to_canonical_hex()?)
        .header(HEADER_SIGNATURE, signature_header_value(&signature)?)
        .header(HEADER_TIMESTAMP_MS, timestamp_ms.to_string())
        .header(HEADER_NONCE, nonce)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let response_context = routed_response_context(status, &headers, path_segments, query_pairs);
    let json_body = if routed_json_empty_body_is_transient(status, &body) {
        JsonValue::Null
    } else {
        norito::json::from_slice(&body)
            .wrap_err_with(|| format!("decode JSON body ({response_context}): {body_text}"))?
    };
    Ok(RoutedJsonResponse {
        permission_semantics: routed_header_string(
            &headers,
            "x-iroha-account-permission-semantics",
        ),
        status,
        body: json_body,
        body_text,
        routed_by: routed_header_string(&headers, "x-iroha-routed-by"),
        route_lane_id: routed_header_string(&headers, "x-iroha-route-lane-id"),
        route_dataspace_id: routed_header_string(&headers, "x-iroha-route-dataspace-id"),
    })
}
async fn wait_for_torii_json_get(
    client: &Client,
    path_segments: &[String],
    query_pairs: &[(String, String)],
    context: &str,
) -> Result<RoutedJsonResponse> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match torii_json_get(client, path_segments, query_pairs).await {
            Ok(response) if !routed_json_response_is_transient(&response) => return Ok(response),
            Ok(response) => {
                last_error = Some(format!(
                    "transient routed JSON response status {} body `{}`",
                    response.status, response.body_text
                ));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last routed JSON error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for routed JSON response{suffix}"
    ))
}
async fn wait_for_torii_json_get_as_account(
    client: &Client,
    account: &AccountId,
    path_segments: &[String],
    query_pairs: &[(String, String)],
    context: &str,
) -> Result<RoutedJsonResponse> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match torii_json_get_as_account(client, account, path_segments, query_pairs).await {
            Ok(response) if !routed_json_response_is_transient(&response) => return Ok(response),
            Ok(response) => {
                last_error = Some(format!(
                    "transient routed JSON response status {} body `{}`",
                    response.status, response.body_text
                ));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last routed JSON error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for routed JSON response{suffix}"
    ))
}
async fn submit_transaction_raw(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<RoutedTransactionSubmitResponse> {
    let http = routed_http_client();
    let request = http
        .post(
            client
                .client()
                .endpoint()
                .join("v1/pipeline/transactions")
                .wrap_err("compose /v1/pipeline/transactions URL")?,
        )
        .header(reqwest::header::CONTENT_TYPE, "application/x-norito")
        .body(encode_versioned_signed_transaction(transaction));
    let request = build_norito_receipt_request(add_client_headers(client, request, false, true))?;
    let response = http.execute(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    let body_text = String::from_utf8_lossy(&body).into_owned();
    let receipt = if status == HttpStatusCode::ACCEPTED {
        Some(decode_transaction_submission_receipt(&headers, &body)?)
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
fn build_norito_receipt_request(request: reqwest::RequestBuilder) -> Result<reqwest::Request> {
    let mut request = request.build()?;
    // Content-Type describes the submitted transaction; Torii negotiates the
    // receipt independently through Accept and otherwise defaults to JSON.
    // Replace inherited Accept values so exactly one binary format is selected.
    request.headers_mut().insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/x-norito"),
    );
    Ok(request)
}
fn decode_transaction_submission_receipt(
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
) -> Result<TransactionSubmissionReceipt> {
    let mut content_types = headers.get_all(reqwest::header::CONTENT_TYPE).iter();
    ensure!(
        content_types.next().and_then(|value| value.to_str().ok()) == Some("application/x-norito")
            && content_types.next().is_none(),
        "transaction receipt must declare exactly one application/x-norito Content-Type"
    );
    decode_from_bytes(body).wrap_err("decode negotiated canonical Norito transaction receipt")
}
async fn submit_transaction_and_expect_route(
    submitter: &Client,
    _confirmation_client: &Client,
    transaction: &SignedTransaction,
    expected_lane_id: LaneId,
    expected_dataspace_id: DataSpaceId,
    context: &str,
) -> Result<TransactionSubmissionReceipt> {
    let started = Instant::now();
    let response = loop {
        let response = submit_transaction_raw(submitter, transaction).await?;
        if response.status == HttpStatusCode::ACCEPTED
            || !routed_submit_response_is_transient(&response)
            || started.elapsed() >= STATUS_WAIT_TIMEOUT
        {
            break response;
        }
        sleep(STATUS_POLL_INTERVAL).await;
    };
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
fn manifest_response_contains_status(
    body: &JsonValue,
    dataspace_id: DataSpaceId,
    expected_status: &str,
    context: &str,
) -> Result<bool> {
    let manifests = body
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: manifest response missing items array"))?;
    Ok(manifests.iter().any(|record| {
        record.get("dataspace_id").and_then(JsonValue::as_u64) == Some(dataspace_id.as_u64())
            && record.get("status").and_then(JsonValue::as_str) == Some(expected_status)
    }))
}
fn manifest_response_contains_dataspace(
    body: &JsonValue,
    dataspace_id: DataSpaceId,
    context: &str,
) -> Result<bool> {
    let manifests = body
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: manifest response missing items array"))?;
    Ok(manifests.iter().any(|record| {
        record.get("dataspace_id").and_then(JsonValue::as_u64) == Some(dataspace_id.as_u64())
    }))
}
fn complete_fixture_collection_page(
    body: &JsonValue,
    limit: usize,
    context: &str,
) -> Result<iroha_torii_shared::list_query::Page<JsonValue>> {
    let page =
        norito::json::from_value::<iroha_torii_shared::list_query::Page<JsonValue>>(body.clone())
            .wrap_err_with(|| format!("{context}: invalid collection page"))?;
    ensure!(
        page.items.len() <= limit
            && page.next_cursor.is_none()
            && page.total == Some(u64::try_from(page.items.len())?),
        "{context}: expected a complete bounded collection page, got {body:?}"
    );
    Ok(page)
}

fn require_no_manifest_publish_permission(
    response: &RoutedJsonResponse,
    dataspace_id: DataSpaceId,
    context: &str,
) -> Result<()> {
    ensure!(
        response.status == HttpStatusCode::OK,
        "{context}: {}: {}",
        response.status,
        response.body_text
    );
    expect_proxy_route_headers(
        response,
        LaneId::new(NEXUS_LANE_INDEX),
        DataSpaceId::new(NEXUS_ID_U64),
        context,
    )?;
    ensure!(
        response.permission_semantics.as_deref() == Some("effective-v1"),
        "{context}: missing effective permission semantics: {:?}",
        response.permission_semantics
    );
    let page = complete_fixture_collection_page(&response.body, 64, context)?;
    for row in &page.items {
        ensure!(
            row.get("name").and_then(JsonValue::as_str) == Some("CanPublishSpaceDirectoryManifest")
                && row
                    .get("payload")
                    .and_then(|payload| payload.get("dataspace"))
                    .and_then(JsonValue::as_u64)
                    .is_some(),
            "{context}: invalid effective permission row for the requested filter: {row:?}"
        );
    }
    ensure!(
        !permission_response_contains(
            &response.body,
            "CanPublishSpaceDirectoryManifest",
            |payload| {
                payload.get("dataspace").and_then(JsonValue::as_u64) == Some(dataspace_id.as_u64())
            },
            context
        )?,
        "{context}: Bob has a manifest publish permission for data space {dataspace_id}"
    );
    Ok(())
}

fn require_empty_manifest_page(
    response: &RoutedJsonResponse,
    dataspace_id: DataSpaceId,
    context: &str,
) -> Result<()> {
    ensure!(
        response.status == HttpStatusCode::OK,
        "{context}: {}: {}",
        response.status,
        response.body_text
    );
    // The visible collection executes once. Its selected route and the requested row's
    // data space are separate fields.
    expect_proxy_route_headers(
        response,
        LaneId::new(NEXUS_LANE_INDEX),
        DataSpaceId::new(NEXUS_ID_U64),
        context,
    )?;
    let page = complete_fixture_collection_page(&response.body, 1, context)?;
    ensure!(
        page.items.is_empty(),
        "{context}: expected no manifests; target data space present={}, active={}, page={:?}",
        manifest_response_contains_dataspace(&response.body, dataspace_id, context)?,
        manifest_response_contains_status(&response.body, dataspace_id, "Active", context)?,
        response.body
    );
    Ok(())
}

async fn require_manifest_api_absence(
    alice: &Client,
    uaid_literal: &str,
    dataspace_id: DataSpaceId,
    deadline: Instant,
    context: &str,
) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "{context}: manifest observation deadline exhausted"
    );
    // Alice's signed genesis grants visibility into restricted data spaces. A public
    // or Bob-scoped empty collection alone would not establish absence.
    let response = tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        wait_for_torii_json_get_as_account(
            alice,
            &ALICE_ID,
            &[
                "v1".to_owned(),
                "space-directory".to_owned(),
                "uaids".to_owned(),
                uaid_literal.to_owned(),
                "manifests".to_owned(),
            ],
            &[
                (
                    "filter".to_owned(),
                    format!("dataspace_id = {}", dataspace_id.as_u64()),
                ),
                ("limit".to_owned(), "1".to_owned()),
                ("include_total".to_owned(), "true".to_owned()),
            ],
            context,
        ),
    )
    .await
    .wrap_err_with(|| format!("{context}: timed out reading manifest collection"))??;
    require_empty_manifest_page(&response, dataspace_id, context)?;
    ensure!(
        Instant::now() < deadline,
        "{context}: manifest observation returned late"
    );
    Ok(())
}

fn is_manifest_publish_permission_rejection(
    reason: &iroha_data_model::transaction::error::TransactionRejectionReason,
) -> bool {
    use iroha_data_model::{
        isi::error::InstructionExecutionError, transaction::error::TransactionRejectionReason,
    };
    matches!(reason, TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvariantViolation(message)
    )) if message.as_ref() == "not permitted: CanPublishSpaceDirectoryManifest")
}

fn require_manifest_publish_rejection(
    client: &Client,
    transaction: &SignedTransaction,
    deadline: Instant,
) -> Result<()> {
    let timeout = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| eyre!("manifest rejection deadline exhausted"))?;
    let observer = client.client().with_request_deadline(deadline);
    let error = observer
        .wait_for_transaction_applied(
            transaction.hash(),
            iroha::client::TransactionWaitOptions {
                timeout,
                poll_interval: STATUS_POLL_INTERVAL,
            },
        )
        .err()
        .ok_or_else(|| eyre!("unauthorized manifest publication unexpectedly applied"))?;
    let Some(failure) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<iroha::client::TransactionFinalityFailure>())
    else {
        return Err(
            error.wrap_err("manifest publication did not reach authoritative terminal rejection")
        );
    };
    failure.validate_for_hash(transaction.hash())?;
    let status = &failure.response().status;
    ensure!(
        status.kind == "Rejected" && status.block_height.is_some_and(|height| height > 0),
        "expected a committed manifest publication rejection, got {status:?}"
    );
    ensure!(
        Instant::now() < deadline,
        "manifest rejection details deadline exhausted"
    );
    let reason = observer.get_transaction_rejection_reason(transaction)?;
    ensure!(
        is_manifest_publish_permission_rejection(&reason),
        "unexpected native manifest rejection: {reason:?}"
    );
    ensure!(
        Instant::now() < deadline,
        "manifest rejection details arrived late"
    );
    Ok(())
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
fn require_private_holder_visibility(
    allowed: &RoutedJsonResponse,
    denied: &RoutedJsonResponse,
) -> Result<()> {
    ensure!(
        allowed.status == HttpStatusCode::OK,
        "private holders read failed: {} {}",
        allowed.status,
        allowed.body_text
    );
    expect_proxy_route_headers(
        allowed,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        "private holder route",
    )?;
    let page = complete_fixture_collection_page(&allowed.body, 1, "private holders")?;
    let [row] = page.items.as_slice() else {
        return Err(eyre!("expected one restricted holder"));
    };
    let partition = restricted_asset_partition();
    ensure!(
        row.get("id").and_then(JsonValue::as_str) == Some(partition.canonical_literal().as_str())
            && row.get("account_id").and_then(JsonValue::as_str)
                == Some(partition.account().to_string().as_str())
            && row.get("asset").and_then(JsonValue::as_str)
                == Some(partition.definition().to_string().as_str())
            && row.get("scope").and_then(JsonValue::as_str)
                == Some(format!("dataspace:{DS1_ID_U64}").as_str())
            && row.get("quantity").and_then(JsonValue::as_str) == Some("7"),
        "unexpected private balance partition: {row:?}"
    );
    ensure!(
        denied.status == HttpStatusCode::OK,
        "foreign partition query failed: {}: {}",
        denied.status,
        denied.body_text
    );
    expect_proxy_route_headers(
        denied,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        "foreign holder route",
    )?;
    let foreign = complete_fixture_collection_page(&denied.body, 1, "foreign holder page")?;
    ensure!(
        foreign.items.is_empty(),
        "foreign reader observed DS1 holder partitions"
    );
    Ok(())
}

fn expect_account_assets_page(
    response: &RoutedJsonResponse,
    holder: &AccountId,
    expected: &[(&AssetDefinitionId, &str)],
    hidden: Option<&AssetDefinitionId>,
    context: &str,
) -> Result<()> {
    ensure!(
        response.status == HttpStatusCode::OK,
        "{context}: expected a successful collection, observed {} body `{}`",
        response.status,
        response.body_text
    );
    // Every route here shares the global World. A non-authoritative observer
    // selects its lowest eligible execution route once; visibility still applies.
    expect_proxy_route_headers(response, LaneId::SINGLE, DataSpaceId::UNIVERSAL, context)?;
    ensure!(
        response.body.get("next_cursor") == Some(&JsonValue::Null),
        "{context}: fixture holdings must fit one complete page"
    );
    let rows = response
        .body
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: missing collection items"))?;
    ensure!(
        rows.len() <= ASSET_PAGE_LIMIT,
        "{context}: server exceeded requested limit"
    );
    let holder = holder.to_string();
    let mut seen = BTreeSet::new();
    for row in rows {
        let asset = row
            .get("asset")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: missing asset identity"))?;
        let scope = row
            .get("scope")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: missing balance scope"))?;
        ensure!(
            row.get("account_id").and_then(JsonValue::as_str) == Some(holder.as_str())
                && seen.insert((asset, scope)),
            "{context}: wrong holder or duplicate balance partition"
        );
    }
    for (asset, quantity) in expected {
        let asset = asset.to_string();
        ensure!(
            rows.iter()
                .any(
                    |row| row.get("asset").and_then(JsonValue::as_str) == Some(asset.as_str())
                        && row.get("scope").and_then(JsonValue::as_str) == Some("global")
                        && row.get("quantity").and_then(JsonValue::as_str) == Some(*quantity)
                ),
            "{context}: missing exact global balance {quantity} for {asset}"
        );
    }
    if let Some(hidden) = hidden {
        ensure!(
            !account_assets_response_contains(&response.body, hidden, context)?,
            "{context}: exposed restricted balance for {hidden}"
        );
    }
    Ok(())
}

#[test]
fn tx_query_cross_dataspace_routing_genesis_preexecution_smoke() {
    let _guard = sandbox::serial_guard();
    let network = localnet_builder().build();
    let genesis = network.genesis();
    let authority = AccountId::new(
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
            .public_key()
            .clone(),
    );
    iroha_core::block::check_genesis_block(&genesis.0, &authority)
        .expect("configured routing genesis authenticates after executing its staking plans");
    assert_eq!(
        genesis.0.da_proof_policies(),
        Some(&multilane_da_proof_policy_bundle()),
        "the configured three-lane policy is part of the authenticated genesis"
    );
    let instructions = genesis
        .0
        .external_transactions()
        .filter_map(|tx| match tx.instructions() {
            iroha_data_model::transaction::Executable::Instructions(instructions) => {
                Some(instructions.iter())
            }
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    let private = partitioned_asset_definition_id();
    let private_domain = partitioned_asset_domain();
    let signed_home = instructions
        .iter()
        .filter_map(|instruction| {
            match instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RegisterBox>()
            {
                Some(iroha_data_model::isi::RegisterBox::AssetDefinition(register))
                    if register.object().id() == &private =>
                {
                    Some(register.object().owning_domain.as_ref())
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(signed_home, [Some(&private_domain)]);
    let read_all = iroha_data_model::permission::Permission::from(
        iroha_executor_data_model::permission::query::CanReadAllLedgerData,
    );
    let readers = instructions
        .iter()
        .filter_map(|instruction| {
            match instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::GrantBox>()
            {
                Some(iroha_data_model::isi::GrantBox::Permission(grant))
                    if grant.object == read_all =>
                {
                    Some(grant.destination.clone())
                }
                _ => None,
            }
        })
        .collect::<BTreeSet<_>>();
    assert!(readers.contains(&*ALICE_ID));
    assert!(!readers.contains(&*BOB_ID));
    let policies = instructions
        .iter()
        .filter_map(|instruction| {
            let set = instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::SetParameter>()?;
            let iroha_data_model::parameter::Parameter::Custom(custom) = set.inner() else {
                return None;
            };
            use iroha_data_model::block::consensus::PublicLaneStakingPolicy;
            (custom.id() == &PublicLaneStakingPolicy::parameter_id()).then(|| {
                PublicLaneStakingPolicy::from_custom_parameter(custom)
                    .expect("canonical original signed staking policy")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        policies,
        vec![genesis_staking_policy()],
        "one signed staking policy preserves the fixture's XOR and ALICE custody"
    );
    let (epoch, _) = iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis.0)
        .expect("signed global genesis authority")
        .into_parts();
    assert_eq!(
        epoch.committee.len(),
        TOTAL_PEERS,
        "the global genesis roster retains thirteen validators"
    );
    let global_voters = epoch
        .committee
        .iter()
        .map(|member| member.validator.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        global_voters,
        network.validators().iter().map(|peer| peer.id()).collect(),
        "the original thirteen validators remain the entire authenticated global committee"
    );
    let permission = iroha_data_model::permission::Permission::from(CanManagePeers);
    let grants = instructions
        .iter()
        .filter(|instruction| {
            matches!(instruction.as_any().downcast_ref::<iroha_data_model::isi::GrantBox>(),
            Some(iroha_data_model::isi::GrantBox::Permission(grant))
            if grant.destination == validator_authority_account_for_peer(0)
                && grant.object == permission)
        })
        .count();
    assert_eq!(
        grants, 1,
        "one signed grant authorizes runtime observer registration"
    );
    let observers = network.observers();
    assert_eq!(observers.len(), INGRESS_OBSERVER_COUNT);
    for observer in observers {
        assert!(!global_voters.contains(&observer.id()));
        let registration = ingress_observer_registration(
            observer.id(),
            observer.bls_pop().expect("real observer PoP"),
        )
        .expect("runtime registry instruction proves the exact observer identity");
        assert_eq!(registration.peer, observer.id());
        assert!(registration.activation_at.is_none() && registration.expiry_at.is_none());
        assert!(
            !network
                .topology_entries()
                .iter()
                .any(|entry| entry.peer == observer.id())
        );
        assert!(
            network.config_layers().all(|layer| {
                layer
                    .get("trusted_peers_pop")
                    .and_then(TomlValue::as_array)
                    .is_none_or(|entries| {
                        entries.iter().all(|entry| {
                            entry.get("public_key").and_then(TomlValue::as_str)
                                != Some(observer.id().public_key().to_string().as_str())
                        })
                    })
            }),
            "ingress observers must never enter the validator PoP roster"
        );
        assert_eq!(
            network
                .config_layers_for_peer(observer)
                .last()
                .and_then(|layer| layer
                    .get("sumeragi")?
                    .get("role")?
                    .as_str()
                    .map(str::to_owned)),
            Some("observer".to_owned()),
            "both ingress processes must retain the non-voting role"
        );
    }
    let (alice_ingress, bob_ingress) = wrong_ingress_clients(&network).unwrap();
    assert_eq!(
        alice_ingress.client().endpoint(),
        &reqwest::Url::parse(&observers[DS2_INGRESS_OBSERVER_INDEX].torii_url()).unwrap()
    );
    assert_eq!(
        bob_ingress.client().endpoint(),
        &reqwest::Url::parse(&observers[DS1_INGRESS_OBSERVER_INDEX].torii_url()).unwrap()
    );
    assert_ne!(
        alice_ingress.client().endpoint(),
        bob_ingress.client().endpoint()
    );
    assert_eq!(
        alice_ingress.client().network_id(),
        bob_ingress.client().network_id()
    );
    assert_eq!(
        alice_ingress.client().key_pair().public_key(),
        ALICE_KEYPAIR.public_key()
    );
    assert_eq!(
        bob_ingress.client().key_pair().public_key(),
        BOB_KEYPAIR.public_key()
    );
    let npos = instructions
        .iter()
        .filter_map(|instruction| {
            let set = instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::SetParameter>()?;
            let iroha_data_model::parameter::Parameter::Custom(custom) = set.inner() else {
                return None;
            };
            iroha_data_model::parameter::system::SumeragiNposParameters::from_custom_parameter(
                custom,
            )
            .expect("valid signed parameters")
        })
        .collect::<Vec<_>>();
    assert_eq!(npos.len(), 1, "one original network XOR selection");
    assert_eq!(
        npos[0].max_validators(),
        u32::try_from(TOTAL_PEERS).unwrap()
    );
    assert_eq!(npos[0].epoch_length_blocks().get(), 3_600);
    assert_eq!(npos[0].xor_asset_definition_id, stake_asset_definition_id());
    let policies = instructions
        .iter()
        .filter_map(|instruction| {
            let set = instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::SetParameter>()?;
            let iroha_data_model::parameter::Parameter::Custom(custom) = set.inner() else {
                return None;
            };
            SumeragiLanePolicy::from_custom_parameter(custom)
                .map(|policy| policy.expect("signed lane policy"))
        })
        .collect::<Vec<_>>();
    let topology = network
        .peers()
        .iter()
        .map(|peer| peer.id())
        .collect::<Vec<_>>();
    assert_eq!(
        policies,
        vec![restricted_lane_policy(
            &topology,
            network.topology_entries()
        )]
    );
    for fixed in &policies[0].fixed {
        assert_eq!(fixed.committee.len(), VALIDATORS_PER_LANE);
        assert!(fixed.committee.iter().all(|member| {
            global_voters.contains(&member.peer)
                && observers
                    .iter()
                    .all(|observer| observer.id() != member.peer)
        }));
    }
    assert_eq!(
        iroha_genesis::signed_genesis_consensus_metadata(&genesis.0)
            .unwrap()
            .mode,
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    );
    assert_eq!(
        iroha_genesis::signed_genesis_root_fee_policy(&genesis.0)
            .expect("signed routing fee policy"),
        iroha_config::parameters::defaults::nexus::fees::genesis_policy(),
    );
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
    wait_for_active_lane_validators(
        &alice,
        LaneId::new(NEXUS_LANE_INDEX),
        &expected_nexus_validators,
        "nexus lane validator activation",
    )?;
    // Restricted membership is deliberately absent from the public endpoint.
    // Check its exact registration/activation in the accepted signed genesis;
    // the transactions and queries below prove live routing and isolation.
    let genesis = network.genesis();
    let mut signed_instructions = Vec::new();
    for transaction in genesis.0.external_transactions() {
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            return Err(eyre!(
                "routing genesis must contain instruction transactions"
            ));
        };
        signed_instructions.extend(instructions.iter());
    }
    for (lane, expected) in [
        (DS1_LANE_INDEX, &expected_ds1_validators),
        (DS2_LANE_INDEX, &expected_ds2_validators),
    ] {
        ensure!(
            genesis_active_lane_bindings(signed_instructions.iter().copied(), LaneId::new(lane))?
                == *expected,
            "restricted lane {lane} must carry its exact signed validator bindings"
        );
    }
    // Native lanes created in genesis become active at height three. Use the
    // same two paid global transactions to register the observer ingress peers.
    // Original-genesis RegisterPeerWithPop would declare extra global voters;
    // runtime registration only creates registry candidates. ALICE/BOB's
    // committed routes remain reserved for their restricted lanes.
    let activation_key = KeyPair::try_from_seed(validator_authority_seed(0), Algorithm::Ed25519)?;
    let activation_client = peers[0].client_for(
        &validator_authority_account_for_peer(0),
        activation_key.private_key().clone(),
    );
    for observer in network.observers() {
        // The SDK quotes this exact instruction, signs it, and waits for Applied.
        activation_client.submit(
            ingress_observer_registration(
                observer.id(),
                observer
                    .bls_pop()
                    .expect("ingress observer has its real PoP"),
            )?,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    let expected_registry = network
        .all_peers()
        .map(|peer| peer.id())
        .collect::<BTreeSet<_>>();
    ensure!(expected_registry.len() == TOTAL_PEERS + INGRESS_OBSERVER_COUNT);
    let topology = peers.iter().map(|peer| peer.id()).collect::<Vec<_>>();
    let policy = restricted_lane_policy(&topology, network.topology_entries());
    wait_for_restricted_lane_instances(&alice, &policy)?;
    // Restricted membership is deliberately absent from the public API, even
    // though the operator-authenticated endpoint verifies the exact committee.
    for lane in [DS1_LANE_INDEX, DS2_LANE_INDEX] {
        let hidden = rt.block_on(torii_json_get(
            &alice,
            &[
                "v1".into(),
                "nexus".into(),
                "public-lanes".into(),
                lane.to_string(),
                "validators".into(),
            ],
            &[],
        ))?;
        ensure!(
            hidden.status == HttpStatusCode::NOT_FOUND,
            "restricted lane membership leaked: {} {}",
            hidden.status,
            hidden.body_text
        );
        ensure!(
            hidden.body.get("code").and_then(JsonValue::as_str) == Some("query_validation_failed"),
            "unexpected restricted lane refusal: {}",
            hidden.body_text
        );
    }
    let lane_sync_height = alice
        .client()
        .get_sumeragi_status()
        .map_err(|err| eyre!(err))?
        .committed_height;
    wait_for_height(
        &bob,
        lane_sync_height,
        "lane validator activation propagation on bob",
    )?;
    ensure!(
        (VALIDATORS_PER_LANE * 2..LANE_VALIDATOR_COUNT).contains(&DS2_VALIDATOR_INDEX),
        "ds2 authoritative validator must retain its native lane seat"
    );
    ensure!(
        (VALIDATORS_PER_LANE..VALIDATORS_PER_LANE * 2).contains(&DS1_VALIDATOR_INDEX),
        "ds1 authoritative validator must retain its native lane seat"
    );
    let (alice_via_ds2, bob_via_ds1) = wrong_ingress_clients(&network)?;
    let alice_on_ds1 =
        peers[DS1_VALIDATOR_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let alice_on_ds2 =
        peers[DS2_VALIDATOR_INDEX].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
    let bob_on_ds2 =
        peers[DS2_VALIDATOR_INDEX].client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
    // Torii's applied height proves both ingress States have installed the
    // active routes; a downloaded Kura block or consensus tip is insufficient.
    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + STATUS_WAIT_TIMEOUT;
        let mut latest = [0_u64; INGRESS_OBSERVER_COUNT];
        loop {
            let mut delay = STATUS_POLL_INTERVAL;
            let mut complete_snapshot = true;
            for (index, client) in [&bob_via_ds1, &alice_via_ds2].into_iter().enumerate() {
                ingress_readiness_before_deadline(
                    &latest,
                    lane_sync_height,
                    tokio::time::Instant::now(),
                    deadline,
                )?;
                let status = tokio::time::timeout_at(deadline, client.client().status().get())
                    .await
                    .wrap_err_with(|| format!("ingress applied readiness timed out: {latest:?}"))?;
                ingress_readiness_before_deadline(
                    &latest,
                    lane_sync_height,
                    tokio::time::Instant::now(),
                    deadline,
                )?;
                match status {
                    Ok(status) => latest[index] = status.blocks,
                    Err(error) => match ingress_status_retry_delay(&error) {
                        Some(retry_delay) => {
                            complete_snapshot = false;
                            delay = delay.max(retry_delay);
                        }
                        None => return Err(eyre!(error)),
                    },
                }
            }
            let ready = ingress_readiness_before_deadline(
                &latest,
                lane_sync_height,
                tokio::time::Instant::now(),
                deadline,
            )?;
            if complete_snapshot && ready {
                // Every possible authoritative peer must have the exact registered
                // ingress identities before any delegated read can be dispatched.
                // Share the original readiness deadline, including all query I/O.
                let mut registered_everywhere = true;
                for peer in network.all_peers() {
                    ingress_readiness_before_deadline(
                        &latest,
                        lane_sync_height,
                        tokio::time::Instant::now(),
                        deadline,
                    )?;
                    let client = peer.client();
                    let account = client.account_client();
                    let mut rows = tokio::time::timeout_at(
                        deadline,
                        account
                            .query(FindPeers)
                            .with_fetch_size(FetchSize {
                                fetch_size: NonZeroU64::new(
                                    u64::try_from(TOTAL_PEERS + INGRESS_OBSERVER_COUNT)
                                        .expect("fixture peer count fits u64"),
                                ),
                            })
                            .execute(),
                    )
                    .await
                    .wrap_err("ingress registry readiness timed out")??;
                    ensure!(
                        !rows.has_more(),
                        "ingress registry query must be one complete bounded page"
                    );
                    let mut registry = Vec::new();
                    while let Some(row) = rows.next().await {
                        registry.push(row?);
                        ensure!(
                            registry.len() <= TOTAL_PEERS + INGRESS_OBSERVER_COUNT,
                            "ingress registry exceeded the fixture's exact bound"
                        );
                    }
                    ingress_readiness_before_deadline(
                        &latest,
                        lane_sync_height,
                        tokio::time::Instant::now(),
                        deadline,
                    )?;
                    registered_everywhere &=
                        ingress_registry_matches(&registry, &expected_registry);
                    let operator = client.client().with_request_deadline(deadline.into_std());
                    let status = tokio::time::timeout_at(
                        deadline,
                        integration_tests::sync::run_on_dedicated_thread(move || {
                            operator.get_sumeragi_status()
                        }),
                    )
                    .await
                    .wrap_err("ingress consensus role observation timed out")???;
                    ingress_readiness_before_deadline(
                        &latest,
                        lane_sync_height,
                        tokio::time::Instant::now(),
                        deadline,
                    )?;
                    let expected_signer = network
                        .validators()
                        .iter()
                        .any(|voter| voter.id() == peer.id())
                        .then(|| peer.id().public_key().clone());
                    ensure!(
                        status.halted.is_none() && status.signer == expected_signer,
                        "peer registration changed the current signing role for {}",
                        peer.id()
                    );
                }
                if registered_everywhere {
                    ingress_readiness_before_deadline(
                        &latest,
                        lane_sync_height,
                        tokio::time::Instant::now(),
                        deadline,
                    )?;
                    return Ok::<(), eyre::Report>(());
                }
            }
            tokio::time::sleep_until(
                tokio::time::Instant::now()
                    .checked_add(delay)
                    .unwrap_or(deadline)
                    .min(deadline),
            )
            .await;
        }
    })?;
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
    let ds1_asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds1coin".parse().expect("asset definition name"),
    );
    let ds2_asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds2coin".parse().expect("asset definition name"),
    );
    let alice_ds1_asset = AssetId::new(ds1_asset_definition_id.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_definition_id.clone(), BOB_ID.clone());
    rt.block_on(async {
        let alice_probe = {
            let account = alice_via_ds2.account_client();
            let mut payload =
                account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                    [InstructionBox::from(Log::new(
                        Level::INFO,
                        "wrong ingress route probe ds1".to_owned(),
                    ))],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))?;
            let quote = account
                .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
                .await?;
            ensure!(
                payload
                    .fee_payment
                    .has_same_payer_and_gas_bound(&quote.intent),
                "route-probe quote changed payer or gas bound"
            );
            payload.fee_payment = quote.intent;
            account.sign_transaction(payload)?
        };
        submit_transaction_and_expect_route(
            &alice_via_ds2,
            &alice_on_ds1,
            &alice_probe,
            ds1_lane_id,
            ds1_dataspace_id,
            "alice tx via ds2 should route to ds1",
        )
        .await?;
        let bob_probe = {
            let account = bob_via_ds1.account_client();
            let mut payload =
                account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                    [InstructionBox::from(Log::new(
                        Level::INFO,
                        "wrong ingress route probe ds2".to_owned(),
                    ))],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))?;
            let quote = account
                .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
                .await?;
            ensure!(
                payload
                    .fee_payment
                    .has_same_payer_and_gas_bound(&quote.intent),
                "route-probe quote changed payer or gas bound"
            );
            payload.fee_payment = quote.intent;
            account.sign_transaction(payload)?
        };
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
        asset_balance(&alice_via_ds2, &alice_ds1_asset)? == Quantity::from(100_u32),
        "alice signed query through ds2 ingress did not route to ds1"
    );
    ensure!(
        asset_balance(&bob_via_ds1, &bob_ds2_asset)? == Quantity::from(200_u32),
        "bob signed query through ds1 ingress did not route to ds2"
    );
    let alice_account = rt.block_on(wait_for_torii_json_get(
        &alice_via_ds2,
        &["v1".to_owned(), "accounts".to_owned(), ALICE_ID.to_string()],
        &[],
        "alice account GET through ds2 ingress",
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
    let asset_page = [("limit".to_owned(), ASSET_PAGE_LIMIT.to_string())];
    let alice_assets = rt.block_on(wait_for_torii_json_get_as_account(
        &alice_via_ds2,
        &ALICE_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            ALICE_ID.to_string(),
            "assets".to_owned(),
        ],
        &asset_page,
        "alice assets query through ds2 ingress",
    ))?;
    let private_asset = partitioned_asset_definition_id();
    expect_account_assets_page(
        &alice_assets,
        &ALICE_ID,
        &[(&ds1_asset_definition_id, "100")],
        None,
        "alice assets query through ds2 ingress",
    )?;
    let bob_assets = rt.block_on(wait_for_torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            BOB_ID.to_string(),
            "assets".to_owned(),
        ],
        &asset_page,
        "bob assets query through ds1 ingress",
    ))?;
    expect_account_assets_page(
        &bob_assets,
        &BOB_ID,
        &[(&ds2_asset_definition_id, "200")],
        None,
        "bob assets query through ds1 ingress",
    )?;
    let alice_assets_as_bob = rt.block_on(wait_for_torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            ALICE_ID.to_string(),
            "assets".to_owned(),
        ],
        &asset_page,
        "alice assets query as bob through ds1 ingress",
    ))?;
    // Both accounts are bound to the universal data space. Bob can read
    // Alice's public holding through the same observer used for his own page.
    expect_account_assets_page(
        &alice_assets_as_bob,
        &ALICE_ID,
        &[(&ds1_asset_definition_id, "100")],
        Some(&private_asset),
        "alice assets query as bob through ds1 ingress",
    )?;
    // Unlike the account list's selector-bound balance routes, the holders
    // collection uses the signer's current definition and partition visibility.
    let holders_path = [
        "v1".to_owned(),
        "assets".to_owned(),
        private_asset.to_string(),
        "holders".to_owned(),
    ];
    let holders_query = [
        ("limit".to_owned(), "1".to_owned()),
        ("include_total".to_owned(), "true".to_owned()),
    ];
    let holder_observer = &network.observers()[DS2_INGRESS_OBSERVER_INDEX];
    let bob_via_ds2 = holder_observer.client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
    let allowed = rt.block_on(wait_for_torii_json_get_as_account(
        &alice_via_ds2,
        &ALICE_ID,
        &holders_path,
        &holders_query,
        "private holders as Alice",
    ))?;
    let denied = rt.block_on(wait_for_torii_json_get_as_account(
        &bob_via_ds2,
        &BOB_ID,
        &holders_path,
        &holders_query,
        "private holders as Bob",
    ))?;
    require_private_holder_visibility(&allowed, &denied)?;
    let alice_via_ds1 = network.observers()[DS1_INGRESS_OBSERVER_INDEX]
        .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
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
                max_amount: Some(Quantity::from(1_u32)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("wrong ingress manifest routing regression".to_owned()),
        }],
    };
    rt.block_on(require_manifest_api_absence(
        &alice_via_ds1,
        &manifest_uaid_literal,
        ds2_dataspace_id,
        Instant::now() + STATUS_WAIT_TIMEOUT,
        "initial manifest read through ds1 ingress",
    ))?;
    let bob_manifest_permissions = rt.block_on(wait_for_torii_json_get_as_account(
        &bob_via_ds1,
        &BOB_ID,
        &[
            "v1".to_owned(),
            "accounts".to_owned(),
            BOB_ID.to_string(),
            "permissions".to_owned(),
        ],
        &[
            (
                "filter".to_owned(),
                r#"name = "CanPublishSpaceDirectoryManifest""#.to_owned(),
            ),
            ("limit".to_owned(), "64".to_owned()),
            ("include_total".to_owned(), "true".to_owned()),
        ],
        "Bob's effective manifest permissions through ds1 ingress",
    ))?;
    require_no_manifest_publish_permission(
        &bob_manifest_permissions,
        ds2_dataspace_id,
        "Bob's effective manifest permissions",
    )?;
    let unauthorized_publish_tx = rt.block_on(async {
        let account = bob_via_ds1.account_client();
        let mut payload =
            account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [InstructionBox::from(PublishSpaceDirectoryManifest {
                    manifest: ds2_manifest.clone(),
                })],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))?;
        let quote = account
            .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
            .await?;
        ensure!(
            payload
                .fee_payment
                .has_same_payer_and_gas_bound(&quote.intent),
            "manifest quote changed payer or gas bound"
        );
        payload.fee_payment = quote.intent;
        Ok::<_, eyre::Report>(account.sign_transaction(payload)?)
    })?;
    rt.block_on(submit_transaction_and_expect_route(
        &bob_via_ds1,
        &bob_on_ds2,
        &unauthorized_publish_tx,
        ds2_lane_id,
        ds2_dataspace_id,
        "bob unauthorized manifest publish via ds1 ingress should reject on ds2",
    ))?;
    let rejection_deadline = Instant::now() + STATUS_WAIT_TIMEOUT;
    require_manifest_publish_rejection(&bob_on_ds2, &unauthorized_publish_tx, rejection_deadline)?;
    rt.block_on(require_manifest_api_absence(
        &alice_via_ds1,
        &manifest_uaid_literal,
        ds2_dataspace_id,
        rejection_deadline,
        "manifest must remain absent after Bob's unauthorized publication",
    ))?;
    ensure!(
        alice
            .client()
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .committed_height
            < 3_600,
        "routing coverage must remain within the initial signed global committee epoch"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{
        ALICE_ID, ALICE_KEYPAIR, AccountId, Algorithm, AssetDefinitionId, BlockHeader, DS1_ID_U64,
        DS1_LANE_INDEX, DS1_MANIFEST_HASH, DS2_ID_U64, DS2_LANE_INDEX, DS2_MANIFEST_HASH, DomainId,
        ExpectedLaneValidatorBinding, Hash, KeyPair, LANE_VALIDATOR_COUNT, Level, Log, NEXUS_ALIAS,
        NEXUS_ID_U64, NEXUS_LANE_INDEX, NetworkId, RegisterConsensusKey,
        RegisterPublicLaneValidator, RoutedJsonResponse, RoutedTransactionSubmitResponse,
        SignedTransaction, TOTAL_PEERS, VALIDATORS_PER_LANE, account_assets_response_contains,
        derive_committee_key_id, encode_versioned_signed_transaction, expect_proxy_fanout_headers,
        expect_proxy_route_headers, expected_lane_binding_for_peer, lane_validator_snapshot,
        manifest_response_contains_dataspace, manifest_response_contains_status,
        multilane_da_proof_policy_bundle, nexus_fee_asset_definition_id,
        npos_multilane_genesis_post_topology_transactions, permission_response_contains,
        routed_header_string, routed_json_empty_body_is_transient,
        routed_json_response_is_transient, routed_response_context, routing_probe_gas_account_id,
        stake_asset_definition_id, stake_asset_id_literal, validator_authority_account_for_peer,
        validator_authority_seed,
    };
    use iroha::data_model::{
        da::commitment::{DaProofPolicyBundle, DaProofScheme},
        transaction::TransactionBuilder,
    };
    use iroha_model_base::peer::PeerId;
    use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
    use norito::{core::DecodeFromSlice, json::Value as JsonValue};
    use reqwest::{
        StatusCode as HttpStatusCode,
        header::{HeaderMap, HeaderValue},
    };
    use std::panic;
    fn routed_json_response(
        status: HttpStatusCode,
        body: JsonValue,
        body_text: &str,
    ) -> RoutedJsonResponse {
        RoutedJsonResponse {
            permission_semantics: None,
            status,
            body,
            body_text: body_text.to_owned(),
            routed_by: None,
            route_lane_id: None,
            route_dataspace_id: None,
        }
    }
    fn routed_json_response_with_route(
        routed_by: Option<&str>,
        route_lane_id: Option<&str>,
        route_dataspace_id: Option<&str>,
    ) -> RoutedJsonResponse {
        RoutedJsonResponse {
            permission_semantics: None,
            status: HttpStatusCode::OK,
            body: JsonValue::Null,
            body_text: String::new(),
            routed_by: routed_by.map(ToOwned::to_owned),
            route_lane_id: route_lane_id.map(ToOwned::to_owned),
            route_dataspace_id: route_dataspace_id.map(ToOwned::to_owned),
        }
    }
    fn ds2_asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("nexus", "universal").expect("asset domain"),
            "ds2coin".parse().expect("asset name"),
        )
    }
    fn deterministic_genesis_topology(
        peer_count: usize,
    ) -> (Vec<PeerId>, Vec<iroha_genesis::GenesisTopologyEntry>) {
        let entries = (0..peer_count)
            .map(|index| {
                let mut seed = vec![0_u8; 32];
                seed[0] = 0xE2;
                seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
                let key_pair = KeyPair::try_from_seed(seed, Algorithm::BlsNormal)
                    .expect("fixture tx/query routing genesis peer key");
                iroha_genesis::GenesisTopologyEntry::new(
                    PeerId::new(key_pair.public_key().clone()),
                    iroha_crypto::bls_normal_pop_prove(key_pair.private_key())
                        .expect("fixture tx/query routing genesis peer PoP"),
                )
            })
            .collect::<Vec<_>>();
        let topology = entries.iter().map(|entry| entry.peer.clone()).collect();
        (topology, entries)
    }
    fn decode_manifest_hash_fixture(raw: &str) -> [u8; 32] {
        assert_eq!(raw.len(), 64);
        let mut hash = [0_u8; 32];
        for (idx, chunk) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let pair = std::str::from_utf8(chunk).expect("hex pair");
            hash[idx] = u8::from_str_radix(pair, 16).expect("manifest hash hex");
        }
        hash
    }
    #[test]
    fn dataspace_fixture_manifest_hashes_derive_config_ids() {
        let ds1_hash = decode_manifest_hash_fixture(DS1_MANIFEST_HASH);
        let ds2_hash = decode_manifest_hash_fixture(DS2_MANIFEST_HASH);
        assert_eq!(NEXUS_ALIAS, "universal");
        assert_eq!(NEXUS_ID_U64, DataSpaceId::UNIVERSAL.as_u64());
        assert_eq!(DataSpaceId::from_hash(&ds1_hash).as_u64(), DS1_ID_U64);
        assert_eq!(DataSpaceId::from_hash(&ds2_hash).as_u64(), DS2_ID_U64);
        assert_ne!(DataSpaceId::from_hash(&ds1_hash), DataSpaceId::UNIVERSAL);
        assert_ne!(DataSpaceId::from_hash(&ds2_hash), DataSpaceId::UNIVERSAL);
    }
    #[test]
    fn fixture_asset_helpers_use_one_real_network_xor() {
        let stake_definition_id = stake_asset_definition_id();
        let fee_definition_id = nexus_fee_asset_definition_id();
        assert_eq!(stake_asset_id_literal(), stake_definition_id.to_string());
        assert_eq!(stake_definition_id, fee_definition_id);
        let policy = super::genesis_staking_policy();
        policy
            .validate()
            .expect("valid original routing staking policy");
        assert_eq!(policy.stake_asset_id, stake_definition_id);
        assert_eq!(
            policy.stake_escrow_account_id,
            routing_probe_gas_account_id()
        );
        assert_eq!(policy.slash_sink_account_id, routing_probe_gas_account_id());
        assert_eq!(
            stake_definition_id.to_string(),
            "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        );
    }
    #[test]
    fn multilane_da_policy_bundle_matches_wrong_ingress_topology() {
        let bundle = multilane_da_proof_policy_bundle();
        let expected_hash = DaProofPolicyBundle::new(bundle.policies().to_vec()).policy_hash();
        assert_eq!(bundle.version(), DaProofPolicyBundle::VERSION_V1);
        assert_eq!(bundle.policy_hash(), expected_hash);
        assert_eq!(bundle.policies().len(), 3);
        assert_eq!(bundle.policies()[0].lane_id.as_u32(), NEXUS_LANE_INDEX);
        assert_eq!(bundle.policies()[0].dataspace_id.as_u64(), NEXUS_ID_U64);
        assert_eq!(bundle.policies()[0].alias, "lane-nexus");
        assert_eq!(
            bundle.policies()[0].proof_scheme,
            DaProofScheme::MerkleSha256
        );
        assert_eq!(bundle.policies()[1].lane_id.as_u32(), DS1_LANE_INDEX);
        assert_eq!(bundle.policies()[1].dataspace_id.as_u64(), DS1_ID_U64);
        assert_eq!(bundle.policies()[1].alias, "lane-ds1");
        assert_eq!(bundle.policies()[2].lane_id.as_u32(), DS2_LANE_INDEX);
        assert_eq!(bundle.policies()[2].dataspace_id.as_u64(), DS2_ID_U64);
        assert_eq!(bundle.policies()[2].alias, "lane-ds2");
    }
    #[test]
    fn multilane_da_policy_bundle_hash_changes_when_policy_order_changes() {
        let bundle = multilane_da_proof_policy_bundle();
        let mut reversed_policies = bundle.policies().to_vec();
        reversed_policies.reverse();
        let reversed_hash = DaProofPolicyBundle::new(reversed_policies).policy_hash();
        assert_eq!(bundle, multilane_da_proof_policy_bundle());
        assert_ne!(bundle.policy_hash(), reversed_hash);
    }
    #[test]
    fn restricted_lane_public_snapshot_requires_the_exact_hidden_response() {
        let hidden = norito::json!({
            "code": "query_validation_failed",
            "message": "Query not found in the live query store."
        });
        let response = |status, body| RoutedJsonResponse {
            status,
            body,
            body_text: String::new(),
            routed_by: None,
            route_lane_id: None,
            route_dataspace_id: None,
        };
        assert!(
            super::expect_restricted_lane_hidden(&response(
                HttpStatusCode::NOT_FOUND,
                hidden.clone()
            ))
            .is_ok()
        );
        for (status, body) in [
            (HttpStatusCode::OK, hidden.clone()),
            (HttpStatusCode::SERVICE_UNAVAILABLE, hidden),
            (
                HttpStatusCode::NOT_FOUND,
                norito::json!({"code": "route_unavailable"}),
            ),
            (
                HttpStatusCode::NOT_FOUND,
                norito::json!({
                    "code": "query_validation_failed",
                    "message": "Query not found in the live query store.",
                    "items": []
                }),
            ),
        ] {
            assert!(super::expect_restricted_lane_hidden(&response(status, body)).is_err());
        }
    }
    #[test]
    fn genesis_lane_bindings_require_exact_ordered_registration_and_activation() {
        let (topology, entries) = deterministic_genesis_topology(TOTAL_PEERS);
        let transactions = npos_multilane_genesis_post_topology_transactions(&topology, &entries);
        for (lane, offset) in [
            (NEXUS_LANE_INDEX, 0),
            (DS1_LANE_INDEX, 4),
            (DS2_LANE_INDEX, 8),
        ] {
            let expected = topology
                .iter()
                .enumerate()
                .skip(offset)
                .take(VALIDATORS_PER_LANE)
                .map(|(index, peer)| expected_lane_binding_for_peer(index, peer))
                .collect();
            assert_eq!(
                super::genesis_active_lane_bindings(
                    transactions.iter().flatten(),
                    LaneId::new(lane)
                )
                .unwrap(),
                expected
            );
        }
        let lane = LaneId::new(DS1_LANE_INDEX);
        let activation_index = transactions[0]
            .iter()
            .position(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<super::ActivatePublicLaneValidator>()
                    .is_some_and(|activation| activation.lane_id == lane)
            })
            .unwrap();
        let mut missing = transactions[0].clone();
        let activation = missing.remove(activation_index);
        assert!(super::genesis_active_lane_bindings(&missing, lane).is_err());
        let mut duplicate = transactions[0].clone();
        duplicate.push(activation.clone());
        assert!(super::genesis_active_lane_bindings(&duplicate, lane).is_err());
        let mut early = transactions[0].clone();
        early.remove(activation_index);
        early.insert(0, activation);
        assert!(super::genesis_active_lane_bindings(&early, lane).is_err());
        assert!(
            super::genesis_active_lane_bindings(transactions.iter().flatten(), LaneId::new(99))
                .is_err()
        );
    }
    #[test]
    fn genesis_post_topology_builder_requires_full_wrong_ingress_roster() {
        let (topology, entries) = deterministic_genesis_topology(TOTAL_PEERS);
        let transactions = npos_multilane_genesis_post_topology_transactions(&topology, &entries);
        assert_eq!(transactions.len(), 3);
        let private_domain = super::partitioned_asset_domain();
        let private_definition = super::partitioned_asset_definition_id();
        let [definition] = transactions[1].as_slice() else {
            panic!("only the public asset definition belongs to its registration transaction");
        };
        use iroha_data_model::isi::{MintBox, RegisterBox};
        assert!(matches!(
            definition.as_any().downcast_ref::<RegisterBox>(), Some(RegisterBox::AssetDefinition(registration))
                if registration.object().id == private_definition
                && registration.object().owning_domain.as_ref() == Some(&private_domain)
                && registration.object().balance_scope_policy == iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted
        ));
        let [mint] = transactions[2].as_slice() else {
            panic!("the restricted balance must follow its definition transaction");
        };
        assert!(
            matches!(mint.as_any().downcast_ref::<MintBox>(), Some(MintBox::Asset(mint))
                if mint.object == iroha_primitives::numeric::Quantity::from(7_u32)
                && mint.destination == super::restricted_asset_partition()
            )
        );
        assert_eq!(
            transactions[0].len(),
            13 + LANE_VALIDATOR_COUNT * 5 + (LANE_VALIDATOR_COUNT - VALIDATORS_PER_LANE + 2)
        );
        let grants = transactions[0]
            .iter()
            .filter_map(|instruction| {
                match instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::GrantBox>()
                {
                    Some(iroha_data_model::isi::GrantBox::Permission(grant)) => {
                        Some((grant.destination.clone(), grant.object.clone()))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        let genesis_authority = AccountId::new(
            iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
                .public_key()
                .clone(),
        );
        let consensus_key_permission = iroha_data_model::permission::Permission::from(
            iroha_executor_data_model::permission::governance::CanManageConsensusKeys,
        );
        // The shared Committee-key helper grants then revokes the genesis
        // authority's temporary capability. The final grant belongs only to
        // the funded global activation signer that registers the observers.
        assert_eq!(
            grants,
            vec![
                (genesis_authority.clone(), consensus_key_permission.clone()),
                (
                    validator_authority_account_for_peer(0),
                    iroha_data_model::permission::Permission::from(super::CanManagePeers)
                ),
            ],
            "the complete grant list must retain both exact authorities and capabilities"
        );
        let revocations = transactions[0]
            .iter()
            .filter_map(|instruction| {
                match instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::RevokeBox>()
                {
                    Some(iroha_data_model::isi::RevokeBox::Permission(revoke)) => {
                        Some((revoke.destination().clone(), revoke.object().clone()))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            revocations,
            vec![(genesis_authority, consensus_key_permission)],
            "genesis key management must be revoked; observer ingress management remains granted"
        );
        let committee_registrations = transactions[0]
            .iter()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterConsensusKey>())
            .collect::<Vec<_>>();
        assert_eq!(
            committee_registrations.len(),
            LANE_VALIDATOR_COUNT - VALIDATORS_PER_LANE
        );
        for (registration, peer) in committee_registrations
            .iter()
            .zip(&topology[VALIDATORS_PER_LANE..LANE_VALIDATOR_COUNT])
        {
            assert_eq!(registration.id, derive_committee_key_id(peer.public_key()));
        }
        let lane_registrations = transactions[0]
            .iter()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
            })
            .collect::<Vec<_>>();
        assert_eq!(lane_registrations.len(), LANE_VALIDATOR_COUNT);
        for registration in &lane_registrations {
            assert_eq!(
                registration.monetary_plan.network_scope,
                iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Genesis
            );
            assert_eq!(registration.monetary_plan.valid_until_height, 1);
            assert_eq!(
                registration.monetary_plan.source_asset,
                iroha_data_model::asset::AssetId::new(
                    stake_asset_definition_id(),
                    registration.validator.clone()
                )
            );
            assert_eq!(
                registration.monetary_plan.destination_asset,
                iroha_data_model::asset::AssetId::new(
                    stake_asset_definition_id(),
                    routing_probe_gas_account_id()
                )
            );
            assert_eq!(
                registration.monetary_plan.amount,
                registration.initial_stake
            );
            assert_eq!(
                registration.monetary_plan.precondition,
                iroha_data_model::nexus::PublicLaneMonetaryPreconditionV1::Registration(
                    iroha_data_model::nexus::PublicLaneMonetaryRegistrationV1 {
                        activation_height: 1
                    }
                )
            );
        }

        assert!(
            lane_registrations
                .iter()
                .all(|register| register.peer_id != topology[LANE_VALIDATOR_COUNT]),
            "the thirteenth global voter must not become a fifth lane validator"
        );
        assert_eq!(
            expected_lane_binding_for_peer(0, &topology[0]).peer_id,
            topology[0].to_string()
        );
        let (short_topology, short_entries) = deterministic_genesis_topology(TOTAL_PEERS - 1);
        let result = panic::catch_unwind(|| {
            npos_multilane_genesis_post_topology_transactions(&short_topology, &short_entries)
        });
        assert!(result.is_err());
    }
    #[test]
    fn observer_registration_requires_the_exact_peer_proof_of_possession() {
        let (peers, entries) = deterministic_genesis_topology(2);
        let first_pop = entries[0].pop_bytes().unwrap().unwrap();
        let registration = super::ingress_observer_registration(peers[0].clone(), &first_pop)
            .expect("matching observer PoP");
        assert_eq!(registration.peer, peers[0]);
        assert_eq!(registration.pop, first_pop);
        assert!(registration.activation_at.is_none());
        assert!(registration.expiry_at.is_none());
        assert!(super::ingress_observer_registration(peers[1].clone(), &first_pop).is_err());
        assert!(super::ingress_observer_registration(peers[0].clone(), &[]).is_err());
    }
    #[test]
    fn ingress_registry_requires_every_exact_identity_once() {
        let (peers, _) = deterministic_genesis_topology(TOTAL_PEERS + 3);
        let expected = peers[..TOTAL_PEERS + 2].iter().cloned().collect();
        assert!(super::ingress_registry_matches(
            &peers[..TOTAL_PEERS + 2],
            &expected
        ));
        assert!(!super::ingress_registry_matches(
            &peers[..TOTAL_PEERS + 1],
            &expected
        ));
        assert!(!super::ingress_registry_matches(&peers, &expected));
        let mut duplicate = peers[..TOTAL_PEERS + 2].to_vec();
        duplicate[TOTAL_PEERS + 1] = peers[0].clone();
        assert!(!super::ingress_registry_matches(&duplicate, &expected));
        duplicate[TOTAL_PEERS + 1] = peers[TOTAL_PEERS + 2].clone();
        assert!(!super::ingress_registry_matches(&duplicate, &expected));
    }
    #[test]
    fn genesis_post_topology_builder_is_stable_for_same_wrong_ingress_roster() {
        let (topology, entries) = deterministic_genesis_topology(TOTAL_PEERS);
        let first = npos_multilane_genesis_post_topology_transactions(&topology, &entries);
        let second = npos_multilane_genesis_post_topology_transactions(&topology, &entries);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }
    #[test]
    fn restricted_lane_policy_and_records_bind_exact_native_membership() {
        use iroha_data_model::sumeragi_lanes::{
            SumeragiLaneFrontier, SumeragiLaneRecord, SumeragiLaneStatus,
        };
        let (topology, entries) = deterministic_genesis_topology(TOTAL_PEERS);
        let policy = super::restricted_lane_policy(&topology, &entries);
        assert_eq!(policy.fixed.len(), 2);
        for (ordinal, fixed) in policy.fixed.iter().enumerate() {
            let start = (ordinal + 1) * VALIDATORS_PER_LANE;
            let mut expected_peers = topology[start..start + VALIDATORS_PER_LANE]
                .iter()
                .collect::<Vec<_>>();
            expected_peers.sort();
            assert_eq!(
                fixed
                    .committee
                    .iter()
                    .map(|member| &member.peer)
                    .collect::<Vec<_>>(),
                expected_peers
            );
            assert!(
                fixed
                    .committee
                    .windows(2)
                    .all(|pair| pair[0].peer < pair[1].peer)
            );
            for member in &fixed.committee {
                assert_eq!(
                    member.pop,
                    entries
                        .iter()
                        .find(|entry| entry.peer == member.peer)
                        .unwrap()
                        .pop_bytes()
                        .unwrap()
                        .unwrap()
                );
            }
        }
        assert_eq!(
            policy.routes[0].account.as_deref(),
            Some(ALICE_ID.to_string().as_str())
        );
        assert_eq!(
            policy.routes[1].account.as_deref(),
            Some(super::BOB_ID.to_string().as_str())
        );
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"native restricted lane record regression",
            )),
        );
        let records = policy
            .fixed
            .iter()
            .enumerate()
            .map(|(ordinal, fixed)| SumeragiLaneStatus {
                record: SumeragiLaneRecord {
                    da_layout: policy.da_layout,
                    lane: fixed.lane,
                    dataspace: fixed.dataspace,
                    incarnation: iroha_core::sumeragi::lanes::step::incarnation(
                        &network_id,
                        fixed.lane,
                        fixed.dataspace,
                        1,
                        ordinal as u64,
                    ),
                    params: policy.lane_params.clone(),
                    committee: fixed.committee.clone(),
                    created_at: 1,
                    active_from: 3,
                    closing: None,
                    anchor_freshness: policy.anchor_freshness,
                    merged: SumeragiLaneFrontier::default(),
                    merged_at: 3,
                    rescued: 0,
                },
                instance: None,
            })
            .collect::<Vec<_>>();
        assert!(super::restricted_lane_records_match(
            &records,
            &policy,
            &network_id
        ));
        assert!(!super::restricted_lane_records_match(
            &records[..1],
            &policy,
            &network_id
        ));
        for mutation in 0..11 {
            let mut changed = records.clone();
            match mutation {
                0 => changed[0].record.committee = records[1].record.committee.clone(),
                1 => changed[0].record.committee[0].pop[0] ^= 1,
                2 => changed[0]
                    .record
                    .committee
                    .push(records[0].record.committee[0].clone()),
                3 => changed[0].record.dataspace = DataSpaceId::new(2),
                4 => changed[0].record.incarnation[0] ^= 1,
                5 => changed[0].record.created_at = 2,
                6 => changed[0].record.active_from = 4,
                7 => changed[0].record.closing = Some(10),
                8 => changed[0].record.anchor_freshness += 1,
                9 => changed[0].record.lane = LaneId::new(2),
                10 => changed[0].record.committee.swap(0, 1),
                _ => unreachable!(),
            }
            assert!(
                !super::restricted_lane_records_match(&changed, &policy, &network_id),
                "mutation {mutation}"
            );
        }
    }
    #[test]
    fn routing_probe_gas_account_uses_alice_subject() {
        let gas_account = routing_probe_gas_account_id();
        assert_eq!(gas_account, ALICE_ID.clone());
        assert_eq!(
            gas_account.canonical_i105().expect("gas account i105"),
            ALICE_ID.canonical_i105().expect("alice i105")
        );
    }
    #[test]
    fn ingress_status_retry_requires_exact_typed_state_busy() {
        use iroha::{Error, StatusFailureReason};
        use std::time::Duration;
        assert_eq!(
            super::ingress_status_retry_delay(&Error::StatusUnavailable {
                reason: Some(StatusFailureReason::StateBusy),
                retry_after: None,
            }),
            Some(super::STATUS_POLL_INTERVAL)
        );
        assert_eq!(
            super::ingress_status_retry_delay(&Error::StatusUnavailable {
                reason: Some(StatusFailureReason::StateBusy),
                retry_after: Some(Duration::from_secs(1)),
            }),
            Some(Duration::from_secs(1))
        );
        for error in [
            Error::StatusUnavailable {
                reason: None,
                retry_after: None,
            },
            Error::StatusUnavailable {
                reason: Some(StatusFailureReason::StateUnavailable),
                retry_after: None,
            },
            Error::StatusUnavailable {
                reason: Some(StatusFailureReason::MailboxUnavailable),
                retry_after: None,
            },
            Error::Http {
                operation: "diagnostic.status",
                status: 503,
                retry_after: None,
                body: b"status_state_busy".to_vec(),
            },
        ] {
            assert!(super::ingress_status_retry_delay(&error).is_none());
        }
    }
    #[test]
    fn ingress_readiness_requires_both_applied_heights_before_the_original_deadline() {
        let before = tokio::time::Instant::now();
        let deadline = before + super::STATUS_WAIT_TIMEOUT;
        assert!(super::ingress_readiness_before_deadline(&[3, 3], 3, before, deadline).unwrap());
        assert!(!super::ingress_readiness_before_deadline(&[3, 2], 3, before, deadline).unwrap());
        assert!(!super::ingress_readiness_before_deadline(&[2, 3], 3, before, deadline).unwrap());
        assert!(super::ingress_readiness_before_deadline(&[3, 3], 3, deadline, deadline).is_err());
        assert!(
            super::ingress_readiness_before_deadline(
                &[3, 3],
                3,
                deadline + super::STATUS_POLL_INTERVAL,
                deadline
            )
            .is_err()
        );
    }
    #[test]
    fn versioned_signed_transaction_encoder_prefixes_v1_and_roundtrips_payload() {
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"cross-dataspace-route-encoder-genesis",
            )),
        );
        let transaction = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "wrong ingress route envelope".to_owned(),
        )])
        .sign(ALICE_KEYPAIR.private_key());
        let adaptive_payload = norito::codec::encode_adaptive(&transaction);
        let encoded = encode_versioned_signed_transaction(&transaction);
        let (decoded, decoded_len) = SignedTransaction::decode_from_slice(&encoded[1..])
            .expect("adaptive signed transaction should decode from raw slice");
        assert_eq!(encoded.first(), Some(&1));
        assert_eq!(encoded.len(), adaptive_payload.len() + 1);
        assert_eq!(&encoded[1..], adaptive_payload.as_slice());
        assert_eq!(decoded_len, adaptive_payload.len());
        assert_eq!(decoded, transaction);
    }
    #[test]
    fn transaction_receipt_request_selects_only_norito_without_changing_submission() {
        use reqwest::header::{ACCEPT, CONTENT_TYPE};
        let body = vec![1, 2, 3, 4];
        let request = super::build_norito_receipt_request(
            reqwest::Client::new()
                .post("http://127.0.0.1:8080/v1/pipeline/transactions")
                .header(ACCEPT, "application/json")
                .header(ACCEPT, "*/*")
                .header(CONTENT_TYPE, "application/x-norito")
                .header("x-iroha-account", "fixture-authority")
                .body(body.clone()),
        )
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.url().path(), "/v1/pipeline/transactions");
        assert_eq!(request.body().unwrap().as_bytes(), Some(body.as_slice()));
        assert_eq!(request.headers()["x-iroha-account"], "fixture-authority");
        assert_eq!(request.headers()[CONTENT_TYPE], "application/x-norito");
        assert_eq!(request.headers().get_all(ACCEPT).iter().count(), 1);
        assert_eq!(request.headers()[ACCEPT], "application/x-norito");
    }
    #[test]
    fn transaction_receipt_decoder_requires_declared_canonical_norito() {
        use iroha_data_model::transaction::{
            TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
        };
        use reqwest::header::CONTENT_TYPE;
        let receipt = TransactionSubmissionReceipt::try_sign(
            TransactionSubmissionReceiptPayload {
                entrypoint_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"receipt entrypoint",
                )),
                signed_transaction_hash: Some(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::new(b"receipt signed transaction"),
                )),
                submitted_at_ms: 123,
                submitted_at_height: 4,
                signer: ALICE_KEYPAIR.public_key().clone(),
            },
            &ALICE_KEYPAIR,
        )
        .unwrap();
        let body = norito::to_bytes(&receipt).unwrap();
        let mut headers = HeaderMap::new();
        assert!(super::decode_transaction_submission_receipt(&headers, &body).is_err());
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        let decoded = super::decode_transaction_submission_receipt(&headers, &body).unwrap();
        assert_eq!(decoded, receipt);
        decoded.verify().unwrap();
        assert!(super::decode_transaction_submission_receipt(&headers, &body[..8]).is_err());
        let json = norito::json::to_vec(&receipt).unwrap();
        assert!(super::decode_transaction_submission_receipt(&headers, &json).is_err());
        for media in [
            "application/json",
            "text/plain",
            "application/x-norito-legacy",
        ] {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(media));
            assert!(super::decode_transaction_submission_receipt(&headers, &body).is_err());
        }
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        headers.append(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        assert!(super::decode_transaction_submission_receipt(&headers, &body).is_err());
    }
    #[test]
    fn expected_lane_binding_for_peer_is_deterministic() {
        let mut seed = vec![0_u8; 32];
        seed[0] = 0x5A;
        let peer_key_pair = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
            .expect("fixture tx/query routing binding peer key");
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());
        let expected_validator_key =
            KeyPair::try_from_seed(validator_authority_seed(5), Algorithm::Ed25519)
                .expect("fixture tx/query routing validator authority key");
        let binding = expected_lane_binding_for_peer(5, &peer_id);
        assert_eq!(binding.peer_id, peer_id.to_string());
        assert_eq!(
            binding.validator,
            validator_authority_account_for_peer(5).to_string()
        );
        assert_eq!(
            validator_authority_account_for_peer(5),
            AccountId::new(expected_validator_key.public_key().clone())
        );
        assert_eq!(
            validator_authority_account_for_peer(5),
            validator_authority_account_for_peer(5)
        );
        assert_ne!(
            validator_authority_account_for_peer(5),
            validator_authority_account_for_peer(6)
        );
    }
    #[test]
    fn lane_validator_snapshot_filters_active_bindings_and_preserves_total() {
        let body = norito::json!({
            "total": 2,
            "items": [
                {
                    "validator": "validator-a",
                    "peer_id": "peer-a",
                    "status": { "type": "Active" },
                },
                {
                    "validator": "validator-b",
                    "peer_id": "peer-b",
                    "status": { "type": "Pending" },
                },
            ],
        });
        let (total, active) =
            lane_validator_snapshot(&body, "lane validators").expect("lane snapshot should parse");
        assert_eq!(total, 2);
        assert_eq!(active.len(), 1);
        assert!(active.contains(&ExpectedLaneValidatorBinding {
            validator: "validator-a".to_owned(),
            peer_id: "peer-a".to_owned(),
        }));
    }
    #[test]
    fn lane_validator_snapshot_rejects_malformed_payloads() {
        for (body, expected) in [
            (JsonValue::Null, "lane validator response is not an object"),
            (
                norito::json!({ "items": [] }),
                "lane validator response is missing total",
            ),
            (
                norito::json!({ "total": 1 }),
                "lane validator response is missing items",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [{ "validator": "validator-a", "peer_id": "peer-a" }],
                }),
                "validator entry missing status.type",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [{ "validator": "validator-a", "status": { "type": "Active" } }],
                }),
                "validator entry missing peer_id literal",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [7],
                }),
                "validator entry is not an object",
            ),
        ] {
            let err = lane_validator_snapshot(&body, "lane validators")
                .expect_err("malformed lane snapshot should fail");
            assert!(
                err.to_string().contains(expected),
                "expected `{expected}` in `{err}`"
            );
        }
    }
    #[test]
    fn routed_response_context_includes_path_query_and_route_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-iroha-routed-by", HeaderValue::from_static("proxy"));
        headers.insert("x-iroha-route-lane-id", HeaderValue::from_static("2"));
        headers.insert("x-iroha-route-dataspace-id", HeaderValue::from_static("7"));
        let path_segments = vec!["v1".to_owned(), "transactions".to_owned()];
        let query_pairs = vec![
            ("account".to_owned(), "alice".to_owned()),
            ("dataspace".to_owned(), "2".to_owned()),
        ];
        let context = routed_response_context(
            HttpStatusCode::ACCEPTED,
            &headers,
            &path_segments,
            &query_pairs,
        );
        assert_eq!(
            routed_header_string(&headers, "x-iroha-routed-by"),
            Some("proxy".to_owned())
        );
        assert!(context.contains("status=202 Accepted"));
        assert!(context.contains("path=/v1/transactions?account=alice&dataspace=2"));
        assert!(context.contains(r#"routed_by=Some("proxy")"#));
        assert!(context.contains(r#"route_lane_id=Some("2")"#));
        assert!(context.contains(r#"route_dataspace_id=Some("7")"#));
    }
    #[test]
    fn routed_response_context_formats_root_path_without_query_or_headers() {
        let context = routed_response_context(HttpStatusCode::OK, &HeaderMap::new(), &[], &[]);
        assert!(context.contains("status=200 OK"));
        assert!(context.contains("path=/"));
        assert!(context.contains("routed_by=None"));
        assert!(context.contains("route_lane_id=None"));
        assert!(context.contains("route_dataspace_id=None"));
    }
    #[test]
    fn routed_response_context_formats_root_path_with_query() {
        let context = routed_response_context(
            HttpStatusCode::BAD_GATEWAY,
            &HeaderMap::new(),
            &[],
            &[("dataspace".to_owned(), DS2_ID_U64.to_string())],
        );
        assert!(context.contains("status=502 Bad Gateway"));
        assert!(context.contains("path=/?dataspace=2"));
    }
    #[test]
    fn routed_header_string_ignores_non_utf8_header_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-iroha-route-lane-id",
            HeaderValue::from_bytes(&[0xFF]).expect("binary header value"),
        );
        assert_eq!(
            routed_header_string(&headers, "x-iroha-route-lane-id"),
            None
        );
    }
    #[test]
    fn expect_proxy_route_headers_accepts_exact_lane_dataspace() {
        let response = routed_json_response_with_route(
            Some("proxy"),
            Some(&DS1_LANE_INDEX.to_string()),
            Some(&DS1_ID_U64.to_string()),
        );
        expect_proxy_route_headers(
            &response,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "ds1 routed response",
        )
        .expect("matching route headers should pass");
        let wrong_dataspace = routed_json_response_with_route(
            Some("proxy"),
            Some(&DS1_LANE_INDEX.to_string()),
            Some(&DS2_ID_U64.to_string()),
        );
        let err = expect_proxy_route_headers(
            &wrong_dataspace,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "ds1 routed response",
        )
        .expect_err("wrong dataspace header should fail");
        assert!(err.to_string().contains("expected routed dataspace 1"));
    }
    #[test]
    fn expect_proxy_route_headers_rejects_missing_proxy_and_wrong_lane() {
        let local_response = routed_json_response_with_route(
            Some("local"),
            Some(&DS1_LANE_INDEX.to_string()),
            Some(&DS1_ID_U64.to_string()),
        );
        let err = expect_proxy_route_headers(
            &local_response,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "local response",
        )
        .expect_err("local response should not satisfy proxy route headers");
        assert!(err.to_string().contains("expected proxied read"));
        let wrong_lane = routed_json_response_with_route(
            Some("proxy"),
            Some(&(DS1_LANE_INDEX + 1).to_string()),
            Some(&DS1_ID_U64.to_string()),
        );
        let err = expect_proxy_route_headers(
            &wrong_lane,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "wrong lane response",
        )
        .expect_err("wrong lane header should fail");
        assert!(err.to_string().contains("expected routed lane 1"));
    }
    #[test]
    fn expect_proxy_route_headers_rejects_missing_lane_or_dataspace() {
        let missing_lane =
            routed_json_response_with_route(Some("proxy"), None, Some(&DS1_ID_U64.to_string()));
        let err = expect_proxy_route_headers(
            &missing_lane,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "missing lane response",
        )
        .expect_err("missing lane header should fail");
        assert!(err.to_string().contains("expected routed lane 1"));
        let missing_dataspace =
            routed_json_response_with_route(Some("proxy"), Some(&DS1_LANE_INDEX.to_string()), None);
        let err = expect_proxy_route_headers(
            &missing_dataspace,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "missing dataspace response",
        )
        .expect_err("missing dataspace header should fail");
        assert!(err.to_string().contains("expected routed dataspace 1"));
    }
    #[test]
    fn expect_proxy_fanout_headers_requires_proxy_without_singular_route() {
        let response = routed_json_response_with_route(Some("proxy"), None, None);
        expect_proxy_fanout_headers(&response, "fanout permissions")
            .expect("fanout response without singular route should pass");
        let local_response = routed_json_response_with_route(Some("local"), None, None);
        let err = expect_proxy_fanout_headers(&local_response, "fanout permissions")
            .expect_err("local fanout response should fail for proxy-only helper");
        assert!(err.to_string().contains("expected proxied fanout read"));
        let missing_route_source = routed_json_response_with_route(None, None, None);
        let err = expect_proxy_fanout_headers(&missing_route_source, "fanout permissions")
            .expect_err("missing fanout route source should fail");
        assert!(err.to_string().contains("expected proxied fanout read"));
        let singular_route =
            routed_json_response_with_route(Some("proxy"), Some(&DS1_LANE_INDEX.to_string()), None);
        let err = expect_proxy_fanout_headers(&singular_route, "fanout permissions")
            .expect_err("fanout response with singular route should fail");
        assert!(
            err.to_string()
                .contains("fanout response should not expose singular route lane")
        );
        let singular_dataspace =
            routed_json_response_with_route(Some("proxy"), None, Some(&DS2_ID_U64.to_string()));
        let err = expect_proxy_fanout_headers(&singular_dataspace, "fanout permissions")
            .expect_err("fanout response with singular dataspace should fail");
        assert!(
            err.to_string()
                .contains("fanout response should not expose singular route dataspace")
        );
    }
    #[test]
    fn permission_response_contains_matches_name_and_dataspace_payload() {
        let body = norito::json!({
            "items": [
                {
                    "name": "CanPublishSpaceDirectoryManifest",
                    "payload": { "dataspace": DS2_ID_U64 },
                },
                {
                    "name": "OtherPermission",
                    "payload": { "dataspace": DS1_ID_U64 },
                },
            ],
        });
        assert!(
            permission_response_contains(
                &body,
                "CanPublishSpaceDirectoryManifest",
                |payload| payload.get("dataspace").and_then(JsonValue::as_u64) == Some(DS2_ID_U64),
                "permission response",
            )
            .expect("permission response should decode")
        );
        assert!(
            !permission_response_contains(
                &body,
                "CanPublishSpaceDirectoryManifest",
                |payload| payload.get("dataspace").and_then(JsonValue::as_u64) == Some(DS1_ID_U64),
                "permission response",
            )
            .expect("permission response should decode")
        );
        let err = permission_response_contains(
            &norito::json!({}),
            "CanPublishSpaceDirectoryManifest",
            |_| true,
            "permission response",
        )
        .expect_err("missing permission items should fail");
        assert!(
            err.to_string()
                .contains("permission response missing items array")
        );
    }
    #[test]
    fn permission_response_contains_ignores_wrong_names_and_missing_payloads() {
        let body = norito::json!({
            "items": [
                { "name": "CanPublishSpaceDirectoryManifest" },
                { "name": "OtherPermission", "payload": { "dataspace": DS2_ID_U64 } },
                { "name": DS2_ID_U64, "payload": { "dataspace": DS2_ID_U64 } },
            ],
        });
        assert!(
            !permission_response_contains(
                &body,
                "CanPublishSpaceDirectoryManifest",
                |payload| payload.get("dataspace").and_then(JsonValue::as_u64) == Some(DS2_ID_U64),
                "permission response",
            )
            .expect("permission response should decode")
        );
    }
    #[test]
    fn response_helpers_reject_non_array_collections() {
        let asset_definition_id = ds2_asset_definition_id();
        let permission_err = permission_response_contains(
            &norito::json!({ "items": {} }),
            "CanPublishSpaceDirectoryManifest",
            |_| true,
            "permission response",
        )
        .expect_err("non-array permissions should fail");
        assert!(
            permission_err
                .to_string()
                .contains("permission response missing items array")
        );
        let manifest_err = manifest_response_contains_status(
            &norito::json!({ "items": {} }),
            DataSpaceId::new(DS2_ID_U64),
            "Active",
            "manifest response",
        )
        .expect_err("non-array manifests should fail");
        assert!(
            manifest_err
                .to_string()
                .contains("manifest response missing items array")
        );
        let assets_err = account_assets_response_contains(
            &norito::json!({ "items": {} }),
            &asset_definition_id,
            "account assets",
        )
        .expect_err("non-array assets should fail");
        assert!(
            assets_err
                .to_string()
                .contains("account assets response missing items array")
        );
    }
    #[test]
    fn manifest_absence_requires_a_complete_empty_selected_page() {
        let ds2 = DataSpaceId::new(DS2_ID_U64);
        let mut response = routed_json_response_with_route(Some("proxy"), Some("0"), Some("0"));
        response.body = norito::json!({"items": [], "next_cursor": null, "total": 0});
        super::require_empty_manifest_page(&response, ds2, "empty").unwrap();
        for body in [
            norito::json!({"manifests": [], "next_cursor": null, "total": 0}),
            norito::json!({"items": [], "next_cursor": "more", "total": 0}),
            norito::json!({"items": [], "next_cursor": null}),
            norito::json!({"items": [], "next_cursor": null, "total": 1}),
            norito::json!({"items": [{"dataspace_id": 2, "status": "Pending"}], "next_cursor": null, "total": 1}),
        ] {
            response.body = body;
            assert!(super::require_empty_manifest_page(&response, ds2, "must fail").is_err());
        }
    }

    #[test]
    fn manifest_permission_absence_requires_effective_scoped_rows() {
        let ds2 = DataSpaceId::new(DS2_ID_U64);
        let mut response = routed_json_response_with_route(Some("proxy"), Some("0"), Some("0"));
        response.body = norito::json!({"items":[{"name":"CanPublishSpaceDirectoryManifest","payload":{"dataspace":1}}],"next_cursor":null,"total":1});
        assert!(
            super::require_no_manifest_publish_permission(&response, ds2, "missing semantics")
                .is_err()
        );
        response.permission_semantics = Some("effective-v1".into());
        super::require_no_manifest_publish_permission(&response, ds2, "other data space").unwrap();
        for body in [
            norito::json!({"items":[{"name":"CanPublishSpaceDirectoryManifest","payload":{"dataspace":2}}],"next_cursor":null,"total":1}),
            norito::json!({"items":[{"name":"CanPublishSpaceDirectoryManifest","payload":{}}],"next_cursor":null,"total":1}),
            norito::json!({"items":[],"next_cursor":"more","total":1}),
        ] {
            response.body = body;
            assert!(
                super::require_no_manifest_publish_permission(&response, ds2, "must fail").is_err()
            );
        }
    }

    #[test]
    fn manifest_publish_rejection_requires_exact_native_permission_failure() {
        use iroha_data_model::{
            ValidationFail, isi::error::InstructionExecutionError,
            transaction::error::TransactionRejectionReason,
        };
        let native = |message: &str| {
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::InvariantViolation(message.into()),
            ))
        };
        assert!(super::is_manifest_publish_permission_rejection(&native(
            "not permitted: CanPublishSpaceDirectoryManifest"
        )));
        assert!(!super::is_manifest_publish_permission_rejection(&native(
            "unknown dataspace"
        )));
        assert!(!super::is_manifest_publish_permission_rejection(
            &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "not permitted: CanPublishSpaceDirectoryManifest".into()
            ))
        ));
    }
    #[test]
    fn manifest_response_helpers_match_dataspace_and_status() {
        let body = norito::json!({
            "items": [
                { "dataspace_id": DS1_ID_U64, "status": "Pending" },
                { "dataspace_id": DS2_ID_U64, "status": "Active" },
            ],
        });
        assert!(
            manifest_response_contains_status(
                &body,
                DataSpaceId::new(DS2_ID_U64),
                "Active",
                "manifest response",
            )
            .expect("manifest response should decode")
        );
        assert!(
            !manifest_response_contains_status(
                &body,
                DataSpaceId::new(DS1_ID_U64),
                "Active",
                "manifest response",
            )
            .expect("manifest response should decode")
        );
        assert!(
            manifest_response_contains_dataspace(
                &body,
                DataSpaceId::new(DS1_ID_U64),
                "manifest response",
            )
            .expect("manifest response should decode")
        );
        let err = manifest_response_contains_dataspace(
            &norito::json!({}),
            DataSpaceId::new(DS1_ID_U64),
            "manifest response",
        )
        .expect_err("missing manifest array should fail");
        assert!(
            err.to_string()
                .contains("manifest response missing items array")
        );
    }
    #[test]
    fn manifest_response_contains_status_requires_manifest_array() {
        let err = manifest_response_contains_status(
            &norito::json!({ "manifests": [] }),
            DataSpaceId::new(DS2_ID_U64),
            "Active",
            "manifest response",
        )
        .expect_err("missing manifest array should fail");
        assert!(
            err.to_string()
                .contains("manifest response missing items array")
        );
    }
    #[test]
    fn manifest_response_contains_dataspace_ignores_missing_or_non_numeric_ids() {
        let body = norito::json!({
            "items": [
                { "dataspace_id": "2", "status": "Active" },
                { "status": "Active" },
            ],
        });
        assert!(
            !manifest_response_contains_dataspace(
                &body,
                DataSpaceId::new(DS2_ID_U64),
                "manifest response",
            )
            .expect("manifest response should decode")
        );
    }
    #[test]
    fn manifest_response_contains_status_ignores_missing_or_non_string_statuses() {
        let body = norito::json!({
            "items": [
                { "dataspace_id": DS2_ID_U64, "status": true },
                { "dataspace_id": DS2_ID_U64 },
                { "dataspace_id": DS1_ID_U64, "status": "Active" },
            ],
        });
        assert!(
            !manifest_response_contains_status(
                &body,
                DataSpaceId::new(DS2_ID_U64),
                "Active",
                "manifest response",
            )
            .expect("manifest response should decode")
        );
    }
    #[test]
    fn account_asset_collection_binds_route_holder_quantity_visibility_and_completion() {
        let public = ds2_asset_definition_id();
        let private = super::partitioned_asset_definition_id();
        let public_row = norito::json!({
            "account_id": (ALICE_ID.to_string()), "asset": (public.to_string()),
            "scope": "global", "quantity": "100"
        });
        let partition = super::restricted_asset_partition();
        let private_row = norito::json!({
            "id": (partition.canonical_literal()), "account_id": (ALICE_ID.to_string()),
            "asset": (private.to_string()), "scope": (format!("dataspace:{DS1_ID_U64}")), "quantity": "7"
        });
        let mut authorized = routed_json_response_with_route(Some("proxy"), Some("0"), Some("0"));
        authorized.body =
            norito::json!({"items": [(private_row.clone())], "next_cursor": null, "total": 1});
        let mut denied = routed_json_response_with_route(Some("proxy"), Some("0"), Some("0"));
        denied.body = norito::json!({"items": [], "next_cursor": null, "total": 0});
        super::require_private_holder_visibility(&authorized, &denied).unwrap();
        denied.route_lane_id = Some("1".into());
        assert!(super::require_private_holder_visibility(&authorized, &denied).is_err());
        denied.route_lane_id = Some("0".into());
        denied.body = authorized.body.clone();
        assert!(super::require_private_holder_visibility(&authorized, &denied).is_err());
        denied.body = norito::json!({"items": [], "next_cursor": "more", "total": 1});
        assert!(super::require_private_holder_visibility(&authorized, &denied).is_err());
        denied.body = norito::json!({"items": [], "next_cursor": null, "total": 0});
        let mut wrong_scope = private_row.clone();
        *wrong_scope
            .as_object_mut()
            .unwrap()
            .get_mut("scope")
            .unwrap() = JsonValue::from("global");
        authorized.body = norito::json!({"items": [wrong_scope], "next_cursor": null, "total": 1});
        assert!(super::require_private_holder_visibility(&authorized, &denied).is_err());
        let mut response = routed_json_response_with_route(Some("proxy"), Some("0"), Some("0"));
        response.body =
            norito::json!({"items": [(public_row.clone()), private_row], "next_cursor": null});
        assert!(
            super::expect_account_assets_page(
                &response,
                &ALICE_ID,
                &[(&public, "100")],
                Some(&private),
                "stranger"
            )
            .is_err()
        );
        response.body = norito::json!({"items": [(public_row.clone())], "next_cursor": null});
        let visible = |response: &RoutedJsonResponse| {
            super::expect_account_assets_page(
                response,
                &ALICE_ID,
                &[(&public, "100")],
                Some(&private),
                "stranger",
            )
        };
        visible(&response).unwrap();
        assert!(
            super::expect_account_assets_page(
                &response,
                &super::BOB_ID,
                &[(&public, "100")],
                None,
                "other holder"
            )
            .is_err()
        );
        assert!(
            super::expect_account_assets_page(
                &response,
                &ALICE_ID,
                &[(&public, "101")],
                None,
                "wrong balance"
            )
            .is_err()
        );
        response.body = norito::json!({"items": [(public_row.clone()), (public_row.clone())], "next_cursor": null});
        assert!(visible(&response).is_err());
        response.body = norito::json!({"items": [(public_row.clone())], "next_cursor": "more"});
        assert!(visible(&response).is_err());
        response.body = norito::json!({"items": [public_row], "next_cursor": null});
        response.route_lane_id = None;
        assert!(visible(&response).is_err());
        response.route_lane_id = Some("0".to_owned());
        response.route_dataspace_id = Some("1".to_owned());
        assert!(visible(&response).is_err());
    }

    #[test]
    fn account_assets_response_contains_matches_asset_definition_literal() {
        let asset_definition_id = ds2_asset_definition_id();
        let asset_literal = asset_definition_id.to_string();
        let other_asset_literal = AssetDefinitionId::derive_from_components(
            DomainId::try_new("nexus", "universal").expect("asset domain"),
            "othercoin".parse().expect("asset name"),
        )
        .to_string();
        let body = norito::json!({
            "items": [
                { "asset": asset_literal },
            ],
        });
        let other_body = norito::json!({
            "items": [
                { "asset": other_asset_literal },
            ],
        });
        assert!(
            account_assets_response_contains(&body, &asset_definition_id, "account assets")
                .expect("account assets response should decode")
        );
        assert!(
            !account_assets_response_contains(&other_body, &asset_definition_id, "account assets")
                .expect("account assets response should decode")
        );
        let err =
            account_assets_response_contains(&norito::json!({}), &asset_definition_id, "assets")
                .expect_err("missing account assets items should fail");
        assert!(
            err.to_string()
                .contains("account assets response missing items array")
        );
    }
    #[test]
    fn account_assets_response_contains_ignores_non_string_assets() {
        let asset_definition_id = ds2_asset_definition_id();
        let asset_literal = asset_definition_id.to_string();
        let body = norito::json!({
            "items": [
                { "asset": DS2_ID_U64 },
                { "not_asset": asset_literal },
            ],
        });
        assert!(
            !account_assets_response_contains(&body, &asset_definition_id, "account assets")
                .expect("account assets response should decode")
        );
    }
    #[test]
    fn account_assets_response_contains_requires_exact_asset_literal() {
        let asset_definition_id = ds2_asset_definition_id();
        let asset_literal = format!("{asset_definition_id}#dataspace:{DS2_ID_U64}");
        let body = norito::json!({
            "items": [
                { "asset": asset_literal },
            ],
        });
        assert!(
            !account_assets_response_contains(&body, &asset_definition_id, "account assets")
                .expect("account assets response should decode")
        );
    }
    #[test]
    fn routed_json_empty_body_is_transient_for_empty_timeout_statuses() {
        assert!(routed_json_empty_body_is_transient(
            HttpStatusCode::REQUEST_TIMEOUT,
            b""
        ));
        assert!(routed_json_empty_body_is_transient(
            HttpStatusCode::BAD_GATEWAY,
            b""
        ));
        assert!(routed_json_empty_body_is_transient(
            HttpStatusCode::SERVICE_UNAVAILABLE,
            b""
        ));
        assert!(routed_json_empty_body_is_transient(
            HttpStatusCode::GATEWAY_TIMEOUT,
            b""
        ));
    }
    #[test]
    fn routed_json_empty_body_is_not_transient_for_non_empty_or_success_statuses() {
        assert!(!routed_json_empty_body_is_transient(
            HttpStatusCode::REQUEST_TIMEOUT,
            br#"{"error":"timeout"}"#
        ));
        assert!(!routed_json_empty_body_is_transient(
            HttpStatusCode::OK,
            b""
        ));
    }
    #[test]
    fn routed_json_response_is_transient_only_for_empty_retryable_routed_failures() {
        let response = routed_json_response(HttpStatusCode::BAD_GATEWAY, JsonValue::Null, "");
        assert!(routed_json_response_is_transient(&response));
    }
    #[test]
    fn routed_json_response_is_not_transient_after_json_body_decodes() {
        let body_text = r#"{"error":"route_unavailable"}"#;
        let response = routed_json_response(
            HttpStatusCode::SERVICE_UNAVAILABLE,
            norito::json::from_str(body_text).expect("decode test JSON"),
            body_text,
        );
        assert!(!routed_json_response_is_transient(&response));
    }
    #[test]
    fn routed_json_response_is_not_transient_for_non_retryable_empty_statuses() {
        let response = routed_json_response(HttpStatusCode::NOT_FOUND, JsonValue::Null, "");
        assert!(!routed_json_response_is_transient(&response));
    }
    #[test]
    fn routed_json_response_is_not_transient_when_null_body_text_is_non_empty() {
        let response =
            routed_json_response(HttpStatusCode::GATEWAY_TIMEOUT, JsonValue::Null, "timeout");
        assert!(!routed_json_response_is_transient(&response));
    }
    #[test]
    fn routed_submit_response_is_transient_for_route_unavailable() {
        let response = RoutedTransactionSubmitResponse {
            status: HttpStatusCode::NOT_FOUND,
            receipt: None,
            body_text: r#"{"error":"route_unavailable"}"#.to_owned(),
            routed_by: Some("proxy".to_owned()),
            route_lane_id: None,
            route_dataspace_id: None,
        };
        assert!(super::routed_submit_response_is_transient(&response));
    }
    #[test]
    fn routed_submit_response_is_not_transient_for_other_not_found() {
        let response = RoutedTransactionSubmitResponse {
            status: HttpStatusCode::NOT_FOUND,
            receipt: None,
            body_text: r#"{"error":"missing"}"#.to_owned(),
            routed_by: Some("proxy".to_owned()),
            route_lane_id: None,
            route_dataspace_id: None,
        };
        assert!(!super::routed_submit_response_is_transient(&response));
    }
    #[test]
    fn routed_submit_response_is_transient_for_empty_proxy_not_found() {
        let response = RoutedTransactionSubmitResponse {
            status: HttpStatusCode::NOT_FOUND,
            receipt: None,
            body_text: String::new(),
            routed_by: Some("proxy".to_owned()),
            route_lane_id: Some(DS2_LANE_INDEX.to_string()),
            route_dataspace_id: Some(DS2_ID_U64.to_string()),
        };
        assert!(super::routed_submit_response_is_transient(&response));
    }
    #[test]
    fn routed_submit_response_is_transient_for_unmarked_empty_not_found() {
        let response = RoutedTransactionSubmitResponse {
            status: HttpStatusCode::NOT_FOUND,
            receipt: None,
            body_text: String::new(),
            routed_by: None,
            route_lane_id: None,
            route_dataspace_id: None,
        };
        assert!(super::routed_submit_response_is_transient(&response));
    }
}
