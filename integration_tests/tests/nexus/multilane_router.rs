#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Multi-lane routing and storage provisioning regression.

use std::{collections::BTreeMap, num::NonZeroU32, sync::Arc, time::Duration};

use eyre::Result;
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{
            Crypto, Kura as KuraConfig, LaneConfig as LaneDerivedConfig, LaneRoutingMatcher,
            LaneRoutingPolicy, LaneRoutingRule,
        },
        defaults,
    },
};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, LaneRouter},
    state::{State, World},
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    da::commitment::DaProofScheme,
    isi::{
        InstructionBox,
        prelude::{Mint, Register},
    },
    metadata::Metadata,
    nexus::{
        AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
        DataSpaceMetadata, LaneCatalog, LaneConfig as LaneConfigMetadata, LaneId,
        LaneStorageProfile, LaneVisibility,
    },
    prelude::*,
    transaction::TransactionBuilder,
};
use iroha_primitives::time::TimeSource;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use tempfile::tempdir;

fn sample_catalogs() -> (LaneCatalog, DataSpaceCatalog, LaneRoutingPolicy) {
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(3).expect("non-zero lane count"),
        vec![
            LaneConfigMetadata {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "core".to_owned(),
                description: Some("Primary execution lane".to_owned()),
                visibility: LaneVisibility::Public,
                lane_type: Some("default_public".to_owned()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
            LaneConfigMetadata {
                id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(1),
                alias: "governance".to_owned(),
                description: Some("Governance & parliament traffic".to_owned()),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("governance".to_owned()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
            LaneConfigMetadata {
                id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(2),
                alias: "zk".to_owned(),
                description: Some("Zero-knowledge attachments".to_owned()),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("attachments".to_owned()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
        ],
    )
    .expect("lane catalog");

    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata {
            id: DataSpaceId::UNIVERSAL,
            alias: "universal".to_owned(),
            description: Some("Single-lane data space".to_owned()),
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "governance".to_owned(),
            description: Some("Governance proposals & manifests".to_owned()),
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(2),
            alias: "zk".to_owned(),
            description: Some("Zero-knowledge proofs and attachments".to_owned()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");

    let policy = LaneRoutingPolicy {
        default_lane: LaneId::new(0),
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![
            LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(1)),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::domain".to_owned()),
                    description: Some("governance lane for registration".to_owned()),
                },
            },
            LaneRoutingRule {
                lane: LaneId::new(2),
                dataspace: Some(DataSpaceId::new(2)),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("mint".to_owned()),
                    description: Some("zk lane for mint flows".to_owned()),
                },
            },
        ],
    };

    (lane_catalog, dataspace_catalog, policy)
}

fn autoscale_elastic_lane(id: LaneId, created_height: u64) -> LaneConfigMetadata {
    let mut metadata = BTreeMap::new();
    metadata.insert(AUTOSCALE_META_MANAGED.to_owned(), "true".to_owned());
    metadata.insert(
        AUTOSCALE_META_CREATED_HEIGHT.to_owned(),
        created_height.to_string(),
    );
    LaneConfigMetadata {
        id,
        dataspace_id: DataSpaceId::UNIVERSAL,
        alias: format!("elastic-lane-{}", id.as_u32()),
        description: Some("Consensus-managed elastic lane".to_owned()),
        visibility: LaneVisibility::Public,
        lane_type: Some("autoscale_elastic".to_owned()),
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata,
    }
}

fn install_state_nexus(
    lane_catalog: LaneCatalog,
    dataspace_catalog: DataSpaceCatalog,
    policy: LaneRoutingPolicy,
    autoscale_range: Option<(u32, u32)>,
) -> Result<State> {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::default(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::default(), kura, query);
    let mut nexus = iroha_config::parameters::actual::Nexus {
        enabled: true,
        routing_policy: policy,
        dataspace_catalog,
        lane_config: LaneDerivedConfig::from_catalog(&lane_catalog),
        lane_catalog,
        ..Default::default()
    };
    if let Some((min_lanes, max_lanes)) = autoscale_range {
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes =
            NonZeroU32::new(min_lanes).expect("autoscale min lanes must be nonzero");
        nexus.autoscale.max_lanes =
            NonZeroU32::new(max_lanes).expect("autoscale max lanes must be nonzero");
    }
    *state.nexus.write() = nexus;
    Ok(state)
}

fn build_tx(
    chain_id: &ChainId,
    authority: &AccountId,
    keypair: &KeyPair,
    instructions: Vec<InstructionBox>,
) -> AcceptedTransaction<'static> {
    let time_source = TimeSource::new_system();
    let tx =
        TransactionBuilder::new_with_time_source(chain_id.clone(), authority.clone(), &time_source)
            .with_instructions(instructions)
            .with_metadata(Metadata::default())
            .sign(keypair.private_key());
    let default_limits = TransactionParameters::default();
    let params = TransactionParameters::with_max_signatures(
        nonzero!(16_u64),
        nonzero!(4096_u64),
        nonzero!(4096_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let crypto_cfg = Crypto::default();
    AcceptedTransaction::accept(tx, chain_id, Duration::from_secs(30), params, &crypto_cfg)
        .expect("transaction should be accepted")
}

#[test]
fn multilane_router_provisions_storage_and_routes_rules() -> Result<()> {
    let (lane_catalog, dataspace_catalog, policy) = sample_catalogs();
    let lane_config = LaneDerivedConfig::from_catalog(&lane_catalog);

    let temp = tempdir()?;
    let store_dir = temp.path().join("kura");
    std::fs::create_dir_all(&store_dir)?;

    let kura_cfg = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(store_dir.clone()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Off,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
        eviction_required_replicas:
            iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
    };

    let (kura, block_count) = Kura::new(&kura_cfg, &lane_config)?;
    assert_eq!(block_count.0, 0, "fresh store should be empty");

    for entry in lane_config.entries() {
        let blocks_dir = entry.blocks_dir(&store_dir);
        assert!(
            blocks_dir.exists(),
            "lane {} blocks dir should be created",
            entry.lane_id.as_u32()
        );
        assert!(
            blocks_dir
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("lane_")),
            "blocks dir should use lane slug naming"
        );

        let merge_log = entry.merge_log_path(&store_dir);
        assert!(
            merge_log.exists(),
            "lane {} merge log should be created",
            entry.lane_id.as_u32()
        );
    }

    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        policy,
        dataspace_catalog,
        lane_catalog,
    ));
    let (authority, keypair) = gen_account_in("nexus");
    let chain_id = ChainId::from("nexus-multilane");

    let governance_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Register::domain(Domain::new(
            DomainId::try_new("gov", "universal")?,
        )))],
    );
    let zk_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Mint::asset_numeric(
            1_u32,
            AssetId::new(
                AssetDefinitionId::new(DomainId::try_new("nexus", "universal")?, "xor".parse()?),
                authority.clone(),
            ),
        ))],
    );
    let default_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(AssetDefinitionId::new(
                DomainId::try_new("nexus", "universal")?,
                "xor".parse()?,
            )),
        ))],
    );

    let decision = router.route(&governance_tx);
    assert_eq!(decision.lane_id, LaneId::new(1));
    assert_eq!(decision.dataspace_id, DataSpaceId::new(1));

    let decision = router.route(&zk_tx);
    assert_eq!(decision.lane_id, LaneId::new(2));
    assert_eq!(decision.dataspace_id, DataSpaceId::new(2));

    let decision = router.route(&default_tx);
    assert_eq!(decision.lane_id, LaneId::new(0));
    assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);

    drop(kura);
    Ok(())
}

