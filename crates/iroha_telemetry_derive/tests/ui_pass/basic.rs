use iroha_telemetry_derive::metrics;

struct DummyCounter;

impl DummyCounter {
    fn with_label_values(&self, _: &[&str]) -> DummyCounterHandle {
        DummyCounterHandle
    }
}

struct DummyCounterHandle;

impl DummyCounterHandle {
    fn inc(&self) {}
}

struct DummyHistogram;

impl DummyHistogram {
    fn with_label_values(&self, _: &[&str]) -> DummyHistogramHandle {
        DummyHistogramHandle
    }
}

struct DummyHistogramHandle;

impl DummyHistogramHandle {
    fn observe(&self, _: f64) {}
}

struct DummyTelemetry {
    isi: DummyCounter,
    isi_times: DummyHistogram,
}

impl DummyTelemetry {
    fn new() -> Self {
        Self {
            isi: DummyCounter,
            isi_times: DummyHistogram,
        }
    }

    fn record_isi_total(&self, name: &str) {
        self.isi.with_label_values(&[name, "total"]).inc();
    }

    fn record_isi_success(&self, name: &str) {
        self.isi.with_label_values(&[name, "success"]).inc();
    }

    fn record_isi_time(&self, name: &str, elapsed: std::time::Duration) {
        self.isi_times
            .with_label_values(&[name])
            .observe(elapsed.as_millis() as f64);
    }
}

struct StateTransaction {
    telemetry: DummyTelemetry,
}

impl StateTransaction {
    fn new() -> Self {
        Self {
            telemetry: DummyTelemetry::new(),
        }
    }

    fn metrics(&self) -> &DummyTelemetry {
        &self.telemetry
    }
}

#[metrics("test_metric")]
fn execute(state: &StateTransaction) -> Result<(), ()> {
    let _ = state;
    Ok(())
}

fn main() {
    let tx = StateTransaction::new();
    let _ = execute(&tx);
}
