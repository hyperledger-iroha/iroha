//! Canonical Nexus lane-lifecycle fixture validation.

use iroha_data_model::nexus::LaneLifecycleStatusV1;
use std::collections::BTreeSet;

#[test]
fn checked_in_lifecycle_status_fixture_is_canonical() {
    let fixture = include_str!("../../../fixtures/nexus/lanes/status_ready.json");
    let value: norito::json::Value =
        norito::json::from_str(fixture).expect("decode lifecycle fixture as JSON value");
    let fields = value
        .as_object()
        .expect("lifecycle fixture is a JSON object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_fields = [
        "catalog_hash",
        "incarnation_root",
        "incarnations",
        "lane_count",
        "lanes",
        "version",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(
        fields, expected_fields,
        "fixture must use the exact current lifecycle status layout"
    );
    let status = norito::json::from_str::<LaneLifecycleStatusV1>(fixture)
        .expect("decode checked-in lifecycle status fixture");
    let catalog = status
        .validate()
        .expect("checked-in lifecycle status fixture must be self-authenticating");
    assert_eq!(catalog.lanes(), status.lanes);
}
