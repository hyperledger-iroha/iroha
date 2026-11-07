//! Claim accumulators: `ProtoGalaxy` folding of IPA generator claims.
//!
//! An IPA generator claim `(G, u)` over generators `g[0..2^16)` is true when
//! `G = <s(u), g>` with `s_i = prod_j u_j^{bit_{15-j}(i)}` (PIPA section 9.2). The
//! relation
//!
//! ```text
//! R_s = { (G, u; s) : G = <s, g>,  s_0 = 1,  s_t = s_{t - 2^j} u_{15-j}  (2^j <= t < 2^{j+1}) }
//! ```
//!
//! has the unique witness `s = s(u)` for a true claim. Its constraints have degree
//! two and its commitment is the claim point itself, so a fresh claim folds without
//! any multi-scalar multiplication: the prover computes `s(u)` (2^16
//! multiplications), `F(X)` through the `pow` tree, `G(2)` and the one coefficient
//! of `K`; the verifier folds `G` with one group operation. A source of size
//! `k < 16` enters with `16 - k` leading zero challenges, as in PIPA-AS-v1.

use ff::Field;
use iroha_pasta::{PastaCurve, msm::{MemoryBudget, msm_public}};

use crate::{
    Error,
    commit::{fold_point, fold_scalar},
    transcript::{TAG_CLAIM_ACCUMULATOR, TAG_CLAIM_DIGEST, TAG_CLAIM_FOLD, Transcript},
};

/// Rounds of a claim (`2^16` generators).
pub const ROUNDS: usize = 16;
/// Generators of a claim.
pub const SIZE: usize = 1 << ROUNDS;

/// One IPA generator claim `(G, u)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim<C: PastaCurve> {
    /// The claimed folded generator.
    pub g: C::AffineExt,
    /// The round challenges, `u_0` first (`u_0` multiplies bit 15 of the index).
    pub u: [C::ScalarExt; ROUNDS],
}

/// A relaxed claim accumulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimAccumulator<C: PastaCurve> {
    /// The folded commitment `<s, g>`.
    pub g: C::AffineExt,
    /// The folded challenges.
    pub u: [C::ScalarExt; ROUNDS],
    /// `ProtoGalaxy` `pow` challenges.
    pub beta: [C::ScalarExt; ROUNDS],
    /// The accumulated error.
    pub e: C::ScalarExt,
}

/// The output of [`fold_claim`]: the new accumulator, its witness `s` and the
/// prover's messages.
pub type ClaimFoldOutput<C> = (
    ClaimAccumulator<C>,
    Vec<<C as PastaCurve>::ScalarExt>,
    ClaimProof<<C as PastaCurve>::ScalarExt>,
);

/// The prover's messages of one claim fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimProof<F> {
    /// Coefficients 1..=16 of `F(X)`.
    pub f: [F; ROUNDS],
    /// The single coefficient of `K(X)`.
    pub k: F,
}

/// `s(u)`, built by doubling.
#[must_use]
pub fn tensor<F: Field>(u: &[F; ROUNDS]) -> Vec<F> {
    let mut s = Vec::with_capacity(SIZE);
    s.push(F::ONE);
    for j in 0..ROUNDS {
        let factor = u[ROUNDS - 1 - j];
        for i in 0..s.len() {
            let v = s[i] * factor;
            s.push(v);
        }
    }
    s
}

fn constraint<F: Field>(s: &[F], u: &[F; ROUNDS], t: usize) -> F {
    if t == 0 {
        return s[0] - F::ONE;
    }
    let j = (usize::BITS - 1 - t.leading_zeros()) as usize;
    s[t] - s[t - (1 << j)] * u[ROUNDS - 1 - j]
}

/// The true claim for challenges `u` over `generators` (`G = <s(u), g>`).
///
/// # Errors
/// [`Error::Shape`] unless there are exactly `2^16` generators; [`Error::Msm`].
pub fn claim_for<C: PastaCurve>(generators: &[C::AffineExt], u: [C::ScalarExt; ROUNDS]) -> Result<Claim<C>, Error> {
    if generators.len() != SIZE {
        return Err(Error::Shape("claim generators"));
    }
    let g = msm_public::<C>(&tensor(&u), generators, MemoryBudget::DEFAULT).map_err(|_| Error::Msm)?;
    Ok(Claim { g: g.to_affine(), u })
}

fn absorb_claim<C: PastaCurve>(transcript: &mut Transcript<C>, g: &C::AffineExt, u: &[C::ScalarExt]) {
    transcript.absorb_point(g);
    transcript.absorb_scalars(u);
}

