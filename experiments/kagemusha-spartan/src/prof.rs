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

/// One line of the timers in milliseconds, cleared.
pub fn line() -> String {
    take()
        .into_iter()
        .map(|(k, v)| format!("{k} {:.1}", v * 1e3))
        .collect::<Vec<_>>()
        .join(", ")
}
