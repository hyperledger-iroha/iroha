//! The daemon exports the native driver's per-instance latency series.

#[test]
fn native_consensus_latency_histograms_export_exact_samples() {
    use iroha_telemetry::metrics::{Metrics, sumeragi::GLOBAL_LANE};

    let metrics = Metrics::default();
    let global = metrics.sumeragi_instance(GLOBAL_LANE);
    global.observe_commit_latency_ms(5);
    global.observe_apply_latency_ms(7);
    let text = metrics.try_to_string().expect("encode metrics");
    for sample in [
        "sumeragi_commit_latency_ms_count{lane=\"global\"} 1",
        "sumeragi_commit_latency_ms_sum{lane=\"global\"} 5",
        "sumeragi_apply_latency_ms_count{lane=\"global\"} 1",
        "sumeragi_apply_latency_ms_sum{lane=\"global\"} 7",
    ] {
        assert!(text.lines().any(|line| line == sample), "missing {sample}");
    }
}
