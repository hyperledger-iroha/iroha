//! Private, committed-mask restructuring of exact generator claims.
//!
//! Four ordered claims are batched before a fresh multilinear evaluation is
//! chosen. All private inputs and masks are committed through D before the
//! batching challenge. The private IPA removes its accumulated H mask exactly;
//! only its fresh (G, u) claim is exported. There is no blinding scalar in the
//! exported claim, and native verification must decide that claim with an MSM.
use crate::{
    fs::{ProofEntropy, Scalar, SpikeRng, Transcript},
    ipa::{Aff, Claim, F, Gens, s_vector},
};
use ff::{Field, PrimeField};
use group::{Curve, Group};
use iroha_pasta::msm::{MemoryBudget, msm_public, msm_secret};
use iroha_pasta::poseidon::PoseidonField;
use iroha_pasta::{PastaAffine, PastaCurve};
use zeroize::{Zeroize, Zeroizing};

pub const SOURCE_COUNT: usize = 4;
pub const IO_START: usize = 160;
const INPUT_DOMAIN: &[u8] = b"generator-restructure/private-input/v1";
const FOLD_DOMAIN: &[u8] = b"generator-restructure/fold/v1";

/// Private witness: never serialize this structure into a Payment or proof.
#[derive(Clone)]
pub struct Fold<C: PastaCurve> {
    pub sources: Vec<Claim<C>>,
    pub masks: Vec<F<C>>,
    pub d: Aff<C>,
    pub lr: Vec<(Aff<C>, Aff<C>)>,
    pub c: F<C>,
    pub output: Claim<C>,
}

impl<C: PastaCurve> Drop for Fold<C> {
    fn drop(&mut self) {
        for source in &mut self.sources {
            source.g.zeroize();
            source.us.zeroize();
        }
        self.masks.zeroize();
        self.d.zeroize();
        for (l, r) in &mut self.lr {
            l.zeroize();
            r.zeroize();
        }
        self.c.zeroize();
        self.output.g.zeroize();
        self.output.us.zeroize();
    }
}

/// Stable offsets in the cross-field, range-checked shared limb stream.
#[derive(Clone, Debug)]
pub struct Layout {
    pub k: usize,
    pub sources: usize,
    pub masks: usize,
    pub digest: usize,
    pub challenges: usize,
    pub c: usize,
    pub gamma: usize,
    pub kz: usize,
    pub cbz: usize,
    pub adjusted_c: usize,
    pub output: usize,
    pub zero: usize,
    pub len: usize,
}
impl Layout {
    pub fn new(k: usize) -> Self {
        let sources = 2;
        let masks = sources + SOURCE_COUNT * (4 + 2 * k);
        let digest = masks + 2 * (1 + 2 * k);
        let challenges = digest + 2;
        let c = challenges + 2 * k + 2;
        let gamma = c + 2;
        let kz = gamma + 2;
        let cbz = kz + 2;
        let adjusted_c = cbz + 2;
        let output = adjusted_c + 2;
        Self {
            k,
            sources,
            masks,
            digest,
            challenges,
            c,
            gamma,
            kz,
            cbz,
            adjusted_c,
            output,
            zero: output + 4 + 2 * k,
            len: output + 5 + 2 * k,
        }
    }
    pub fn source(&self, j: usize) -> usize {
        self.sources + j * (4 + 2 * self.k)
    }
    pub fn r(&self, j: usize) -> usize {
        self.challenges + 1 + j
    }
    pub fn zeta(&self) -> usize {
        self.challenges + 1 + self.k
    }
    pub fn u(&self, j: usize) -> usize {
        self.challenges + 2 + self.k + j
    }
}

#[derive(Clone)]
pub struct Trace<C: PastaCurve> {
    pub relation: [u8; 32],
    pub layout: Layout,
    pub shared: Vec<u128>,
    pub input_header: ([C::Base; 3], usize),
    pub fold_header: ([C::Base; 3], usize),
    pub digest: C::Base,
    /// mu, r[0..k], zeta, u[0..k], with the native squeeze's freshness.
    pub challenges: Vec<(Vec<bool>, bool)>,
    pub mu: F<C>,
    pub point: Vec<F<C>>,
    pub zeta: F<C>,
    pub gamma: F<C>,
    pub value: F<C>,
    pub b_final: F<C>,
    pub commitment: C,
}

