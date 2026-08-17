#[test]
fn apply_sora_profile_enables_discovery_with_parsed_admission() {
    let mut root = minimal_root_with_sorafs_admission();
    let trusted_council_keys = root
        .torii
        .sorafs_discovery
        .admission
        .as_ref()
        .expect("parsed admission policy")
        .trusted_council_keys
        .clone();
    assert!(!root.torii.sorafs_discovery.discovery_enabled);
    root.apply_sora_profile();
    let admission = root
        .torii
        .sorafs_discovery
        .admission
        .as_ref()
        .expect("profile must preserve parsed admission policy");
    assert!(root.torii.sorafs_discovery.discovery_enabled);
    assert_eq!(admission.trusted_council_keys, trusted_council_keys);
    assert_eq!(admission.signature_threshold.get(), 1);
    assert_eq!(admission.envelopes_dir, PathBuf::from("admission"));
}
#[test]
fn apply_sora_profile_enables_nexus_and_sets_catalogs_on_defaults() {
    let mut root = minimal_root();
    root.apply_sora_profile();
    assert!(root.nexus.enabled, "Sora profile must enable Nexus runtime");
    assert_eq!(root.nexus.lane_catalog, sora_lane_catalog());
    assert_eq!(
        root.nexus.configured_lane_catalog, root.nexus.lane_catalog,
        "the profile catalog must become the immutable consensus-policy baseline"
    );
    assert_eq!(root.nexus.dataspace_catalog, sora_dataspace_catalog());
    assert_eq!(root.nexus.routing_policy, sora_routing_policy());
    assert_eq!(
        root.nexus.lane_config.entries().len(),
        root.nexus.lane_catalog.lanes().len()
    );
    assert_eq!(
        root.tiered_state
            .da_store_root
            .as_ref()
            .expect("DA store root should be defaulted"),
        &PathBuf::from(defaults::tiered_state::DEFAULT_DA_STORE_ROOT)
    );
}
#[test]
fn has_lane_overrides_detects_single_lane_changes() {
    let mut root = minimal_root();
    root.nexus.enabled = false;
    root.nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(1).expect("nonzero lane count"),
        vec![LaneConfigMetadata {
            alias: "custom".to_string(),
            ..LaneConfigMetadata::default()
        }],
    )
    .expect("lane catalog");
    assert!(root.nexus.has_lane_overrides());
    assert!(
        !root.nexus.uses_multilane_catalogs(),
        "single-lane overrides should not be treated as multi-lane"
    );
    assert!(
        root.uses_sora_features(),
        "single-lane Nexus policy overrides still require the Sora runtime"
    );
}
#[test]
fn apply_sora_profile_preserves_custom_catalogs_but_enables_flag() {
    let mut root = minimal_root();
    let custom_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("non-zero lane count"),
        vec![
            LaneConfigMetadata {
                id: LaneId::new(0),
                alias: "alpha".to_string(),
                description: None,
                ..LaneConfigMetadata::default()
            },
            LaneConfigMetadata {
                id: LaneId::new(1),
                alias: "beta".to_string(),
                description: None,
                ..LaneConfigMetadata::default()
            },
        ],
    )
    .expect("valid custom catalog");
    root.nexus.lane_config = LaneConfig::from_catalog(&custom_catalog);
    root.nexus.configured_lane_catalog = custom_catalog.clone();
    root.nexus.lane_catalog = custom_catalog.clone();
    root.apply_sora_profile();
    assert!(root.nexus.enabled, "Sora profile must enable Nexus runtime");
    assert_eq!(
        root.tiered_state
            .da_store_root
            .as_ref()
            .expect("DA store root should be defaulted"),
        &PathBuf::from(defaults::tiered_state::DEFAULT_DA_STORE_ROOT)
    );
    assert_eq!(root.nexus.lane_catalog, custom_catalog);
    assert_eq!(root.nexus.configured_lane_catalog, custom_catalog);
    assert_eq!(
        root.nexus
            .lane_config
            .entry(LaneId::new(1))
            .expect("lane config should be preserved")
            .alias,
        "beta"
    );
}
#[test]
fn apply_sora_profile_preserves_explicit_single_lane_shard_policy() {
    let mut root = minimal_root();
    let pinned_shard = ShardId::new(9);
    let custom_catalog = LaneCatalog::new(
        NonZeroU32::new(1).expect("non-zero lane count"),
        vec![LaneConfigMetadata {
            shard_id: Some(pinned_shard),
            ..LaneConfigMetadata::default()
        }],
    )
    .expect("valid single-lane shard override");
    root.nexus.lane_config = LaneConfig::from_catalog(&custom_catalog);
    root.nexus.configured_lane_catalog = custom_catalog.clone();
    root.nexus.lane_catalog = custom_catalog.clone();

    root.apply_sora_profile();

    assert!(root.nexus.enabled, "Sora profile must enable Nexus runtime");
    assert_eq!(root.nexus.lane_catalog, custom_catalog);
    assert_eq!(root.nexus.configured_lane_catalog, custom_catalog);
    assert_eq!(
        root.nexus
            .lane_config
            .entry(LaneId::SINGLE)
            .expect("single lane remains configured")
            .shard_id,
        pinned_shard.as_u32(),
    );
}
#[test]
fn apply_storage_budget_clamps_component_caps() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    root.nexus.storage.local_budget_bytes = Some(Bytes(1_000));
    root.nexus.storage.max_wsv_memory_bytes = Bytes(512);
    root.nexus.storage.disk_budget_weights = NexusStorageWeights {
        kura_blocks_bps: 5_000,
        wsv_snapshots_bps: 2_000,
        sorafs_bps: 2_000,
        soranet_spool_bps: 500,
        soravpn_spool_bps: 500,
    };
    root.tiered_state.enabled = false;
    root.tiered_state.cold_store_root = None;
    root.tiered_state.da_store_root = None;
    root.kura.max_disk_usage_bytes = Bytes(0);
    root.tiered_state.max_cold_bytes = Bytes(0);
    root.torii.sorafs_storage.max_capacity_bytes = Bytes(0);
    root.streaming.soranet.provision_spool_max_bytes = Bytes(0);
    root.streaming.soravpn.provision_spool_max_bytes = Bytes(0);
    root.apply_storage_budget();
    assert_eq!(
        root.nexus
            .storage
            .effective_local_budget_bytes
            .map(Bytes::get),
        Some(1_000)
    );
    assert_eq!(root.kura.max_disk_usage_bytes.get(), 500);
    assert_eq!(root.tiered_state.max_cold_bytes.get(), 200);
    assert_eq!(root.torii.sorafs_storage.max_capacity_bytes.get(), 200);
    assert_eq!(root.streaming.soranet.provision_spool_max_bytes.get(), 50);
    assert_eq!(root.streaming.soravpn.provision_spool_max_bytes.get(), 50);
    assert!(root.tiered_state.enabled, "tiered state should be enabled");
    assert_eq!(root.tiered_state.hot_retained_bytes.get(), 512);
    assert!(root.tiered_state.da_store_root.is_none());
    assert_eq!(
        root.tiered_state
            .cold_store_root
            .as_ref()
            .expect("cold store root defaulted")
            .as_os_str(),
        defaults::tiered_state::DEFAULT_COLD_STORE_ROOT
    );
}
#[test]
fn apply_derived_storage_budget_uses_filesystem_group_caps() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    let filesystem_budgets = vec![
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(800).expect("non-zero budget"),
            components: vec![
                NexusStorageBudgetComponent::Kura,
                NexusStorageBudgetComponent::Sorafs,
            ],
        },
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(1_200).expect("non-zero budget"),
            components: vec![
                NexusStorageBudgetComponent::WsvCold,
                NexusStorageBudgetComponent::SoranetSpool,
                NexusStorageBudgetComponent::SoravpnSpool,
            ],
        },
    ];
    root.nexus.storage.max_wsv_memory_bytes = Bytes(256);
    root.nexus.storage.disk_budget_weights = NexusStorageWeights {
        kura_blocks_bps: 5_000,
        wsv_snapshots_bps: 2_000,
        sorafs_bps: 2_000,
        soranet_spool_bps: 500,
        soravpn_spool_bps: 500,
    };
    root.tiered_state.enabled = false;
    root.tiered_state.cold_store_root = None;
    root.tiered_state.da_store_root = None;
    root.kura.max_disk_usage_bytes = Bytes(0);
    root.tiered_state.max_cold_bytes = Bytes(0);
    root.torii.sorafs_storage.max_capacity_bytes = Bytes(0);
    root.streaming.soranet.provision_spool_max_bytes = Bytes(0);
    root.streaming.soravpn.provision_spool_max_bytes = Bytes(0);
    let aggregate = root
        .apply_derived_storage_budget(&filesystem_budgets)
        .expect("valid filesystem budgets");
    assert_eq!(aggregate.get(), 2_000);
    assert!(root.nexus.storage.local_budget_bytes.is_none());
    assert_eq!(
        root.nexus
            .storage
            .effective_local_budget_bytes
            .map(Bytes::get),
        Some(2_000)
    );
    assert_eq!(root.kura.max_disk_usage_bytes.get(), 572);
    assert_eq!(root.tiered_state.max_cold_bytes.get(), 800);
    assert_eq!(root.torii.sorafs_storage.max_capacity_bytes.get(), 228);
    assert_eq!(root.streaming.soranet.provision_spool_max_bytes.get(), 200);
    assert_eq!(root.streaming.soravpn.provision_spool_max_bytes.get(), 200);
    assert_eq!(root.tiered_state.hot_retained_bytes.get(), 256);
}
#[test]
fn runtime_storage_budget_reconciliation_is_not_ratchet_bound() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    root.nexus.storage.local_budget_bytes = None;
    root.nexus.storage.disk_budget_weights = NexusStorageWeights::default();
    root.kura.max_disk_usage_bytes = Bytes(1_000);
    let budget = |bytes| NexusStorageFilesystemBudget {
        budget_bytes: NonZeroU64::new(bytes).expect("non-zero budget"),
        components: vec![NexusStorageBudgetComponent::Kura],
    };
    root.apply_derived_storage_budget(&[budget(200)])
        .expect("valid filesystem budget");
    assert_eq!(root.kura.max_disk_usage_bytes.get(), 200);
    root.apply_storage_budget();
    assert_eq!(
        root.nexus
            .storage
            .effective_local_budget_bytes
            .map(Bytes::get),
        Some(200),
        "an absent operator budget must not erase the runtime-derived effective budget"
    );
    root.apply_derived_storage_budget(&[budget(800)])
        .expect("valid filesystem budget");
    assert_eq!(
        root.kura.max_disk_usage_bytes.get(),
        800,
        "a later filesystem probe may raise the cap back toward its configured ceiling"
    );
    assert!(root.nexus.storage.local_budget_bytes.is_none());
    assert_eq!(
        root.nexus
            .storage
            .effective_local_budget_bytes
            .map(Bytes::get),
        Some(800)
    );
}
#[test]
fn derived_storage_budget_rejects_an_overflowing_internal_aggregate() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    let filesystem_budgets = [
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(u64::MAX).expect("non-zero budget"),
            components: vec![NexusStorageBudgetComponent::Kura],
        },
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(1).expect("non-zero budget"),
            components: vec![NexusStorageBudgetComponent::WsvCold],
        },
    ];
    let error = root
        .apply_derived_storage_budget(&filesystem_budgets)
        .expect_err("the aggregate must use checked arithmetic");
    assert_eq!(error, NexusStorageBudgetApplicationError::AggregateOverflow);
    assert!(
        root.nexus.storage.effective_local_budget_bytes.is_none(),
        "an invalid aggregate must be rejected before mutating effective configuration"
    );
}
#[test]
fn derived_storage_budget_rejects_inconsistent_component_metadata_before_mutation() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    let empty = NexusStorageFilesystemBudget {
        budget_bytes: NonZeroU64::new(100).expect("non-zero budget"),
        components: Vec::new(),
    };
    assert_eq!(
        root.apply_derived_storage_budget(&[empty])
            .expect_err("empty component sets must be rejected"),
        NexusStorageBudgetApplicationError::EmptyComponentSet { group_index: 0 }
    );
    let noncanonical = NexusStorageFilesystemBudget {
        budget_bytes: NonZeroU64::new(100).expect("non-zero budget"),
        components: vec![
            NexusStorageBudgetComponent::Sorafs,
            NexusStorageBudgetComponent::Kura,
        ],
    };
    assert!(matches!(
        root.apply_derived_storage_budget(&[noncanonical]),
        Err(NexusStorageBudgetApplicationError::NonCanonicalComponentOrder { group_index: 0, .. })
    ));
    let duplicate_within_group = NexusStorageFilesystemBudget {
        budget_bytes: NonZeroU64::new(100).expect("non-zero budget"),
        components: vec![
            NexusStorageBudgetComponent::Kura,
            NexusStorageBudgetComponent::Kura,
        ],
    };
    assert_eq!(
        root.apply_derived_storage_budget(&[duplicate_within_group])
            .expect_err("within-group duplicates must be rejected"),
        NexusStorageBudgetApplicationError::DuplicateComponent {
            component: NexusStorageBudgetComponent::Kura,
        }
    );
    let duplicate = [
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(100).expect("non-zero budget"),
            components: vec![NexusStorageBudgetComponent::Kura],
        },
        NexusStorageFilesystemBudget {
            budget_bytes: NonZeroU64::new(100).expect("non-zero budget"),
            components: vec![NexusStorageBudgetComponent::Kura],
        },
    ];
    assert_eq!(
        root.apply_derived_storage_budget(&duplicate)
            .expect_err("cross-group duplicates must be rejected"),
        NexusStorageBudgetApplicationError::DuplicateComponent {
            component: NexusStorageBudgetComponent::Kura,
        }
    );
    assert!(
        root.nexus.storage.effective_local_budget_bytes.is_none(),
        "invalid filesystem metadata must not mutate effective configuration"
    );
}
#[test]
fn derived_storage_budget_rejects_zero_component_caps() {
    let mut root = minimal_root();
    root.nexus.enabled = true;
    let budget = NexusStorageFilesystemBudget {
        budget_bytes: NonZeroU64::new(1).expect("non-zero budget"),
        components: vec![
            NexusStorageBudgetComponent::Kura,
            NexusStorageBudgetComponent::Sorafs,
        ],
    };
    assert_eq!(
        root.apply_derived_storage_budget(&[budget])
            .expect_err("zero means unlimited to component cap consumers"),
        NexusStorageBudgetApplicationError::ZeroComponentAllocation {
            group_index: 0,
            component: NexusStorageBudgetComponent::Sorafs,
        }
    );
    assert!(root.nexus.storage.effective_local_budget_bytes.is_none());
}
#[test]
fn storage_budget_splitting_is_exact_at_u64_max() {
    let weights = NexusStorageWeights::default();
    let global = derive_global_nexus_storage_component_caps(u64::MAX, weights);
    assert_eq!(global.total(), u64::MAX);
    let filesystem = split_filesystem_budget_across_components(
        u64::MAX,
        &NexusStorageBudgetComponent::ORDER,
        weights,
    );
    assert_eq!(filesystem.total(), u64::MAX);
    for component in NexusStorageBudgetComponent::ORDER {
        assert!(filesystem.budget_for(component) > 0);
    }
}
#[test]
fn streaming_soravpn_defaults_match_constants() {
    let config = StreamingSoravpn::from_defaults();
    assert_eq!(
        config.provision_spool_dir,
        PathBuf::from(defaults::streaming::soravpn::PROVISION_SPOOL_DIR)
    );
    assert_eq!(
        config.provision_spool_max_bytes.get(),
        defaults::streaming::soravpn::PROVISION_SPOOL_MAX_BYTES.get()
    );
}
#[test]
fn soranet_vpn_defaults_construct_with_canonical_operator_account() {
    let config = SoranetVpn::default();
    assert!(!config.enabled);
    assert_eq!(
        config.operator_account_id,
        defaults::governance::bond_escrow_account_id()
    );
}
