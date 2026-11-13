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
use iroha_pasta::fold::fold_generators_vartime;
use iroha_pasta::msm::{BudgetExceeded, FixedBaseTable, MemoryBudget, msm_public, msm_secret};
use rayon::prelude::*;

use crate::fs::{SpikeRng, Transcript};

pub type F<C> = <C as PastaCurve>::ScalarExt;
pub type Aff<C> = <C as PastaCurve>::AffineExt;

/// Domain of the experiment's public commitment bases. Changing it changes
/// every commitment and the relation; it is not a production parameter ID.
pub const GENERATOR_DOMAIN: &str = "kagemusha-zks-spike/gens";

pub struct Gens<C: PastaCurve> {
    pub g: Vec<Aff<C>>,
    pub h: Aff<C>,
    pub u: Aff<C>,
    /// Fixed-base tables of `g[..active]` by (active, window), built on first
    /// use (iroha_pasta `FixedBaseTable`, 2b50a7f647).
    pub tables: std::sync::Mutex<Vec<(usize, usize, std::sync::Arc<FixedBaseTable<C>>)>>,
    /// Fixed-base tables of the round slots by (offset, grid size): the
    /// slot's grid generators, then `h` (review lane ebb983).
    pub round_tables: std::sync::Mutex<Vec<(usize, usize, std::sync::Arc<FixedBaseTable<C>>)>>,
}

/// Window of the round-slot tables: 28 bases, fastest at 9 on the Mac
/// (`ZKS_KBENCH=roundcommit`: 0.19 ms against 0.51 ms for the generic MSM).
const ROUND_WINDOW: usize = 9;

/// The backend has one pinned prover implementation. Benchmark environment
/// variables cannot change a native wallet's cryptographic execution path.
pub fn old_kernels() -> bool {
    false
}

impl<C: PastaCurve> Gens<C> {
    /// Derive `g[i] = H_curve(GENERATOR_DOMAIN, LE64(i))`, with `h` at `n`
    /// and `u` at `n + 1`. The full capacity therefore identifies the
    /// blinding bases, even when only a prefix of `g` is used.
    pub fn new(n: usize) -> Self {
        let points: Vec<C> = (0..n + 2)
            .into_par_iter()
            .map(|i| {
                C::hash_to_curve(GENERATOR_DOMAIN, &(i as u64).to_le_bytes())
                    .expect("hash to curve")
            })
            .collect();
        let affine = iroha_pasta::curve::batch_normalize(&points);
        Self {
            g: affine[..n].to_vec(),
            h: affine[n],
            u: affine[n + 1],
            tables: Default::default(),
            round_tables: Default::default(),
        }
    }

    /// The same generators without built tables. Preserve the parent set's
    /// `h` and `u`: `Gens::new(n)` would select different blinding bases.
    pub fn prefix(&self, n: usize) -> Self {
        Self {
            g: self.g[..n].to_vec(),
            h: self.h,
            u: self.u,
            tables: Default::default(),
            round_tables: Default::default(),
        }
    }

    /// The fixed-base table of the round slot at `offset` holding `len` grid
    /// values: bases `g[offset + position]` in grid order, then `h` for the
    /// blind. Built once per generator set.
    pub fn round_table(&self, offset: usize, len: usize) -> std::sync::Arc<FixedBaseTable<C>> {
        let mut tables = self.round_tables.lock().unwrap();
        if let Some((_, _, t)) = tables.iter().find(|(o, l, _)| *o == offset && *l == len) {
            return t.clone();
        }
        let mut bases: Vec<Aff<C>> = crate::grid::slot_positions(crate::grid::grid_vars(len))
            .iter()
            .map(|p| self.g[offset + p])
            .collect();
        bases.push(self.h);
        let table = std::sync::Arc::new(
            FixedBaseTable::<C>::with_window(&bases, ROUND_WINDOW, MemoryBudget::DEFAULT)
                .expect("table within budget"),
        );
        tables.push((offset, len, table.clone()));
        table
    }