impl<C: PastaCurve> ClaimAccumulator<C> {
    /// The digest of the accumulator (`kgwfcld1`: `G`, `u`, `beta`, `e`), which an
    /// IVC state hash carries.
    #[must_use]
    pub fn digest(&self, domain: &[u8; 32]) -> C::Base {
        let mut transcript = Transcript::<C>::new(TAG_CLAIM_DIGEST, domain);
        absorb_claim::<C>(&mut transcript, &self.g, &self.u);
        transcript.absorb_scalars(&self.beta);
        transcript.absorb_scalars(&[self.e]);
        transcript.digest()
    }

    /// Starts an accumulator from one claim (`e = 0`, `beta` from the transcript).
    /// Returns the accumulator and its witness `s(u)`; a false claim yields an
    /// accumulator that [`decide_claims`] rejects.
    #[must_use]
    pub fn from_claim(domain: &[u8; 32], claim: &Claim<C>) -> (Self, Vec<C::ScalarExt>) {
        let mut transcript = Transcript::<C>::new(TAG_CLAIM_ACCUMULATOR, domain);
        absorb_claim::<C>(&mut transcript, &claim.g, &claim.u);
        let beta = core::array::from_fn(|_| transcript.challenge());
        (
            Self { g: claim.g, u: claim.u, beta, e: <C::ScalarExt as Field>::ZERO },
            tensor(&claim.u),
        )
    }
}

fn mul_linear<F: Field>(poly: &[F], a: F, b: F) -> Vec<F> {
    let mut out = vec![F::ZERO; poly.len() + 1];
    for (i, c) in poly.iter().enumerate() {
        out[i] += *c * a;
        out[i + 1] += *c * b;
    }
    out
}

fn fold_transcript<C: PastaCurve>(domain: &[u8; 32], acc: &ClaimAccumulator<C>, claim: &Claim<C>) -> Transcript<C> {
    let mut transcript = Transcript::<C>::new(TAG_CLAIM_FOLD, domain);
    absorb_claim::<C>(&mut transcript, &acc.g, &acc.u);
    transcript.absorb_scalars(&acc.beta);
    transcript.absorb_scalars(&[acc.e]);
    absorb_claim::<C>(&mut transcript, &claim.g, &claim.u);
    transcript
}

fn delta_powers<F: Field>(delta: F) -> [F; ROUNDS] {
    let mut d = delta;
    core::array::from_fn(|_| {
        let out = d;
        d = d.square();
        out
    })
}

fn finish<C: PastaCurve>(
    acc: &ClaimAccumulator<C>,
    claim: &Claim<C>,
    beta_star: [C::ScalarExt; ROUNDS],
    f_alpha: C::ScalarExt,
    k: C::ScalarExt,
    gamma: C::ScalarExt,
) -> ClaimAccumulator<C> {
    let one = <C::ScalarExt as Field>::ONE;
    ClaimAccumulator {
        g: fold_point::<C>(&acc.g, &claim.g, &gamma),
        u: core::array::from_fn(|i| fold_scalar(&acc.u[i], &claim.u[i], &gamma)),
        beta: beta_star,
        e: f_alpha * (one - gamma) + gamma * (gamma - one) * k,
    }
}

