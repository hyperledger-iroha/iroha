//! Integration tests for `iroha_logger` telemetry behavior.
//!
//! Verifies that regular channel receivers obtain non-`telemetry::` logs
//! and that field extraction matches expected event structures.

use std::time::Duration;

use iroha_logger::{
    info,
    telemetry::{Channel, Event, Fields},
    test_logger,
};
use iroha_data_model::nexus::{DataSpaceId, LaneId};
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
            (
                "lane_id",
                norito::json!(u64::from(LaneId::SINGLE.as_u32())),
            ),
            ("dataspace_id", norito::json!(DataSpaceId::GLOBAL.as_u64())),
            ("a", norito::json!(2)),
            ("c", norito::json!(true)),
            ("d", norito::json!("this won't be logged")),
        ]),
    };
    let output = time::timeout(Duration::from_millis(10), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output, telemetry);
}
