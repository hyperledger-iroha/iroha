//! Measurement harness (M12): native `sigma_send` / `sigma_recv` step proofs
//! on Vesta in the KAGEMUSHA step format, at the shape within the 3.5 KB
//! budget and at the smallest `k`.
//!
//! Each case prints one `M12` line per thread count: the shape, key
//! generation (parameters excluded), and for [`RUNS`] proofs after one
//! warm-up: prove wall time, prove CPU time, verification, each as
//! min/median/p95/max (nearest rank), with the proof length and the
//! 1-minute load average sampled before every proof.
//!
//! - **CPU source.** Process CPU time from
//!   `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)` (nanosecond resolution on
//!   macOS and Linux), read before and after each proof. The line names the
//!   source (`cpu_source=`).
//! - **Load.** `vm.loadavg` (macOS) or `/proc/loadavg` (Linux). A series is
//!   gate-grade only if every sample stayed below [`MAX_GATE_LOAD`]; the
//!   line says so (`gate_grade=`).
//! - **Build.** The harness refuses to run in a debug build.
//! - **Keys.** The keys generated on 1 and on 4 threads must be identical.
//!   The process holds both proving keys while it compares them, so a
//!   measurement process's peak RSS is not a footprint; the
//!   `m12_footprint_*` workloads are.
//!
//! Run one case per process under `/usr/bin/time -l` for its peak RSS, in
//! release:
//!
//! ```text
//! cargo test --release -p iroha_kagemusha_proof --test measure -- --ignored \
//!     --nocapture --test-threads=1 <case>
//! ```

mod common;

use std::time::Instant;

use common::{
    TWO_LEVEL_BUDGET, TWO_LEVEL_SMALLEST, budget_shape, recovery, smallest_shape, vesta_params,
};
use iroha_kagemusha_proof::{
    KeyOptions, Mutation, PrefixMode, ProofFormat, RelationShape, SigmaParams, SigmaProver,
    SigmaShape, StateLayout, StepRelation, limb_bits_for, sample_witness,
};
use iroha_pasta::{Eq, Fp};

/// Timed proofs per series (after one warm-up proof).
const RUNS: usize = 20;
/// The 1-minute load average at or above which a series is not gate-grade.
const MAX_GATE_LOAD: f64 = 4.0;
/// The CPU time source, as printed.
const CPU_SOURCE: &str = "clock_gettime(CLOCK_PROCESS_CPUTIME_ID)";

/// Whether this is an optimized build without debug assertions (the
/// measurements refuse to run otherwise).
fn release_build() -> bool {
    !cfg!(debug_assertions)
}

/// Process CPU time of this process in milliseconds (all threads).
fn process_cpu_ms() -> f64 {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::ProcessCPUTime);
    // Seconds and nanoseconds of a process's CPU time fit an f64 exactly
    // for any realistic run (below 2^53 ns, about 104 days).
    #[allow(clippy::cast_precision_loss, reason = "exact below 2^53 ns")]
    let ms = (time.tv_sec as f64).mul_add(1_000.0, time.tv_nsec as f64 / 1_000_000.0);
    ms
}

