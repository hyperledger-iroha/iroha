#![cfg(feature = "metric-instrumentation")]

use iroha_telemetry_derive::{metric_success_label, metric_total_label};

fn main() {
    let total: &str = metric_total_label!();
    let success: &str = metric_success_label!();
    let _ = (total, success);
}
