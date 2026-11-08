//! Explicit ignored planner sweep: device evidence for the cost model, not a gate.
//!
//! For each workload (points, public bound, workers, secret or public mode),
//! every candidate plan in a bounded neighbourhood of the planner's choice runs
//! through the production kernel, interleaved over rounds. The printed medians
//! calibrate the planner's cost model on a phone. Every candidate's result is
//! checked against the planner's own plan, so a broken candidate cannot pass
//! silently. Synthetic inputs only; no proof or production claim.

use super::*;
use crate::Eq;
use crate::msm::MemoryBudget;
use group::Group;
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};
use std::time::Instant;

struct Workload {
    name: &'static str,
    n: usize,
    bits: usize,
    threads: usize,
    secret: bool,
}

const fn w(name: &'static str, n: usize, bits: usize, threads: usize, secret: bool) -> Workload {
    Workload {
        name,
        n,
        bits,
        threads,
        secret,
    }
}

/// Secret MSMs of the Path 2 prover (IPA rounds, single commitments, narrow
/// classes), plus single-worker rows that isolate the kernel constants from
/// scheduling.
const WORKLOADS: [Workload; 16] = [
    w("full", 4096, 255, 8, true),
    w("full", 8192, 255, 8, true),
    w("full", 16384, 255, 8, true),
    w("full", 32768, 255, 8, true),
    w("full", 65536, 255, 8, true),
    w("bounded", 16384, 1, 8, true),
    w("bounded", 16384, 11, 8, true),
    w("bounded", 16384, 13, 8, true),
    w("bounded", 16384, 16, 8, true),
    w("bounded", 16384, 88, 8, true),
    w("bounded", 16384, 180, 8, true),
    w("bounded", 32768, 13, 8, true),
    w("serial", 16384, 255, 1, true),
    w("serial", 16384, 13, 1, true),
    w("serial", 16384, 16, 1, true),
    w("serial", 16384, 88, 1, true),
];

/// Plans around the planner's choice: window widths `c - 1 ..= c + 3`, a few
/// window groupings and point splits, every one within the default budget.
fn candidates(n: usize, bits: usize, threads: usize, chosen: Plan) -> Vec<Plan> {
    let budget = MemoryBudget::DEFAULT.bytes();
    let mut out = vec![chosen];
    for c in chosen.c.saturating_sub(1).max(MIN_WINDOW)..=(chosen.c + 3).min(MAX_WINDOW) {
        let nw = num_windows(bits, c);
        let nb_per = 1usize << (c - 1);
        let max_per_task = ((2usize << 20) / (nb_per * BUCKET_BYTES)).max(1);
        let mut groupings = vec![1, 2, 4, nw.div_ceil(threads.min(nw)), nw];
        groupings.retain(|&g| g <= nw.min(max_per_task));
        groupings.sort_unstable();
        groupings.dedup();
        for per_group in groupings {
            let groups = nw.div_ceil(per_group);
            let mut splits = vec![1, 2, 4, threads.div_ceil(groups)];
            splits.retain(|&k| k == 1 || (n >= 8192 && groups * k <= 4 * threads));
            splits.sort_unstable();
            splits.dedup();
            for chunks in splits {
                let concurrency = (groups * chunks).min(threads).max(1);
                let bytes = fixed_bytes(n, nw).unwrap()
                    + concurrency * loop_bytes(nw, per_group, nb_per).unwrap();
                let candidate = Plan {
                    c,
                    nw,
                    groups,
                    per_group,
                    chunks,
                    concurrency,
                    bytes,
                };
                if bytes <= budget && !out.contains(&candidate) {
                    out.push(candidate);
                }
            }
        }
    }
    out
}

/// A canonical integer below `2^bits`, built limb by limb from the top.
fn below<F: crate::field::PastaField>(rng: &mut ChaCha20Rng, bits: usize) -> F {
    if bits >= 255 {
        return F::random(&mut *rng);
    }
    let shift = F::from(u64::MAX) + F::ONE;
    let mut value = F::ZERO;
    for limb in (0..bits.div_ceil(64)).rev() {
        let kept = (bits - 64 * limb).min(64);
        let mask = if kept == 64 {
            u64::MAX
        } else {
            (1_u64 << kept) - 1
        };
        value = value * shift + F::from(rng.next_u64() & mask);
    }
    value
}

