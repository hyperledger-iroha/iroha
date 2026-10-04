//! Measurement harness (M12): native `sigma_send` / `sigma_recv` step proofs
//! on Vesta in the KAGEMUSHA step format, at the shape within the 3.5 KB
//! budget and at the smallest `k`.
//!
//! Each case prints one `M12` line: the shape, key generation (parameters
//! excluded), prove wall time and process CPU time at 1 thread (6 runs) and
//! 4 threads (4 runs) after one warm-up proof, verification, the proof
//! length and the 1-minute load average. Process CPU comes from `ps` (10 ms
//! resolution) before and after each proof. Run one case per process under
//! `/usr/bin/time -l` for its peak RSS, in release:
//!
//! ```text
//! cargo test --release -p iroha_kagemusha_proof --test measure -- --ignored \
//!     --nocapture --test-threads=1 <case>
//! ```

mod common;

use std::{process::Command, time::Instant};

use common::{budget_shape, recovery, smallest_shape, vesta_params};
use iroha_kagemusha_proof::{
    KeyOptions, Mutation, PrefixMode, ProofFormat, RelationShape, SigmaParams, SigmaProver,
    SigmaShape, StateLayout, StepRelation, limb_bits_for, sample_witness,
};
use iroha_pasta::{Eq, Fp};

/// Process CPU time of this process in milliseconds (`ps -o time=`).
fn process_cpu_ms() -> f64 {
    let output = Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seconds = 0.0;
    for part in text.trim().split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().expect("ps time field");
    }
    seconds * 1_000.0
}

/// The 1-minute load average.
fn load1() -> String {
    Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .nth(1)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "?".to_owned())
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        f64::midpoint(values[mid - 1], values[mid])
    } else {
        values[mid]
    }
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
    };
    for run in 0..runs {
        let seed = u8::try_from(run + 2).expect("small run index");
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
    let relation = shape.params.relation();
    let load_start = load1();
    let started = Instant::now();
    let params = vesta_params(shape.k);
    let params_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let mut keygen = Vec::new();
    let mut prover = None;
    for threads in [1, 4] {
        // Drop the previous keys first: one proving key at a time.
        drop(prover.take());
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
        prover = Some(keys);
    }
    let prover = prover.expect("keys");
    let inventory = shape.inventory::<Fp>().expect("inventory");
    let descriptor = prover.proving_key().binding().descriptor().clone();
    let bytes = shape
        .proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP)
        .expect("length");
    for (threads, runs, keygen_ms) in [(1, 6, keygen[0]), (4, 4, keygen[1])] {
        let mut s = series(&prover, relation.step, threads, runs);
        let fmt = |values: &[f64]| {
            values
                .iter()
                .map(|value| format!("{value:.1}"))
                .collect::<Vec<_>>()
                .join("/")
        };
        println!(
            "M12 {label} case={} k={} lanes={} limb_bits={} advice_cols={} fixed_cols={} \
             permutations={} cells={} threads={threads} runs={runs} params_ms={params_ms:.0} \
             keygen_ms={keygen_ms:.0} prove_wall_ms=[{}] prove_cpu_ms=[{}] verify_ms=[{}] \
             prove_wall_median_ms={:.1} prove_cpu_median_ms={:.1} verify_median_ms={:.1} \
             proof_bytes={bytes} load1_start={load_start} load1_end={}",
            relation.label(),
            shape.k,
            shape.params.lanes(),
            shape.params.limb_bits(),
            descriptor.num_advice_columns,
            descriptor.num_fixed_columns,
            inventory.permutations(),
            inventory.cells,
            fmt(&s.wall),
            fmt(&s.cpu),
            fmt(&s.verify),
            median(&mut s.wall),
            median(&mut s.cpu),
            median(&mut s.verify),
            load1(),
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

/// Repeats `runs` proofs (or verifications) of the budget-shape send relation
/// on one thread, for a sampling profiler.
fn profile(step: StepRelation, runs: usize, verify_only: bool) {
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

/// One key generation, one proof and one verification on one thread: the
/// process's peak RSS is the footprint of a single step prover.
///
/// The shape is built directly (a deployed prover pins it; the selector is a
/// build-time tool), so no selection dry run inflates the footprint.
fn footprint(step: StepRelation, smallest: bool, options: KeyOptions) {
    let relation = two_level(step);
    let (k, lanes) = if smallest { (10, 2) } else { (11, 1) };
    let params = SigmaParams::new(relation, lanes, limb_bits_for(k)).expect("params");
    let shape = SigmaShape::new(params, k);
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
            relation.label(),
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
fn median_and_cpu_probes() {
    assert!((median(&mut [3.0, 1.0, 2.0]) - 2.0).abs() < f64::EPSILON);
    assert!((median(&mut [4.0, 1.0, 2.0, 3.0]) - 2.5).abs() < f64::EPSILON);
    assert!(process_cpu_ms() >= 0.0);
    assert!(!load1().is_empty());
}
