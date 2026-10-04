//! Release microbenchmarks of the native `iroha_pasta` kernels against the
//! vendored ones, side by side.
//!
//! Every benchmark is an ignored test. Run them in release, one test thread,
//! on a host that is as idle as possible:
//!
//! ```text
//! cargo test --release -p iroha_plonk_oracle --test kernel_benchmarks -- \
//!     --ignored --nocapture --test-threads=1
//! ```
//!
//! Each row runs the vendored and the native kernel on the same inputs, in the
//! same process and in the same Rayon pool (1 or 4 threads), after one
//! untimed call of each that also asserts identical outputs. Timed batches
//! alternate vendored, native, vendored, native, so load changes during a row
//! affect both sides. A row reports, per call:
//!
//! - `min` and `med`: minimum and median wall time over every timed call;
//! - `cpu`: process CPU time (all threads) over the faster batch divided by
//!   its calls, sampled with `ps` (10 ms resolution, so short kernels use many
//!   calls); `-` where the host offers no sampler;
//! - `load1`: the one-minute load average when the row started.
//!
//! Kernels:
//!
//! - `msm`: `best_multiexp` against `msm_public`, `msm_secret` and (up to
//!   2^16 points, table built outside the timing) `FixedBaseTable::msm_public`,
//!   random full-width scalars and distinct bases, 2^10..=2^18 points. The
//!   vendored large-MSM path (window at least 10, about 2,060 points and up)
//!   runs on its own process-wide pool of two workers whatever the caller's
//!   pool, so its 1-thread wall time uses two cores; compare `cpu` as well.
//! - `fold`: the full k-round IPA generator fold (`generator_collapse`, the
//!   vendored `parallel_generator_collapse`, against `fold_generators_vartime`)
//!   from the `ParamsIPA` generators with random challenges, k = 11..=16.
//! - `fft`: `best_fft` against `FftDomain::fft`, `lagrange_to_coeff` against
//!   `ifft`, and `coeff_to_extended` (quotient degree 1, so the coset has the
//!   base size) against `coset_fft`, over `Fp`, k = 11..=16.
//!
//! Results are recorded in the scratch measurement notes, not in repository
//! documents.

use std::{
    hint::black_box,
    process::Command,
    time::{Duration, Instant},
};

use halo2_axiom::{
    arithmetic::{best_fft, best_multiexp},
    halo2curves::{
        ff::{Field, WithSmallOrderMulGroup},
        group::{Curve, Group, GroupEncoding},
    },
    poly::EvaluationDomain,
};
use iroha_pasta::{
    fft::FftDomain,
    fold::fold_generators_vartime,
    msm::{FixedBaseTable, MemoryBudget, msm_public, msm_secret},
    params::ParamsIpa,
};
use iroha_plonk_oracle::{
    convert::{
        CurveBridge, NativeAffine, NativeScalar, Vesta, native_affine, native_scalars,
        vendored_affines,
    },
    vendored::generator_collapse,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use rayon::{ThreadPool, ThreadPoolBuilder};

/// Pool sizes every row runs at.
const THREADS: [usize; 2] = [1, 4];
/// Alternating batches per kernel and row.
const ROUNDS: usize = 2;

/// The benchmarked half of the cycle (scalars `Fp`, points on Vesta).
type Bench = Vesta;
/// Vendored scalars of [`Bench`].
type VScalar = <Bench as CurveBridge>::VScalar;
/// Vendored affine points of [`Bench`].
type VAffine = <Bench as CurveBridge>::Vendored;

/// A pool of `threads` workers.
fn pool(threads: usize) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("build a Rayon pool")
}

/// Parses a `ps` CPU time (`[[dd-]hh:]mm:ss.cc`, minutes may exceed 59) into
/// seconds.
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

/// CPU seconds consumed so far by this process (all threads), if the host
/// offers a sampler.
fn process_cpu_seconds() -> Option<f64> {
    let output = Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    parse_cpu_time(std::str::from_utf8(&output.stdout).ok()?)
}

/// Parses the one-minute load average from `sysctl -n vm.loadavg`
/// (`{ 1.00 2.00 3.00 }`) or `/proc/loadavg` (`1.00 2.00 3.00 ...`) text.
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

/// Timings of one kernel in one row.
#[derive(Clone, Debug, Default)]
struct Timing {
    /// Wall time of every timed call.
    calls: Vec<Duration>,
    /// Lowest per-call process CPU time over the batches.
    cpu_per_call: Option<f64>,
}

