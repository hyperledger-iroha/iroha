//! Zero-knowledge inner-product argument for `<x, b> = kappa` against a
//! hiding commitment `P = <x, G> + blind H`, with an arbitrary public weight
//! vector `b`.
//!
//! Bulletproofs-style folding `x' = u x_lo + x_hi`, `b' = b_lo + u b_hi`,
//! `G' = G_lo + u G_hi`, `P' = u P + u^2 L + R`. Every `L, R` carries a fresh
//! blind; the final opening `(a, f)` of `P_final = a (G_f + b_f U) + f H` is
//! replaced by a Schnorr proof `(A, z1, z2)`, so no full-length masking
//! vector is committed. The folded generator `G_f` is sent: the recursive
//! design accumulates it as the proof's own generator claim; this spike
//! decides it natively with one public MSM.

use ff::Field;
use iroha_pasta::PastaCurve;
use iroha_pasta::fold::{fold_generators_vartime, folded_generators_vartime};
use iroha_pasta::msm::{MemoryBudget, SharedMemoryBudget, msm_public, msm_secret_cancellable};
use rayon::prelude::*;
use zeroize::Zeroizing;

use crate::fs::{SpikeRng, Transcript};

pub type F<C> = <C as PastaCurve>::ScalarExt;
pub type Aff<C> = <C as PastaCurve>::AffineExt;

/// Domain of the experiment's public commitment bases. Changing it changes
/// every commitment and the relation; it is not a production parameter ID.
pub const GENERATOR_DOMAIN: &str = "kagemusha-zks-spike/gens";

/// The public commitment bases. Private commitments sum them through the
/// secret Pippenger MSM ([`secret_msm`], `spartan::column_sums`), which keeps
/// no per-shape table, so a generator set holds the bases alone.
pub struct Gens<C: PastaCurve> {
    pub g: Vec<Aff<C>>,
    pub h: Aff<C>,
    pub u: Aff<C>,
}

/// The backend has one pinned prover implementation. Benchmark environment
/// variables cannot change a native wallet's cryptographic execution path.
pub fn old_kernels() -> bool {
    false
}

/// `sum_i scalars[i] * bases[i]` for private scalars over public bases,
/// through iroha_pasta's secret Pippenger kernel (`msm_secret_cancellable`).
///
/// The kernel is variable time in the scalars: buckets are chosen by digit
/// and zero digits are skipped, so timing and memory accesses follow the
/// private values. The owner accepted that exposure for witness rows, round
/// values, the opening's L/R vectors and the generator-claim folds (TODO.md
/// G8, 2026-11-24). Its plan assumes full-width scalars, its bucket inversion
/// is constant time and its digit and bucket scratch is wiped. Scratch stays
/// within the default per-kernel budget and the process MSM ceiling. The group
/// element is the one every other MSM gives, so proofs are unchanged.
///
/// # Panics
/// Only on a caller error the kernel reports instead of a sum: unequal
/// lengths, more than `u32::MAX` bases, or one base exceeding the 256 MiB
/// default budget. No identity or partial sum is ever substituted.
pub(crate) fn secret_msm<C: PastaCurve>(scalars: &[F<C>], bases: &[Aff<C>]) -> C {
    msm_secret_cancellable::<C>(
        scalars,
        bases,
        MemoryBudget::DEFAULT,
        &SharedMemoryBudget::process_default(),
        None,
    )
    .expect("secret MSM")
}

impl<C: PastaCurve> Gens<C> {
    /// Derive `g[i] = H_curve(GENERATOR_DOMAIN, LE64(i))`, with `h` at `n`
    /// and `u` at `n + 1`. The full capacity therefore identifies the
    /// blinding bases, even when only a prefix of `g` is used.
    pub fn new(n: usize) -> Self {
        // Normalize each public hash-to-curve batch directly into its final slots.
        const MAX_BATCH: usize = 2048;
        let mut affine = vec![C::AffineExt::default(); n + 2];
        let batch_size = affine
            .len()
            .div_ceil(rayon::current_num_threads())
            .min(MAX_BATCH);
        affine
            .par_chunks_mut(batch_size)
            .enumerate()
            .for_each(|(batch, out)| {
                let first = batch * batch_size;
                let points: Vec<C> = (first..first + out.len())
                    .map(|i| {
                        C::hash_to_curve(GENERATOR_DOMAIN, &(i as u64).to_le_bytes())
                            .expect("hash to curve")
                    })
                    .collect();
                C::batch_normalize(&points, out);
            });
        let u = affine.pop().expect("U generator");
        let h = affine.pop().expect("H generator");
        Self { g: affine, h, u }
    }