impl<C: PastaCurve> Drop for Trace<C> {
    fn drop(&mut self) {
        self.shared.zeroize();
        self.digest.zeroize();
        self.challenges.zeroize();
        self.mu.zeroize();
        self.point.zeroize();
        self.zeta.zeroize();
        self.gamma.zeroize();
        self.value.zeroize();
        self.b_final.zeroize();
        self.commitment.zeroize();
    }
}

pub(crate) fn header<C: PastaCurve>(input: bool, k: usize) -> Transcript<C> {
    let mut t = Transcript::new(if input { INPUT_DOMAIN } else { FOLD_DOMAIN });
    t.bytes(b"slots", &(SOURCE_COUNT as u64).to_le_bytes());
    t.bytes(b"rounds", &(k as u64).to_le_bytes());
    // Complete the constant header; callers replay its exact pending state.
    t
}
fn limbs(bytes: [u8; 32]) -> [u128; 2] {
    [
        u128::from_le_bytes(bytes[..16].try_into().unwrap()),
        u128::from_le_bytes(bytes[16..].try_into().unwrap()),
    ]
}
fn push_point<C: PastaCurve>(out: &mut Vec<u128>, p: Aff<C>) {
    let (x, y) = Option::from(p.coordinates()).unwrap_or((C::Base::ZERO, C::Base::ZERO));
    out.extend(limbs(x.to_repr()));
    out.extend(limbs(y.to_repr()));
}
fn absorb_limbs<C: PastaCurve>(t: &mut Transcript<C>, s: F<C>) {
    for x in limbs(s.to_repr()) {
        t.absorb(C::Base::from_u128(x));
    }
}
fn input_digest<C: PastaCurve>(
    relation: &[u8; 32],
    sources: &[Claim<C>],
    masks: &[F<C>],
) -> C::Base {
    let k = sources[0].us.len();
    let mut t = header::<C>(true, k);
    for x in limbs(*relation) {
        t.absorb(C::Base::from_u128(x));
    }
    for s in sources {
        t.point(b"source", &s.g);
        for u in &s.us {
            absorb_limbs(&mut t, *u);
        }
    }
    for m in masks {
        absorb_limbs(&mut t, *m);
    }
    let _ = t.challenge_full(b"input-digest");
    t.last_word()
}
fn d_open<C: PastaCurve>(gens: &Gens<C>, digest: C::Base, rho: F<C>) -> C {
    let h = limbs(digest.to_repr());
    C::from(gens.g[0]) * F::<C>::from_u128(h[0])
        + C::from(gens.g[1]) * F::<C>::from_u128(h[1])
        + C::from(gens.h) * rho
}
fn challenge<C: PastaCurve>(
    t: &mut Transcript<C>,
    out: &mut Vec<(Vec<bool>, bool)>,
    endo: bool,
) -> F<C> {
    let fresh = !t.has_spare();
    let bits = t.challenge_bits();
    let v = if endo {
        crate::fs::endo_scalar(&bits)
    } else {
        crate::fs::raw_scalar(&bits)
    };
    out.push((bits, fresh));
    v
}
fn closed<S: Field>(us: &[S], point: &[S]) -> S {
    us.iter()
        .zip(point)
        .map(|(u, r)| S::ONE - r + *r * u)
        .product()
}
fn inner<S: Field>(a: &[S], b: &[S]) -> S {
    a.iter().zip(b).map(|(a, b)| *a * b).sum()
}
fn eq_weights<S: Field>(point: &[S]) -> Vec<S> {
    let mut out = vec![S::ONE];
    for r in point {
        out = out
            .into_iter()
            .flat_map(|v| [v * (S::ONE - r), v * r])
            .collect();
    }
    out
}
fn msm<C: PastaCurve>(s: &[F<C>], g: &[Aff<C>]) -> C {
    msm_secret::<C>(s, g, MemoryBudget::DEFAULT).expect("bounded generator restructuring MSM")
}

