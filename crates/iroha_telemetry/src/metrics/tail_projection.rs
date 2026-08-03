fn record_gauge_stats(gauge: &GaugeVec, samples: &[f64]) {
    const AVG_LABEL: [&str; 1] = ["avg"];
    const P95_LABEL: [&str; 1] = ["p95"];
    const MAX_LABEL: [&str; 1] = ["max"];
    const COUNT_LABEL: [&str; 1] = ["count"];

    if samples.is_empty() {
        gauge.with_label_values(&AVG_LABEL).set(0.0);
        gauge.with_label_values(&P95_LABEL).set(0.0);
        gauge.with_label_values(&MAX_LABEL).set(0.0);
        gauge.with_label_values(&COUNT_LABEL).set(0.0);
        return;
    }

    let len = samples.len();
    let count = u64::try_from(len).map_or_else(|_| u64_to_f64(u64::MAX), u64_to_f64);
    let sum: f64 = samples.iter().copied().sum();
    let avg = sum / count.max(1.0);

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);

    let max = *sorted.last().expect("non-empty after guard");
    let rank = ((len as u128) * 95).div_ceil(100);
    let p95_index = rank
        .saturating_sub(1)
        .try_into()
        .map_or(len - 1, |idx: usize| idx.min(len - 1));
    let p95 = sorted[p95_index];

    gauge.with_label_values(&AVG_LABEL).set(avg);
    gauge.with_label_values(&P95_LABEL).set(p95);
    gauge.with_label_values(&MAX_LABEL).set(max);
    gauge.with_label_values(&COUNT_LABEL).set(count);
}

#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

fn clamp_u32_to_i64(value: u32) -> i64 {
    i64::from(value)
}

fn u128_to_f64(value: u128) -> f64 {
    u64::try_from(value).map_or(f64::MAX, u64_to_f64)
}

fn quantity_to_micro_f64(value: &iroha_data_model::prelude::Quantity) -> f64 {
    let micros = value.as_numeric().to_f64_lossy() * 1_000_000.0;
    if micros.is_finite() { micros } else { f64::MAX }
}

/// Project an exact quantity into the legacy nano-unit `f64` used only by
/// telemetry instruments. This projection never feeds consensus state.
fn quantity_to_nano_f64(value: &Quantity) -> f64 {
    let nanos = value.as_numeric().to_f64_lossy() * 1_000_000_000.0;
    if nanos.is_finite() { nanos } else { f64::MAX }
}

fn family_has_lane_labels(family: &prometheus::proto::MetricFamily) -> bool {
    family
        .get_metric()
        .iter()
        .flat_map(prometheus::proto::Metric::get_label)
        .any(|label| {
            matches!(
                label.name(),
                "lane" | "lane_id" | "dataspace" | "dataspace_id"
            )
        })
}
