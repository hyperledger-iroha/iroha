//! Named wall-clock timers for the prover's kernels (review lane ebb983).
//! `ZKS_PROF=1` prints them after each proof; otherwise they only accumulate.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

static TIMES: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());

pub fn add(name: &'static str, seconds: f64) {
    *TIMES.lock().unwrap().entry(name).or_insert(0.0) += seconds;
}

/// Runs `f` and charges its wall time to `name`.
pub fn time<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    let clock = Instant::now();
    let out = f();
    add(name, clock.elapsed().as_secs_f64());
    out
}

/// The timers since the last call, cleared.
pub fn take() -> BTreeMap<&'static str, f64> {
    std::mem::take(&mut *TIMES.lock().unwrap())
}

pub fn enabled() -> bool {
    std::env::var("ZKS_PROF").is_ok()
}

/// `ZKS_PROF` set to anything but `time`: also trace and account memory.
pub fn memory_enabled() -> bool {
    std::env::var("ZKS_PROF").is_ok_and(|v| v != "time")
}

/// One line of the timers in milliseconds, cleared.
pub fn line() -> String {
    take()
        .into_iter()
        .map(|(k, v)| format!("{k} {:.1}", v * 1e3))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resident set size and physical footprint of this process now, in bytes
/// (0 where unsupported). The footprint leaves out pages the allocator has
/// freed and marked reusable, which the resident set still counts.
#[cfg(target_os = "macos")]
pub fn resident_and_footprint() -> (u64, u64) {
    // <mach/task_info.h>: TASK_VM_INFO (22); resident_size at byte 16,
    // phys_footprint at byte 144.
    unsafe extern "C" {
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: i32, info: *mut u64, count: *mut u32) -> i32;
    }
    let mut info = [0u64; 48];
    let mut count = (info.len() * 2) as u32;
    // SAFETY: task_info writes at most `count` 32-bit words into `info`.
    let ok = unsafe { task_info(mach_task_self_, 22, info.as_mut_ptr(), &mut count) } == 0;
    if ok && count >= 38 {
        (info[2], info[18])
    } else {
        (0, 0)
    }
}

/// Resident set size of this process now, in bytes, twice (no separate
/// footprint); 0 where unsupported.
#[cfg(not(target_os = "macos"))]
pub fn resident_and_footprint() -> (u64, u64) {
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb * 1024);
    (rss, rss)
}

/// Returns the allocator's free pages to the system (macOS), so that a
/// measurement's starting resident set holds live memory only and a phase's
/// peak shows its own allocations rather than reused free pages.
#[cfg(target_os = "macos")]
pub fn release_free_memory() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }
    // SAFETY: a null zone asks every malloc zone to release free memory.
    unsafe {
        malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn release_free_memory() {}

/// Peak resident memory per prover phase (`ZKS_PROF=1`, not `time`): a thread samples
/// the process's resident set every millisecond; each [`MemTrace::mark`]
/// closes a phase with the largest sample since the previous mark.
pub struct MemTrace {
    state: std::sync::Arc<(
        std::sync::atomic::AtomicU64,
        std::sync::atomic::AtomicU64,
        std::sync::atomic::AtomicBool,
    )>,
    sampler: Option<std::thread::JoinHandle<()>>,
    start: (u64, u64),
    clock: Instant,
    phases: Mutex<Vec<(&'static str, u64, u64, f64)>>,
}

impl MemTrace {
    pub fn start() -> Option<Self> {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        if !memory_enabled() {
            return None;
        }
        release_free_memory();
        let start = resident_and_footprint();
        let state = std::sync::Arc::new((
            AtomicU64::new(start.0),
            AtomicU64::new(start.1),
            AtomicBool::new(false),
        ));
        let shared = state.clone();
        let sampler = std::thread::spawn(move || {
            while !shared.2.load(Ordering::Relaxed) {
                let (rss, footprint) = resident_and_footprint();
                shared.0.fetch_max(rss, Ordering::Relaxed);
                shared.1.fetch_max(footprint, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        Some(MemTrace {
            state,
            sampler: Some(sampler),
            start,
            clock: Instant::now(),
            phases: Mutex::new(Vec::new()),
        })
    }

    /// Ends phase `name` with its peak resident set and peak footprint.
    pub fn mark(&self, name: &'static str) {
        use std::sync::atomic::Ordering;
        let (rss, footprint) = resident_and_footprint();
        let peak = self.state.0.swap(rss, Ordering::Relaxed).max(rss);
        let peak_footprint = self
            .state
            .1
            .swap(footprint, Ordering::Relaxed)
            .max(footprint);
        let t = self.clock.elapsed().as_secs_f64();
        self.phases
            .lock()
            .unwrap()
            .push((name, peak, peak_footprint, t));
    }

    /// One line: start, overall peaks, and each phase's peak resident set /
    /// peak footprint (and end time).
    pub fn finish(mut self, label: &str) -> String {
        use std::sync::atomic::Ordering;
        self.state.2.store(true, Ordering::Relaxed);
        if let Some(s) = self.sampler.take() {
            let _ = s.join();
        }
        let mb = |b: u64| b as f64 / 1e6;
        let phases = self.phases.lock().unwrap();
        let peak = phases.iter().map(|p| p.1).max().unwrap_or(self.start.0);
        let peak_footprint = phases.iter().map(|p| p.2).max().unwrap_or(self.start.1);
        let list = phases
            .iter()
            .map(|(n, p, f, t)| format!("{n} {:.0}/{:.0}@{t:.2}s", mb(*p), mb(*f)))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "prover memory {label}: start rss {:.0} MB footprint {:.0} MB; peak rss {:.0} MB (+{:.0}), footprint {:.0} MB (+{:.0}); total {:.3} s; phase peaks rss/footprint MB: {list}",
            mb(self.start.0),
            mb(self.start.1),
            mb(peak),
            mb(peak.saturating_sub(self.start.0)),
            mb(peak_footprint),
            mb(peak_footprint.saturating_sub(self.start.1)),
            self.clock.elapsed().as_secs_f64()
        )
    }
}