    /// The fixed-base table of `g[..active]` with window `window`, when its
    /// construction fits the fixed budget. The second value is build time.
    pub fn table(
        &self,
        active: usize,
        window: usize,
    ) -> Result<(std::sync::Arc<FixedBaseTable<C>>, f64), BudgetExceeded> {
        self.table_with_budget(active, window, MemoryBudget::DEFAULT)
    }

    pub(crate) fn table_with_budget(
        &self,
        active: usize,
        window: usize,
        budget: MemoryBudget,
    ) -> Result<(std::sync::Arc<FixedBaseTable<C>>, f64), BudgetExceeded> {
        let mut tables = self.tables.lock().unwrap();
        if let Some((_, _, t)) = tables.iter().find(|(a, w, _)| *a == active && *w == window) {
            // A cache hit needs no construction scratch, but its retained
            // points and skip flags still belong to this call's budget.
            budget.check(t.table_bytes().saturating_add(active))?;
            return Ok((t.clone(), 0.0));
        }
        let clock = std::time::Instant::now();
        let table = std::sync::Arc::new(FixedBaseTable::<C>::with_window(
            &self.g[..active],
            window,
            budget,
        )?);
        let build = clock.elapsed().as_secs_f64();
        tables.push((active, window, table.clone()));
        Ok((table, build))
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
const ROUND_NAMES: [&str; 17] = [
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
];

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

/// Absorbs the inherited claims and folds them into the commitment, the
/// value and (for the prover) the vector: returns (P', kappa', xi, s_j).
pub fn fold_claims<C: PastaCurve>(
    t: &mut Transcript<C>,
    p: &C,
    b: &[F<C>],
    kappa: F<C>,
    claims: &[Claim<C>],
) -> (C, F<C>, Vec<Vec<F<C>>>) {
    if claims.is_empty() {
        return (*p, kappa, Vec::new());
    }
    for claim in claims {
        t.point(b"ipa/claim", &claim.g);
    }
    let xi: F<C> = t.endo_challenge(b"ipa/xi");
    let (mut p2, mut k2, mut power) = (*p, kappa, F::<C>::ONE);
    let mut svecs = Vec::with_capacity(claims.len());
    for claim in claims {
        power *= xi;
        let s = s_vector(&claim.us);
        assert_eq!(s.len(), b.len(), "claim over the same generators");
        k2 += power * inner(&s, b);
        p2 += C::from(claim.g) * power;
        svecs.push(s.into_iter().map(|v| v * power).collect());
    }
    (p2, k2, svecs)
}

#[allow(clippy::too_many_arguments)]
pub fn prove<C: PastaCurve>(
    gens: &Gens<C>,
    t: &mut Transcript<C>,
    p: &C,
    mut x: Vec<F<C>>,
    mut b: Vec<F<C>>,
    kappa: F<C>,
    mut blind: F<C>,
    rng: &mut SpikeRng,
    claims: &[Claim<C>],
) -> IpaProof<C> {
    let n = x.len();
    assert!(n.is_power_of_two() && b.len() == n && gens.g.len() == n);
    let (p, kappa, svecs) = fold_claims(t, p, &b, kappa, claims);
    for s in &svecs {
        x.iter_mut().zip(s).for_each(|(v, w)| *v += w);
    }
    let u_point = begin(gens, t, &p, &kappa);
    let h = C::from(gens.h);
    let old = old_kernels();
    // U as a base of the L/R sums (the new kernels fold the value and blind
    // terms into each sum instead of four separate scalar multiplications).
    let u_affine = group::Curve::to_affine(&u_point);
    let mut g = gens.g.clone();
    let mut lr = Vec::with_capacity(n.trailing_zeros() as usize);
    let mut len = n;
    while len > 1 {
        let half = len / 2;
        let (lam, mu): (F<C>, F<C>) = (rng.scalar(), rng.scalar());
        let lr_clock = std::time::Instant::now();
        let (l, r) = if old {
            let (x_lo, x_hi) = x.split_at(half);
            let (b_lo, b_hi) = b.split_at(half);
            let (g_lo, g_hi) = g[..len].split_at(half);
            let l = msm_secret::<C>(x_lo, g_hi, MemoryBudget::DEFAULT).expect("msm")
                + u_point * inner(x_lo, b_hi)
                + h * lam;
            let r = msm_secret::<C>(x_hi, g_lo, MemoryBudget::DEFAULT).expect("msm")
                + u_point * inner(x_hi, b_lo)
                + h * mu;
            (l.to_affine(), r.to_affine())
        } else {
            // L and R at the same time, each one sum over its generators, U
            // and H: the same group elements as above.
            let (x_lo, x_hi) = x.split_at(half);
            let (b_lo, b_hi) = b.split_at(half);
            let (g_lo, g_hi) = g[..len].split_at(half);
            let side = |xs: &[F<C>], bs: &[F<C>], gs: &[Aff<C>], blind: F<C>| -> Aff<C> {
                let mut scalars = Vec::with_capacity(half + 2);
                scalars.extend_from_slice(xs);
                scalars.push(inner(xs, bs));
                scalars.push(blind);
                let mut bases = Vec::with_capacity(half + 2);
                bases.extend_from_slice(gs);
                bases.push(u_affine);
                bases.push(gens.h);
                group::Curve::to_affine(
                    &msm_secret::<C>(&scalars, &bases, MemoryBudget::DEFAULT).expect("msm"),
                )
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
        crate::prof::add(
            ROUND_NAMES[half.trailing_zeros() as usize],
            lr_clock.elapsed().as_secs_f64(),
        );
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
        fold_generators_vartime::<C>(&mut g[..len], &u);
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
    let mut s = vec![S::ONE];
    for u in us {
        let mut next = Vec::with_capacity(s.len() * 2);
        for value in &s {
            next.push(*value);
            next.push(*value * u);
        }
        s = next;
    }
    s
}

pub fn verify<C: PastaCurve>(
    gens: &Gens<C>,
    t: &mut Transcript<C>,
    p: &C,
    mut b: Vec<F<C>>,
    kappa: F<C>,
    proof: &IpaProof<C>,
    claims: &[Claim<C>],
) -> Result<(Vec<F<C>>, F<C>), String> {
    let n = b.len();
    if proof.lr.len() != n.trailing_zeros() as usize || gens.g.len() != n {
        return Err("ipa: round count".into());
    }
    let (p, kappa, _) = fold_claims(t, p, &b, kappa, claims);
    let u_point = begin(gens, t, &p, &kappa);
    let mut folded = p + u_point * kappa;
    let mut us = Vec::with_capacity(proof.lr.len());
    let mut len = n;
    for (l, r) in &proof.lr {
        t.point(b"ipa/L", l);
        t.point(b"ipa/R", r);
        let u: F<C> = t.endo_challenge(b"ipa/u");
        folded = folded * u + C::from(*l) * u.square() + C::from(*r);
        let half = len / 2;
        let (b_lo, b_hi) = b.split_at_mut(half);
        b_lo.par_iter_mut()
            .zip(b_hi.par_iter())
            .for_each(|(lo, hi)| *lo += u * hi);
        b.truncate(half);
        us.push(u);
        len = half;
    }
    // Own generator claim, decided natively here (accumulated in recursion).
    let g_final = msm_public::<C>(&s_vector(&us), &gens.g, MemoryBudget::DEFAULT)
        .map_err(|e| format!("ipa: msm {e:?}"))?;
    if g_final.to_affine() != proof.g_final {
        return Err("ipa: folded generator".into());
    }
    t.point(b"ipa/G", &proof.g_final);
    t.point(b"ipa/A", &proof.a);
    let c: F<C> = t.endo_challenge(b"ipa/c");
    let lhs = (C::from(proof.g_final) + u_point * b[0]) * proof.z1 + C::from(gens.h) * proof.z2;
    let rhs = C::from(proof.a) + folded * c;
    if lhs != rhs {
        return Err("ipa: final opening".into());
    }
    if bool::from(C::from(proof.a).is_identity()) {
        return Err("ipa: identity commitment".into());
    }
    Ok((us, b[0]))
}

#[cfg(test)]
mod generator_tests {
    use super::*;
    use group::{Curve, GroupEncoding};
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};

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
