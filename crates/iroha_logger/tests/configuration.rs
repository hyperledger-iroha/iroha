//! Integration tests for `iroha_logger` configuration.
//!
//! Ensures telemetry events are routed to the expected channel and
//! that structured fields are preserved as emitted.
use iroha_logger::{
    info,
    telemetry::{Channel, Event, Fields},
    test_logger,
};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use std::time::Duration;
use tokio::time;
#[tokio::test]
async fn telemetry_separation_custom() {
    let mut receiver = test_logger()
        .subscribe_on_telemetry(Channel::Regular)
        .await
        .unwrap();
    info!(target: "telemetry::test", a = 2, c = true, d = "this won't be logged");
    info!("This will be logged in bunyan-readable format");
    let telemetry = Event {
        target: "test",
        // Event fields keep their emitted order and tracing's integer type;
        // default lane/dataspace scopes are appended only when absent.
        fields: Fields(vec![
            ("level", norito::json!("INFO")),
            ("a", norito::json::Value::from(2_i64)),
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
