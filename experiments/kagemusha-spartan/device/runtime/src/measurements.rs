//! Observed wall timings and OS process measurements for the isolated device run.
use super::{Result, fail};
use connect_norito_bridge::kagemusha_wallet_ffi::spartan_device_fixture::process_metrics;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};

#[derive(norito::JsonSerialize)]
struct Event {
    operation: String,
    phase: String,
    elapsed_us: u64,
    returned_ok: bool,
    process_lifetime_peak_rss_bytes: Option<u64>,
    current_process_threads: Option<u32>,
}

#[derive(norito::JsonSerialize)]
pub(super) struct Measurements {
    schema: String,
    succeeded: bool,
    elapsed_us: u64,
    complete_payment_bytes: Option<usize>,
    complete_payment_sha256: Option<String>,
    program_bundle_sha256: Option<String>,
    receive_observed_flow_us: Option<u64>,
    receive_test_drill_us: Option<u64>,
    receive_instrumentation_us: Option<u64>,
    receive_normal_flow_us: Option<u64>,
    process_lifetime_peak_rss_bytes: Option<u64>,
    current_process_threads: Option<u32>,
    instrumentation_us: u64,
    retained_operations: Vec<String>,
    events: Vec<Event>,
}

pub(super) fn micros(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}

impl Measurements {
    pub fn new() -> Self {
        Self {
            schema: "kagemusha.experimental.device-observations.v1".into(),
            succeeded: false,
            elapsed_us: 0,
            complete_payment_bytes: None,
            complete_payment_sha256: None,
            program_bundle_sha256: None,
            receive_observed_flow_us: None,
            receive_test_drill_us: None,
            receive_instrumentation_us: None,
            receive_normal_flow_us: None,
            process_lifetime_peak_rss_bytes: None,
            current_process_threads: None,
            instrumentation_us: 0,
            retained_operations: Vec::new(),
            events: Vec::new(),
        }
    }
    pub fn measure<T>(
        &mut self,
        operation: &str,
        phase: &str,
        run: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let start = Instant::now();
        let result = run();
        let elapsed_us = micros(start);
        let observation = Instant::now();
        let process = process_metrics();
        self.events.push(Event {
            operation: operation.into(),
            phase: phase.into(),
            elapsed_us,
            returned_ok: result.is_ok(),
            process_lifetime_peak_rss_bytes: process.peak_rss_bytes,
            current_process_threads: process.process_threads,
        });
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(observation));
        result
    }
    pub fn instrumentation_us(&self) -> u64 {
        self.instrumentation_us
    }
    pub fn retained(&mut self, operation: &str) {
        self.retained_operations.push(operation.into());
    }
    pub fn test_drill_us(&self, operation: &str) -> u64 {
        self.events
            .iter()
            .filter(|e| e.operation == operation && e.phase.ends_with("_drill"))
            .map(|e| e.elapsed_us)
            .fold(0, u64::saturating_add)
    }
    pub fn program_bundle(&mut self, bytes: &[u8]) {
        let start = Instant::now();
        self.program_bundle_sha256 = Some(hex_digest(bytes));
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(start));
    }
    pub fn payment(&mut self, bytes: &[u8]) {
        let start = Instant::now();
        self.complete_payment_bytes = Some(bytes.len());
        self.complete_payment_sha256 = Some(hex_digest(bytes));
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(start));
    }
    pub fn receive_flow(&mut self, start: Instant, instrumentation_before: u64) {
        let observed = micros(start);
        let drills = self.test_drill_us("Receive");
        let instrumentation = self
            .instrumentation_us
            .saturating_sub(instrumentation_before);
        self.receive_observed_flow_us = Some(observed);
        self.receive_test_drill_us = Some(drills);
        self.receive_instrumentation_us = Some(instrumentation);
        // All phases run serially on this thread. This is observed end-to-end
        // time minus measured fixture-only drills and measurement overhead.
        self.receive_normal_flow_us = Some(
            observed
                .saturating_sub(drills)
                .saturating_sub(instrumentation),
        );
    }
    pub fn finish(&mut self, root: &Path, start: Instant, succeeded: bool) -> Result<String> {
        self.elapsed_us = micros(start);
        self.succeeded = succeeded;
        let process = process_metrics();
        self.process_lifetime_peak_rss_bytes = process.peak_rss_bytes;
        self.current_process_threads = process.process_threads;
        let report = norito::json::to_json(self).map_err(fail)?;
        std::fs::write(root.join("device-measurements.json"), &report).map_err(fail)?;
        use std::io::Write;
        let mut history = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("device-measurements.jsonl"))
            .map_err(fail)?;
        writeln!(history, "{report}").map_err(fail)?;
        history.sync_all().map_err(fail)?;
        Ok(report)
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut out, "{byte:02x}").expect("writing to a String");
    }
    out
}
