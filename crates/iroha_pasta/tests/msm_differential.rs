//! Differential tests of the MSM kernels.
//!
//! - sizes 0, 1, 2, 3, 31..=65 and 2^10..=2^16;
//! - scalars 0, 1, -1, the other Pasta modulus minus one, 1..=64-bit values
//!   and full-width values;
//! - repeated, negated and identity bases;
//! - Rayon pools of 1, 2, 4 and 7 threads, which must give identical results;
//! - tight memory budgets (other window sizes) and a nested-pool run that must
//!   not deadlock.
//!
//! References: independent scalar multiplications (`msm_naive`) up to 2^12
//! points, and the lockstep GLV batch multiplication (`batch_mul_vartime`,
//! a separate code path) summed point by point above that.

use ff::Field;
use group::prime::PrimeCurveAffine;
use group::{Curve, Group};
use iroha_pasta::msm::{FixedBaseTable, MemoryBudget, msm_naive, msm_public, msm_secret};
use iroha_pasta::{Ep, Eq, EqAffine, Fp, Fq, PastaCurve, PastaField};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("pool")
}

/// Scalars mixing every special kind.
fn scalars<F: PastaField>(
    n: usize,
    rng: &mut ChaCha20Rng,
    other_modulus_minus_one: [u64; 4],
) -> Vec<F> {
    (0..n)
        .map(|i| match i % 9 {
            0 => F::ZERO,
            1 => F::ONE,
            2 => -F::ONE,
            3 => F::from_raw_reduced(other_modulus_minus_one),
            4 => F::from(rng.next_u64() & 0xFFFF),
            5 => F::from(rng.next_u64()),
            6 => F::from_u128((u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64())),
            7 => -F::from(rng.next_u64()),
            _ => F::random(&mut *rng),
        })
        .collect()
}

/// Bases with repeats, negations and identities mixed in.
fn bases<C: PastaCurve>(n: usize, rng: &mut ChaCha20Rng) -> Vec<C::AffineExt> {
    let step = C::random(&mut *rng);
    let mut cur = C::random(&mut *rng);
    let projective: Vec<C> = (0..n)
        .map(|_| {
            cur += step;
            cur
        })
        .collect();
    let mut out = vec![C::AffineExt::default(); n];
    C::batch_normalize(&projective, &mut out);
    for i in 0..n {
        match i % 13 {
            5 if i > 0 => out[i] = out[i - 1],      // repeated base
            7 if i > 0 => out[i] = -out[i - 1],     // negated base
            11 => out[i] = C::AffineExt::default(), // identity base
            _ => {}
        }
    }
    out
}

fn reference<C: PastaCurve>(s: &[C::ScalarExt], b: &[C::AffineExt]) -> C {
    if s.len() <= 1 << 12 {
        msm_naive::<C>(s, b)
    } else {
        let products = iroha_pasta::curve::batch_mul_vartime::<C>(b, s).expect("equal lengths");
        products
            .par_iter()
            .map(PrimeCurveAffine::to_curve)
            .reduce(C::identity, |a, b| a + b)
    }
}

const P_MINUS_1: [u64; 4] = [
    0x992d_30ed_0000_0000,
    0x2246_98fc_094c_f91b,
    0,
    0x4000_0000_0000_0000,
];
const Q_MINUS_1: [u64; 4] = [
    0x8c46_eb21_0000_0000,
    0x2246_98fc_0994_a8dd,
    0,
    0x4000_0000_0000_0000,
];

fn check_size<C: PastaCurve>(n: usize, seed: u64, other: [u64; 4], pools: &[rayon::ThreadPool]) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let s = scalars::<C::ScalarExt>(n, &mut rng, other);
    let b = bases::<C>(n, &mut rng);
    let expected = reference::<C>(&s, &b);
    for p in pools {
        let public = p
            .install(|| msm_public::<C>(&s, &b, MemoryBudget::DEFAULT))
            .expect("msm");
        assert_eq!(
            public,
            expected,
            "public n = {n} threads = {}",
            p.current_num_threads()
        );
        let secret = p
            .install(|| msm_secret::<C>(&s, &b, MemoryBudget::DEFAULT))
            .expect("msm");
        assert_eq!(
            secret,
            expected,
            "secret n = {n} threads = {}",
            p.current_num_threads()
        );
    }
}

#[test]
fn small_sizes_all_pools() {
    let pools: Vec<_> = [1, 2, 4, 7].into_iter().map(pool).collect();
    for n in [0usize, 1, 2, 3].into_iter().chain(31..=65) {
        check_size::<Eq>(n, n as u64, Q_MINUS_1, &pools);
        check_size::<Ep>(n, 1000 + n as u64, P_MINUS_1, &pools);
    }
}

#[test]
fn power_of_two_sizes_up_to_2_14() {
    let pools: Vec<_> = [1, 2, 4, 7].into_iter().map(pool).collect();
    for k in 10..=14u32 {
        check_size::<Eq>(1 << k, u64::from(k), Q_MINUS_1, &pools);
    }
    for k in [10u32, 12, 14] {
        check_size::<Ep>(1 << k, 100 + u64::from(k), P_MINUS_1, &pools);
    }
}

