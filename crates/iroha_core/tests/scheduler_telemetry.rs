//! Scheduler telemetry smoke tests (telemetry feature only).
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensures layer metrics and utilization are populated after block application.
#![allow(unused_imports)]

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use iroha_config::parameters::actual::{
    LaneCompliance, LaneConfig as RuntimeLaneConfig, LaneRelayEmergency, NexusAxt,
    NexusEndorsement, NexusFees, NexusStaking, NexusStorage,
};
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::{
        GovernanceHooks, GovernanceRules, LaneManifestRegistry, LaneManifestStatus,
        RuntimeUpgradeHook,
    },
    state::StateReadOnly,
    telemetry::LaneTeuGaugeUpdate,
};
use iroha_data_model::{
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig as ModelLaneConfig, LaneId, LaneStorageProfile, LaneVisibility,
    },
    prelude::*,
};

#[test]
#[cfg(feature = "telemetry")]
#[allow(clippy::too_many_lines)]
fn scheduler_layer_metrics_and_utilization_populated() {
    let chain_id = ChainId::from("chain");
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (carol_id, _) = iroha_test_samples::gen_account_in("wonderland");

    // World: two accounts + one asset def
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let acc_b = Account::new(bob_id.clone()).build(&alice_id);
    let acc_c = Account::new(carol_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b, acc_c], [ad]);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
    let telemetry = iroha_core::telemetry::StateTelemetry::new(metrics.clone(), true);
    let state = iroha_core::state::State::with_telemetry(world, kura, query, telemetry);

    // Build 3 txs with trivial conflicts to force at least two layers:
    // 1) Mint to Alice (independent)
    // 2) Transfer from Alice to Bob (depends on 1)
    // 3) Set metadata on Carol (independent)
    let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "coin".parse().unwrap(),
    );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(10_u32, a_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx2 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            a_coin.clone(),
            5_u32,
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx3 = TransactionBuilder::new(chain_id.clone(), carol_id.clone())
        .with_instructions([SetKeyValue::account(
            carol_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

    let acc: Vec<_> = vec![tx1, tx2, tx3]
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(t)))
        .collect();
    let new_block = BlockBuilder::new(acc)
        .chain(0, None)
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let _events = sb.apply_without_execution(&cb, Vec::new());

    // Assert telemetry populated
    let m = Arc::clone(&metrics);
    assert!(
        m.pipeline_layer_count.get() >= 1,
        "layer count should be set"
    );
    // Utilization pct is 0..100
    let util = m.pipeline_scheduler_utilization_pct.get();
    assert!(util <= 100, "utilization pct must be <= 100");
    // Peak width >= 1, and average width <= peak
    let peak = m.pipeline_peak_layer_width.get();
    let avg = m.pipeline_layer_avg_width.get();
    if peak > 0 {
        assert!(avg <= peak, "avg width must be <= peak width");
    }
    let lane_label = LaneId::SINGLE.as_u32().to_string();
    let gauge_capacity = m
        .nexus_scheduler_lane_teu_capacity
        .with_label_values(&[lane_label.as_str()])
        .get();
    let gauge_starvation = m
        .nexus_scheduler_starvation_bound_slots
        .with_label_values(&[lane_label.as_str()])
        .get();
    let lane_snapshots = m
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned");
    let lane_snapshot = lane_snapshots
        .get(&LaneId::SINGLE.as_u32())
        .expect("lane snapshot missing");
    assert_eq!(lane_snapshot.capacity, gauge_capacity);
    assert_eq!(lane_snapshot.starvation_bound_slots, gauge_starvation);
    assert!(
        !lane_snapshot.manifest_required,
        "default lane should not require a manifest"
    );
    assert!(
        lane_snapshot.manifest_ready,
        "default lane should report manifest ready"
    );
    assert!(lane_snapshot.manifest_path.is_none());
    assert!(lane_snapshot.manifest_validators.is_empty());
    assert!(lane_snapshot.manifest_quorum.is_none());
    assert!(lane_snapshot.manifest_protected_namespaces.is_empty());
    assert!(lane_snapshot.manifest_runtime_upgrade.is_none());
    assert!(
        lane_snapshot.layer_count >= 1,
        "lane layer count should be populated"
    );
    assert!(
        lane_snapshot.peak_layer_width >= lane_snapshot.avg_layer_width,
        "peak width must be >= avg width"
    );
    assert!(
        lane_snapshot.scheduler_utilization_pct <= 100,
        "lane utilization pct must be <= 100"
    );
    assert_eq!(
        lane_snapshot.layer_width_buckets.as_slice()[7],
        lane_snapshot.layer_count,
        "histogram upper bucket should equal layer count"
    );
    let dataspace_label = lane_snapshot.dataspace_id.to_string();
    let block_height_metric = m
        .nexus_lane_block_height
        .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
        .get();
    assert_eq!(
        block_height_metric, lane_snapshot.block_height,
        "block height gauge should reflect lane snapshot height"
    );
    let finality_lag_metric = m
        .nexus_lane_finality_lag_slots
        .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
        .get();
    assert_eq!(
        finality_lag_metric, lane_snapshot.finality_lag_slots,
        "finality lag gauge should mirror lane snapshot lag"
    );
    assert_eq!(
        m.nexus_scheduler_dataspace_teu_backlog
            .with_label_values(&[lane_label.as_str(), "0"])
            .get(),
        0
    );
}

#[test]
#[cfg(feature = "telemetry")]
#[allow(clippy::too_many_lines)]
fn nexus_lane_and_dataspace_metadata_exposed() {
    use iroha_telemetry::metrics::Metrics;
    use nonzero_ext::nonzero;

    let metrics = Arc::new(Metrics::default());
    let telemetry = iroha_core::telemetry::StateTelemetry::new(metrics.clone(), true);

    let mut lane_core = ModelLaneConfig {
        id: LaneId::new(0),
        dataspace_id: DataSpaceId::new(7),
        alias: "core".to_string(),
        visibility: LaneVisibility::Public,
        lane_type: Some("execution".to_string()),
        governance: Some("parliament".to_string()),
        settlement: Some("xor".to_string()),
        storage: LaneStorageProfile::FullReplica,
        ..ModelLaneConfig::default()
    };
    lane_core
        .metadata
        .insert("scheduler.teu_capacity".to_string(), "4096".to_string());
    lane_core.metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "33".to_string(),
    );
    let lane_ops = ModelLaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(11),
        alias: "ops".to_string(),
        lane_type: Some("ops".to_string()),
        settlement: Some("xor_lane_weighted".to_string()),
        ..ModelLaneConfig::default()
    };
    let primary_dataspace = DataSpaceMetadata {
        id: DataSpaceId::new(7),
        alias: "alpha".to_string(),
        description: Some("Primary execution dataspace".to_string()),
        fault_tolerance: 1,
    };
    let secondary_dataspace = DataSpaceMetadata {
        id: DataSpaceId::new(11),
        alias: "beta".to_string(),
        description: Some("Operations dataspace".to_string()),
        fault_tolerance: 1,
    };
    let lane_catalog = LaneCatalog::new(nonzero!(4_u32), vec![lane_core.clone(), lane_ops.clone()])
        .expect("lane catalog");
    let dataspace_catalog =
        DataSpaceCatalog::new(vec![primary_dataspace.clone(), secondary_dataspace.clone()])
            .expect("dataspace catalog");

    telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);

    telemetry.record_nexus_scheduler_lane_teu(
        LaneId::new(0),
        iroha_core::telemetry::LaneTeuGaugeUpdate {
            capacity: 4_096,
            committed: 512,
            buckets: iroha_telemetry::metrics::NexusLaneTeuBuckets {
                floor: 256,
                headroom: 512,
                must_serve: 0,
                circuit_breaker: 0,
            },
            trigger_level: 0,
            starvation_bound_slots: 10,
        },
    );

    telemetry.record_nexus_scheduler_dataspace_teu(
        LaneId::new(0),
        DataSpaceId::new(7),
        iroha_core::telemetry::DataspaceTeuGaugeUpdate {
            backlog: 42,
            age_slots: 3,
            virtual_finish: 11,
        },
    );

    let lane_snapshot = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .get(&0)
        .cloned()
        .expect("lane snapshot missing");
    assert_eq!(lane_snapshot.alias, "core");
    assert_eq!(lane_snapshot.dataspace_id, 7);
    assert_eq!(lane_snapshot.dataspace_alias.as_deref(), Some("alpha"));
    assert_eq!(lane_snapshot.visibility.as_deref(), Some("public"));
    assert_eq!(lane_snapshot.storage_profile, "full_replica");
    assert_eq!(lane_snapshot.lane_type.as_deref(), Some("execution"));
    assert_eq!(lane_snapshot.governance.as_deref(), Some("parliament"));
    assert_eq!(lane_snapshot.settlement.as_deref(), Some("xor"));
    assert_eq!(lane_snapshot.scheduler_teu_capacity_override, Some(4_096));
    assert_eq!(lane_snapshot.scheduler_starvation_bound_override, Some(33));

    let dataspace_snapshot = metrics
        .nexus_scheduler_dataspace_teu_status
        .read()
        .expect("dataspace TEU cache poisoned")
        .get(&(0, 7))
        .cloned()
        .expect("dataspace snapshot missing");
    assert_eq!(dataspace_snapshot.alias, "alpha");
    assert_eq!(
        dataspace_snapshot.description.as_deref(),
        Some("Primary execution dataspace")
    );

    telemetry.inc_nexus_scheduler_lane_teu_deferral(LaneId::new(0), "cap_exceeded", 5);

    let deferral_metric = metrics
        .nexus_scheduler_lane_teu_deferral_total
        .with_label_values(&["0", "cap_exceeded"])
        .get();
    assert_eq!(deferral_metric, 5);

    let lane_snapshot_updated = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .get(&0)
        .cloned()
        .expect("lane snapshot missing after deferral update");
    assert_eq!(lane_snapshot_updated.deferrals.cap_exceeded, 5);

    // Trim catalogs so the secondary lane/dataspace disappear and caches drop stale entries.
    let trimmed_lane_catalog =
        LaneCatalog::new(nonzero!(2_u32), vec![lane_core.clone()]).expect("trimmed lane catalog");
    let trimmed_dataspace_catalog =
        DataSpaceCatalog::new(vec![primary_dataspace.clone()]).expect("trimmed dataspace catalog");
    telemetry.set_nexus_catalogs(&trimmed_lane_catalog, &trimmed_dataspace_catalog);

    let stale_lane_snapshot = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .get(&1)
        .cloned();
    assert!(
        stale_lane_snapshot.is_none(),
        "secondary lane entry should be purged when catalog shrinks"
    );

    let stale_dataspace_snapshot = metrics
        .nexus_scheduler_dataspace_teu_status
        .read()
        .expect("dataspace TEU cache poisoned")
        .get(&(0, 11))
        .cloned();
    assert!(
        stale_dataspace_snapshot.is_none(),
        "stale dataspace pair should be removed"
    );
}

