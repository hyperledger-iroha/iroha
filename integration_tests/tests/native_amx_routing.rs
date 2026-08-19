#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native AMX multidataspace routing integration coverage.
use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::nexus;
use iroha::{
    client::Client,
    crypto::{Hash, HashOf, SignatureOf},
    data_model::{
        Level, ValidationFail,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::{
            ExternalExecutionRouteLeg, ExternalExecutionRouteRole, Header, SignedBlock,
            consensus::{
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, LaneBlockCommitment,
                NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
            },
        },
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            Grant, InstructionBox, Log, Mint, Register, SetParameter,
            musubi::{
                AddMusubiArchiveLocationV1, PublishMusubiReleaseV1, RegisterMusubiArchiveV1,
                RegisterMusubiNamespaceBindingV1, RegisterMusubiProviderBundleAttestationV1,
            },
            sorafs::{
                CompleteReplicationOrder, IssueReplicationOrder, RegisterPinManifest,
                SetProviderIngestCompletionAuthority,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        musubi::{
            ArchiveId, MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1, MUSUBI_REGISTRY_VERSION_V1,
            MusubiAbiBindingV1, MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1,
            MusubiArchiveLocationQueryV1, MusubiArchiveLocationStateV1,
            MusubiArchiveRetentionDispositionV1, MusubiArchiveRetentionQueryV1,
            MusubiContentDigestV1, MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1,
            MusubiKotodamaEditionV1, MusubiNamespaceBindingV1, MusubiOrderedPrefixQueryV1,
            MusubiOrderedPrefixV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiPageRequestV1,
            MusubiProviderBundleVerificationApprovalV1,
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
            MusubiPublicationV1, MusubiRegistrySnapshotV1, MusubiReleaseIdV1,
            MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiResolutionProofV1,
            MusubiResolverIndexQueryV1, MusubiSeedIngressReceiptApprovalV1,
            MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptPayloadV1,
            MusubiSeedIngressReceiptV1, MusubiStorageAvailabilityV1, MusubiVerificationLockV1,
            musubi_provider_bundle_attestation_set_digest_v1,
        },
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneRelayEnvelope,
            LaneVisibility, compute_settlement_hash,
        },
        parameter::{Parameter, system::SumeragiNposParameters},
        peer::PeerId,
        permission::Permission,
        prelude::Quantity,
        query::{
            block::prelude::FindBlocks,
            error::QueryExecutionFail,
            musubi::prelude::{
                FindMusubiArchiveLocationsV1, FindMusubiArchiveRetentionV1,
                FindMusubiExactPackageV1, FindMusubiExactReleaseV1, FindMusubiOrderedPrefixV1,
                FindMusubiResolverIndexV1,
            },
        },
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ChunkerProfileHandle, ManifestDigest, ManifestRootCid,
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
                ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
            },
        },
        transaction::{FeePaymentIntent, SignedTransaction, TransactionEntrypoint},
    },
    query::QueryError,
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
use iroha_executor_data_model::permission::sorafs::{
    CanCompleteSorafsReplicationOrder, CanIssueSorafsReplicationOrder,
};
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, dataspace_setup_instruction,
    domain_setup_instruction_in_dataspace, genesis_factory_with_post_topology,
    init_instruction_registry,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use norito::json::{self, Value as JsonValue};
