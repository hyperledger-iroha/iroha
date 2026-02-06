#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Multi-lane routing and storage provisioning integration.

use std::{collections::BTreeMap, num::NonZeroU32, sync::Arc, time::Duration};

use eyre::Result;
use iroha_config::{
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{
            Crypto, Kura as KuraConfig, LaneConfig as LaneDerivedConfig, LaneRoutingMatcher,
            LaneRoutingPolicy, LaneRoutingRule,
        },
        defaults,
    },
};
use iroha_config_base::WithOrigin;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, LaneRouter, RoutingDecision},
    state::{State, World},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    da::commitment::DaProofScheme,
    isi::{InstructionBox, prelude::SetKeyValue},
    metadata::Metadata,
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig as LaneConfigMetadata, LaneId, LaneStorageProfile, LaneVisibility,
    },
    prelude::*,
    transaction::TransactionBuilder,
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use tempfile::tempdir;

const TEST_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";

fn sample_transaction(
    authority: &AccountId,
    signer: &iroha_crypto::PrivateKey,
    instructions: Vec<InstructionBox>,
) -> AcceptedTransaction<'static> {
    let chain_id = ChainId::from(TEST_CHAIN_ID);
    let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_instructions(instructions)
        .with_metadata(Metadata::default())
        .sign(signer);
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
    AcceptedTransaction::accept(tx, &chain_id, Duration::from_secs(30), params, &crypto_cfg)
        .expect("transaction should be accepted")
}

fn blank_state() -> State {
    let world = World::default();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    {
        let telemetry = iroha_core::telemetry::StateTelemetry::default();
        State::with_telemetry(world, kura, query, telemetry)
    }
    #[cfg(not(feature = "telemetry"))]
    State::new(world, kura, query)
}

#[test]
#[allow(clippy::too_many_lines)]
fn multilane_catalog_sets_up_storage_and_routing() -> Result<()> {
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(3).expect("non-zero lane count"),
        vec![
            LaneConfigMetadata {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "core".to_string(),
                description: Some("Primary execution lane".to_string()),
                visibility: LaneVisibility::Public,
                lane_type: Some("default_public".to_string()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
            LaneConfigMetadata {
                id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(1),
                alias: "governance".to_string(),
                description: Some("Governance & parliament traffic".to_string()),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("governance".to_string()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
            LaneConfigMetadata {
                id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(2),
                alias: "zk".to_string(),
                description: Some("Zero-knowledge attachments".to_string()),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("attachments".to_string()),
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            },
        ],
    )
    .expect("static multi-lane catalog");
    let lane_config = LaneDerivedConfig::from_catalog(&lane_catalog);

    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "governance".to_string(),
            description: Some("Governance proposals & manifests".to_string()),
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(2),
            alias: "zk".to_string(),
            description: Some("Zero-knowledge proofs and attachments".to_string()),
            fault_tolerance: 1,
        },
    ])
    .expect("static multi-dataspace catalog");

    let temp = tempdir()?;
    let kura_cfg = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(temp.path().to_path_buf()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Off,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
    };

    let (_kura, block_count) = Kura::new(&kura_cfg, &lane_config)?;
    assert_eq!(block_count.0, 0, "fresh Kura should have no blocks");

    for entry in lane_config.entries() {
        let blocks_dir = entry.blocks_dir(temp.path());
        assert!(
            blocks_dir.is_dir(),
            "expected blocks dir for lane {} at {}",
            entry.alias,
            blocks_dir.display()
        );
        let merge_log = entry.merge_log_path(temp.path());
        assert!(
            merge_log.is_file(),
            "expected merge log for lane {} at {}",
            entry.alias,
            merge_log.display()
        );
    }

    let (core_account, core_keys) = gen_account_in("core");
    let (gov_account, gov_keys) = gen_account_in("governance");
    let (zk_account, zk_keys) = gen_account_in("zk");

    let routing_policy = LaneRoutingPolicy {
        default_lane: LaneId::new(0),
        default_dataspace: DataSpaceId::GLOBAL,
        rules: vec![
            LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(1)),
                matcher: LaneRoutingMatcher {
                    account: Some(gov_account.to_string()),
                    instruction: None,
                    description: Some("route governance account to lane 1".to_string()),
                },
            },
            LaneRoutingRule {
                lane: LaneId::new(2),
                dataspace: Some(DataSpaceId::new(2)),
                matcher: LaneRoutingMatcher {
                    account: Some(zk_account.to_string()),
                    instruction: None,
                    description: Some("route zk account to lane 2".to_string()),
                },
            },
        ],
    };

    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        routing_policy.clone(),
        dataspace_catalog,
        lane_catalog.clone(),
    ));
    let state = blank_state();

    let tx_core = sample_transaction(
        &core_account,
        core_keys.private_key(),
        vec![InstructionBox::from(SetKeyValue::account(
            core_account.clone(),
            "k0".parse().expect("metadata key"),
            Json::new("v0"),
        ))],
    );
    let tx_gov = sample_transaction(
        &gov_account,
        gov_keys.private_key(),
        vec![InstructionBox::from(SetKeyValue::account(
            gov_account.clone(),
            "gk".parse().expect("metadata key"),
            Json::new("gv"),
        ))],
    );
    let tx_zk = sample_transaction(
        &zk_account,
        zk_keys.private_key(),
        vec![InstructionBox::from(SetKeyValue::account(
            zk_account.clone(),
            "zk".parse().expect("metadata key"),
            Json::new("zv"),
        ))],
    );

    let decision_core = router.route(&tx_core, &state.view());
    assert_eq!(
        decision_core,
        RoutingDecision::new(LaneId::new(0), DataSpaceId::GLOBAL)
    );

    let decision_gov = router.route(&tx_gov, &state.view());
    assert_eq!(
        decision_gov,
        RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1))
    );

    let decision_zk = router.route(&tx_zk, &state.view());
    assert_eq!(
        decision_zk,
        RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2))
    );

    Ok(())
}