#[test]
fn power_of_two_sizes_2_15_and_2_16() {
    let pools: Vec<_> = [1, 2, 4, 7].into_iter().map(pool).collect();
    check_size::<Eq>(1 << 15, 15, Q_MINUS_1, &pools);
    check_size::<Eq>(1 << 16, 16, Q_MINUS_1, &pools);
    check_size::<Ep>(1 << 16, 116, P_MINUS_1, &pools[2..3]);
}

#[test]
fn uniform_scalars_and_tight_budgets() {
    let mut rng = ChaCha20Rng::seed_from_u64(77);
    let n = 4096;
    let b = bases::<Eq>(n, &mut rng);
    // Every scalar equal: every point lands in the same bucket of every window.
    for v in [Fp::ONE, -Fp::ONE, Fp::from(3u64), Fp::random(&mut rng)] {
        let s = vec![v; n];
        let expected = reference::<Eq>(&s, &b);
        assert_eq!(
            msm_public::<Eq>(&s, &b, MemoryBudget::DEFAULT).unwrap(),
            expected
        );
        assert_eq!(
            msm_secret::<Eq>(&s, &b, MemoryBudget::DEFAULT).unwrap(),
            expected
        );
    }
    // Every base equal (and its negation): doublings and cancellations inside batches.
    let g = Eq::generator().to_affine();
    let same: Vec<EqAffine> = (0..n).map(|i| if i % 2 == 0 { g } else { -g }).collect();
    let s = scalars::<Fp>(n, &mut rng, Q_MINUS_1);
    assert_eq!(
        msm_public::<Eq>(&s, &same, MemoryBudget::DEFAULT).unwrap(),
        msm_naive::<Eq>(&s, &same)
    );
    // Tight budgets force small windows and low concurrency.
    let s = scalars::<Fp>(n, &mut rng, Q_MINUS_1);
    let expected = msm_naive::<Eq>(&s, &b);
    for budget in [320 << 10, 1 << 20, 4 << 20, 64 << 20] {
        let got = pool(4).install(|| msm_public::<Eq>(&s, &b, MemoryBudget::new(budget)));
        assert_eq!(got.unwrap(), expected, "budget {budget}");
    }
    assert!(msm_public::<Eq>(&s, &b, MemoryBudget::new(64 << 10)).is_err());
    // Short scalars select the window from the actual bit length.
    let short: Vec<Fp> = (0..n).map(|_| Fp::from(rng.next_u64() >> 40)).collect();
    assert_eq!(
        msm_public::<Eq>(&short, &b, MemoryBudget::DEFAULT).unwrap(),
        msm_naive::<Eq>(&short, &b)
    );
}

#[test]
fn fixed_base_tables_match() {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    for k in [6u32, 10, 12] {
        let n = 1usize << k;
        let b = bases::<Ep>(n, &mut rng);
        let s = scalars::<Fq>(n, &mut rng, P_MINUS_1);
        let expected = msm_naive::<Ep>(&s, &b);
        for threads in [1usize, 4, 7] {
            let p = pool(threads);
            let table = p
                .install(|| FixedBaseTable::<Ep>::new(&b, MemoryBudget::DEFAULT))
                .unwrap();
            assert_eq!(
                p.install(|| table.msm_public(&s, MemoryBudget::DEFAULT))
                    .unwrap(),
                expected
            );
            assert_eq!(
                p.install(|| table.msm_secret(&s, MemoryBudget::DEFAULT))
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn nested_pools_do_not_deadlock() {
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    let n = 2048;
    let b = bases::<Eq>(n, &mut rng);
    let sets: Vec<Vec<Fp>> = (0..8)
        .map(|_| scalars::<Fp>(n, &mut rng, Q_MINUS_1))
        .collect();
    let expected: Vec<Eq> = sets.iter().map(|s| msm_naive::<Eq>(s, &b)).collect();
    let outer = pool(2);
    let inner = pool(3);
    // MSMs running inside tasks of an outer pool, some of them installing a
    // second pool from inside an outer task.
    let got: Vec<Eq> = outer.install(|| {
        sets.par_iter()
            .enumerate()
            .map(|(i, s)| {
                if i % 2 == 0 {
                    msm_public::<Eq>(s, &b, MemoryBudget::DEFAULT).unwrap()
                } else {
                    inner.install(|| msm_secret::<Eq>(s, &b, MemoryBudget::DEFAULT).unwrap())
                }
            })
            .collect()
    });
    assert_eq!(got, expected);
    // Single-thread pool calling into a pool of 7 and back.
    let single = pool(1);
    let wide = pool(7);
    let r = single.install(|| {
        wide.install(|| single.install(|| msm_public::<Eq>(&sets[0], &b, MemoryBudget::DEFAULT)))
    });
    assert_eq!(r.unwrap(), expected[0]);
}
