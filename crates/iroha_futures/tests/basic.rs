//! Integration test for `iroha_futures` telemetry.
//!
//! Verifies that `#[telemetry_future]` records per-poll durations and that
//! collected `FuturePollTelemetry` entries match the expected sleep timings.
use iroha_futures::FuturePollTelemetry;
use iroha_logger::telemetry::{Channel, Event, Fields};
use norito::json::Value;
use std::{thread, time::Duration};
use tokio::{sync::broadcast, task};
#[iroha_futures::telemetry_future]
async fn sleep(times: Vec<Duration>) -> i32 {
    for time in times {
        thread::sleep(time);
        task::yield_now().await;
    }
    // Just random result
    10_i32
}

#[allow(dead_code, improper_ctypes_definitions, unsafe_code)]
#[iroha_futures::telemetry_future]
async unsafe extern "C" fn qualified_abi(value: u32) -> u32 {
    value
}

#[allow(dead_code)]
#[iroha_futures::telemetry_future]
async fn qualified_generic<T>(value: T) -> T
where
    T: Send,
{
    value
}

fn almost_equal(a: Duration, b: Duration) -> bool {
    a.abs_diff(b) < (b / 9)
}

async fn collect_future_telemetry(
    mut receiver: broadcast::Receiver<Event>,
    limit: usize,
) -> Vec<FuturePollTelemetry> {
    let mut telemetry = Vec::new();
    while telemetry.len() < limit {
        match receiver.recv().await {
            Ok(event) => {
                if let Ok(event) = FuturePollTelemetry::try_from(event) {
                    telemetry.push(event);
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    telemetry
}

fn telemetry_event(id: u64) -> Event {
    Event {
        target: "iroha_futures",
        fields: Fields(vec![
            ("id", Value::from(id)),
            ("name", Value::from("collection::test")),
            ("duration", Value::from(100_u64)),
        ]),
    }
}

#[tokio::test]
async fn telemetry_collection_skips_invalid_events_and_stops_at_limit() {
    let (sender, receiver) = broadcast::channel(8);
    let mut wrong_target = telemetry_event(1);
    wrong_target.target = "other";
    let mut invalid_duration = telemetry_event(2);
    invalid_duration.fields.0[2].1 = Value::from(-1_i64);
    for event in [
        wrong_target,
        Event {
            target: "iroha_futures",
            fields: Fields::default(),
        },
        telemetry_event(7),
        invalid_duration,
        telemetry_event(8),
        telemetry_event(9),
    ] {
        sender.send(event).unwrap();
    }
    let telemetry = tokio::time::timeout(
        Duration::from_secs(1),
        collect_future_telemetry(receiver, 2),
    )
    .await
    .expect("collection must stop at its limit while the sender remains open");
    assert_eq!(
        telemetry
            .iter()
            .map(|event| (event.id, event.name.as_str(), event.duration))
            .collect::<Vec<_>>(),
        [(7, "collection::test", 100), (8, "collection::test", 100)]
    );
}

#[tokio::test]
async fn telemetry_collection_recovers_from_lag_and_drains_on_close() {
    for (ids, expected) in [
        (&[][..], &[][..]),
        (&[1][..], &[1][..]),
        (&[1, 2, 3][..], &[2, 3][..]),
    ] {
        let (sender, receiver) = broadcast::channel(2);
        for &id in ids {
            sender.send(telemetry_event(id)).unwrap();
        }
        drop(sender);
        let telemetry = tokio::time::timeout(
            Duration::from_secs(1),
            collect_future_telemetry(receiver, 4),
        )
        .await
        .expect("closed channel must finish after its retained events");
        assert_eq!(
            telemetry.iter().map(|event| event.id).collect::<Vec<_>>(),
            expected
        );
    }
}

#[tokio::test]
async fn telemetry_collection_waits_for_data_unless_limit_is_zero() {
    let (sender, receiver) = broadcast::channel(1);
    let empty = tokio::time::timeout(
        Duration::ZERO,
        collect_future_telemetry(sender.subscribe(), 0),
    )
    .await
    .expect("zero limit must not wait for data");
    assert!(empty.is_empty());
    assert!(
        tokio::time::timeout(
            Duration::from_millis(10),
            collect_future_telemetry(receiver, 1),
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[allow(unsafe_code)]
async fn telemetry_attribute_preserves_function_qualifiers() {
    // SAFETY: the test function has no unsafe preconditions.
    assert_eq!(unsafe { qualified_abi(7) }.await, 7);
    assert_eq!(qualified_generic("value").await, "value");
}
#[tokio::test(flavor = "multi_thread")]
async fn test_sleep() {
    if cfg!(not(feature = "telemetry")) {
        return;
    }
    let sleep_times = vec![
        Duration::from_millis(100),
        Duration::from_millis(70),
        Duration::from_millis(80),
    ];
    let future_telemetry = iroha_logger::test_logger()
        .subscribe_on_telemetry(Channel::Future)
        .await
        .unwrap();
    assert_eq!(sleep(sleep_times.clone()).await, 10_i32);
    let telemetry = collect_future_telemetry(future_telemetry, 3);
    let telemetry = tokio::time::timeout(Duration::from_secs(5), telemetry)
        .await
        .expect("timed out waiting for future telemetry");
    assert_eq!(telemetry.len(), 3);
    let id = telemetry[0].id;
    let times = telemetry
        .iter()
        .map(|telemetry_item| Duration::from_nanos(telemetry_item.duration));
    assert!(
        telemetry
            .iter()
            .all(|telemetry_item| telemetry_item.name == "basic::sleep")
    );
    assert!(
        telemetry
            .iter()
            .all(|telemetry_item| telemetry_item.id == id)
    );
    assert!(times.zip(sleep_times).all(|(a, b)| almost_equal(a, b)));
}
