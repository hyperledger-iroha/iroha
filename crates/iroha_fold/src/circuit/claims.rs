//! In-circuit claim-accumulator folds and digests, bit for bit
//! [`crate::claims`] (output `IVC.md` section 6).
//!
//! A deferred IPA claim `(G, u)` is folded by the circuit that produces it, over
//! the base field of the claim's curve: Pallas claims (from a Pallas Ω verifier)
//! in an A circuit, Vesta claims (from a Vesta σ verifier) in a Q circuit. The
//! group part is one native multiplication; the sixteen challenges and the
//! `pow` vector fold in foreign arithmetic.

use ff::Field;
use iroha_pasta::PastaCurve;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Word, ecc::AssignedPoint};

use super::{
    FoldLanes, ScalarCell, transcript as t,
    verify::{fold_commitments, horner},
};
use crate::{
    claims::{Claim, ClaimAccumulator, ClaimProof, ROUNDS},
    transcript::{TAG_CLAIM_DIGEST, TAG_CLAIM_FOLD},
};

/// A claim `(G, u)` in circuit.
#[derive(Clone, Debug)]
pub struct ClaimCells<C: PastaCurve> {
    /// The claimed folded generator.
    pub g: AssignedPoint<C::Base>,
    /// The sixteen round challenges.
    pub u: Vec<ScalarCell<C>>,
}

/// A relaxed claim accumulator in circuit.
#[derive(Clone, Debug)]
pub struct ClaimAccumulatorCells<C: PastaCurve> {
    /// The folded generator.
    pub g: AssignedPoint<C::Base>,
    /// The folded challenges.
    pub u: Vec<ScalarCell<C>>,
    /// The `pow` challenges.
    pub beta: Vec<ScalarCell<C>>,
    /// The accumulated error.
    pub e: ScalarCell<C>,
}

/// The messages of one claim fold in circuit.
#[derive(Clone, Debug)]
pub struct ClaimProofCells<C: PastaCurve> {
    /// Coefficients `1..=16` of `F(X)`.
    pub f: Vec<ScalarCell<C>>,
    /// The single coefficient of `K(X)`.
    pub k: ScalarCell<C>,
}

fn value_of<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

fn scalars<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    values: Option<&[C::ScalarExt; ROUNDS]>,
) -> Result<Vec<ScalarCell<C>>, Error> {
    (0..ROUNDS).map(|i| lanes.assign_scalar(region, value_of(values.map(|v| &v[i])))).collect()
}

/// Assigns a claim.
///
/// # Errors
/// Layout errors.
pub fn assign_claim<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    value: Option<&Claim<C>>,
) -> Result<ClaimCells<C>, Error> {
    let g = lanes.assign_point(region, value_of(value.map(|c| C::from(c.g)).as_ref()))?;
    let u = scalars(lanes, region, value.map(|c| &c.u))?;
    Ok(ClaimCells { g, u })
}

/// Assigns a claim accumulator.
///
/// # Errors
/// Layout errors.
pub fn assign_claim_accumulator<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    value: Option<&ClaimAccumulator<C>>,
) -> Result<ClaimAccumulatorCells<C>, Error> {
    let g = lanes.assign_point(region, value_of(value.map(|a| C::from(a.g)).as_ref()))?;
    let u = scalars(lanes, region, value.map(|a| &a.u))?;
    let beta = scalars(lanes, region, value.map(|a| &a.beta))?;
    let e = lanes.assign_scalar(region, value_of(value.map(|a| &a.e)))?;
    Ok(ClaimAccumulatorCells { g, u, beta, e })
}

/// Assigns the messages of a claim fold.
///
/// # Errors
/// Layout errors.
pub fn assign_claim_proof<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    value: Option<&ClaimProof<C::ScalarExt>>,
) -> Result<ClaimProofCells<C>, Error> {
    let f = scalars(lanes, region, value.map(|p| &p.f))?;
    let k = lanes.assign_scalar(region, value_of(value.map(|p| &p.k)))?;
    Ok(ClaimProofCells { f, k })
}