#[test]
#[cfg(feature = "telemetry")]
fn lane_manifest_details_exposed_via_telemetry() {
    let (metrics, telemetry, validators, allowed_ids) = governance_lane_setup();
    install_governance_manifest(&telemetry, &validators, &allowed_ids);

    let governance_sealed = metrics
        .nexus_lane_governance_sealed
        .with_label_values(&["governance"])
        .get();
    assert_eq!(governance_sealed, 0);

    telemetry.record_nexus_scheduler_lane_teu(
        LaneId::new(1),
        LaneTeuGaugeUpdate {
            capacity: 4_096,
            committed: 512,
            ..LaneTeuGaugeUpdate::default()
        },
    );

    assert_governance_manifest_snapshot(metrics.as_ref(), &validators, &allowed_ids);
    clear_governance_manifest(&telemetry);

    let governance_sealed_after_loss = metrics
        .nexus_lane_governance_sealed
        .with_label_values(&["governance"])
        .get();
    assert_eq!(governance_sealed_after_loss, 1);
}

#[test]
#[cfg(feature = "telemetry")]
fn sealed_lane_gauges_reset_when_lane_removed() {
    let (metrics, telemetry, _validators, _allowed_ids) = governance_lane_setup();

    // Simulate a governance lane that is still sealed (manifest missing).
    clear_governance_manifest(&telemetry);
    let sealed_before = metrics
        .nexus_lane_governance_sealed
        .with_label_values(&["governance"])
        .get();
    assert_eq!(sealed_before, 1);

    // Lane disappears from the registry; the gauges should be reset.
    telemetry.record_lane_governance_statuses(&[]);
    let sealed_after = metrics
        .nexus_lane_governance_sealed
        .with_label_values(&["governance"])
        .get();
    assert_eq!(sealed_after, 0);
    assert_eq!(
        metrics.nexus_lane_governance_sealed_total.get(),
        0,
        "total sealed gauge should be zeroed"
    );
    assert!(
        metrics.lane_governance_sealed_aliases().is_empty(),
        "cached sealed alias list should be cleared"
    );
}