#[test]
fn multilane_router_shards_default_route_over_autoscale_elastic_lanes() -> Result<()> {
    let (base_lane_catalog, dataspace_catalog, policy) = sample_catalogs();
    let mut lanes = base_lane_catalog.lanes().to_vec();
    lanes.push(autoscale_elastic_lane(LaneId::new(3), 7));
    lanes.push(autoscale_elastic_lane(LaneId::new(4), 7));
    let lane_catalog =
        LaneCatalog::new(NonZeroU32::new(5).expect("lane count"), lanes).expect("lane catalog");
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        policy.clone(),
        dataspace_catalog.clone(),
        lane_catalog.clone(),
    ));
    let state = install_state_nexus(lane_catalog, dataspace_catalog, policy, Some((3, 5)))?;

    let (authority, keypair) = gen_account_in("nexus");
    let chain_id = ChainId::from("nexus-multilane-autoscale");

    let governance_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Register::domain(Domain::new(
            DomainId::try_new("governed", "universal")?,
        )))],
    );
    let governance = router.route_with_view(&governance_tx, &state.view());
    assert_eq!(governance.lane_id, LaneId::new(1));
    assert_eq!(governance.dataspace_id, DataSpaceId::new(1));

    let zk_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Mint::asset_numeric(
            1_u32,
            AssetId::new(
                AssetDefinitionId::new(DomainId::try_new("nexus", "universal")?, "xor".parse()?),
                authority.clone(),
            ),
        ))],
    );
    let zk = router.route_with_view(&zk_tx, &state.view());
    assert_eq!(zk.lane_id, LaneId::new(2));
    assert_eq!(zk.dataspace_id, DataSpaceId::new(2));

    let mut lanes_seen = std::collections::BTreeSet::new();
    for idx in 0..512 {
        let role_id = iroha_data_model::role::RoleId {
            name: format!("elasticroute{idx}")
                .parse()
                .expect("valid role name"),
        };
        let default_tx = build_tx(
            &chain_id,
            &authority,
            &keypair,
            vec![InstructionBox::from(Register::role(
                iroha_data_model::role::Role::new(role_id, authority.clone()),
            ))],
        );
        let decision = router.route_with_view(&default_tx, &state.view());
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
        assert!(
            matches!(
                decision.lane_id,
                lane if lane == LaneId::new(0)
                    || lane == LaneId::new(3)
                    || lane == LaneId::new(4)
            ),
            "default-route autoscale sharding chose unexpected lane {:?}",
            decision.lane_id
        );
        lanes_seen.insert(decision.lane_id);
        if lanes_seen.len() == 3 {
            break;
        }
    }

    assert_eq!(
        lanes_seen,
        std::collections::BTreeSet::from([LaneId::new(0), LaneId::new(3), LaneId::new(4)]),
        "default-route traffic should shard across the base lane and autoscale elastic lanes"
    );

    Ok(())
}