fn run(
    plan: Plan,
    scalars: &[<Eq as PastaCurve>::ScalarExt],
    bases: &[<Eq as PastaCurve>::AffineExt],
    secret: bool,
) -> Eq {
    if secret {
        super::super::run_plan::<Eq, true>(scalars, bases, plan, None).unwrap()
    } else {
        super::super::run_plan::<Eq, false>(scalars, bases, plan, None).unwrap()
    }
}

#[test]
#[ignore = "explicit device planner sweep; synthetic MSM component only"]
fn planner_sweep() {
    let rounds: usize = std::env::var("PLAN_SWEEP_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let mut rng = ChaCha20Rng::seed_from_u64(0x706c_616e_7377_6565);
    let largest = WORKLOADS.iter().map(|work| work.n).max().unwrap();
    // Distinct bases by repeated addition of a random step (cheap on a phone);
    // the kernel's cost does not depend on their discrete logarithms.
    let step = Eq::random(&mut rng);
    let mut point = Eq::random(&mut rng);
    let projective: Vec<Eq> = (0..largest)
        .map(|_| {
            point += step;
            point
        })
        .collect();
    let bases = crate::curve::batch_normalize(&projective);
    for work in &WORKLOADS {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(work.threads)
            .build()
            .unwrap();
        let scalars: Vec<<Eq as PastaCurve>::ScalarExt> =
            (0..work.n).map(|_| below(&mut rng, work.bits)).collect();
        let bases = &bases[..work.n];
        let chosen = plan(
            work.n,
            work.bits,
            work.threads,
            MemoryBudget::DEFAULT.bytes(),
        )
        .unwrap();
        let plans = candidates(work.n, work.bits, work.threads, chosen);
        let expected = pool.install(|| run(chosen, &scalars, bases, work.secret));
        let mut times = vec![Vec::with_capacity(rounds); plans.len()];
        for round in 0..rounds {
            // Alternate the order so drift does not favour early candidates.
            let order: Vec<usize> = if round % 2 == 0 {
                (0..plans.len()).collect()
            } else {
                (0..plans.len()).rev().collect()
            };
            for index in order {
                let started = Instant::now();
                let sum = pool.install(|| run(plans[index], &scalars, bases, work.secret));
                times[index].push(started.elapsed().as_nanos());
                assert_eq!(sum, expected, "candidate {:?}", plans[index]);
            }
        }
        let mut best = (u128::MAX, 0);
        for (index, (plan, samples)) in plans.iter().zip(&mut times).enumerate() {
            samples.sort_unstable();
            let median = samples[samples.len() / 2];
            if median < best.0 {
                best = (median, index);
            }
            println!(
                "PLAN_SWEEP workload={} n={} bits={} threads={} secret={} c={} nw={} per_group={} groups={} chunks={} tasks={} concurrency={} bytes={} chosen={} median_ns={median} min_ns={}",
                work.name,
                work.n,
                work.bits,
                work.threads,
                work.secret,
                plan.c,
                plan.nw,
                plan.per_group,
                plan.groups,
                plan.chunks,
                plan.groups * plan.chunks,
                plan.concurrency,
                plan.bytes,
                u8::from(index == 0),
                samples[0],
            );
        }
        let chosen_median = times[0][times[0].len() / 2];
        let fastest = plans[best.1];
        println!(
            "PLAN_SWEEP_BEST workload={} n={} bits={} threads={} secret={} chosen_ns={chosen_median} best_ns={} chosen_over_best_permille={} best_c={} best_per_group={} best_chunks={} candidates={}",
            work.name,
            work.n,
            work.bits,
            work.threads,
            work.secret,
            best.0,
            chosen_median * 1000 / best.0.max(1),
            fastest.c,
            fastest.per_group,
            fastest.chunks,
            plans.len(),
        );
    }
}