    /// The first `n` generators. Preserve the parent set's `h` and `u`:
    /// `Gens::new(n)` would select different blinding bases.
    pub fn prefix(&self, n: usize) -> Self {
        Self {
            g: self.g[..n].to_vec(),
            h: self.h,
            u: self.u,
        }
    }
}

/// An inherited generator claim `G_old = <s(u_old), G>`, folded into this
/// opening in clear (owner decision, 2026-11-08): the proof opens
/// `P + sum_j xi^(j+1) G_old_j` against `kappa + sum_j xi^(j+1) <s(u_old_j), b>`.
#[derive(Clone)]
pub struct Claim<C: PastaCurve> {
    pub us: Vec<F<C>>,
    pub g: Aff<C>,
}

#[derive(Clone)]
pub struct IpaProof<C: PastaCurve> {
    pub lr: Vec<(Aff<C>, Aff<C>)>,
    pub g_final: Aff<C>,
    pub a: Aff<C>,
    pub z1: F<C>,
    pub z2: F<C>,
}

/// Timer names of the L/R work by log2 of the half length.
const ROUND_NAMES: [&str; 19] = [
    "lr.h00001",
    "lr.h00002",
    "lr.h00004",
    "lr.h00008",
    "lr.h00016",
    "lr.h00032",
    "lr.h00064",
    "lr.h00128",
    "lr.h00256",
    "lr.h00512",
    "lr.h01024",
    "lr.h02048",
    "lr.h04096",
    "lr.h08192",
    "lr.h16384",
    "lr.h32768",
    "lr.h65536",
    "lr.h131072",
    "lr.h262144",
];

fn round_name(half: usize) -> &'static str {
    ROUND_NAMES
        .get(half.trailing_zeros() as usize)
        .copied()
        .unwrap_or("lr.large")
}

fn inner<S: Field>(a: &[S], b: &[S]) -> S {
    a.par_iter()
        .zip(b.par_iter())
        .with_min_len(64)
        .with_max_len(1024)
        .map(|(x, y)| *x * y)
        .sum()
}

fn begin<C: PastaCurve>(gens: &Gens<C>, t: &mut Transcript<C>, p: &C, kappa: &F<C>) -> C {
    t.point(b"ipa/commitment", &p.to_affine());
    t.scalar(b"ipa/value", kappa);
    let zeta: F<C> = t.endo_challenge(b"ipa/zeta");
    C::from(gens.u) * zeta
}

/// Bind the inherited claims in order. The prover opens their full vectors;
/// the verifier evaluates the same scalar inner products in factored form.
fn fold_claims<C: PastaCurve>(
    t: &mut Transcript<C>,
    p: &C,
    kappa: F<C>,
    claims: &[Claim<C>],
    mut value: impl FnMut(&[F<C>], F<C>) -> F<C>,
) -> (C, F<C>) {
    if claims.is_empty() {
        return (*p, kappa);
    }
    for claim in claims {
        t.point(b"ipa/claim", &claim.g);
    }
    let xi: F<C> = t.endo_challenge(b"ipa/xi");
    let (mut p2, mut k2, mut power) = (*p, kappa, F::<C>::ONE);
    for claim in claims {
        power *= xi;
        k2 += power * value(&claim.us, power);
        p2 += C::from(claim.g) * power;
    }
    (p2, k2)
}