#[test]
fn multilane_router_fails_closed_when_elastic_range_contains_corruption() -> Result<()> {
    struct CorruptionCase {
        name: &'static str,
        lane: LaneConfigMetadata,
    }

    let mut malformed_managed = autoscale_elastic_lane(LaneId::new(4), 7);
    malformed_managed.alias = "malformed-elastic-lane".to_owned();
    let mut off_default_managed = autoscale_elastic_lane(LaneId::new(4), 7);
    off_default_managed.dataspace_id = DataSpaceId::new(1);

    let cases = [
        CorruptionCase {
            name: "manual",
            lane: LaneConfigMetadata {
                id: LaneId::new(4),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "manual-elastic-range".to_owned(),
                description: Some("Manual lane occupying autoscale range".to_owned()),
                visibility: LaneVisibility::Public,
                lane_type: Some("manual".to_owned()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
        },
        CorruptionCase {
            name: "malformed",
            lane: malformed_managed,
        },
        CorruptionCase {
            name: "offdefault",
            lane: off_default_managed,
        },
    ];

    for case in cases {
        let (base_lane_catalog, dataspace_catalog, policy) = sample_catalogs();
        let mut lanes = base_lane_catalog.lanes().to_vec();
        lanes.push(autoscale_elastic_lane(LaneId::new(3), 7));
        lanes.push(case.lane);
        let lane_catalog = LaneCatalog::new(NonZeroU32::new(5).expect("lane count"), lanes)
            .unwrap_or_else(|err| {
                panic!("{}: corrupted lane catalog should build: {err}", case.name)
            });
        let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
            policy.clone(),
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        ));
        let state = install_state_nexus(lane_catalog, dataspace_catalog, policy, Some((3, 5)))
            .unwrap_or_else(|err| panic!("{}: install Nexus state: {err}", case.name));

        let (authority, keypair) = gen_account_in("nexus");
        let chain_id = ChainId::from(format!("nexus-multilane-corrupt-{}", case.name));
        let mut lanes_seen = std::collections::BTreeSet::new();

        for idx in 0..128 {
            let role_id = iroha_data_model::role::RoleId {
                name: format!("corrupt{}{}", case.name, idx)
                    .parse()
                    .expect("valid role name"),
            };
            let default_tx = build_tx(
                &chain_id,
                &authority,
                &keypair,
                vec![InstructionBox::from(Register::role(
                    iroha_data_model::role::Role::new(role_id, authority.clone()),
                ))],
            );
            let decision = router.route_with_view(&default_tx, &state.view());
            assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
            lanes_seen.insert(decision.lane_id);
        }

        assert_eq!(
            lanes_seen,
            std::collections::BTreeSet::from([LaneId::new(0)]),
            "{} corruption inside the active autoscale range must keep default traffic on the base lane",
            case.name
        );
    }

    Ok(())
}

