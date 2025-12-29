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

struct DummyMetrics {
    isi: DummyCounter,
    isi_times: DummyHistogram,
}

impl DummyMetrics {
    fn new() -> Self {
        Self {
            isi: DummyCounter,
            isi_times: DummyHistogram,
        }
    }
}

struct StateTransaction {
    metrics: DummyMetrics,
}

impl StateTransaction {
    fn new() -> Self {
        Self {
            metrics: DummyMetrics::new(),
        }
    }
}

#[metrics(+"timed_metric")]
fn execute(state: &StateTransaction) -> Result<(), ()> {
    let _ = state;
    Ok(())
}

fn main() {
    let tx = StateTransaction::new();
    let _ = execute(&tx);
}
