use super::*;
fn state_for_lifecycle_test() -> Arc<CoreState> {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    let kura_config = iroha_config::parameters::actual::Kura {
        init_mode: iroha_config::kura::InitMode::Strict,
        // The authenticated temporary constructor owns the isolated storage directory.
        store_dir: iroha_config::base::WithOrigin::inline(std::path::PathBuf::new()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
        lane_history_retention: iroha_config::parameters::defaults::kura::LANE_HISTORY_RETENTION,
        fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
    };
    let kura = Kura::new_temporary_with_configured_lane_catalog(
        &kura_config,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
    )
    .expect("open the authenticated lifecycle fixture catalog before State startup");
    let mut state = CoreState::new_for_testing(
        iroha_core::state::World::default(),
        kura,
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    state
        .prepare_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)
        .expect("prepare the authenticated primary lane anchor");
    state
        .restore_kura_lane_segments_before_startup_replay()
        .expect("restore the authenticated lifecycle fixture primary geometry");
    state
        .set_nexus_from_config(nexus)
        .expect("install Nexus catalog for lifecycle test");
    Arc::new(state)
}
#[test]
fn lane_lifecycle_status_binds_exact_current_catalog() {
    let state = state_for_lifecycle_test();
    let status = handle_get_nexus_lane_lifecycle(&state).expect("lifecycle status");
    let view = state.view();
    assert_eq!(
        status.validate().expect("validate lifecycle status"),
        view.nexus.lane_catalog
    );
    let expected_incarnations =
        iroha_data_model::nexus::LaneLifecycleParameterV1::canonical_incarnations(
            &view.nexus.lane_catalog,
            &view.lane_incarnations,
        )
        .expect("canonical committed incarnations");
    assert_eq!(status.incarnations, expected_incarnations);
    assert_eq!(status.runtime_catalog_hash, None);
}

#[test]
fn lane_lifecycle_status_exposes_native_runtime_root_and_propagates_invalid_state() {
    use iroha_data_model::{
        nexus::NexusRuntimeCatalogV1,
        parameter::{CustomParameter, Parameter},
    };
    let state = state_for_lifecycle_test();
    let before = handle_get_nexus_lane_lifecycle(&state).unwrap();
    let runtime = NexusRuntimeCatalogV1 {
        version: NexusRuntimeCatalogV1::VERSION,
        baseline_dataspaces_hash: iroha_data_model::nexus::dataspace_catalog_hash(
            &state.nexus_snapshot().configured_dataspace_catalog,
        ),
        baseline_manifests_hash: Hash::new(b"runtime readback fixture manifest baseline"),
        dataspaces: Vec::new(),
        manifests: Vec::new(),
    };
    let expected = runtime.canonical_hash().unwrap();
    let mut world = state.world.block();
    world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(runtime.into_custom_parameter().unwrap()));
    world.commit();
    let status = handle_get_nexus_lane_lifecycle(&state).unwrap();
    assert_eq!(status.runtime_catalog_hash, Some(expected));
    assert_eq!(status.catalog_hash, before.catalog_hash);
    assert_eq!(status.incarnation_root, before.incarnation_root);
    let mut world = state.world.block();
    world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(CustomParameter::new(
            NexusRuntimeCatalogV1::parameter_id(),
            iroha_primitives::json::Json::new(norito::json!({"version": 255})),
        )));
    world.commit();
    let error = handle_get_nexus_lane_lifecycle(&state)
        .expect_err("malformed protected state must never become an absent runtime root");
    let native_error = state
        .view()
        .runtime_catalog_hash()
        .expect_err("the committed malformed parameter must fail native readback");
    // Torii's outer Query display omits the cause; inspect its typed conversion payload.
    match error {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
        )) => assert_eq!(
            message,
            format!("invalid committed runtime catalog: {native_error}"),
        ),
        other => panic!("expected the native runtime catalog conversion error, got {other:?}"),
    }
}