/// The exact claim decision, also required for the fresh exported claim.
pub fn decide<C: PastaCurve>(gens: &Gens<C>, claim: &Claim<C>) -> bool {
    if claim.us.len() >= usize::BITS as usize {
        return false;
    }
    let n = 1usize << claim.us.len();
    n <= gens.g.len()
        && msm_public::<C>(&s_vector(&claim.us), &gens.g[..n], MemoryBudget::DEFAULT)
            .is_ok_and(|g| g == C::from(claim.g))
}

/// Fresh operation entropy is mandatory at production call sites.
pub fn prove<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    entropy: &ProofEntropy,
    purpose: &[u8],
) -> (Fold<C>, Trace<C>)
where
    F<C>: Scalar,
{
    prove_with_rng(gens, relation, sources, &mut entropy.stream(purpose))
}
pub(crate) fn prove_with_rng<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    rng: &mut SpikeRng,
) -> (Fold<C>, Trace<C>)
where
    F<C>: Scalar,
{
    assert_eq!(sources.len(), SOURCE_COUNT);
    let k = sources[0].us.len();
    let n = 1usize << k;
    assert!(k > 0 && n <= gens.g.len());
    assert!(
        sources.iter().all(|s| s.us.len() == k && decide(gens, s)),
        "exact source generator claims"
    );
    let masks = (0..1 + 2 * k).map(|_| rng.scalar()).collect::<Vec<_>>();
    let digest = input_digest::<C>(&relation, &sources, &masks);
    let d = d_open(gens, digest, masks[0]).to_affine();
    let mut t = header::<C>(false, k);
    for h in limbs(relation) {
        t.absorb(C::Base::from_u128(h));
    }
    t.point(b"D", &d);
    let mut ch = vec![];
    let mu = challenge(&mut t, &mut ch, true);
    let mut commitment = C::identity();
    let mut coeff = Zeroizing::new(vec![F::<C>::ZERO; n]);
    let mut weight = F::<C>::ONE;
    for s in &sources {
        commitment += C::from(s.g) * weight;
        for (x, v) in coeff.iter_mut().zip(s_vector(&s.us)) {
            *x += weight * v;
        }
        weight *= mu;
    }
    t.point(b"C-star", &commitment.to_affine());
    let point = (0..k)
        .map(|_| challenge(&mut t, &mut ch, false))
        .collect::<Vec<_>>();
    let zeta = challenge(&mut t, &mut ch, true);
    let mut b = Zeroizing::new(eq_weights(&point));
    let u = C::from(gens.u) * zeta;
    let mut g = Zeroizing::new(gens.g[..n].to_vec());
    let mut lr = vec![];
    let mut us = vec![];
    for round in 0..k {
        let h = coeff.len() / 2;
        let l = msm::<C>(&coeff[..h], &g[h..])
            + u * inner(&coeff[..h], &b[h..])
            + C::from(gens.h) * masks[1 + 2 * round];
        let r = msm::<C>(&coeff[h..], &g[..h])
            + u * inner(&coeff[h..], &b[..h])
            + C::from(gens.h) * masks[2 + 2 * round];
        t.point(b"L", &l.to_affine());
        t.point(b"R", &r.to_affine());
        let challenge = challenge(&mut t, &mut ch, true);
        us.push(challenge);
        for j in 0..h {
            coeff[j] = challenge * coeff[j] + coeff[j + h];
            b[j] = b[j] + challenge * b[j + h];
            g[j] = (C::from(g[j]) + C::from(g[j + h]) * challenge).to_affine();
        }
        coeff.truncate(h);
        b.truncate(h);
        g.truncate(h);
        lr.push((l.to_affine(), r.to_affine()));
    }
    let fold = Fold {
        sources,
        masks,
        d,
        lr,
        c: coeff[0],
        output: Claim { us, g: g[0] },
    };
    let trace = trace(gens, relation, &fold).expect("fresh restructuring verifies");
    (fold, trace)
}

