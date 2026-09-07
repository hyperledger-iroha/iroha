// Frozen lane-manifest snapshots must never revive files that were unknown to
// the active catalog captured at materialization time.
#[test]
fn nexus_reconfigure_does_not_revive_unknown_manifest_without_explicit_reload() {
    let dir = tempdir().expect("manifest directory");
    let manifest_path = dir.path().join("future.manifest.json");
    fs::write(
        &manifest_path,
        r#"{"lane":"future","governance":"parliament","version":1}"#,
    )
    .expect("write future manifest");
    let registry_cfg = LaneRegistry {
        manifest_directory: Some(dir.path().to_path_buf()),
        ..LaneRegistry::default()
    };
    let mut governance = GovernanceCatalog::default();
    governance.modules.insert(
        "parliament".to_owned(),
        iroha_config::parameters::actual::GovernanceModule::default(),
    );
    let frozen = Arc::new(LaneManifestRegistry::from_config(
        &LaneCatalog::default(),
        &governance,
        &registry_cfg,
    ));
    let frozen_digest = frozen.consensus_policy_digest();
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    queue.install_lane_manifests_with_state(&frozen, &state);
    assert!(!frozen.has_manifest_source_alias("future"));
    let expanded = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "future".to_owned(),
                governance: Some("parliament".to_owned()),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("expanded catalog");
    let mut nexus = state.nexus_snapshot();
    nexus.governance = governance;
    nexus.registry = registry_cfg;
    nexus.lane_catalog = expanded.clone();
    nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&expanded);
    queue.reconfigure_nexus_with_state(&nexus, &state, None);
    let installed = queue.lane_manifests.read().clone();
    assert_eq!(installed.consensus_policy_digest(), frozen_digest);
    assert_eq!(
        installed
            .ensure_lane_ready(LaneId::new(1))
            .expect_err("unknown future source must not revive during rebind")
            .reason(),
        crate::governance::manifest::GovernanceGuardReason::MissingManifest
    );
    assert!(!installed.has_manifest_source_alias("future"));
    let explicitly_reloaded =
        LaneManifestRegistry::from_config(&expanded, &nexus.governance, &nexus.registry);
    assert!(
        explicitly_reloaded
            .ensure_lane_ready(LaneId::new(1))
            .is_ok()
    );
    assert!(explicitly_reloaded.has_manifest_source_alias("future"));
    assert_ne!(explicitly_reloaded.consensus_policy_digest(), frozen_digest);
}

#[test]
fn nexus_reconfiguration_uses_state_authority_instead_of_empty_or_stale_queue_cache() {
    let (canonical_validator, _) = gen_account_in("wonderland");
    let (stale_validator, _) = gen_account_in("wonderland");
    let registry = |validator: AccountId| {
        let lane = LaneConfig::default();
        Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
            lane.id,
            LaneManifestStatus {
                lane: lane.id,
                alias: lane.alias,
                dataspace: lane.dataspace_id,
                visibility: lane.visibility,
                storage: lane.storage,
                governance: lane.governance,
                manifest_path: Some(PathBuf::from("/tmp/state-authority.manifest.json")),
                governance_rules: Some(GovernanceRules {
                    validators: vec![validator],
                    ..GovernanceRules::default()
                }),
                privacy_commitments: Vec::new(),
            },
        )])))
    };
    // Exercise both State entry points: a fresh empty Queue and a stale Queue
    // must adopt installed authority; stale Queue policy must never resurrect
    // authority absent from State.
    for with_view in [false, true] {
        for (state_has_authority, queue_is_stale) in [(true, false), (true, true), (false, true)] {
            let state = State::new_for_testing(
                World::default(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            if state_has_authority {
                state.install_lane_manifests(&registry(canonical_validator.clone()));
            }
            let installed = state.lane_manifests.read().clone();
            let nexus = state.nexus_snapshot();
            let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
            let queue = Queue::test(config_factory(), &time_source);
            if queue_is_stale {
                queue.install_lane_manifests(&registry(stale_validator.clone()));
            }
            assert_ne!(
                queue.lane_manifests.read().consensus_policy_digest(),
                installed.consensus_policy_digest(),
                "the fixture must start with different State and Queue authority"
            );
            if with_view {
                queue.reconfigure_nexus(&nexus, &state.view(), None);
            } else {
                queue.reconfigure_nexus_with_state(&nexus, &state, None);
            }
            assert!(
                Arc::ptr_eq(&state.lane_manifests.read(), &installed),
                "routing refresh cannot replace installed State policy"
            );
            let queue_registry = queue.lane_manifests.read().clone();
            assert_eq!(
                queue_registry.consensus_policy_digest(),
                installed.consensus_policy_digest(),
                "Queue must consume the installed State policy"
            );
            if state_has_authority {
                assert_eq!(
                    queue_registry
                        .lane_rules(LaneId::SINGLE)
                        .expect("retain the installed State validators")
                        .validators,
                    vec![canonical_validator.clone()]
                );
            } else {
                assert!(
                    queue_registry.lane_rules(LaneId::SINGLE).is_none(),
                    "stale Queue validators must not revive missing State authority"
                );
            }
        }
    }
}
