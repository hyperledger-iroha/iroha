//! Differential tests of the generator fold, the field FFT and the group
//! IFFT against naive references.
//!
//! - Fold: the lockstep GLV kernel against the vendored-style collapse
//!   `G[i] + u * G[i + h]` with constant-time multiplication, for single
//!   rounds, a full IPA collapse, exceptional inputs and Rayon pools of 1, 2,
//!   4 and 7 threads.
//! - FFT: forward, inverse and coset transforms against an `O(n^2)` DFT for
//!   `k <= 10`, and against point evaluations by Horner's rule at sampled
//!   indices up to `k = 16`, at several pool sizes.
//! - Group IFFT: `sum_j a_j * g_lagrange[j] = sum_i (IFFT a)_i * g[i]`.
#![allow(clippy::many_single_char_names)]

use ff::{Field, WithSmallOrderMulGroup};
use group::prime::PrimeCurveAffine;
use iroha_pasta::fft::{FftDomain, naive_coset_dft};
use iroha_pasta::fold::{FoldChallenge, fold_generators_vartime, fold_generators_with};
use iroha_pasta::msm::{MemoryBudget, msm_public};
use iroha_pasta::params::ParamsIpa;
use iroha_pasta::{Ep, Eq, EqAffine, Fp, Fq, PastaCurve, PastaField};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("pool")
}

fn random_points<C: PastaCurve>(n: usize, rng: &mut ChaCha20Rng) -> Vec<C::AffineExt> {
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
    out
}

/// The vendored collapse: constant-time `lo + u * hi`, batch normalised.
fn collapse_reference<C: PastaCurve>(g: &[C::AffineExt], u: &C::ScalarExt) -> Vec<C::AffineExt> {
    let h = g.len() / 2;
    let tmp: Vec<C> = (0..h).map(|i| g[i].to_curve() + g[i + h] * *u).collect();
    let mut out = vec![C::AffineExt::default(); h];
    C::batch_normalize(&tmp, &mut out);
    out
}

#[test]
fn fold_rounds_match_reference_on_all_pools() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let pools: Vec<_> = [1usize, 2, 4, 7].into_iter().map(pool).collect();
    for k in 1..=12u32 {
        let g = random_points::<Eq>(1 << k, &mut rng);
        let u = Fp::random(&mut rng);
        let expected = collapse_reference::<Eq>(&g, &u);
        for p in &pools {
            let mut ours = g.clone();
            let h = p.install(|| fold_generators_vartime::<Eq>(&mut ours, &u));
            ours.truncate(h);
            assert_eq!(
                ours,
                expected,
                "k = {k} threads = {}",
                p.current_num_threads()
            );
        }
    }
}

#[test]
fn full_ipa_collapse_matches_reference() {
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    for (k, threads) in [(10u32, 1usize), (11, 4), (12, 7)] {
        let mut ours = random_points::<Ep>(1 << k, &mut rng);
        let mut theirs = ours.clone();
        let p = pool(threads);
        for _ in 0..k {
            let u = Fq::random(&mut rng);
            let ch = FoldChallenge::new::<Ep>(&u).expect("GLV split");
            let h = p.install(|| fold_generators_with::<Ep>(&mut ours, &ch));
            ours.truncate(h);
            theirs = collapse_reference::<Ep>(&theirs, &u);
        }
        assert_eq!(ours.len(), 1);
        assert_eq!(ours, theirs, "k = {k}");
    }
}

#[test]
fn fold_exceptional_inputs() {
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let mut g = random_points::<Eq>(256, &mut rng);
    // lo = hi with u = 1 needs a doubling; lo = -hi with u = 1 cancels;
    // identities on either side; u = 0, 1, -1 and small values.
    g[3] = g[128 + 3];
    g[4] = -g[128 + 4];
    g[5] = EqAffine::default();
    g[128 + 6] = EqAffine::default();
    for u in [
        Fp::ONE,
        -Fp::ONE,
        Fp::ZERO,
        Fp::from(2u64),
        Fp::ZETA,
        Fp::random(&mut rng),
    ] {
        let expected = collapse_reference::<Eq>(&g, &u);
        let mut ours = g.clone();
        let h = fold_generators_vartime::<Eq>(&mut ours, &u);
        ours.truncate(h);
        assert_eq!(ours, expected, "u = {u:?}");
    }
    // Odd lengths ignore the last element, as the vendored collapse does.
    let mut odd = random_points::<Ep>(7, &mut rng);
    let expected = collapse_reference::<Ep>(&odd, &Fq::from(9u64));
    let h = fold_generators_vartime::<Ep>(&mut odd, &Fq::from(9u64));
    assert_eq!(&odd[..h], &expected[..]);
}