/// Folds `claim` into `acc` (witness `s`), returning the new accumulator, its
/// witness and the prover's messages.
///
/// # Errors
/// [`Error::Shape`] for a witness of the wrong length.
pub fn fold_claim<C: PastaCurve>(
    domain: &[u8; 32],
    acc: &ClaimAccumulator<C>,
    s: &[C::ScalarExt],
    claim: &Claim<C>,
) -> Result<ClaimFoldOutput<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    if s.len() != SIZE {
        return Err(Error::Shape("claim witness"));
    }
    let mut transcript = fold_transcript(domain, acc, claim);
    let delta = delta_powers(transcript.challenge());
    let mut stack: Vec<(usize, Vec<S<C>>)> = Vec::new();
    for t in 0..SIZE {
        let mut node = (0usize, vec![constraint(s, &acc.u, t)]);
        while stack.last().is_some_and(|(level, _)| *level == node.0) {
            let (level, mut left) = stack.pop().expect("checked");
            let lifted = mul_linear(&node.1, acc.beta[level], delta[level]);
            if left.len() < lifted.len() {
                left.resize(lifted.len(), S::<C>::ZERO);
            }
            for (l, r) in left.iter_mut().zip(&lifted) {
                *l += *r;
            }
            node = (level + 1, left);
        }
        stack.push(node);
    }
    let mut f_full = stack.pop().map(|(_, p)| p).unwrap_or_default();
    f_full.resize(ROUNDS + 1, S::<C>::ZERO);
    debug_assert_eq!(f_full[0], acc.e, "the accumulator must satisfy R_s");
    let f: [S<C>; ROUNDS] = core::array::from_fn(|i| f_full[i + 1]);
    transcript.absorb_scalars(&f);
    let alpha = transcript.challenge();
    let f_alpha = f.iter().rev().fold(S::<C>::ZERO, |a, c| a * alpha + c) * alpha + acc.e;
    let beta_star: [S<C>; ROUNDS] = core::array::from_fn(|i| acc.beta[i] + alpha * delta[i]);
    let fresh = tensor(&claim.u);
    let two = S::<C>::from(2u64);
    let s2: Vec<S<C>> = s.iter().zip(&fresh).map(|(a, n)| *a + two * (*n - *a)).collect();
    let u2: [S<C>; ROUNDS] = core::array::from_fn(|i| acc.u[i] + two * (claim.u[i] - acc.u[i]));
    // pow(beta*) by doubling, consumed in index order.
    let mut pow = Vec::with_capacity(SIZE);
    pow.push(S::<C>::ONE);
    for b in &beta_star {
        for i in 0..pow.len() {
            let v = pow[i] * b;
            pow.push(v);
        }
    }
    let g2 = (0..SIZE).fold(S::<C>::ZERO, |a, t| a + pow[t] * constraint(&s2, &u2, t));
    // G(2) = F(alpha) L0(2) + Z(2) K with L0(2) = -1 and Z(2) = 2.
    let k = (g2 + f_alpha) * two.invert().expect("two is invertible");
    transcript.absorb_scalars(&[k]);
    let gamma = transcript.challenge_128();
    let folded = finish(acc, claim, beta_star, f_alpha, k, gamma);
    let witness = s.iter().zip(&fresh).map(|(a, n)| fold_scalar(a, n, &gamma)).collect();
    Ok((folded, witness, ClaimProof { f, k }))
}

/// The native claim-fold verifier: recomputes the challenges and the folded
/// accumulator.
#[must_use]
pub fn verify_claim_fold<C: PastaCurve>(
    domain: &[u8; 32],
    acc: &ClaimAccumulator<C>,
    claim: &Claim<C>,
    proof: &ClaimProof<C::ScalarExt>,
) -> ClaimAccumulator<C> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let mut transcript = fold_transcript(domain, acc, claim);
    let delta = delta_powers(transcript.challenge());
    transcript.absorb_scalars(&proof.f);
    let alpha = transcript.challenge();
    let f_alpha = proof.f.iter().rev().fold(S::<C>::ZERO, |a, c| a * alpha + c) * alpha + acc.e;
    let beta_star = core::array::from_fn(|i| acc.beta[i] + alpha * delta[i]);
    transcript.absorb_scalars(&[proof.k]);
    let gamma = transcript.challenge_128();
    finish(acc, claim, beta_star, f_alpha, proof.k, gamma)
}

/// Decides a claim accumulator natively: `G = <s, g>` and
/// `sum_t pow_t(beta) f_t(s, u) = e`.
///
/// # Errors
/// [`Error::Rejected`] naming the failing check; [`Error::Shape`], [`Error::Msm`].
pub fn decide_claims<C: PastaCurve>(
    generators: &[C::AffineExt],
    acc: &ClaimAccumulator<C>,
    s: &[C::ScalarExt],
) -> Result<(), Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    if generators.len() != SIZE || s.len() != SIZE {
        return Err(Error::Shape("claim decide"));
    }
    let g = msm_public::<C>(s, generators, MemoryBudget::DEFAULT).map_err(|_| Error::Msm)?;
    if g.to_affine() != acc.g {
        return Err(Error::Rejected("claim commitment"));
    }
    let mut pow = Vec::with_capacity(SIZE);
    pow.push(S::<C>::ONE);
    for b in &acc.beta {
        for i in 0..pow.len() {
            let v = pow[i] * b;
            pow.push(v);
        }
    }
    let e = (0..SIZE).fold(S::<C>::ZERO, |a, t| a + pow[t] * constraint(s, &acc.u, t));
    if e != acc.e {
        return Err(Error::Rejected("claim error"));
    }
    Ok(())
}
