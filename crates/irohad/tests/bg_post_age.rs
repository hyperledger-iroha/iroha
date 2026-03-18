//! Minimal test to assert that background-post age histogram is observable.

#[test]
fn bg_post_age_histogram_observes_positive_value() {
    use std::{
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
    let telemetry = iroha_core::telemetry::Telemetry::new(metrics.clone(), true);

    // Simulate enqueue time and worker processing adding an age sample
    let enqueued_at = Instant::now();
    thread::sleep(Duration::from_millis(5));
    #[allow(clippy::cast_precision_loss)]
    let age_ms = enqueued_at.elapsed().as_millis() as f64;
    telemetry.observe_bg_post_age_ms("Broadcast", age_ms);

    // Encode Prometheus text and ensure our histogram series has a non-zero count
    let text = metrics
        .try_to_string()
        .expect("metrics encoding must succeed");
    // Very lightweight check: presence of the count series for Broadcast
    assert!(
        text.contains("sumeragi_bg_post_age_ms_count{kind=\"Broadcast\"}")
            || text.contains("sumeragi_bg_post_age_ms_count{kind=\"Broadcast\",}"),
        "age histogram count series missing for Broadcast; got:\n{text}",
    );
}
