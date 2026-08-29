//! Integration tests for `iroha_logger` telemetry behavior.
//!
//! Verifies that regular channel receivers obtain non-`telemetry::` logs
//! and that field extraction matches expected event structures.
use iroha_data_model::nexus::{DataSpaceId, LaneId};
use iroha_logger::{
    info,
    telemetry::{Channel, Event, Fields},
    test_logger,
};
use std::time::Duration;
use tokio::time;
#[tokio::test]
async fn telemetry_separation_default() {
    let mut receiver = test_logger()
        .subscribe_on_telemetry(Channel::Regular)
        .await
        .unwrap();
    info!(target: "telemetry::test", a = 2, c = true, d = "this won't be logged");
    info!("This will be logged");
    let telemetry = Event {
        target: "test",
        fields: Fields(vec![
            ("level", norito::json!("INFO")),
            ("a", norito::json!(2)),
            ("c", norito::json!(true)),
            ("d", norito::json!("this won't be logged")),
            ("lane_id", norito::json!(u64::from(LaneId::SINGLE.as_u32()))),
            (
                "dataspace_id",
                norito::json!(DataSpaceId::UNIVERSAL.as_u64()),
            ),
        ]),
    };
    let output = time::timeout(Duration::from_millis(10), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output, telemetry);
}

#[tokio::test]
async fn explicit_routing_fields_replace_defaults_without_duplicates() {
    let mut receiver = test_logger()
        .subscribe_on_telemetry(Channel::Regular)
        .await
        .unwrap();
    info!(
        target: "telemetry::routing",
        lane_id = 7_u64,
        dataspace_id = 9_u64,
        "explicit routing"
    );
    let output = time::timeout(Duration::from_millis(10), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let lane_ids: Vec<_> = output
        .fields
        .iter()
        .filter(|(key, _)| *key == "lane_id")
        .collect();
    let dataspace_ids: Vec<_> = output
        .fields
        .iter()
        .filter(|(key, _)| *key == "dataspace_id")
        .collect();
    assert_eq!(lane_ids.len(), 1);
    assert_eq!(&lane_ids[0].1, &norito::json!(7_u64));
    assert_eq!(dataspace_ids.len(), 1);
    assert_eq!(&dataspace_ids[0].1, &norito::json!(9_u64));
}