/// The claim-accumulator digest, as [`ClaimAccumulator::digest`].
///
/// # Errors
/// Layout errors, or another transcript live on the lane.
pub fn claim_accumulator_digest<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    acc: &ClaimAccumulatorCells<C>,
) -> Result<Word<C::Base>, Error> {
    t::start(lanes, TAG_CLAIM_DIGEST, domain)?;
    t::absorb_point(lanes, &acc.g);
    t::absorb_scalars(lanes, region, &acc.u)?;
    t::absorb_scalars(lanes, region, &acc.beta)?;
    t::absorb_scalar(lanes, region, &acc.e)?;
    t::finish(lanes, region)
}

/// Verifies one fold of `claim` into `acc` and returns the folded accumulator,
/// as [`crate::claims::verify_claim_fold`]. Nothing is accepted here; the claim
/// decider (or its restructured IPA claim) accepts the result or not.
///
/// # Errors
/// Layout errors, or inputs of another shape.
pub fn verify_claim_fold<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    acc: &ClaimAccumulatorCells<C>,
    claim: &ClaimCells<C>,
    proof: &ClaimProofCells<C>,
) -> Result<ClaimAccumulatorCells<C>, Error> {
    if acc.u.len() != ROUNDS || acc.beta.len() != ROUNDS || claim.u.len() != ROUNDS || proof.f.len() != ROUNDS {
        return Err(Error::Synthesis);
    }
    t::start(lanes, TAG_CLAIM_FOLD, domain)?;
    t::absorb_point(lanes, &acc.g);
    t::absorb_scalars(lanes, region, &acc.u)?;
    t::absorb_scalars(lanes, region, &acc.beta)?;
    t::absorb_scalar(lanes, region, &acc.e)?;
    t::absorb_point(lanes, &claim.g);
    t::absorb_scalars(lanes, region, &claim.u)?;
    let delta = t::challenge(lanes, region)?;
    t::absorb_scalars(lanes, region, &proof.f)?;
    let alpha = t::challenge(lanes, region)?;
    t::absorb_scalar(lanes, region, &proof.k)?;
    let (gamma_limb, gamma) = t::challenge_128_last(lanes, region)?;

    let mut f_coefficients = vec![&acc.e.ff];
    f_coefficients.extend(proof.f.iter().map(|c| &c.ff));
    let f_alpha = horner(lanes, region, &f_coefficients, &alpha.ff)?;
    let mut beta = Vec::with_capacity(ROUNDS);
    let mut power = delta.ff.clone();
    for (i, b) in acc.beta.iter().enumerate() {
        if i > 0 {
            power = lanes.mul(region, &power, &power)?;
        }
        let scaled = lanes.mul(region, &alpha.ff, &power)?;
        let folded = lanes.add(region, &b.ff, &scaled)?;
        beta.push(lanes.export(region, folded)?);
    }
    let one = lanes.constant(region, <C::ScalarExt as Field>::ONE)?;
    let one_minus_gamma = lanes.sub(region, &one, &gamma.ff)?;
    let left = lanes.mul(region, &f_alpha, &one_minus_gamma)?;
    let gamma_minus_one = lanes.sub(region, &gamma.ff, &one)?;
    let z = lanes.mul(region, &gamma.ff, &gamma_minus_one)?;
    let right = lanes.mul(region, &z, &proof.k.ff)?;
    let e = lanes.add(region, &left, &right)?;
    let e = lanes.export(region, e)?;
    let mut u = Vec::with_capacity(ROUNDS);
    for (a, b) in acc.u.iter().zip(&claim.u) {
        let folded = lanes.lerp(region, &a.ff, &b.ff, &gamma.ff)?;
        u.push(lanes.export(region, folded)?);
    }
    let g = fold_commitments(lanes, region, core::slice::from_ref(&acc.g), core::slice::from_ref(&claim.g), &gamma_limb)?
        .pop()
        .ok_or(Error::Synthesis)?;
    Ok(ClaimAccumulatorCells { g, u, beta, e })
}