impl Timing {
    /// Times one batch of `reps` calls of `f`, adding it to the record.
    fn batch(&mut self, reps: usize, mut f: impl FnMut()) {
        let cpu_start = process_cpu_seconds();
        for _ in 0..reps {
            let start = Instant::now();
            f();
            self.calls.push(start.elapsed());
        }
        if let (Some(start), Some(end)) = (cpu_start, process_cpu_seconds()) {
            let reps = u32::try_from(reps).expect("small rep count");
            let per_call = (end - start) / f64::from(reps);
            self.cpu_per_call = Some(
                self.cpu_per_call
                    .map_or(per_call, |best| best.min(per_call)),
            );
        }
    }

    /// Minimum wall time in milliseconds.
    fn min_ms(&self) -> f64 {
        self.calls
            .iter()
            .min()
            .map_or(f64::NAN, |d| d.as_secs_f64() * 1e3)
    }

    /// Median wall time in milliseconds.
    fn median_ms(&self) -> f64 {
        let mut sorted = self.calls.clone();
        sorted.sort_unstable();
        sorted
            .get(sorted.len() / 2)
            .map_or(f64::NAN, |d| d.as_secs_f64() * 1e3)
    }

    /// `min/med/cpu` in milliseconds for a table cell.
    fn cell(&self) -> String {
        let cpu = self
            .cpu_per_call
            .map_or_else(|| "-".to_owned(), |cpu| format!("{:.2}", cpu * 1e3));
        format!("{:.2} / {:.2} / {cpu}", self.min_ms(), self.median_ms())
    }
}

/// Times named kernels in alternating batches; returns one [`Timing`] each.
fn compare(reps: usize, kernels: &mut [(&str, &mut dyn FnMut())]) -> Vec<Timing> {
    let mut timings = vec![Timing::default(); kernels.len()];
    for _ in 0..ROUNDS {
        for ((_, kernel), timing) in kernels.iter_mut().zip(&mut timings) {
            timing.batch(reps, kernel);
        }
    }
    timings
}

/// Native-over-vendored speedup of the minimum wall times, as `x1.23`.
fn speedup(vendored: &Timing, native: &Timing) -> String {
    format!("x{:.2}", vendored.min_ms() / native.min_ms())
}

/// Distinct pseudo-random native bases (a random arithmetic progression).
fn bases(n: usize, rng: &mut ChaCha20Rng) -> Vec<NativeAffine<Bench>> {
    let step = <Bench as CurveBridge>::Native::random(&mut *rng);
    let mut current = <Bench as CurveBridge>::Native::random(&mut *rng);
    let projective: Vec<_> = (0..n)
        .map(|_| {
            current += step;
            current
        })
        .collect();
    iroha_pasta::curve::batch_normalize_vartime(&projective)
}

/// Calls per batch for an MSM of `n` points.
fn msm_reps(n: usize) -> usize {
    match n {
        0..=4096 => 20,
        4097..=16384 => 8,
        16385..=65536 => 4,
        _ => 2,
    }
}