#[allow(clippy::too_many_arguments)]
pub fn prove<C: PastaCurve>(
    gens: &Gens<C>,
    t: &mut Transcript<C>,
    p: &C,
    x: Vec<F<C>>,
    mut b: Vec<F<C>>,
    kappa: F<C>,
    mut blind: F<C>,
    rng: &mut SpikeRng,
    claims: &[Claim<C>],
) -> IpaProof<C> {
    // The opening vector is private: wiped on every exit, also when unwinding.
    let mut x = Zeroizing::new(x);
    let n = x.len();
    assert!(n.is_power_of_two() && b.len() == n && gens.g.len() == n);
    let (p, kappa) = fold_claims(t, p, kappa, claims, |us, power| {
        let s = s_vector(us);
        assert_eq!(s.len(), b.len(), "claim over the same generators");
        let value = inner(&s, &b);
        x.iter_mut().zip(s).for_each(|(v, w)| *v += power * w);
        value
    });
    let u_point = begin(gens, t, &p, &kappa);
    let h = C::from(gens.h);
    // U as a base of the L/R sums: the value and blind terms are folded into
    // each sum instead of separate scalar multiplications.
    let u_affine = group::Curve::to_affine(&u_point);
    // The first round reads the installed generators. Its fold allocates
    // only the reduced vector, which later rounds update in place.
    let mut g = std::borrow::Cow::Borrowed(gens.g.as_slice());
    let mut lr = Vec::with_capacity(n.trailing_zeros() as usize);
    let mut len = n;
    while len > 1 {
        let half = len / 2;
        let (lam, mu): (F<C>, F<C>) = (rng.scalar(), rng.scalar());
        let lr_clock = std::time::Instant::now();
        let (l, r) = {
            // Each side adds the same two private terms to its vector
            // commitment. The secret kernel accepts both slices directly, so
            // neither secret scalars nor bases are copied.
            let (x_lo, x_hi) = x.split_at(half);
            let (b_lo, b_hi) = b.split_at(half);
            let (g_lo, g_hi) = g[..len].split_at(half);
            let side = |xs: &[F<C>], bs: &[F<C>], gs: &[Aff<C>], blind: F<C>| -> Aff<C> {
                let tail = Zeroizing::new([inner(xs, bs), blind]);
                let vector = secret_msm::<C>(xs, gs);
                let masks = secret_msm::<C>(&tail[..], &[u_affine, gens.h]);
                group::Curve::to_affine(&(vector + masks))
            };
            rayon::join(
                || side(x_lo, b_hi, g_hi, lam),
                || side(x_hi, b_lo, g_lo, mu),
            )
        };
        crate::prof::add(
            if len == n {
                "ipa.lr1"
            } else if len * 2 == n {
                "ipa.lr2"
            } else if len * 4 == n {
                "ipa.lr3"
            } else {
                "ipa.lr4+"
            },
            lr_clock.elapsed().as_secs_f64(),
        );
        crate::prof::add(round_name(half), lr_clock.elapsed().as_secs_f64());
        t.point(b"ipa/L", &l);
        t.point(b"ipa/R", &r);
        let u: F<C> = t.endo_challenge(b"ipa/u");
        let xb_clock = std::time::Instant::now();
        {
            let (x_lo, x_hi) = x.split_at_mut(half);
            x_lo.par_iter_mut()
                .zip(x_hi.par_iter())
                .with_min_len(64)
                .with_max_len(1024)
                .for_each(|(lo, hi)| *lo = *lo * u + hi);
            let (b_lo, b_hi) = b.split_at_mut(half);
            b_lo.par_iter_mut()
                .zip(b_hi.par_iter())
                .with_min_len(64)
                .with_max_len(1024)
                .for_each(|(lo, hi)| *lo += u * hi);
        }
        crate::prof::add("ipa.xb", xb_clock.elapsed().as_secs_f64());
        let fold_clock = std::time::Instant::now();
        match &mut g {
            std::borrow::Cow::Borrowed(initial) => {
                g = std::borrow::Cow::Owned(folded_generators_vartime::<C>(initial, &u));
            }
            std::borrow::Cow::Owned(folded) => {
                fold_generators_vartime::<C>(&mut folded[..len], &u);
            }
        }
        crate::prof::add(
            if len == n {
                "ipa.fold1"
            } else if len * 2 == n {
                "ipa.fold2"
            } else if len * 4 == n {
                "ipa.fold3"
            } else {
                "ipa.fold4+"
            },
            fold_clock.elapsed().as_secs_f64(),
        );
        x.truncate(half);
        b.truncate(half);
        blind = blind * u + lam * u.square() + mu;
        lr.push((l, r));
        len = half;
    }
    let (a, g_final, b_final) = (x[0], g[0], b[0]);
    let base = C::from(g_final) + u_point * b_final;
    let (d, delta): (F<C>, F<C>) = (rng.scalar(), rng.scalar());
    let a_commit = (base * d + h * delta).to_affine();
    t.point(b"ipa/G", &g_final);
    t.point(b"ipa/A", &a_commit);
    let c: F<C> = t.endo_challenge(b"ipa/c");
    IpaProof {
        lr,
        g_final,
        a: a_commit,
        z1: d + c * a,
        z2: delta + c * blind,
    }
}