use sorafs_manifest::{
    DagCodecId, ManifestBuilder, ManifestV1, PinPolicy as ManifestPinPolicy,
    REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    ReplicationOrderV1, StorageClass as ManifestStorageClass,
    chunker_registry::{MANIFEST_DAG_CODEC, default_descriptor},
};
use std::{
    collections::BTreeSet,
    fs,
    num::{NonZeroU32, NonZeroUsize},
    time::{Duration, Instant},
};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};
#[path = "native_amx_routing/qualification_scenarios.rs"]
mod qualification_scenarios;
#[path = "native_amx_routing/selectable_publication_gate.rs"]
mod selectable_publication_gate;
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
const MUSUBI_FAULT_DOMAIN: &str = "musubifault";
const MUSUBI_FAULT_PACKAGE: &str = "atomic-replay";
const MUSUBI_FAULT_NAMESPACE: &str = "musubifault.acme";
const MUSUBI_FAULT_RETENTION_EPOCH: u64 = 1_000;
const MUSUBI_FAULT_RENEW_EPOCH: u64 = 500;
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
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}
fn fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
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
            AssetDefinition::numeric(
                asset_definition_id.clone(),
                "xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .into(),
        Register::asset_definition({
            let asset_definition_id = fee_asset_id.clone();
            AssetDefinition::numeric(
                asset_definition_id.clone(),
                "xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
fn musubi_fault_provider() -> ProviderId {
    ProviderId::new([0xD7; 32])
}
fn musubi_fault_replica_providers() -> [ProviderId; 3] {
    [
        ProviderId::new([0xD7; 32]),
        ProviderId::new([0xD8; 32]),
        ProviderId::new([0xD9; 32]),
    ]
}
fn musubi_fault_localnet_builder() -> NetworkBuilder {
    let owner = ALICE_ID
        .canonical_i105()
        .expect("canonical SoraFS provider owner");
    let provider_owners = musubi_fault_replica_providers()
        .into_iter()
        .map(|provider| {
            (
                hex::encode(provider.as_bytes()),
                TomlValue::String(owner.clone()),
            )
        })
        .collect::<Table>();
    localnet_builder().with_config_layer(move |layer| {
        layer.write(
            ["governance", "sorafs_provider_owners"],
            TomlValue::Table(provider_owners.clone()),
        );
    })
}
fn musubi_fault_package() -> MusubiPackageIdV1 {
    MusubiPackageIdV1::new(
        DataSpaceId::new(ACME_DATASPACE),
        MusubiPackageScopeV1::Domain(
            MUSUBI_FAULT_DOMAIN
                .parse()
                .expect("Musubi fault domain scope"),
        ),
        MUSUBI_FAULT_PACKAGE
            .parse()
            .expect("Musubi fault package name"),
    )
}
fn musubi_fault_namespace_binding() -> MusubiNamespaceBindingV1 {
    MusubiNamespaceBindingV1 {
        namespace: MUSUBI_FAULT_NAMESPACE
            .parse()
            .expect("Musubi fault namespace"),
        home_dataspace: DataSpaceId::new(ACME_DATASPACE),
        scope: musubi_fault_package().scope,
        generation: 1,
    }
}
fn musubi_fault_archive_commitment() -> MusubiArchiveCommitmentV1 {
    MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([0x81; 32])
            .expect("Musubi fault archive root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new([0x82; 32]),
        por_root: MusubiContentDigestV1::new([0x83; 32]),
        content_length: 1,
        car_digest: MusubiContentDigestV1::new([0x84; 32]),
        car_size: 1,
        bundle_digest: MusubiContentDigestV1::new([0x85; 32]),
        source_tree_digest: MusubiContentDigestV1::new([0x86; 32]),
        descriptor_digest: MusubiContentDigestV1::new([0x87; 32]),
        file_count: 1,
        chunk_count: 1,
    }
}
fn musubi_fault_release_manifest_and_lock() -> (MusubiReleaseManifestV1, MusubiVerificationLockV1) {
    let release = MusubiReleaseIdV1::new(
        musubi_fault_package(),
        "1.0.0".parse().expect("Musubi fault release version"),
    );
    let lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release.clone(),
        root_dependencies: Vec::new(),
        nodes: Vec::new(),
    };
    let manifest = MusubiReleaseManifestV1 {
        release,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([0x88; 32]).expect("Musubi fault ABI binding"),
        dependencies: Vec::new(),
        exports: Vec::new(),
        interface_digest: MusubiContentDigestV1::new([0x89; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id: musubi_fault_archive_commitment().archive_id(),
        verification_lock_digest: lock.digest(),
    };
    (manifest, lock)
}
const fn musubi_fault_page() -> MusubiPageRequestV1 {
    MusubiPageRequestV1 {
        limit: 50,
        cursor: None,
    }
}
fn musubi_fault_completion_authority(
    owner: &AccountId,
    provider: ProviderId,
) -> ProviderIngestCompletionAuthorityV1 {
    let seed = provider.as_bytes()[0];
    ProviderIngestCompletionAuthorityV1::new(
        owner.clone(),
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [seed.wrapping_add(1); 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [seed.wrapping_add(2); 32],
        },
    )
}
fn musubi_fault_pin_manifest(
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<(ManifestV1, ManifestDigest)> {
    let descriptor = default_descriptor();
    ensure!(
        commitment.chunker.profile_id == descriptor.id.0
            && commitment.chunker.namespace == descriptor.namespace
            && commitment.chunker.name == descriptor.name
            && commitment.chunker.semver == descriptor.semver
            && commitment.chunker.multihash_code == descriptor.multihash_code,
        "Musubi fault commitment must use the canonical default SoraFS chunker"
    );
    let manifest = ManifestBuilder::new()
        .root_cid(commitment.root_cid.as_bytes().to_vec())
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_from_registry(descriptor.id)
        .chunk_digest_sha3_256(*commitment.chunk_plan_digest.as_bytes())
        .por_root(*commitment.por_root.as_bytes())
        .content_length(commitment.content_length)
        .car_digest(*commitment.car_digest.as_bytes())
        .car_size(commitment.car_size)
        .pin_policy(ManifestPinPolicy {
            min_replicas: 3,
            storage_class: ManifestStorageClass::Hot,
            retention_epoch: MUSUBI_FAULT_RETENTION_EPOCH,
        })
        .build()
        .wrap_err("build canonical selectable Musubi SoraFS manifest")?;
    let digest = ManifestDigest::from_manifest(&manifest)
        .wrap_err("derive selectable Musubi SoraFS manifest digest")?;
    Ok((manifest, digest))
}
fn musubi_fault_replication_order(
    commitment: &MusubiArchiveCommitmentV1,
    manifest_digest: ManifestDigest,
    order_id: ReplicationOrderId,
) -> ReplicationOrderV1 {
    ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: *order_id.as_bytes(),
        manifest_cid: commitment.root_cid.as_bytes().to_vec(),
        manifest_digest: *manifest_digest.as_bytes(),
        chunking_profile: commitment.chunker.to_handle(),
        target_replicas: 3,
        assignments: musubi_fault_replica_providers()
            .into_iter()
            .map(|provider| ReplicationAssignmentV1 {
                provider_id: *provider.as_bytes(),
                slice_gib: 1,
                lane: None,
            })
            .collect(),
        issued_at: 1,
        deadline_at: 100,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 10,
            min_availability_percent_milli: 99_500,
            min_por_success_percent_milli: 98_000,
        },
        metadata: Vec::new(),
    }
}
fn musubi_fault_finalized_anchor(client: &Client) -> Result<ProviderIngestFinalizedAnchorV1> {
    let blocks = client.query(FindBlocks).execute_all()?;
    // `FindBlocks` is newest-first, so the first row is the exact finalized
    // prefix on which the completion transaction is prepared.
    let latest = blocks
        .first()
        .ok_or_else(|| eyre!("selectable Musubi fixture has no finalized anchor block"))?;
    Ok(ProviderIngestFinalizedAnchorV1 {
        height: latest.header().height().get(),
        block_hash: *latest.hash().as_ref(),
    })
}
fn musubi_fault_provider_attestations(
    client: &Client,
    commitment: &MusubiArchiveCommitmentV1,
    manifest: &MusubiReleaseManifestV1,
    order_id: ReplicationOrderId,
    anchor: ProviderIngestFinalizedAnchorV1,
) -> Vec<MusubiProviderBundleVerificationAttestationV1> {
    musubi_fault_replica_providers()
        .into_iter()
        .map(|provider| {
            let payload = MusubiProviderBundleVerificationPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                binding: MusubiProviderBundleVerificationBindingV1 {
                    network_id: client.network_id,
                    provider_id: provider,
                    completed_by: client.account.clone(),
                    completion_authority: musubi_fault_completion_authority(
                        &client.account,
                        provider,
                    ),
                    replication_order: order_id,
                    assignment_revision: 1,
                    completion_epoch: 3,
                    finalized_anchor: anchor,
                    archive_id: commitment.archive_id(),
                    bundle_digest: commitment.bundle_digest,
                    descriptor_digest: commitment.descriptor_digest,
                    semantic_release_manifest_digest: manifest.semantic_digest(),
                    verification_lock_digest: manifest.verification_lock_digest,
                    source_tree_digest: commitment.source_tree_digest,
                },
            };
            MusubiProviderBundleVerificationAttestationV1 {
                approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                    public_key: client.key_pair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        client.key_pair.private_key(),
                        payload.signing_hash(),
                    )
                    .expect("sign selectable Musubi provider attestation"),
                }],
                payload,
            }
        })
        .collect()
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
async fn submit_approved_and_wait_for_all_peers(
    network: &sandbox::SerializedNetwork,
    submitter: &Client,
    transaction: SignedTransaction,
    context: &str,
) -> Result<SignedBlock> {
    let entrypoint_hash = transaction.hash_as_entrypoint();
    submit_and_wait_for_approval(submitter, transaction).await?;
    let mut canonical: Option<SignedBlock> = None;
    for (index, peer) in network.peers().iter().enumerate() {
        let block = wait_for_block_with_entrypoint(
            &peer.client(),
            entrypoint_hash,
            &format!("{context}: peer {index}"),
        )
        .await?;
        if let Some(expected) = canonical.as_ref() {
            ensure!(
                block.hash() == expected.hash(),
                "{context}: peer {index} committed a different block"
            );
        } else {
            canonical = Some(block);
        }
    }
    canonical.ok_or_else(|| eyre!("{context}: four-peer network returned no committed block"))
}
fn assert_musubi_universal_home_execution_context(
    block: &SignedBlock,
    transaction: &SignedTransaction,
) -> Result<NativeAmxReceipt> {
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let context = block
        .execution_context()
        .and_then(|bundle| {
            bundle
                .external
                .iter()
                .find(|context| context.entrypoint_hash == entrypoint_hash)
        })
        .ok_or_else(|| eyre!("Musubi Native AMX block omitted its execution context"))?;
    ensure!(
        context.lane_id == LaneId::new(UNIVERSAL_LANE)
            && context.dataspace_id == DataSpaceId::UNIVERSAL,
        "Musubi Native AMX coordinator was not universal"
    );
    ensure!(
        context.routing_plan_legs
            == vec![
                ExternalExecutionRouteLeg::new(
                    LaneId::new(UNIVERSAL_LANE),
                    DataSpaceId::UNIVERSAL,
                    ExternalExecutionRouteRole::Coordinator,
                ),
                ExternalExecutionRouteLeg::new(
                    LaneId::new(ACME_LANE),
                    DataSpaceId::new(ACME_DATASPACE),
                    ExternalExecutionRouteRole::Participant,
                ),
            ],
        "Musubi Native AMX plan did not contain exactly universal coordinator + home participant: {:?}",
        context.routing_plan_legs
    );
    let receipt = context
        .native_amx_receipt
        .as_ref()
        .ok_or_else(|| eyre!("Musubi Native AMX execution context omitted its receipt"))?;
    ensure!(
        receipt.plan_digest == context.routing_plan_digest && receipt.legs.len() == 1,
        "Musubi Native AMX receipt did not bind the exact one-participant plan"
    );
    let leg = receipt
        .legs
        .first()
        .ok_or_else(|| eyre!("Musubi Native AMX receipt omitted its home leg"))?;
    ensure!(
        leg.lane_id == LaneId::new(ACME_LANE)
            && leg.dataspace_id == DataSpaceId::new(ACME_DATASPACE),
        "Musubi Native AMX receipt used a non-home participant"
    );
    ensure!(
        leg.prepare_qc.body.phase == NativeAmxPhase::Prepare
            && leg.commit_qc.body.phase == NativeAmxPhase::Commit
            && leg.prepare_qc.body.plan_digest == context.routing_plan_digest
            && leg.commit_qc.body.plan_digest == context.routing_plan_digest
            && leg.prepare_qc.validator_set().len() == PEERS
            && leg.commit_qc.validator_set().len() == PEERS,
        "Musubi Native AMX home leg omitted exact four-peer prepare/commit evidence"
    );
    Ok(receipt.clone())
}
async fn wait_for_rejected_transaction(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<()> {
    let hash = transaction.hash();
    let started = Instant::now();
    let mut last_status: Option<String> = None;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let client = client.clone();
        let response = spawn_blocking(move || client.get_transaction_status_response(hash))
            .await
            .map_err(|error| eyre!("{context}: status task join error: {error}"))??;
        if let Some(response) = response {
            let kind = response.status.kind.clone();
            if kind == "Rejected" {
                return Ok(());
            }
            ensure!(kind != "Applied", "{context}: fault transaction applied");
            last_status = Some(kind);
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    Err(eyre!(
        "{context}: timed out waiting for rejection; last status={last_status:?}"
    ))
}
fn musubi_fault_snapshot_and_time(client: &Client) -> Result<(MusubiRegistrySnapshotV1, u64)> {
    let resolver =
        client.query_single(FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
            package: musubi_fault_package(),
            requirement: None,
            page: musubi_fault_page(),
        }))?;
    ensure!(
        resolver.items.is_empty(),
        "Musubi fault package unexpectedly exists before publication"
    );
    ensure!(
        resolver.network_id == client.network_id,
        "Musubi resolver page used a different network identity"
    );
    let blocks = client.query(FindBlocks).execute_all()?;
    let latest = blocks
        .first()
        .ok_or_else(|| eyre!("Musubi fault fixture has no finalized block"))?;
    let latest_time_ms = u64::try_from(latest.header().creation_time().as_millis())
        .wrap_err("Musubi fault fixture block time overflows u64")?;
    Ok((resolver.snapshot, latest_time_ms))
}
fn musubi_fault_staging_receipt(
    client: &Client,
    latest_time_ms: u64,
    commitment: &MusubiArchiveCommitmentV1,
    manifest: &MusubiReleaseManifestV1,
) -> MusubiSeedIngressReceiptV1 {
    let issued_at_ms = latest_time_ms.saturating_sub(1).max(1);
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: MusubiSeedIngressReceiptBindingV1 {
            network_id: client.network_id,
            publisher: client.account.clone(),
            ingress_broker: client.account.clone(),
            seed_provider: musubi_fault_provider(),
            semantic_release_manifest_digest: manifest.semantic_digest(),
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x8A; 32],
        },
        issued_at_ms,
        expires_at_ms: issued_at_ms
            .checked_add(MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1)
            .expect("Musubi fault receipt expiry"),
    };
    MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: client.key_pair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                client.key_pair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign Musubi fault receipt"),
        }],
        payload,
    }
}
struct SelectableMusubiPublicationFixture {
    transaction: SignedTransaction,
    binding: MusubiNamespaceBindingV1,
    release: MusubiReleaseIdV1,
    manifest: MusubiReleaseManifestV1,
    archive_id: ArchiveId,
    location_id: MusubiArchiveLocationIdV1,
    pin_manifest: ManifestDigest,
    replication_order: ReplicationOrderId,
}
async fn prepare_selectable_musubi_publication(
    network: &sandbox::SerializedNetwork,
    submitter: &Client,
    context: &str,
) -> Result<SelectableMusubiPublicationFixture> {
    let providers = musubi_fault_replica_providers();
    let mut provider_instructions = Vec::with_capacity(providers.len());
    for provider in providers {
        provider_instructions.push(InstructionBox::from(
            SetProviderIngestCompletionAuthority::new(
                provider,
                None,
                musubi_fault_completion_authority(&submitter.account, provider),
            ),
        ));
    }
    let provider_transaction = submitter.build_transaction(
        provider_instructions,
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        provider_transaction,
        &format!("{context}: register three replica providers"),
    )
    .await?;
    let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
    let domain =
        DomainId::try_new(MUSUBI_FAULT_DOMAIN, "acme").expect("Musubi fault namespace domain");
    let namespace_home_transaction = submitter.build_transaction(
        [
            dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
            domain_setup_instruction_in_dataspace(&domain, acme_dataspace, &submitter.account)?,
        ],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        namespace_home_transaction,
        &format!("{context}: establish namespace home"),
    )
    .await?;
    let binding = musubi_fault_namespace_binding();
    let binding_transaction = submitter.build_transaction(
        [InstructionBox::from(RegisterMusubiNamespaceBindingV1::new(
            binding.clone(),
            1,
        ))],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    let binding_block = submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        binding_transaction.clone(),
        &format!("{context}: register namespace binding"),
    )
    .await?;
    assert_musubi_universal_home_execution_context(&binding_block, &binding_transaction)?;
    let commitment = musubi_fault_archive_commitment();
    let archive_id = commitment.archive_id();
    let (manifest, lock) = musubi_fault_release_manifest_and_lock();
    let (_, latest_time_ms) = musubi_fault_snapshot_and_time(submitter)?;
    let staging_receipt =
        musubi_fault_staging_receipt(submitter, latest_time_ms, &commitment, &manifest);
    let archive_transaction = submitter.build_transaction(
        [InstructionBox::from(RegisterMusubiArchiveV1::new(
            commitment.clone(),
            staging_receipt,
            1,
        ))],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        archive_transaction,
        &format!("{context}: register archive"),
    )
    .await?;
    let (pin_manifest, pin_manifest_digest) = musubi_fault_pin_manifest(&commitment)?;
    let pin_transaction = submitter.build_transaction(
        [InstructionBox::from(RegisterPinManifest::new(
            pin_manifest
                .encode()
                .wrap_err("encode selectable Musubi pin manifest")?,
            None,
            None,
        ))],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        pin_transaction,
        &format!("{context}: register registry-grade pin"),
    )
    .await?;
    let replication_order = ReplicationOrderId::new([0xDA; 32]);
    let canonical_order =
        musubi_fault_replication_order(&commitment, pin_manifest_digest, replication_order);
    canonical_order
        .validate()
        .wrap_err("validate selectable Musubi replication order")?;
    let issue_transaction = submitter.build_transaction(
        [InstructionBox::from(
            IssueReplicationOrder::new(
                replication_order,
                norito::encode_canonical(&canonical_order)
                    .wrap_err("encode selectable Musubi replication order")?,
                2,
                MUSUBI_FAULT_RETENTION_EPOCH,
            )
            .for_musubi_archive(commitment.archive_id()),
        )],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        issue_transaction,
        &format!("{context}: issue three-replica order"),
    )
    .await?;
    let anchor = musubi_fault_finalized_anchor(submitter)?;
    let completion_transaction = submitter.build_transaction(
        musubi_fault_replica_providers().map(|provider| {
            InstructionBox::from(CompleteReplicationOrder::new(
                replication_order,
                provider,
                3,
                musubi_fault_completion_authority(&submitter.account, provider),
                1,
                anchor,
            ))
        }),
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        completion_transaction,
        &format!("{context}: finalize three distinct replicas"),
    )
    .await?;
    let location_id = MusubiArchiveLocationIdV1::new([0xDB; 32]);
    let provider_attestations = musubi_fault_provider_attestations(
        submitter,
        &commitment,
        &manifest,
        replication_order,
        anchor,
    );
    let provider_attestation_transaction = submitter.build_transaction(
        provider_attestations.iter().cloned().map(|attestation| {
            InstructionBox::from(RegisterMusubiProviderBundleAttestationV1::new(
                attestation,
                1,
            ))
        }),
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        provider_attestation_transaction,
        &format!("{context}: register three provider bundle attestations"),
    )
    .await?;
    let provider_attestation_references = provider_attestations
        .iter()
        .map(MusubiProviderBundleVerificationAttestationV1::reference)
        .collect::<Vec<_>>();
    let provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        archive_id,
        replication_order,
        &provider_attestation_references,
    )?;
    let location_transaction = submitter.build_transaction(
        [InstructionBox::from(AddMusubiArchiveLocationV1 {
            archive_id,
            location_id,
            pin_manifest: pin_manifest_digest,
            replication_order,
            provider_attestation_set_digest,
            renew_after_epoch: MUSUBI_FAULT_RENEW_EPOCH,
            expires_at_epoch: MUSUBI_FAULT_RETENTION_EPOCH,
            expected_location_revision: 1,
        })],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    submit_approved_and_wait_for_all_peers(
        network,
        submitter,
        location_transaction,
        &format!("{context}: bind selectable archive location"),
    )
    .await?;
    let (snapshot, _) = musubi_fault_snapshot_and_time(submitter)?;
    let publication = MusubiPublicationV1 {
        manifest: manifest.clone(),
        resolution: MusubiResolutionProofV1 { snapshot, lock },
    };
    publication
        .validate()
        .wrap_err("validate selectable Musubi publication")?;
    let transaction = submitter.build_transaction(
        [InstructionBox::from(PublishMusubiReleaseV1::new(
            binding.namespace.clone(),
            publication,
            None,
            1,
            None,
        ))],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    Ok(SelectableMusubiPublicationFixture {
        transaction,
        binding,
        release: manifest.release.clone(),
        manifest,
        archive_id,
        location_id,
        pin_manifest: pin_manifest_digest,
        replication_order,
    })
}
fn is_query_not_found(error: &QueryError) -> bool {
    matches!(
        error,
        QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
    )
}
fn assert_musubi_publication_absent(
    client: &Client,
    release: &MusubiReleaseIdV1,
    archive_id: ArchiveId,
    context: &str,
) -> Result<MusubiRegistrySnapshotV1> {
    let package_error = client
        .query_single(FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
            package: release.package.clone(),
        }))
        .expect_err("faulted Musubi publication must not create its home package");
    ensure!(
        is_query_not_found(&package_error),
        "{context}: exact package query failed unexpectedly: {package_error:?}"
    );
    let release_error = client
        .query_single(FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 {
            release: release.clone(),
        }))
        .expect_err("faulted Musubi publication must not create its home release");
    ensure!(
        is_query_not_found(&release_error),
        "{context}: exact release query failed unexpectedly: {release_error:?}"
    );
    let resolver =
        client.query_single(FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
            package: release.package.clone(),
            requirement: None,
            page: musubi_fault_page(),
        }))?;
    ensure!(
        resolver.items.is_empty() && resolver.next_cursor.is_none(),
        "{context}: faulted publication left a universal resolver row"
    );
    let directory_prefix = format!("{MUSUBI_FAULT_NAMESPACE}/");
    let directory =
        client.query_single(FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&directory_prefix)
                .expect("Musubi fault directory prefix"),
            page: musubi_fault_page(),
        }))?;
    ensure!(
        directory.namespace_binding == musubi_fault_namespace_binding()
            && directory.items.is_empty()
            && directory.next_cursor.is_none(),
        "{context}: faulted publication changed its binding or public-directory projection"
    );
    let locations = client.query_single(FindMusubiArchiveLocationsV1::new(
        MusubiArchiveLocationQueryV1 {
            archive_id,
            page: musubi_fault_page(),
        },
    ))?;
    ensure!(
        locations.archive.archive_id == archive_id
            && locations.items.is_empty()
            && locations.next_cursor.is_none(),
        "{context}: fault fixture archive registration or location state changed"
    );
    let retention = client.query_single(FindMusubiArchiveRetentionV1::new(
        MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![archive_id],
            expected_snapshot: None,
        },
    ))?;
    let [decision] = retention.items.as_slice() else {
        return Err(eyre!(
            "{context}: exact archive-retention query returned the wrong item count"
        ));
    };
    ensure!(
        decision.archive_id == archive_id
            && decision.disposition == MusubiArchiveRetentionDispositionV1::PruneUnreferenced
            && decision.active_releases == 0
            && decision.yanked_releases == 0
            && decision.taken_down_releases == 0
            && decision.storage.is_some_and(|storage| {
                storage.archive_id == archive_id
                    && storage.availability == MusubiStorageAvailabilityV1::Unavailable
            }),
        "{context}: faulted publication left an archive reverse reference: {decision:?}"
    );
    ensure!(
        resolver.snapshot == directory.snapshot
            && resolver.snapshot == locations.snapshot
            && resolver.snapshot == retention.snapshot,
        "{context}: Musubi home/universal absence tuple was read from different snapshots"
    );
    Ok(resolver.snapshot)
}
fn assert_selectable_musubi_archive_without_release(
    client: &Client,
    fixture: &SelectableMusubiPublicationFixture,
    context: &str,
) -> Result<MusubiRegistrySnapshotV1> {
    let package_error = client
        .query_single(FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
            package: fixture.release.package.clone(),
        }))
        .expect_err("unpublished selectable fixture must not create its home package");
    ensure!(
        is_query_not_found(&package_error),
        "{context}: exact package query failed unexpectedly: {package_error:?}"
    );
    let release_error = client
        .query_single(FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 {
            release: fixture.release.clone(),
        }))
        .expect_err("unpublished selectable fixture must not create its home release");
    ensure!(
        is_query_not_found(&release_error),
        "{context}: exact release query failed unexpectedly: {release_error:?}"
    );
    let resolver =
        client.query_single(FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
            package: fixture.release.package.clone(),
            requirement: None,
            page: musubi_fault_page(),
        }))?;
    ensure!(
        resolver.items.is_empty() && resolver.next_cursor.is_none(),
        "{context}: unpublished selectable fixture has a resolver row"
    );
    let directory_prefix = format!("{MUSUBI_FAULT_NAMESPACE}/");
    let directory =
        client.query_single(FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&directory_prefix)
                .expect("Musubi fault directory prefix"),
            page: musubi_fault_page(),
        }))?;
    ensure!(
        directory.namespace_binding == fixture.binding
            && directory.items.is_empty()
            && directory.next_cursor.is_none(),
        "{context}: unpublished selectable fixture changed its directory projection"
    );
    let locations = client.query_single(FindMusubiArchiveLocationsV1::new(
        MusubiArchiveLocationQueryV1 {
            archive_id: fixture.archive_id,
            page: musubi_fault_page(),
        },
    ))?;
    let [location] = locations.items.as_slice() else {
        return Err(eyre!(
            "{context}: selectable fixture returned {} locations instead of one",
            locations.items.len()
        ));
    };
    ensure!(
        location.location_id == fixture.location_id
            && location.archive_id == fixture.archive_id
            && location.pin_manifest == fixture.pin_manifest
            && location.replication_order == fixture.replication_order
            && location.providers == musubi_fault_replica_providers()
            && location.state == MusubiArchiveLocationStateV1::Healthy,
        "{context}: selectable archive location differs from finalized evidence: {location:?}"
    );
    let retention = client.query_single(FindMusubiArchiveRetentionV1::new(
        MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![fixture.archive_id],
            expected_snapshot: None,
        },
    ))?;
    let [decision] = retention.items.as_slice() else {
        return Err(eyre!(
            "{context}: selectable fixture retention query returned the wrong item count"
        ));
    };
    ensure!(
        decision.disposition == MusubiArchiveRetentionDispositionV1::PruneUnreferenced
            && decision.active_releases == 0
            && decision.yanked_releases == 0
            && decision.taken_down_releases == 0
            && decision.storage.is_some_and(|storage| {
                storage.archive_id == fixture.archive_id
                    && storage.availability == MusubiStorageAvailabilityV1::Selectable
                    && storage.healthy_replicas == 3
                    && storage.active_locations == 1
            }),
        "{context}: selectable unpublished archive has the wrong retention state: {decision:?}"
    );
    ensure!(
        resolver.snapshot == directory.snapshot
            && resolver.snapshot == locations.snapshot
            && resolver.snapshot == retention.snapshot,
        "{context}: selectable absence tuple was read from different snapshots"
    );
    Ok(resolver.snapshot)
}
fn assert_selectable_musubi_publication_present(
    client: &Client,
    fixture: &SelectableMusubiPublicationFixture,
    context: &str,
) -> Result<MusubiRegistrySnapshotV1> {
    let package =
        client.query_single(FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
            package: fixture.release.package.clone(),
        }))?;
    ensure!(
        package.package == fixture.release.package
            && package.claimed_namespace == fixture.binding.namespace
            && package.owners == vec![ALICE_ID.clone()]
            && package.member_accounts == vec![ALICE_ID.clone()],
        "{context}: home package record is incomplete: {package:?}"
    );
    let release =
        client.query_single(FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 {
            release: fixture.release.clone(),
        }))?;
    ensure!(
        release.home_release.manifest == fixture.manifest
            && release.home_release.release_digest == fixture.manifest.release_digest()
            && release.home_release.published_by == ALICE_ID.clone()
            && !release.home_release.yank.yanked,
        "{context}: home release record is incomplete: {release:?}"
    );
    let resolver =
        client.query_single(FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
            package: fixture.release.package.clone(),
            requirement: None,
            page: musubi_fault_page(),
        }))?;
    let [row] = resolver.items.as_slice() else {
        return Err(eyre!(
            "{context}: resolver returned {} rows instead of one",
            resolver.items.len()
        ));
    };
    ensure!(
        resolver.next_cursor.is_none()
            && row.release == fixture.release
            && row.release_digest == fixture.manifest.release_digest()
            && row.archive_id == fixture.archive_id
            && row.source_digest == musubi_fault_archive_commitment().source_tree_digest
            && row.interface_digest == fixture.manifest.interface_digest
            && row.abi == fixture.manifest.abi
            && row.dependencies.is_empty()
            && row.selection.fresh_selectable(),
        "{context}: universal resolver row is incomplete: {row:?}"
    );
    let directory_prefix = format!("{MUSUBI_FAULT_NAMESPACE}/");
    let directory =
        client.query_single(FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&directory_prefix)
                .expect("Musubi fault directory prefix"),
            page: musubi_fault_page(),
        }))?;
    let [entry] = directory.items.as_slice() else {
        return Err(eyre!(
            "{context}: directory returned {} entries instead of one",
            directory.items.len()
        ));
    };
    ensure!(
        directory.namespace_binding == fixture.binding
            && directory.next_cursor.is_none()
            && entry.package == fixture.release.package
            && entry.selector.namespace == fixture.binding.namespace
            && entry.selector.name == fixture.release.package.name
            && entry.latest_selectable.as_ref() == Some(&fixture.release.version),
        "{context}: universal directory entry is incomplete: {entry:?}"
    );
    let locations = client.query_single(FindMusubiArchiveLocationsV1::new(
        MusubiArchiveLocationQueryV1 {
            archive_id: fixture.archive_id,
            page: musubi_fault_page(),
        },
    ))?;
    let [location] = locations.items.as_slice() else {
        return Err(eyre!(
            "{context}: published archive returned {} locations instead of one",
            locations.items.len()
        ));
    };
    ensure!(
        location.location_id == fixture.location_id
            && location.pin_manifest == fixture.pin_manifest
            && location.replication_order == fixture.replication_order
            && location.providers == musubi_fault_replica_providers()
            && location.state == MusubiArchiveLocationStateV1::Healthy,
        "{context}: published archive location is incomplete: {location:?}"
    );
    let retention = client.query_single(FindMusubiArchiveRetentionV1::new(
        MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![fixture.archive_id],
            expected_snapshot: None,
        },
    ))?;
    let [decision] = retention.items.as_slice() else {
        return Err(eyre!(
            "{context}: published retention query returned the wrong item count"
        ));
    };
    ensure!(
        decision.disposition == MusubiArchiveRetentionDispositionV1::RetainReferenced
            && decision.active_releases == 1
            && decision.yanked_releases == 0
            && decision.taken_down_releases == 0
            && decision.storage.is_some_and(|storage| {
                storage.archive_id == fixture.archive_id
                    && storage.availability == MusubiStorageAvailabilityV1::Selectable
                    && storage.healthy_replicas == 3
                    && storage.active_locations == 1
            }),
        "{context}: published archive retention state is incomplete: {decision:?}"
    );
    ensure!(
        resolver.snapshot == directory.snapshot
            && resolver.snapshot == locations.snapshot
            && resolver.snapshot == retention.snapshot,
        "{context}: home/universal publication tuple was read from different snapshots"
    );
    Ok(resolver.snapshot)
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
            leg.prepare_qc.validator_set().len() == PEERS
                && leg.commit_qc.validator_set().len() == PEERS,
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
fn next_universal_autonomous_lane_author_peer(
    peers: &[NetworkPeer],
    context: &str,
) -> Result<usize> {
    let diagnostics = peers
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let diagnostics = peer
                .client()
                .get_sumeragi_diagnostics()
                .wrap_err_with(|| format!("{context}: query pre-cut peer {index} diagnostics"))?;
            let ownership = diagnostics
                .lane_payload_ownerships
                .iter()
                .find(|ownership| {
                    ownership.lane_id == LaneId::new(UNIVERSAL_LANE)
                        && ownership.dataspace_id == DataSpaceId::UNIVERSAL
                })
                .cloned()
                .ok_or_else(|| {
                    eyre!("{context}: pre-cut peer {index} has no universal-lane frontier")
                })?;
            ownership.validate_replay_material().map_err(|err| {
                eyre!(
                    "{context}: pre-cut peer {index} has an invalid universal-lane frontier: {err:?}"
                )
            })?;
            let descriptor_hash = ownership.lane_block_descriptor_hash.ok_or_else(|| {
                eyre!("{context}: universal-lane frontier has no descriptor hash")
            })?;
            ensure!(
                diagnostics.committed_lane_blocks.iter().any(|block| {
                    block.lane_id == ownership.lane_id
                        && block.dataspace_id == ownership.dataspace_id
                        && block.lane_incarnation == ownership.lane_incarnation
                        && block.lane_block_height == ownership.lane_block_height
                        && block.lane_block_view == ownership.lane_block_view
                        && block.descriptor_hash == descriptor_hash
                        && block.executable_payload_available
                        && matches!(
                            block.execution_status.as_str(),
                            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
                                | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
                        )
                }),
                "{context}: pre-cut peer {index} universal-lane frontier is not durably applied"
            );
            Ok(ownership)
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .wrap_err_with(|| format!("{context}: derive exact pre-cut universal-lane frontiers"))?;
    let reference = diagnostics
        .first()
        .ok_or_else(|| eyre!("{context}: phase-cut network has no lane diagnostics"))?;
    ensure!(
        diagnostics.iter().all(|ownership| ownership == reference),
        "{context}: validators do not share one exact universal-lane frontier"
    );
    let validator_set = &reference.lane_block_descriptor_validator_set;
    let controlled_peers = peers.iter().map(NetworkPeer::id).collect::<BTreeSet<_>>();
    let committee = validator_set.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(
        validator_set.len() == PEERS
            && committee.len() == validator_set.len()
            && committee == controlled_peers
            && reference.lane_block_descriptor_validator_count
                == u32::try_from(validator_set.len()).unwrap_or(u32::MAX),
        "{context}: universal-lane committee is not the exact controlled four-peer set"
    );
    let next_lane_block_height = reference
        .lane_block_height
        .checked_add(1)
        .ok_or_else(|| eyre!("{context}: universal lane height overflow"))?;
    let validator_count = u64::try_from(validator_set.len())
        .wrap_err("universal-lane validator count does not fit u64")?;
    let author_index = usize::try_from((next_lane_block_height - 1) % validator_count)
        .wrap_err("universal-lane author index does not fit usize")?;
    let author = validator_set
        .get(author_index)
        .ok_or_else(|| eyre!("{context}: universal-lane author index is out of bounds"))?;
    let matching_peers = peers
        .iter()
        .enumerate()
        .filter(|(_, peer)| peer.id() == author.clone())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [target_index] = matching_peers.as_slice() else {
        return Err(eyre!(
            "{context}: exact universal-lane author maps to {} controlled peers",
            matching_peers.len()
        ));
    };
    Ok(*target_index)
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
        .map(Client::prepare_transaction_payload)
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
        lane_history_retention: defaults::kura::LANE_HISTORY_RETENTION,
        replica_advert: defaults::kura::REPLICA_ADVERT_POLICY,
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
    qualification_scenarios::run_mixed_dataspace_native_amx_routes_and_commits_with_receipts().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_amx_queue_journal_replays_plan_after_restart() -> Result<()> {
    qualification_scenarios::run_native_amx_queue_journal_replays_plan_after_restart().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn musubi_publication_below_quorum_queue_crash_replay_keeps_projection_tuple_absent()
-> Result<()> {
    qualification_scenarios::run_musubi_publication_below_quorum_queue_crash_replay_keeps_projection_tuple_absent().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn musubi_selectable_publication_phase_cut_matrix_is_atomic_after_replay() -> Result<()> {
    selectable_publication_gate::run().await
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
    qualification_scenarios::run_native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs().await
}