fn random_vec<F: PastaField>(n: usize, rng: &mut ChaCha20Rng) -> Vec<F> {
    (0..n).map(|_| F::random(&mut *rng)).collect()
}

fn horner<F: PastaField>(a: &[F], x: F) -> F {
    a.iter().rev().fold(F::ZERO, |acc, c| acc * x + c)
}

fn check_fft<F: PastaField>(k: u32, rng: &mut ChaCha20Rng, pools: &[rayon::ThreadPool]) {
    let d = FftDomain::<F>::new(k).unwrap();
    let n = d.n();
    let a = random_vec::<F>(n, rng);
    let shift = F::ZETA;
    let mut reference: Option<(Vec<F>, Vec<F>)> = None;
    for p in pools {
        let mut evals = a.clone();
        p.install(|| d.fft(&mut evals)).unwrap();
        let mut coset = a.clone();
        p.install(|| d.coset_fft(&mut coset, shift)).unwrap();
        match &reference {
            None => reference = Some((evals.clone(), coset.clone())),
            Some((e, c)) => {
                assert_eq!(&evals, e, "k = {k}: pool-dependent output");
                assert_eq!(&coset, c, "k = {k}: pool-dependent coset output");
            }
        }
        if k <= 10 {
            assert_eq!(evals, naive_coset_dft(&a, d.omega(), F::ONE), "k = {k}");
            assert_eq!(
                coset,
                naive_coset_dft(&a, d.omega(), shift),
                "coset k = {k}"
            );
        } else {
            for _ in 0..8 {
                let i = usize::try_from(rng.next_u64() % (n as u64)).expect("index below n");
                let x = d.omega().pow_vartime([i as u64]);
                assert_eq!(evals[i], horner(&a, x), "k = {k} i = {i}");
                assert_eq!(coset[i], horner(&a, shift * x), "coset k = {k} i = {i}");
            }
        }
        p.install(|| d.ifft(&mut evals)).unwrap();
        assert_eq!(evals, a, "ifft k = {k}");
        p.install(|| d.coset_ifft(&mut coset, shift)).unwrap();
        assert_eq!(coset, a, "coset ifft k = {k}");
    }
}

#[test]
fn fft_matches_naive_and_horner() {
    let mut rng = ChaCha20Rng::seed_from_u64(4);
    let pools: Vec<_> = [1usize, 4, 7].into_iter().map(pool).collect();
    for k in 0..=16u32 {
        check_fft::<Fp>(k, &mut rng, &pools);
    }
    for k in [1u32, 5, 12, 15] {
        check_fft::<Fq>(k, &mut rng, &pools);
    }
}

#[test]
fn group_ifft_satisfies_commitment_identity() {
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    for k in [1u32, 4, 8, 10] {
        let params = ParamsIpa::<Eq>::new(k).unwrap();
        let d = FftDomain::<Fp>::new(k).unwrap();
        // Evaluations a over the domain; coefficients c = IFFT(a).
        let a = random_vec::<Fp>(1 << k, &mut rng);
        let mut c = a.clone();
        d.ifft(&mut c).unwrap();
        let lhs = msm_public::<Eq>(&a, params.g_lagrange(), MemoryBudget::DEFAULT).unwrap();
        let rhs = msm_public::<Eq>(&c, params.g(), MemoryBudget::DEFAULT).unwrap();
        assert_eq!(lhs, rhs, "k = {k}");
        assert!(!bool::from(params.w().is_identity()));
    }
}