/// Reconstruct every transcript and equation. Does not decide the output MSM;
/// native callers must additionally call `decide` (or use `verify`).
pub fn trace<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    fold: &Fold<C>,
) -> Option<Trace<C>> {
    let k = fold.lr.len();
    if k == 0
        || k >= usize::BITS as usize
        || (1usize << k) > gens.g.len()
        || fold.sources.len() != SOURCE_COUNT
        || fold.sources.iter().any(|s| s.us.len() != k)
        || fold.masks.len() != 1 + 2 * k
        || fold.output.us.len() != k
    {
        return None;
    }
    let digest = input_digest::<C>(&relation, &fold.sources, &fold.masks);
    if d_open(gens, digest, fold.masks[0]) != C::from(fold.d) {
        return None;
    }
    let input_header = header::<C>(true, k).snapshot();
    let fold_header = header::<C>(false, k).snapshot();
    let mut t = header::<C>(false, k);
    for h in limbs(relation) {
        t.absorb(C::Base::from_u128(h));
    }
    t.point(b"D", &fold.d);
    let mut challenges = vec![];
    let mu = challenge(&mut t, &mut challenges, true);
    let mut commitment = C::identity();
    let mut weight = F::<C>::ONE;
    for source in &fold.sources {
        commitment += C::from(source.g) * weight;
        weight *= mu;
    }
    t.point(b"C-star", &commitment.to_affine());
    let point = (0..k)
        .map(|_| challenge(&mut t, &mut challenges, false))
        .collect::<Vec<_>>();
    let zeta = challenge(&mut t, &mut challenges, true);
    let mut value = F::<C>::ZERO;
    weight = F::<C>::ONE;
    for source in &fold.sources {
        value += weight * closed(&source.us, &point);
        weight *= mu;
    }
    let mut folded = commitment + C::from(gens.u) * (value * zeta);
    let mut gamma = F::<C>::ZERO;
    let mut us = vec![];
    for (i, (l, r)) in fold.lr.iter().enumerate() {
        t.point(b"L", l);
        t.point(b"R", r);
        let u = challenge(&mut t, &mut challenges, true);
        us.push(u);
        folded = folded * u + C::from(*l) * u.square() + C::from(*r);
        gamma = gamma * u + fold.masks[1 + 2 * i] * u.square() + fold.masks[2 + 2 * i];
    }
    if us != fold.output.us {
        return None;
    }
    let b_final = closed(&us, &point);
    if folded
        != C::from(fold.output.g) * fold.c
            + C::from(gens.u) * (fold.c * b_final * zeta)
            + C::from(gens.h) * gamma
    {
        return None;
    }
    let layout = Layout::new(k);
    let mut shared = limbs(relation).to_vec();
    for source in &fold.sources {
        push_point::<C>(&mut shared, source.g);
        for u in &source.us {
            shared.extend(limbs(u.to_repr()));
        }
    }
    for mask in &fold.masks {
        shared.extend(limbs(mask.to_repr()));
    }
    shared.extend(limbs(digest.to_repr()));
    for (bits, _) in &challenges {
        shared.push(
            bits.iter()
                .enumerate()
                .fold(0, |n, (i, b)| n | ((*b as u128) << i)),
        );
    }
    let offset = F::<C>::from(2).pow_vartime([255]) + F::<C>::ONE;
    for s in [
        fold.c,
        gamma,
        value * zeta,
        fold.c * b_final * zeta,
        (fold.c - offset) * F::<C>::from(2).invert().unwrap(),
    ] {
        shared.extend(limbs(s.to_repr()));
    }
    push_point::<C>(&mut shared, fold.output.g);
    for u in &us {
        shared.extend(limbs(u.to_repr()));
    }
    shared.push(0);
    assert_eq!(shared.len(), layout.len);
    Some(Trace {
        relation,
        layout,
        shared,
        input_header,
        fold_header,
        digest,
        challenges,
        mu,
        point,
        zeta,
        gamma,
        value,
        b_final,
        commitment,
    })
}
pub fn verify<C: PastaCurve>(gens: &Gens<C>, relation: [u8; 32], fold: &Fold<C>) -> bool {
    trace(gens, relation, fold).is_some() && decide(gens, &fold.output)
}

#[path = "generator_claim_scalar.rs"]
mod scalar;
pub use scalar::{ScalarPorts, append_scalar};

#[cfg(test)]
#[path = "generator_claim_tests.rs"]
mod tests;
