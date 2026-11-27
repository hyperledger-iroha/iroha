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
    /// Resident set when the phase started, including memory earlier phases
    /// left resident.
    phase_start_rss_bytes: Option<u64>,
    /// Peak resident set during this phase, observed as `phase_peak_method`
    /// says; it includes the start size. None where it cannot be read.
    phase_peak_rss_bytes: Option<u64>,
    process_lifetime_peak_rss_bytes: Option<u64>,
    current_process_threads: Option<u32>,
    /// Apple physical footprint (`task_vm_info.phys_footprint`, the measure
    /// jetsam enforces) when the phase started. It leaves out pages the
    /// allocator has freed and marked reusable, which the resident set still
    /// counts. None off Apple platforms.
    phase_start_footprint_bytes: Option<u64>,
    /// Peak physical footprint during this phase, sampled like the resident
    /// set (`phase_peak_method`). None off Apple platforms.
    phase_peak_footprint_bytes: Option<u64>,
}

#[derive(norito::JsonSerialize)]
pub(super) struct Measurements {
    schema: String,
    succeeded: bool,
    elapsed_us: u64,
    /// This wallet's own Send10 to the counterparty.
    outgoing_payment_bytes: Option<usize>,
    outgoing_payment_sha256: Option<String>,
    /// The counterparty's return Payment, the measured Receive's carrier.
    incoming_payment_bytes: Option<usize>,
    incoming_payment_sha256: Option<String>,
    program_bundle_sha256: Option<String>,
    receive_observed_flow_us: Option<u64>,
    receive_test_drill_us: Option<u64>,
    receive_instrumentation_us: Option<u64>,
    receive_normal_flow_us: Option<u64>,
    process_lifetime_peak_rss_bytes: Option<u64>,
    /// The kernel's lifetime peak physical footprint
    /// (`task_vm_info.ledger_phys_footprint_peak`, the jetsam measure), or
    /// the largest sampled footprint where the ledger is unavailable. None
    /// off Apple platforms.
    process_lifetime_peak_footprint_bytes: Option<u64>,
    current_process_threads: Option<u32>,
    /// How each event's phase peak was observed: "linux-vmhwm-reset" (exact)
    /// or "sampled-5ms" (a lower bound; one extra fixture thread).
    phase_peak_method: Option<String>,
    #[norito(skip)]
    peak_method: Option<PeakMethod>,
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
            schema: "kagemusha.experimental.device-observations.v2".into(),
            succeeded: false,
            elapsed_us: 0,
            outgoing_payment_bytes: None,
            outgoing_payment_sha256: None,
            incoming_payment_bytes: None,
            incoming_payment_sha256: None,
            program_bundle_sha256: None,
            receive_observed_flow_us: None,
            receive_test_drill_us: None,
            receive_instrumentation_us: None,
            receive_normal_flow_us: None,
            process_lifetime_peak_rss_bytes: None,
            process_lifetime_peak_footprint_bytes: None,
            current_process_threads: None,
            phase_peak_method: None,
            peak_method: None,
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
        let reset = Instant::now();
        // Keep the peak reached since the last observation (including any
        // unmeasured gap) before the reset discards it.
        let gap = self.phase_peak();
        self.observe_peak(gap);
        let gap_footprint = sampler::peak_footprint();
        self.observe_footprint(gap_footprint);
        let reset_ok = self.begin_phase();
        let start_rss = current_rss_bytes();
        let start_footprint = footprint::current();
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(reset));
        let start = Instant::now();
        let result = run();
        let elapsed_us = micros(start);
        let observation = Instant::now();
        let process = process_metrics();
        let phase_peak = if reset_ok { self.phase_peak() } else { None };
        self.observe_peak(phase_peak);
        self.observe_peak(process.peak_rss_bytes);
        let phase_peak_footprint = if reset_ok {
            sampler::peak_footprint()
        } else {
            None
        };
        self.observe_footprint(phase_peak_footprint);
        self.events.push(Event {
            operation: operation.into(),
            phase: phase.into(),
            elapsed_us,
            returned_ok: result.is_ok(),
            phase_start_rss_bytes: start_rss,
            phase_peak_rss_bytes: phase_peak,
            process_lifetime_peak_rss_bytes: self.process_lifetime_peak_rss_bytes,
            current_process_threads: process.process_threads,
            phase_start_footprint_bytes: start_footprint,
            phase_peak_footprint_bytes: phase_peak_footprint,
        });
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(observation));
        result
    }
    /// Reset this phase's peak to the current resident size. The first call
    /// selects the method: the exact Linux/Android VmHWM reset, otherwise the
    /// sampler.
    fn begin_phase(&mut self) -> bool {
        match self.peak_method {
            Some(PeakMethod::Hwm) => reset_hwm(),
            Some(PeakMethod::Sampled) => sampler::reset(),
            None => {
                let (method, reset) = if reset_hwm() {
                    (PeakMethod::Hwm, true)
                } else {
                    (PeakMethod::Sampled, sampler::reset())
                };
                self.peak_method = Some(method);
                self.phase_peak_method = Some(method.name().into());
                reset
            }
        }
    }
    fn phase_peak(&self) -> Option<u64> {
        match self.peak_method? {
            PeakMethod::Hwm => hwm_bytes(),
            PeakMethod::Sampled => sampler::peak(),
        }
    }
    /// A clear_refs reset also lowers what getrusage reports, so the lifetime
    /// peak is the maximum of every observation instead.
    fn observe_peak(&mut self, bytes: Option<u64>) {
        if let Some(bytes) = bytes {
            self.process_lifetime_peak_rss_bytes =
                Some(self.process_lifetime_peak_rss_bytes.unwrap_or(0).max(bytes));
        }
    }
    /// The lifetime footprint peak: the kernel ledger's when it reports one,
    /// never below any sampled footprint.
    fn observe_footprint(&mut self, sampled: Option<u64>) {
        for bytes in [sampled, footprint::lifetime_peak()].into_iter().flatten() {
            self.process_lifetime_peak_footprint_bytes = Some(
                self.process_lifetime_peak_footprint_bytes
                    .unwrap_or(0)
                    .max(bytes),
            );
        }
    }
    pub fn instrumentation_us(&self) -> u64 {
        self.instrumentation_us
    }
    pub fn retained(&mut self, operation: &str) {
        self.retained_operations.push(operation.into());
    }
    pub fn was_retained(&self, operation: &str) -> bool {
        self.retained_operations
            .iter()
            .any(|name| name == operation)
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
    pub fn outgoing_payment(&mut self, bytes: &[u8]) {
        let start = Instant::now();
        self.outgoing_payment_bytes = Some(bytes.len());
        self.outgoing_payment_sha256 = Some(hex_digest(bytes));
        self.instrumentation_us = self.instrumentation_us.saturating_add(micros(start));
    }
    pub fn incoming_payment(&mut self, bytes: &[u8]) {
        let start = Instant::now();
        self.incoming_payment_bytes = Some(bytes.len());
        self.incoming_payment_sha256 = Some(hex_digest(bytes));
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
        let last = self.phase_peak();
        self.observe_peak(last);
        self.observe_peak(process.peak_rss_bytes);
        let last_footprint = sampler::peak_footprint();
        self.observe_footprint(last_footprint);
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

#[derive(Clone, Copy)]
enum PeakMethod {
    Hwm,
    Sampled,
}
impl PeakMethod {
    fn name(self) -> &'static str {
        match self {
            Self::Hwm => "linux-vmhwm-reset",
            Self::Sampled => "sampled-5ms",
        }
    }
}

/// Linux resets the process's peak resident set (VmHWM) to its current size
/// when "5" is written to /proc/self/clear_refs.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn reset_hwm() -> bool {
    std::fs::write("/proc/self/clear_refs", b"5").is_ok()
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn reset_hwm() -> bool {
    false
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn status_bytes(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib: u64 = status
        .lines()
        .find_map(|line| line.strip_prefix(field))?
        .trim()
        .strip_suffix("kB")?
        .trim()
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn hwm_bytes() -> Option<u64> {
    status_bytes("VmHWM:")
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn hwm_bytes() -> Option<u64> {
    None
}

/// Current resident set size of this process.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn current_rss_bytes() -> Option<u64> {
    status_bytes("VmRSS:")
}
#[cfg(target_vendor = "apple")]
fn current_rss_bytes() -> Option<u64> {
    /// `struct mach_task_basic_info` (mach/task_info.h); 12 natural_t words.
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [i32; 2],
        system_time: [i32; 2],
        policy: i32,
        suspend_count: i32,
    }
    const MACH_TASK_BASIC_INFO: u32 = 20;
    unsafe extern "C" {
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: u32, info: *mut i32, count: *mut u32) -> i32;
    }
    let mut info = std::mem::MaybeUninit::<MachTaskBasicInfo>::zeroed();
    let mut count = (std::mem::size_of::<MachTaskBasicInfo>() / 4) as u32;
    // SAFETY: writable storage of the documented flavor size; the task port
    // names this process.
    let status = unsafe {
        task_info(
            mach_task_self_,
            MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    // SAFETY: KERN_SUCCESS initialized the structure.
    (status == 0).then(|| unsafe { info.assume_init() }.resident_size)
}
#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn current_rss_bytes() -> Option<u64> {
    None
}

/// Apple physical footprint from `task_info(TASK_VM_INFO)`: the jetsam
/// measure, which excludes freed reusable pages the resident set includes.
mod footprint {
    #[cfg(target_vendor = "apple")]
    fn vm_info() -> Option<([u64; 48], u32)> {
        // <mach/task_info.h>: TASK_VM_INFO is flavor 22; `task_vm_info_data_t`
        // is a sequence of 64-bit fields (with two 32-bit fields packed into
        // the second word). `count` is in 32-bit words.
        const TASK_VM_INFO: u32 = 22;
        unsafe extern "C" {
            static mach_task_self_: u32;
            fn task_info(task: u32, flavor: u32, info: *mut u64, count: *mut u32) -> i32;
        }
        let mut info = [0u64; 48];
        let mut count = (info.len() * 2) as u32;
        // SAFETY: task_info writes at most `count` 32-bit words into `info`;
        // the task port names this process.
        let status =
            unsafe { task_info(mach_task_self_, TASK_VM_INFO, info.as_mut_ptr(), &mut count) };
        (status == 0).then_some((info, count))
    }
    /// `phys_footprint` (field 18, revision 1 and later).
    #[cfg(target_vendor = "apple")]
    pub(super) fn current() -> Option<u64> {
        let (info, count) = vm_info()?;
        (count >= 38).then_some(info[18])
    }
    /// `ledger_phys_footprint_peak` (field 21, revision 3 and later).
    #[cfg(target_vendor = "apple")]
    pub(super) fn lifetime_peak() -> Option<u64> {
        let (info, count) = vm_info()?;
        (count >= 44).then_some(info[21])
    }
    #[cfg(not(target_vendor = "apple"))]
    pub(super) fn current() -> Option<u64> {
        None
    }
    #[cfg(not(target_vendor = "apple"))]
    pub(super) fn lifetime_peak() -> Option<u64> {
        None
    }
}

/// Phase peaks where no exact reset exists: one fixture thread folds the
/// resident size into a running maximum every 5 ms.
mod sampler {
    use std::sync::{
        Once,
        atomic::{AtomicU64, Ordering},
    };
    static PEAK: AtomicU64 = AtomicU64::new(0);
    static PEAK_FOOTPRINT: AtomicU64 = AtomicU64::new(0);
    static START: Once = Once::new();
    pub(super) fn reset() -> bool {
        let Some(now) = super::current_rss_bytes() else {
            return false;
        };
        START.call_once(|| {
            // Without the thread, peak() still reports max(start, end).
            let _ = std::thread::Builder::new()
                .name("rss-sampler".into())
                .spawn(|| {
                    loop {
                        if let Some(now) = super::current_rss_bytes() {
                            PEAK.fetch_max(now, Ordering::Relaxed);
                        }
                        if let Some(now) = super::footprint::current() {
                            PEAK_FOOTPRINT.fetch_max(now, Ordering::Relaxed);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                });
        });
        PEAK.store(now, Ordering::Relaxed);
        PEAK_FOOTPRINT.store(super::footprint::current().unwrap_or(0), Ordering::Relaxed);
        true
    }
    pub(super) fn peak() -> Option<u64> {
        let now = super::current_rss_bytes()?;
        Some(PEAK.fetch_max(now, Ordering::Relaxed).max(now))
    }
    /// The phase's largest sampled physical footprint (Apple only).
    pub(super) fn peak_footprint() -> Option<u64> {
        let now = super::footprint::current()?;
        Some(PEAK_FOOTPRINT.fetch_max(now, Ordering::Relaxed).max(now))
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