#[test]
#[ignore = "release microbenchmark: MSM 2^10..=2^18, 1 and 4 threads"]
fn msm_vendored_vs_native() {
    println!(
        "| n | threads | vendored best_multiexp | native msm_public | native msm_secret | \
         native fixed-base | public speedup | load1 |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    let budget = MemoryBudget::DEFAULT;
    for log_n in 10..=18 {
        let n = 1_usize << log_n;
        let mut rng = ChaCha20Rng::seed_from_u64(log_n);
        let native_bases = bases(n, &mut rng);
        let vendored_bases: Vec<VAffine> = vendored_affines::<Bench>(&native_bases);
        let vendored_scalars: Vec<VScalar> = (0..n).map(|_| VScalar::random(&mut rng)).collect();
        let scalars = native_scalars::<Bench>(&vendored_scalars);
        let table = (n <= 1 << 16).then(|| {
            FixedBaseTable::<<Bench as CurveBridge>::Native>::new(&native_bases, budget)
                .expect("table fits the default budget")
        });
        for threads in THREADS {
            pool(threads).install(|| {
                let load = load1();
                let expected = best_multiexp(&vendored_scalars, &vendored_bases).to_affine();
                let public =
                    msm_public::<<Bench as CurveBridge>::Native>(&scalars, &native_bases, budget)
                        .expect("msm_public");
                assert_eq!(native_affine::<Bench>(&expected), public.to_affine());
                let mut vendored = || {
                    black_box(best_multiexp(&vendored_scalars, &vendored_bases));
                };
                let mut native_public = || {
                    black_box(msm_public::<<Bench as CurveBridge>::Native>(
                        &scalars,
                        &native_bases,
                        budget,
                    ))
                    .expect("msm_public");
                };
                let mut native_secret = || {
                    black_box(msm_secret::<<Bench as CurveBridge>::Native>(
                        &scalars,
                        &native_bases,
                        budget,
                    ))
                    .expect("msm_secret");
                };
                let mut fixed = || {
                    if let Some(table) = &table {
                        black_box(table.msm_public(&scalars, budget)).expect("fixed-base");
                    }
                };
                if let Some(table) = &table {
                    assert_eq!(
                        table
                            .msm_public(&scalars, budget)
                            .expect("fixed-base")
                            .to_affine(),
                        public.to_affine()
                    );
                }
                let timings = compare(
                    msm_reps(n),
                    &mut [
                        ("vendored", &mut vendored),
                        ("public", &mut native_public),
                        ("secret", &mut native_secret),
                        ("fixed", &mut fixed),
                    ],
                );
                let fixed_cell = if table.is_some() {
                    timings[3].cell()
                } else {
                    "-".to_owned()
                };
                println!(
                    "| 2^{log_n} | {threads} | {} | {} | {} | {fixed_cell} | {} | {load:.1} |",
                    timings[0].cell(),
                    timings[1].cell(),
                    timings[2].cell(),
                    speedup(&timings[0], &timings[1]),
                );
            });
        }
    }
}

/// Folds `g` with every challenge in turn through the vendored collapse.
fn vendored_fold(g: &[VAffine], challenges: &[VScalar]) -> VAffine {
    let mut g = g.to_vec();
    let mut len = g.len();
    for u in challenges {
        generator_collapse(&mut g[..len], *u);
        len /= 2;
    }
    g[0]
}

/// Folds `g` with every challenge in turn through the native kernel.
fn native_fold(
    g: &[NativeAffine<Bench>],
    challenges: &[NativeScalar<Bench>],
) -> NativeAffine<Bench> {
    let mut g = g.to_vec();
    let mut len = g.len();
    for u in challenges {
        len = fold_generators_vartime::<<Bench as CurveBridge>::Native>(&mut g[..len], u);
    }
    g[0]
}

#[test]
#[ignore = "release microbenchmark: full IPA generator fold k = 11..=16, 1 and 4 threads"]
fn fold_vendored_vs_native() {
    println!("| k | threads | vendored collapse | native fold | speedup | load1 |");
    println!("|---|---|---|---|---|---|");
    for k in 11..=16_u32 {
        let params = ParamsIpa::<<Bench as CurveBridge>::Native>::new(k).expect("params");
        let native_g = params.g().to_vec();
        let vendored_g = vendored_affines::<Bench>(&native_g);
        let mut rng = ChaCha20Rng::seed_from_u64(u64::from(k));
        let challenges: Vec<VScalar> = (0..k).map(|_| VScalar::random(&mut rng)).collect();
        let native_challenges = native_scalars::<Bench>(&challenges);
        let reps = if k <= 13 { 4 } else { 2 };
        for threads in THREADS {
            pool(threads).install(|| {
                let load = load1();
                assert_eq!(
                    vendored_fold(&vendored_g, &challenges).to_bytes(),
                    native_fold(&native_g, &native_challenges).to_bytes()
                );
                let mut vendored = || {
                    black_box(vendored_fold(&vendored_g, &challenges));
                };
                let mut native = || {
                    black_box(native_fold(&native_g, &native_challenges));
                };
                let timings = compare(
                    reps,
                    &mut [("vendored", &mut vendored), ("native", &mut native)],
                );
                println!(
                    "| {k} | {threads} | {} | {} | {} | {load:.1} |",
                    timings[0].cell(),
                    timings[1].cell(),
                    speedup(&timings[0], &timings[1]),
                );
            });
        }
    }
}

#[test]
#[ignore = "release microbenchmark: FFT, IFFT and coset FFT k = 11..=16, 1 and 4 threads"]
fn fft_vendored_vs_native() {
    println!("| k | threads | transform | vendored | native | speedup | load1 |");
    println!("|---|---|---|---|---|---|---|");
    for k in 11..=16_u32 {
        let n = 1_usize << k;
        let vendored_domain = EvaluationDomain::<VScalar>::new(2, k);
        assert_eq!(
            vendored_domain.extended_k(),
            k,
            "quotient degree 1 keeps the size"
        );
        let native_domain = FftDomain::<NativeScalar<Bench>>::new(k).expect("domain");
        let mut rng = ChaCha20Rng::seed_from_u64(u64::from(k) + 100);
        let values: Vec<VScalar> = (0..n).map(|_| VScalar::random(&mut rng)).collect();
        let native_values = native_scalars::<Bench>(&values);
        let zeta = VScalar::ZETA;
        let native_zeta = iroha_plonk_oracle::convert::native_scalar::<Bench>(&zeta);
        let reps = if k <= 13 { 20 } else { 8 };
        for threads in THREADS {
            pool(threads).install(|| {
                let data = vendored_domain.get_fft_data(n);
                let omega = vendored_domain.get_omega();
                // Forward.
                let load = load1();
                let mut expected = values.clone();
                best_fft(&mut expected, omega, k, data, false);
                let mut native = native_values.clone();
                native_domain.fft(&mut native).expect("length");
                assert_eq!(native, native_scalars::<Bench>(&expected));
                let mut vendored_forward = || {
                    let mut a = values.clone();
                    best_fft(&mut a, omega, k, data, false);
                    black_box(a);
                };
                let mut native_forward = || {
                    let mut a = native_values.clone();
                    native_domain.fft(&mut a).expect("length");
                    black_box(a);
                };
                let timings = compare(
                    reps,
                    &mut [("v", &mut vendored_forward), ("n", &mut native_forward)],
                );
                println!(
                    "| {k} | {threads} | fft | {} | {} | {} | {load:.1} |",
                    timings[0].cell(),
                    timings[1].cell(),
                    speedup(&timings[0], &timings[1]),
                );
                // Inverse.
                let load = load1();
                let mut vendored_inverse = || {
                    black_box(
                        vendored_domain
                            .lagrange_to_coeff(vendored_domain.lagrange_from_vec(values.clone())),
                    );
                };
                let mut native_inverse = || {
                    let mut a = native_values.clone();
                    native_domain.ifft(&mut a).expect("length");
                    black_box(a);
                };
                let timings = compare(
                    reps,
                    &mut [("v", &mut vendored_inverse), ("n", &mut native_inverse)],
                );
                println!(
                    "| {k} | {threads} | ifft | {} | {} | {} | {load:.1} |",
                    timings[0].cell(),
                    timings[1].cell(),
                    speedup(&timings[0], &timings[1]),
                );
                // Coset.
                let load = load1();
                let coefficients = vendored_domain.coeff_from_vec(values.clone());
                let expected = vendored_domain.coeff_to_extended(&coefficients);
                let mut native = native_values.clone();
                native_domain
                    .coset_fft(&mut native, native_zeta)
                    .expect("length");
                assert_eq!(native, native_scalars::<Bench>(&expected));
                let mut vendored_coset = || {
                    black_box(vendored_domain.coeff_to_extended(&coefficients));
                };
                let mut native_coset = || {
                    let mut a = native_values.clone();
                    native_domain
                        .coset_fft(&mut a, native_zeta)
                        .expect("length");
                    black_box(a);
                };
                let timings = compare(
                    reps,
                    &mut [("v", &mut vendored_coset), ("n", &mut native_coset)],
                );
                println!(
                    "| {k} | {threads} | coset_fft | {} | {} | {} | {load:.1} |",
                    timings[0].cell(),
                    timings[1].cell(),
                    speedup(&timings[0], &timings[1]),
                );
            });
        }
    }
}

#[test]
fn harness_helpers_parse_and_time() {
    assert_eq!(parse_cpu_time("  0:00.25\n"), Some(0.25));
    assert_eq!(parse_cpu_time("60:12.58"), Some(3612.58));
    assert_eq!(parse_cpu_time("1:02:03.5"), Some(3723.5));
    assert_eq!(parse_cpu_time("2-00:00:01"), Some(172_801.0));
    assert_eq!(parse_cpu_time("garbage"), None);
    assert_eq!(parse_load1("{ 3.46 13.49 15.69 }\n"), Some(3.46));
    assert_eq!(parse_load1("0.50 0.40 0.30 1/200 999\n"), Some(0.5));
    assert_eq!(parse_load1(""), None);
    let mut timing = Timing::default();
    timing.batch(3, || std::thread::sleep(Duration::from_millis(1)));
    assert_eq!(timing.calls.len(), 3);
    assert!(timing.min_ms() >= 1.0 && timing.median_ms() >= timing.min_ms());
    assert!(timing.cell().contains(" / "));
    let mut counter = 0;
    let mut kernel = || counter += 1;
    let timings = compare(2, &mut [("count", &mut kernel)]);
    assert_eq!(timings[0].calls.len(), 2 * ROUNDS);
    assert_eq!(counter, 2 * ROUNDS);
    assert!(speedup(&timing, &timing).starts_with("x1.00"));
    assert_eq!(msm_reps(1 << 10), 20);
    assert_eq!(msm_reps(1 << 18), 2);
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let points = bases(3, &mut rng);
    assert_ne!(points[0], points[1]);
    let g = vendored_affines::<Bench>(&points[..2]);
    let u = [VScalar::from(3)];
    assert_eq!(
        vendored_fold(&g, &u).to_bytes(),
        native_fold(&points[..2], &native_scalars::<Bench>(&u)).to_bytes()
    );
    assert!(load1().is_nan() || load1() >= 0.0);
}