/// `s_i = prod_j u_j^{bit_j(i)}`, round 1 the most significant bit.
pub fn s_vector<S: Field>(us: &[S]) -> Vec<S> {
    let mut s = Vec::with_capacity(1 << us.len());
    s.push(S::ONE);
    // Add one more significant index bit at a time. The old prefix is the
    // low half and remains in place; only the new high half is written.
    for &u in us.iter().rev() {
        for i in 0..s.len() {
            s.push(s[i] * u);
        }
    }
    s
}

/// Exact generator decision for verification that does not need the individual
/// block sums. The generators and all transcript challenges are public.
pub(crate) fn decide_generator<C: PastaCurve>(
    gens: &Gens<C>,
    us: &[F<C>],
    point: Aff<C>,
) -> Result<(), String> {
    let result = msm_public::<C>(&s_vector(us), &gens.g, MemoryBudget::DEFAULT)
        .map_err(|e| format!("ipa: msm {e:?}"))?;
    if result.to_affine() != point {
        return Err("ipa: folded generator".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn verify<C: PastaCurve, D: crate::generator_claim::GeneratorDecision<C>>(
    gens: &Gens<C>,
    t: &mut Transcript<C>,
    p: &C,
    n: usize,
    kappa: F<C>,
    proof: &IpaProof<C>,
    claims: &[Claim<C>],
    folded_weight: impl Fn(&[F<C>]) -> F<C>,
) -> Result<D, String> {
    if !n.is_power_of_two() || proof.lr.len() != n.trailing_zeros() as usize || gens.g.len() != n {
        return Err("ipa: round count".into());
    }
    if claims.iter().any(|claim| claim.us.len() != proof.lr.len()) {
        return Err("ipa: inherited claim dimension".into());
    }
    let (p, kappa) = fold_claims(t, p, kappa, claims, |us, _| folded_weight(us));
    let u_point = begin(gens, t, &p, &kappa);
    let mut folded = p + u_point * kappa;
    let mut us = Vec::with_capacity(proof.lr.len());
    for (l, r) in &proof.lr {
        t.point(b"ipa/L", l);
        t.point(b"ipa/R", r);
        let u: F<C> = t.endo_challenge(b"ipa/u");
        folded = folded * u + C::from(*l) * u.square() + C::from(*r);
        us.push(u);
    }
    let b_final = folded_weight(&us);
    let decision = D::decide(gens, &us, proof.g_final)?;
    t.point(b"ipa/G", &proof.g_final);
    t.point(b"ipa/A", &proof.a);
    let c: F<C> = t.endo_challenge(b"ipa/c");
    let lhs = (C::from(proof.g_final) + u_point * b_final) * proof.z1 + C::from(gens.h) * proof.z2;
    let rhs = C::from(proof.a) + folded * c;
    if lhs != rhs {
        return Err("ipa: final opening".into());
    }
    if bool::from(C::from(proof.a).is_identity()) {
        return Err("ipa: identity commitment".into());
    }
    Ok(decision)
}

#[cfg(test)]
mod generator_tests {
    use super::*;
    use group::{Curve, GroupEncoding};
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

    #[test]
    fn generator_weights_follow_the_transcript_bit_order() {
        fn check<S: Field>(two: S) {
            for u in [&[two, S::ZERO, two][..], &[]] {
                let buffer = s_vector(u);
                for (index, &value) in buffer.iter().enumerate() {
                    let expected = u.iter().rev().enumerate().fold(S::ONE, |v, (bit, x)| {
                        if index & (1 << bit) == 0 { v } else { v * x }
                    });
                    assert_eq!(value, expected);
                }
                assert_eq!(buffer.len(), 1 << u.len());
            }
        }
        check(iroha_pasta::Fp::from(2));
        check(iroha_pasta::Fq::from(2));
    }

    #[test]
    fn round_profile_labels_cover_every_pointer_width() {
        assert_eq!(round_name(1 << 17), "lr.h131072");
        assert_eq!(round_name(1 << 18), "lr.h262144");
        for bits in 0..usize::BITS {
            assert!(!round_name(1usize << bits).is_empty());
        }
    }

    fn encoded<C: PastaCurve>(point: Aff<C>) -> String {
        point
            .to_bytes()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn generator_vectors() {
        let vectors = [
            (
                0_u64,
                "92d4370a0545d7e6af93c9172cff147530b173ce0be081858559d4f2d967d61a",
                "3ae1888e2e0bec26037e0a130a4471c685c00c553636aba8746723d2bfd63cb4",
            ),
            (
                1,
                "94d34b5f192429be9a6aeccf60e185818e22f9a860ce45faf5878f9e07e4a43b",
                "00d65619dbe240ab5601d6b97ac12d572095b95e0cfbb1de36f046cb5cdf1a9f",
            ),
            (
                3583,
                "1d4173991307201e60fa5a4807b81a25edf5090d9d802f1be17efb290c64828a",
                "33887a6805c24219d6d797cb37866261003598b987b96b18e35fdefa9cae53a3",
            ),
            (
                65536,
                "f4beed75aa29b2699a1c8af3dfd45e3555a18f2987a95868cb59e519f266200b",
                "662b108da0f453bc9f75e4407b5e54c5b3b087e5dbb78e5d0bc142afbd62be06",
            ),
            (
                65537,
                "afa3051ff0191066f7ffa561736759908bd5ed34b352236fff9152b14258bbb6",
                "267c6fe8131a8571bc649e5ad693fb7e0406bfdd34342001e157c69983020187",
            ),
        ];
        for (index, expected_p, expected_v) in vectors {
            let p = Pallas::hash_to_curve(GENERATOR_DOMAIN, &index.to_le_bytes()).unwrap();
            let v = Vesta::hash_to_curve(GENERATOR_DOMAIN, &index.to_le_bytes()).unwrap();
            assert_eq!(
                encoded::<Pallas>(p.to_affine()),
                expected_p,
                "Pallas {index}"
            );
            assert_eq!(encoded::<Vesta>(v.to_affine()), expected_v, "Vesta {index}");
        }
    }

    #[test]
    fn batches_keep_the_public_indices_and_both_final_bases() {
        fn check<C: PastaCurve>(n: usize) {
            let gens = Gens::<C>::new(n);
            assert_eq!(gens.g.len(), n);
            for (i, point) in gens.g.iter().chain([&gens.h, &gens.u]).enumerate() {
                let expected =
                    C::hash_to_curve(GENERATOR_DOMAIN, &(i as u64).to_le_bytes()).unwrap();
                assert_eq!(*point, expected.to_affine());
            }
        }
        for n in [0, 2051] {
            check::<Pallas>(n);
            check::<Vesta>(n);
        }
    }

    #[test]
    fn prefix_keeps_the_parent_blinding_bases() {
        let parent = Gens::<Vesta>::new(8);
        let prefix = parent.prefix(4);
        let independent = Gens::<Vesta>::new(4);
        assert_eq!(prefix.g, independent.g);
        assert_eq!(prefix.h, parent.h);
        assert_eq!(prefix.u, parent.u);
        assert_ne!(prefix.h, independent.h);
        assert_ne!(prefix.u, independent.u);
        assert_ne!(prefix.h, prefix.u);
        assert!(prefix.g.iter().all(|g| *g != prefix.h && *g != prefix.u));
    }
}