#[test]
#[cfg(feature = "telemetry")]
fn lane_catalog_size_reflected_in_metric() {
    use iroha_telemetry::metrics::Metrics;

    let metrics = Arc::new(Metrics::default());
    let telemetry = iroha_core::telemetry::StateTelemetry::new(metrics.clone(), true);

    let initial_catalog = LaneCatalog::new(
        NonZeroU32::new(3).expect("non-zero"),
        vec![
            ModelLaneConfig {
                id: LaneId::new(0),
                alias: "core".to_string(),
                ..ModelLaneConfig::default()
            },
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "governance".to_string(),
                ..ModelLaneConfig::default()
            },
            ModelLaneConfig {
                id: LaneId::new(2),
                alias: "zk".to_string(),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    telemetry.set_nexus_catalogs(&initial_catalog, &DataSpaceCatalog::default());

    assert_eq!(
        metrics.nexus_lane_configured_total.get(),
        3,
        "gauge should reflect initial lane count"
    );

    let trimmed_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("non-zero"),
        vec![
            ModelLaneConfig {
                id: LaneId::new(0),
                alias: "core".to_string(),
                ..ModelLaneConfig::default()
            },
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "governance".to_string(),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    telemetry.set_nexus_catalogs(&trimmed_catalog, &DataSpaceCatalog::default());

    assert_eq!(
        metrics.nexus_lane_configured_total.get(),
        2,
        "gauge should update when the lane catalog shrinks"
    );
}

#[cfg(feature = "telemetry")]
fn governance_lane_setup() -> (
    Arc<iroha_telemetry::metrics::Metrics>,
    iroha_core::telemetry::StateTelemetry,
    Vec<AccountId>,
    BTreeSet<String>,
) {
    use iroha_telemetry::metrics::Metrics;

    let metrics = Arc::new(Metrics::default());
    let telemetry = iroha_core::telemetry::StateTelemetry::new(metrics.clone(), true);
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("non-zero"),
        vec![
            ModelLaneConfig::default(),
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "governance".to_string(),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("governance".to_string()),
                governance: Some("parliament".to_string()),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    telemetry.set_nexus_catalogs(&lane_catalog, &DataSpaceCatalog::default());

    let validators = vec![
        iroha_test_samples::ALICE_ID.clone(),
        iroha_test_samples::BOB_ID.clone(),
    ];
    let allowed_ids = ["upgrade-1".to_string(), "upgrade-2".to_string()]
        .into_iter()
        .collect::<BTreeSet<_>>();

    (metrics, telemetry, validators, allowed_ids)
}

#[cfg(feature = "telemetry")]
fn governance_rules(validators: &[AccountId], allowed_ids: &BTreeSet<String>) -> GovernanceRules {
    let protected_namespaces = ["governance", "treasury"]
        .into_iter()
        .map(|ns| Name::from_str(ns).expect("namespace"))
        .collect::<BTreeSet<_>>();
    GovernanceRules {
        version: 1,
        validators: validators.to_vec(),
        quorum: Some(2),
        protected_namespaces,
        hooks: GovernanceHooks {
            runtime_upgrade: Some(RuntimeUpgradeHook {
                allow: false,
                require_metadata: true,
                metadata_key: Some(Name::from_str("upgrade_id").expect("metadata key")),
                allowed_ids: Some(allowed_ids.clone()),
            }),
            unknown: BTreeMap::new(),
        },
    }
}

#[cfg(feature = "telemetry")]
fn install_governance_manifest(
    telemetry: &iroha_core::telemetry::StateTelemetry,
    validators: &[AccountId],
    allowed_ids: &BTreeSet<String>,
) {
    let registry = LaneManifestRegistry::from_statuses(BTreeMap::from([(
        LaneId::new(1),
        LaneManifestStatus {
            lane: LaneId::new(1),
            alias: "governance".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Restricted,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/manifests/governance.manifest.json")),
            governance_rules: Some(governance_rules(validators, allowed_ids)),
            privacy_commitments: Vec::new(),
        },
    )]));
    telemetry.set_lane_manifest_registry(Arc::new(registry));
}

#[cfg(feature = "telemetry")]
fn clear_governance_manifest(telemetry: &iroha_core::telemetry::StateTelemetry) {
    let registry = LaneManifestRegistry::from_statuses(BTreeMap::from([(
        LaneId::new(1),
        LaneManifestStatus {
            lane: LaneId::new(1),
            alias: "governance".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Restricted,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        },
    )]));
    telemetry.set_lane_manifest_registry(Arc::new(registry));
}

#[cfg(feature = "telemetry")]
fn assert_governance_manifest_snapshot(
    metrics: &iroha_telemetry::metrics::Metrics,
    validators: &[AccountId],
    allowed_ids: &BTreeSet<String>,
) {
    let lane_snapshot = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .get(&1)
        .cloned()
        .expect("lane snapshot missing");
    assert!(lane_snapshot.manifest_required);
    assert!(lane_snapshot.manifest_ready);
    let manifest_path_str = lane_snapshot.manifest_path.as_ref().expect("manifest path");
    assert!(
        manifest_path_str.ends_with("governance.manifest.json"),
        "manifest path should include file name"
    );
    let expected_validators = validators
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(lane_snapshot.manifest_validators, expected_validators);
    assert_eq!(lane_snapshot.manifest_quorum, Some(2));
    assert_eq!(
        lane_snapshot.manifest_protected_namespaces,
        vec!["governance".to_string(), "treasury".to_string()]
    );
    let hook = lane_snapshot
        .manifest_runtime_upgrade
        .expect("runtime upgrade hook missing");
    assert!(!hook.allow);
    assert!(hook.require_metadata);
    assert_eq!(hook.metadata_key.as_deref(), Some("upgrade_id"));
    let expected_ids = allowed_ids.iter().cloned().collect::<Vec<_>>();
    assert_eq!(hook.allowed_ids, expected_ids);
}

#[test]
#[cfg(feature = "telemetry")]
fn nexus_config_diff_counter_and_event_emitted() {
    use iroha_config::parameters::actual::{
        Autoscale, Commit, Da, Fusion, GovernanceCatalog, LaneRegistry, LaneRoutingMatcher,
        LaneRoutingPolicy, LaneRoutingRule, Nexus, NexusAxt,
    };
    use nonzero_ext::nonzero;
    use norito::json::to_string as to_json_string;

    let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
    let telemetry = iroha_core::telemetry::StateTelemetry::new(metrics.clone(), true);

    let lane_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            ModelLaneConfig::default(),
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "governance".to_string(),
                visibility: LaneVisibility::Restricted,
                lane_type: Some("governance".to_string()),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "gov".to_string(),
            description: Some("Governance dataspace".to_string()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let routing_policy = LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::GLOBAL,
        rules: vec![LaneRoutingRule {
            lane: LaneId::new(1),
            dataspace: Some(DataSpaceId::new(1)),
            matcher: LaneRoutingMatcher {
                account: Some(
                    "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
                        .to_string(),
                ),
                ..LaneRoutingMatcher::default()
            },
        }],
    };

    let registry = LaneRegistry {
        poll_interval: std::time::Duration::from_secs(120),
        ..LaneRegistry::default()
    };

    let governance = GovernanceCatalog {
        default_module: Some("parliament".to_string()),
        ..GovernanceCatalog::default()
    };

    let fusion = Fusion {
        floor_teu: 5_000,
        ..Fusion::default()
    };

    let da = Da {
        sample_size_base: nonzero!(80_u16),
        ..Da::default()
    };

    let nexus = Nexus {
        enabled: true,
        storage: NexusStorage::default(),
        staking: NexusStaking::default(),
        fees: NexusFees::default(),
        hf_shared_leases: Default::default(),
        uploaded_models: Default::default(),
        endorsement: NexusEndorsement::default(),
        axt: NexusAxt::default(),
        lane_relay_emergency: LaneRelayEmergency::default(),
        lane_config: RuntimeLaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        dataspace_catalog,
        routing_policy,
        registry,
        governance,
        compliance: LaneCompliance::default(),
        fusion,
        autoscale: Autoscale::default(),
        commit: Commit::default(),
        da,
    };

    let event = telemetry
        .record_nexus_config_diff(&nexus)
        .expect("diff event expected");
    let event_json = to_json_string(&event).expect("serialize event");
    assert!(
        event_json.contains("nexus.lane_catalog.count"),
        "diff event should mention lane catalog count"
    );

    let counter = metrics
        .nexus_config_diff_total
        .with_label_values(&["nexus.lane_catalog.count", "active"])
        .get();
    assert!(
        counter > 0,
        "nexus_config_diff_total should increment for lane count diff"
    );
}