/// The 1-minute load average, if the platform reports one.
fn load1() -> Option<f64> {
    if cfg!(target_os = "linux") {
        let text = std::fs::read_to_string("/proc/loadavg").ok()?;
        return text.split_whitespace().next()?.parse().ok();
    }
    let output = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()?;
    // `{ 1.23 4.56 7.89 }`.
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// `min/median/p95/max` (nearest rank) of `values`.
fn summary(values: &[f64]) -> [f64; 4] {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let Some((&min, &max)) = sorted.first().zip(sorted.last()) else {
        return [f64::NAN; 4];
    };
    let rank = |percent: usize| {
        // The smallest value with at least `percent`% of the samples at or
        // below it.
        let index = (percent * sorted.len()).div_ceil(100).max(1) - 1;
        sorted[index.min(sorted.len() - 1)]
    };
    [min, rank(50), rank(95), max]
}

/// `min/median/p95/max` as text.
fn format_summary(values: &[f64]) -> String {
    summary(values)
        .iter()
        .map(|value| format!("{value:.1}"))
        .collect::<Vec<_>>()
        .join("/")
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("pool")
}

/// One timed series: `runs` proofs (after one warm-up) and `runs` verifies.
struct Series {
    wall: Vec<f64>,
    cpu: Vec<f64>,
    verify: Vec<f64>,
    load: Vec<f64>,
}

impl Series {
    /// Whether every load sample stayed below [`MAX_GATE_LOAD`] (and the
    /// platform reported one).
    fn gate_grade(&self) -> bool {
        self.load.len() == self.wall.len() && self.load.iter().all(|load| *load < MAX_GATE_LOAD)
    }
}

fn series(prover: &SigmaProver<Eq>, step: StepRelation, threads: usize, runs: usize) -> Series {
    let pool = pool(threads);
    let witness = sample_witness::<Fp>(7, step, Mutation::None);
    let verifier = prover.verifier();
    let warm = pool
        .install(|| prover.prove(&witness, recovery(1)))
        .expect("warm-up proof");
    verifier
        .verify(&warm.public, &warm.bytes)
        .expect("warm-up verifies");
    let mut out = Series {
        wall: Vec::new(),
        cpu: Vec::new(),
        verify: Vec::new(),
        load: Vec::new(),
    };
    for run in 0..runs {
        let seed = u8::try_from(run % 200 + 2).expect("small run index");
        out.load.extend(load1());
        let cpu = process_cpu_ms();
        let started = Instant::now();
        let proof = pool
            .install(|| prover.prove(&witness, recovery(seed)))
            .expect("proof");
        out.wall.push(started.elapsed().as_secs_f64() * 1_000.0);
        out.cpu.push(process_cpu_ms() - cpu);
        let started = Instant::now();
        pool.install(|| verifier.verify(&proof.public, &proof.bytes))
            .expect("verifies");
        out.verify.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    out
}

fn measure(label: &str, shape: SigmaShape, options: KeyOptions) {
    assert!(release_build(), "M12 measurements need a release build");
    let relation = shape.params.relation();
    let started = Instant::now();
    let params = vesta_params(shape.k);
    let params_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let mut keygen = Vec::new();
    let mut prover: Option<SigmaProver<Eq>> = None;
    for threads in [1, 4] {
        let started = Instant::now();
        let keys = pool(threads)
            .install(|| {
                SigmaProver::keygen_with_options(
                    shape,
                    ProofFormat::KAGEMUSHA_STEP,
                    params.clone(),
                    options,
                )
            })
            .expect("keygen");
        keygen.push(started.elapsed().as_secs_f64() * 1_000.0);
        // The pool size changes no key byte.
        if let Some(previous) = prover.take() {
            assert_eq!(
                previous.verifier().vk_bytes(),
                keys.verifier().vk_bytes(),
                "keys depend on the pool size"
            );
        }
        prover = Some(keys);
    }
    let prover = prover.expect("keys");
    let inventory = shape.inventory::<Fp>().expect("inventory");
    let descriptor = prover.proving_key().binding().descriptor().clone();
    let bytes = shape
        .proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP)
        .expect("length");
    for (threads, keygen_ms) in [(1, keygen[0]), (4, keygen[1])] {
        let s = series(&prover, relation.step, threads, RUNS);
        println!(
            "M12 {label} case={} k={} lanes={} limb_bits={} advice_cols={} fixed_cols={} \
             permutations={} cells={} threads={threads} runs={RUNS} params_ms={params_ms:.0} \
             keygen_ms={keygen_ms:.0} prove_wall_ms={} prove_cpu_ms={} verify_ms={} \
             proof_bytes={bytes} cpu_source={CPU_SOURCE} load1={} gate_grade={}",
            relation.label(),
            shape.k,
            shape.params.lanes(),
            shape.params.limb_bits(),
            descriptor.num_advice_columns,
            descriptor.num_fixed_columns,
            inventory.permutations(),
            inventory.cells,
            format_summary(&s.wall),
            format_summary(&s.cpu),
            format_summary(&s.verify),
            format_summary(&s.load),
            s.gate_grade(),
        );
    }
}

fn two_level(step: StepRelation) -> RelationShape {
    RelationShape::new(step, StateLayout::TwoLevel, PrefixMode::Folded)
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_send_two_level_budget() {
    measure(
        "budget",
        budget_shape(two_level(StepRelation::Send)),
        KeyOptions::default(),
    );
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_recv_two_level_budget() {
    measure(
        "budget",
        budget_shape(two_level(StepRelation::Receive)),
        KeyOptions::default(),
    );
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_send_two_level_smallest_k() {
    measure(
        "smallest_k",
        smallest_shape(two_level(StepRelation::Send)),
        KeyOptions::default(),
    );
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_recv_two_level_smallest_k() {
    measure(
        "smallest_k",
        smallest_shape(two_level(StepRelation::Receive)),
        KeyOptions::default(),
    );
}

/// The two-level shape at `k = 11` with two lanes: over the size budget,
/// measured for comparison.
fn k11_two_lanes(step: StepRelation) -> SigmaShape {
    let params = SigmaParams::new(two_level(step), 2, limb_bits_for(11)).expect("params");
    SigmaShape::new(params, 11)
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_send_two_level_k11_two_lanes() {
    measure(
        "k11_two_lanes",
        k11_two_lanes(StepRelation::Send),
        KeyOptions::default(),
    );
}

/// Repeats `runs` proofs (or verifications) of the budget-shape relation on
/// one thread, for a sampling profiler.
fn profile(step: StepRelation, runs: usize, verify_only: bool) {
    assert!(release_build(), "profile a release build");
    let shape = budget_shape(two_level(step));
    let prover =
        SigmaProver::keygen_with_params(shape, ProofFormat::KAGEMUSHA_STEP, vesta_params(shape.k))
            .expect("keygen");
    let verifier = prover.verifier();
    let witness = sample_witness::<Fp>(7, step, Mutation::None);
    let pool = pool(1);
    let proof = pool
        .install(|| prover.prove(&witness, recovery(1)))
        .expect("proof");
    let started = Instant::now();
    for run in 0..runs {
        if verify_only {
            pool.install(|| verifier.verify(&proof.public, &proof.bytes))
                .expect("verifies");
        } else {
            let seed = u8::try_from(run % 200).expect("seed");
            pool.install(|| prover.prove(&witness, recovery(seed)))
                .expect("proof");
        }
    }
    println!(
        "M12_PROFILE step={step:?} verify_only={verify_only} runs={runs} mean_ms={:.2}",
        started.elapsed().as_secs_f64() * 1_000.0 / f64::from(u32::try_from(runs).expect("runs"))
    );
}

/// The shape a footprint workload pins: the budget or the smallest-`k`
/// shape of the two-level relation.
fn footprint_shape(step: StepRelation, smallest: bool) -> SigmaShape {
    let (k, lanes) = if smallest {
        TWO_LEVEL_SMALLEST
    } else {
        TWO_LEVEL_BUDGET
    };
    let params = SigmaParams::new(two_level(step), lanes, limb_bits_for(k)).expect("params");
    SigmaShape::new(params, k)
}

/// One key generation, one proof and one verification on one thread: the
/// process's peak RSS is the footprint of a single step prover.
///
/// The shape is built directly (a deployed prover pins it; the selector is a
/// build-time tool), so no selection dry run inflates the footprint;
/// `tests/shapes.rs` checks the pinned shapes are the selector's.
fn footprint(step: StepRelation, smallest: bool, options: KeyOptions) {
    assert!(release_build(), "M12 footprints need a release build");
    let shape = footprint_shape(step, smallest);
    pool(1).install(|| {
        let params = vesta_params(shape.k);
        let prover = SigmaProver::<Eq>::keygen_with_options(
            shape,
            ProofFormat::KAGEMUSHA_STEP,
            params,
            options,
        )
        .expect("keys");
        let witness = sample_witness::<Fp>(7, step, Mutation::None);
        let proof = prover.prove(&witness, recovery(1)).expect("proof");
        prover
            .verifier()
            .verify(&proof.public, &proof.bytes)
            .expect("verifies");
        println!(
            "M12_FOOTPRINT case={} k={} lanes={} tables={} proof_bytes={}",
            shape.params.relation().label(),
            shape.k,
            shape.params.lanes(),
            options.commitment_tables.is_some(),
            proof.bytes.len()
        );
    });
}

#[test]
#[ignore = "M12 footprint; run in release under /usr/bin/time -l"]
fn m12_footprint_send_budget() {
    footprint(StepRelation::Send, false, KeyOptions::default());
}

#[test]
#[ignore = "M12 footprint; run in release under /usr/bin/time -l"]
fn m12_footprint_recv_budget() {
    footprint(StepRelation::Receive, false, KeyOptions::default());
}

#[test]
#[ignore = "M12 footprint; run in release under /usr/bin/time -l"]
fn m12_footprint_send_smallest_k() {
    footprint(StepRelation::Send, true, KeyOptions::default());
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_send_two_level_budget_tables() {
    measure(
        "budget_tables",
        budget_shape(two_level(StepRelation::Send)),
        KeyOptions::WITH_TABLES,
    );
}

#[test]
#[ignore = "M12 measurement; run in release, one case per process"]
fn m12_recv_two_level_budget_tables() {
    measure(
        "budget_tables",
        budget_shape(two_level(StepRelation::Receive)),
        KeyOptions::WITH_TABLES,
    );
}

#[test]
#[ignore = "M12 footprint; run in release under /usr/bin/time -l"]
fn m12_footprint_send_budget_tables() {
    footprint(StepRelation::Send, false, KeyOptions::WITH_TABLES);
}

#[test]
#[ignore = "M12 footprint; run in release under /usr/bin/time -l"]
fn m12_footprint_recv_budget_tables() {
    footprint(StepRelation::Receive, false, KeyOptions::WITH_TABLES);
}

#[test]
#[ignore = "profiling workload; run in release under a sampling profiler"]
fn m12_profile_send_prove() {
    profile(StepRelation::Send, 40, false);
}

#[test]
#[ignore = "profiling workload; run in release under a sampling profiler"]
fn m12_profile_send_verify() {
    profile(StepRelation::Send, 400, true);
}

#[test]
fn summaries_and_probes() {
    let bits = |values: [f64; 4]| values.map(f64::to_bits);
    assert_eq!(bits(summary(&[3.0, 1.0, 2.0])), bits([1.0, 2.0, 3.0, 3.0]));
    // Nearest rank of 20 samples: the median is the 10th, p95 the 19th.
    let values: Vec<f64> = (1..=20).map(f64::from).collect();
    assert_eq!(bits(summary(&values)), bits([1.0, 10.0, 19.0, 20.0]));
    assert_eq!(release_build(), !cfg!(debug_assertions));
    assert!(summary(&[]).iter().all(|value| value.is_nan()));
    assert_eq!(format_summary(&[2.0, 1.0]), "1.0/1.0/2.0/2.0");
    // The CPU clock advances with work and has sub-millisecond resolution.
    let start = process_cpu_ms();
    let mut x = 1_u64;
    for i in 0..2_000_000_u64 {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(i);
    }
    assert_ne!(x, 0);
    let spent = process_cpu_ms() - start;
    assert!(spent > 0.0, "{spent}");
    assert!(load1().is_none_or(|load| load >= 0.0));
    let series = Series {
        wall: vec![1.0, 2.0],
        cpu: vec![1.0, 2.0],
        verify: vec![1.0, 1.0],
        load: vec![1.0, MAX_GATE_LOAD],
    };
    assert!(!series.gate_grade());
    assert!(!series.cpu.is_empty() && !series.verify.is_empty());
    // The comparison shape is k = 11 with two lanes.
    let k11 = k11_two_lanes(StepRelation::Send);
    assert_eq!((k11.k, k11.params.lanes()), (11, 2));
    // The footprint shapes are the pinned ones.
    assert_eq!(
        (
            footprint_shape(StepRelation::Send, false).k,
            footprint_shape(StepRelation::Send, false).params.lanes()
        ),
        TWO_LEVEL_BUDGET
    );
}
