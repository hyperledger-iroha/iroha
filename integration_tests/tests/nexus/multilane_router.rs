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
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig as LaneConfigMetadata, LaneId, LaneStorageProfile, LaneVisibility,
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
                dataspace_id: DataSpaceId::GLOBAL,
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
            id: DataSpaceId::GLOBAL,
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
        default_dataspace: DataSpaceId::GLOBAL,
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

fn blank_state() -> State {
    let world = World::default();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_core::telemetry::StateTelemetry::default();
    #[cfg(feature = "telemetry")]
    return State::with_telemetry(world, kura, query, telemetry);
    #[cfg(not(feature = "telemetry"))]
    State::new(world, kura, query)
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
            "gov".parse()?,
        )))],
    );
    let zk_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Mint::asset_numeric(
            1_u32,
            AssetId::new("xor#nexus".parse()?, authority.clone()),
        ))],
    );
    let default_tx = build_tx(
        &chain_id,
        &authority,
        &keypair,
        vec![InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric("xor#nexus".parse()?),
        ))],
    );

    let _state = blank_state();
    let decision = router.route(&governance_tx);
    assert_eq!(decision.lane_id, LaneId::new(1));
    assert_eq!(decision.dataspace_id, DataSpaceId::new(1));

    let decision = router.route(&zk_tx);
    assert_eq!(decision.lane_id, LaneId::new(2));
    assert_eq!(decision.dataspace_id, DataSpaceId::new(2));

    let decision = router.route(&default_tx);
    assert_eq!(decision.lane_id, LaneId::new(0));
    assert_eq!(decision.dataspace_id, DataSpaceId::GLOBAL);

    drop(kura);
    Ok(())
}
