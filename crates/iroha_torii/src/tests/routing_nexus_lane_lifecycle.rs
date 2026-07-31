use super::*;

fn enabled_state_for_lifecycle_test() -> Arc<CoreState> {
    let mut state = CoreState::new_for_testing(
        iroha_core::state::World::default(),
        Kura::blank_kura_for_testing(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    let configured_catalog = state.nexus_snapshot().lane_catalog;
    state
        .prepare_configured_primary_geometry_anchor(&configured_catalog)
        .expect("prepare the authenticated primary lane anchor");
    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        })
        .expect("enable Nexus for lifecycle test");
    Arc::new(state)
}

#[test]
fn lane_lifecycle_status_binds_exact_current_catalog() {
    let state = enabled_state_for_lifecycle_test();
    let status = handle_get_nexus_lane_lifecycle(&state).expect("lifecycle status");
    assert!(status.nexus_enabled);
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
}
