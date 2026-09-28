//! Integration tests for `iroha_logger` telemetry behavior.
//!
//! Verifies that regular channel receivers obtain non-`telemetry::` logs
//! and that field extraction matches expected event structures.
use iroha_logger::{
    info,
    telemetry::{Channel, Event, Fields},
    test_logger,
};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use std::{sync::OnceLock, time::Duration};
use tokio::{sync::broadcast, time};
/// Start the process-wide test logger on a runtime that outlives every test.
///
/// `test_logger` spawns its actor on the first caller's runtime, and each
/// `#[tokio::test]` runtime stops with its test, so the first call happens on a
/// dedicated runtime that never stops.
fn start_logger() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build logger runtime");
            runtime.block_on(async move {
                let _ = test_logger();
                ready_tx.send(()).expect("signal logger readiness");
                std::future::pending::<()>().await;
            });
        });
        ready_rx.recv().expect("logger runtime started");
    });
}
/// Receive the next event for `target`, skipping events emitted by concurrent tests.
async fn next_event(receiver: &mut broadcast::Receiver<Event>, target: &str) -> Event {
    loop {
        let event = time::timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("telemetry event arrives")
            .expect("telemetry channel stays open");
        if event.target == target {
            return event;
        }
    }
}
#[tokio::test]
async fn telemetry_separation_default() {
    start_logger();
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
    let output = next_event(&mut receiver, "test").await;
    assert_eq!(output, telemetry);
}

#[tokio::test]
async fn explicit_routing_fields_replace_defaults_without_duplicates() {
    start_logger();
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
    let output = next_event(&mut receiver, "routing").await;
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
