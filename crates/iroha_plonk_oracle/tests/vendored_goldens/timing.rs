//! Native against vendored prove time on the k = 11 sigma golden (oracle
//! builds, ignored; run in release, one test thread, on a host as idle as
//! possible).
//!
//! For each curve and pool size (1 and 4 threads) the row first proves once
//! on each side and checks both proofs against the golden constant, then
//! alternates [`ROUNDS`] timed batches of [`CALLS`] proofs: vendored, native,
//! vendored, native, so load changes affect both sides. A row reports per
//! proof the minimum and median wall time, the process CPU time of the
//! faster batch (all threads, sampled with `ps` at 10 ms resolution) and the
//! one-minute load average when the row started and ended.
//!
//! The vendored prover synthesizes the witness inside `create_proof`; the
//! native prover proves the exported tables, so the export (both capture
//! passes) is timed separately per curve.

use std::{
    process::Command,
    time::{Duration, Instant},
};

use iroha_plonk_oracle::convert::{CurveBridge, Pallas, Vesta};
use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::{
    cases::{Family, Setup, cases_for, setup},
    golden_proof_bytes::digest_hex,
    proof_parity::prove_native,
};

/// Pool sizes every row runs at.
const THREADS: [usize; 2] = [1, 4];
/// Alternating timed batches per side.
const ROUNDS: usize = 3;
/// Proofs per batch.
const CALLS: u32 = 3;

/// A pool of `threads` workers.
fn pool(threads: usize) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("build a Rayon pool")
}

/// Parses a `ps` CPU time (`[[dd-]hh:]mm:ss.cc`) into seconds.
fn parse_cpu_time(text: &str) -> Option<f64> {
    let text = text.trim();
    let (days, clock) = match text.split_once('-') {
        Some((days, clock)) => (days.parse::<f64>().ok()?, clock),
        None => (0.0, text),
    };
    let mut seconds = 0.0;
    for part in clock.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(days.mul_add(86_400.0, seconds))
}

/// CPU seconds consumed so far by this process (all threads), if `ps` is
/// available.
fn process_cpu_seconds() -> Option<f64> {
    let output = Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    parse_cpu_time(std::str::from_utf8(&output.stdout).ok()?)
}

/// Parses the one-minute load average from `sysctl -n vm.loadavg` or
/// `/proc/loadavg` text.
fn parse_load1(text: &str) -> Option<f64> {
    text.split_whitespace()
        .find(|token| *token != "{")
        .and_then(|token| token.parse().ok())
}

/// The one-minute load average, or `NaN` when unavailable.
fn load1() -> f64 {
    let text = std::fs::read_to_string("/proc/loadavg").ok().or_else(|| {
        Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
    });
    text.as_deref().and_then(parse_load1).unwrap_or(f64::NAN)
}

/// Wall times of every call and the CPU time per call of each batch.
#[derive(Debug, Default)]
struct Timing {
    walls: Vec<Duration>,
    cpu_per_call: Vec<f64>,
}

impl Timing {
    /// Runs one batch of [`CALLS`] calls of `f` inside `pool`.
    fn batch(&mut self, pool: &ThreadPool, f: &(dyn Fn() -> Vec<u8> + Sync)) {
        let cpu_start = process_cpu_seconds();
        for _ in 0..CALLS {
            let start = Instant::now();
            let proof = pool.install(f);
            self.walls.push(start.elapsed());
            std::hint::black_box(proof);
        }
        if let (Some(start), Some(end)) = (cpu_start, process_cpu_seconds()) {
            self.cpu_per_call.push((end - start) / f64::from(CALLS));
        }
    }

    /// `min / median` wall time in milliseconds and the smallest CPU time
    /// per call in milliseconds (`-` without a sampler).
    fn summary(&self) -> (f64, f64, String) {
        let mut walls: Vec<f64> = self.walls.iter().map(|d| d.as_secs_f64() * 1e3).collect();
        walls.sort_by(f64::total_cmp);
        let min = walls.first().copied().unwrap_or(f64::NAN);
        let median = walls.get(walls.len() / 2).copied().unwrap_or(f64::NAN);
        let cpu = self
            .cpu_per_call
            .iter()
            .copied()
            .min_by(f64::total_cmp)
            .map_or_else(|| "-".to_owned(), |cpu| format!("{:.0}", cpu * 1e3));
        (min, median, cpu)
    }
}

/// Times one curve; returns its Markdown rows.
fn time_curve<B: CurveBridge>() -> Vec<String> {
    let setup: std::sync::Arc<Setup<B>> = setup::<B>(Family::Sigma, 11);
    let case = cases_for(Family::Sigma, B::NAME, 11).remove(0);
    let seed = case.seed_bytes();
    let export_start = Instant::now();
    let exported = setup.reexport();
    let export_ms = export_start.elapsed().as_secs_f64() * 1e3;
    assert_eq!(exported.advice(), setup.exported.advice());
    let mut rows = vec![format!(
        "| {} | export (both capture passes) | - | {export_ms:.1} | - | - |",
        B::NAME
    )];
    for threads in THREADS {
        let pool = pool(threads);
        let load_start = load1();
        let vendored = pool.install(|| setup.prove_vendored(seed));
        let native = pool.install(|| prove_native(&setup, seed));
        assert_eq!(digest_hex(&vendored), case.sha256, "vendored {}", case.name);
        assert_eq!(native, vendored, "native {}", case.name);
        let (mut v, mut n) = (Timing::default(), Timing::default());
        for _ in 0..ROUNDS {
            v.batch(&pool, &|| setup.prove_vendored(seed));
            n.batch(&pool, &|| prove_native(&setup, seed));
        }
        let load_end = load1();
        let (v_min, v_med, v_cpu) = v.summary();
        let (n_min, n_med, n_cpu) = n.summary();
        rows.push(format!(
            "| {} | {threads} | {v_min:.1} / {v_med:.1} / {v_cpu} | {n_min:.1} / {n_med:.1} / {n_cpu} \
             | x{:.2} | {load_start:.1} -> {load_end:.1} |",
            B::NAME,
            v_med / n_med
        ));
    }
    rows
}

#[test]
#[ignore = "release timing of the k = 11 sigma golden; run with --nocapture --test-threads=1"]
fn timing_sigma_k11_native_vs_vendored() {
    println!(
        "| curve | threads | vendored min / med / cpu (ms) | native min / med / cpu (ms) \
         | speedup (med) | load1 start -> end |"
    );
    println!("|---|---|---|---|---|---|");
    for row in time_curve::<Vesta>()
        .into_iter()
        .chain(time_curve::<Pallas>())
    {
        println!("{row}");
    }
}

#[test]
fn timing_parsers_read_ps_and_sysctl_output() {
    assert_eq!(parse_cpu_time("01:02.50"), Some(62.5));
    assert_eq!(parse_cpu_time("1-00:00:01.00"), Some(86_401.0));
    assert_eq!(parse_cpu_time("x"), None);
    assert_eq!(parse_load1("{ 1.50 2.00 3.00 }"), Some(1.5));
    assert_eq!(parse_load1("0.25 0.30 0.35 1/100 42"), Some(0.25));
    let mut timing = Timing::default();
    assert!(timing.summary().0.is_nan());
    timing.walls.push(Duration::from_millis(3));
    timing.walls.push(Duration::from_millis(1));
    timing.walls.push(Duration::from_millis(2));
    let (min, median, cpu) = timing.summary();
    assert!((min - 1.0).abs() < 1e-9 && (median - 2.0).abs() < 1e-9);
    assert_eq!(cpu, "-");
}