#[test]
fn multilane_router_ignores_stale_autoscale_lanes_when_autoscale_disabled() -> Result<()> {
    let (base_lane_catalog, dataspace_catalog, policy) = sample_catalogs();
    let mut lanes = base_lane_catalog.lanes().to_vec();
    lanes.push(autoscale_elastic_lane(LaneId::new(3), 7));
    lanes.push(autoscale_elastic_lane(LaneId::new(4), 7));
    let lane_catalog =
        LaneCatalog::new(NonZeroU32::new(5).expect("lane count"), lanes).expect("lane catalog");
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        policy.clone(),
        dataspace_catalog.clone(),
        lane_catalog.clone(),
    ));
    let state = install_state_nexus(lane_catalog, dataspace_catalog, policy, Some((3, 5)))?;
    state.nexus.write().autoscale.enabled = false;

    let (authority, keypair) = gen_account_in("nexus");
    let chain_id = ChainId::from("nexus-multilane-autoscale-disabled");
    let mut lanes_seen = std::collections::BTreeSet::new();

    for idx in 0..128 {
        let role_id = iroha_data_model::role::RoleId {
            name: format!("staleelasticdisabled{idx}")
                .parse()
                .expect("valid role name"),
        };
        let default_tx = build_tx(
            &chain_id,
            &authority,
            &keypair,
            vec![InstructionBox::from(Register::role(
                iroha_data_model::role::Role::new(role_id, authority.clone()),
            ))],
        );
        let decision = router.route_with_view(&default_tx, &state.view());
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
        lanes_seen.insert(decision.lane_id);
    }

    assert_eq!(
        lanes_seen,
        std::collections::BTreeSet::from([LaneId::new(0)]),
        "disabled autoscale must keep default-route traffic on the base lane even when stale managed lanes remain in the catalog"
    );

    Ok(())
}

#[test]
fn multilane_router_ignores_stale_autoscale_lanes_when_nexus_disabled() -> Result<()> {
    let (base_lane_catalog, dataspace_catalog, policy) = sample_catalogs();
    let mut lanes = base_lane_catalog.lanes().to_vec();
    lanes.push(autoscale_elastic_lane(LaneId::new(3), 7));
    lanes.push(autoscale_elastic_lane(LaneId::new(4), 7));
    let lane_catalog =
        LaneCatalog::new(NonZeroU32::new(5).expect("lane count"), lanes).expect("lane catalog");
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        policy.clone(),
        dataspace_catalog.clone(),
        lane_catalog.clone(),
    ));
    let state = install_state_nexus(lane_catalog, dataspace_catalog, policy, Some((3, 5)))?;
    state.nexus.write().enabled = false;

    let (authority, keypair) = gen_account_in("nexus");
    let chain_id = ChainId::from("nexus-multilane-nexus-disabled");
    let mut lanes_seen = std::collections::BTreeSet::new();

    for idx in 0..128 {
        let role_id = iroha_data_model::role::RoleId {
            name: format!("staleelasticnexusdisabled{idx}")
                .parse()
                .expect("valid role name"),
        };
        let default_tx = build_tx(
            &chain_id,
            &authority,
            &keypair,
            vec![InstructionBox::from(Register::role(
                iroha_data_model::role::Role::new(role_id, authority.clone()),
            ))],
        );
        let decision = router.route_with_view(&default_tx, &state.view());
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);
        lanes_seen.insert(decision.lane_id);
    }

    assert_eq!(
        lanes_seen,
        std::collections::BTreeSet::from([LaneId::new(0)]),
        "disabled Nexus must keep default-route traffic on the base lane even when stale managed lanes remain in the catalog"
    );

    Ok(())
}
