//! Canonical Nexus lane-lifecycle fixture validation.

use iroha_data_model::nexus::LaneLifecycleStatusV1;

#[test]
fn checked_in_lifecycle_status_fixture_is_canonical() {
    let fixture = include_str!("../../../fixtures/nexus/lanes/status_ready.json");
    let status = norito::json::from_str::<LaneLifecycleStatusV1>(fixture)
        .expect("decode checked-in lifecycle status fixture");
    let catalog = status
        .validate()
        .expect("checked-in lifecycle status fixture must be self-authenticating");
    assert_eq!(catalog.lanes(), status.lanes);
}
