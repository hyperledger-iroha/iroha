//! In-circuit claim-accumulator folds and digests, bit for bit
//! [`crate::claims`] (output `IVC.md` section 6).
//!
//! A deferred IPA claim `(G, u)` is folded by the circuit that produces it, over
//! the base field of the claim's curve: Pallas claims (from a Pallas Ω verifier)
//! in an A circuit, Vesta claims (from a Vesta σ verifier) in a Q circuit. The
//! group part is one native multiplication; the sixteen challenges and the
//! `pow` vector fold in foreign arithmetic.
//!
//! The restructuring sumcheck ([`verify_restructure_field`]) is the field side
//! of the claim decision: a circuit over the claims' scalar field checks it
//! natively and hands `rho` and `v` (as S6 limbs) to the committed-mask IPA
//! (`iroha_plonk_recursion::restructure`, Claude b04a57) on the commitment
//! side. The claim digest crosses the other way as limbs.

use ff::{Field, PrimeField};
use iroha_pasta::PastaCurve;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Bit, Word, ecc::AssignedPoint};

use super::{
    FoldLanes, ScalarCell,
    field::FieldLanes,
    transcript as t,
    verify::{fold_commitments, horner},
};
use crate::{
    claims::{Claim, ClaimAccumulator, ClaimProof, ROUNDS, RestructureProof},
    transcript::{
        TAG_CLAIM_ACCUMULATOR, TAG_CLAIM_DIGEST, TAG_CLAIM_FOLD, TAG_CLAIM_RESTRUCTURE, s6_limbs,
    },
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
    (0..ROUNDS)
        .map(|i| lanes.assign_scalar(region, value_of(values.map(|v| &v[i]))))
        .collect()
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
    claim_fold_cells(lanes, region, domain, acc, claim, proof, None)
}

/// [`verify_claim_fold`], or, where `start` is one, the accumulator started from
/// `claim` ([`ClaimAccumulator::from_claim`]): the fold with `gamma = 1` (so
/// `G* = G`, `u* = u` and `e* = 0`) and `beta` from `kgwfcla1`; `acc` and the
/// messages are ignored when starting. The first claim a side folds starts its
/// accumulator, so one circuit serves both.
///
/// # Errors
/// Layout errors, or inputs of another shape.
pub fn verify_claim_fold_or_start<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    acc: &ClaimAccumulatorCells<C>,
    claim: &ClaimCells<C>,
    proof: &ClaimProofCells<C>,
    start: &Bit<C::Base>,
) -> Result<ClaimAccumulatorCells<C>, Error> {
    claim_fold_cells(lanes, region, domain, acc, claim, proof, Some(start))
}

#[allow(clippy::too_many_arguments)]
fn claim_fold_cells<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    acc: &ClaimAccumulatorCells<C>,
    claim: &ClaimCells<C>,
    proof: &ClaimProofCells<C>,
    start: Option<&Bit<C::Base>>,
) -> Result<ClaimAccumulatorCells<C>, Error> {
    if acc.u.len() != ROUNDS
        || acc.beta.len() != ROUNDS
        || claim.u.len() != ROUNDS
        || proof.f.len() != ROUNDS
    {
        return Err(Error::Synthesis);
    }
    // The beta of an accumulator started from the claim (kgwfcla1).
    let started = match start {
        Some(_) => {
            t::start(lanes, TAG_CLAIM_ACCUMULATOR, domain)?;
            t::absorb_point(lanes, &claim.g);
            t::absorb_scalars(lanes, region, &claim.u)?;
            let mut beta = Vec::with_capacity(ROUNDS);
            for j in 0..ROUNDS {
                beta.push(if j + 1 == ROUNDS {
                    t::challenge_last(lanes, region)?
                } else {
                    t::challenge(lanes, region)?
                });
            }
            Some(beta)
        }
        None => None,
    };
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
    let one = lanes.constant(region, <C::ScalarExt as Field>::ONE)?;
    // Starting is the fold with gamma = 1.
    let (gamma_limb, gamma) = match start {
        None => (gamma_limb, gamma.ff),
        Some(bit) => {
            let one_word = lanes.glue.constant(region, <C::Base as Field>::ONE)?;
            let selected = lanes
                .glue
                .select(region, bit, &one_word, gamma_limb.word())?;
            let limb = lanes.uint().range_check::<128>(region, &selected)?;
            let scalar = lanes.ff.select(lanes.glue, region, bit, &one, &gamma.ff)?;
            (limb, scalar)
        }
    };

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
        let mut folded = lanes.add(region, &b.ff, &scaled)?;
        if let (Some(bit), Some(started)) = (start, &started) {
            folded = lanes
                .ff
                .select(lanes.glue, region, bit, &started[i].ff, &folded)?;
        }
        beta.push(lanes.export(region, folded)?);
    }
    let one_minus_gamma = lanes.sub(region, &one, &gamma)?;
    let left = lanes.mul(region, &f_alpha, &one_minus_gamma)?;
    let gamma_minus_one = lanes.sub(region, &gamma, &one)?;
    let z = lanes.mul(region, &gamma, &gamma_minus_one)?;
    let right = lanes.mul(region, &z, &proof.k.ff)?;
    let e = lanes.add(region, &left, &right)?;
    let e = lanes.export(region, e)?;
    let mut u = Vec::with_capacity(ROUNDS);
    for (a, b) in acc.u.iter().zip(&claim.u) {
        let folded = lanes.lerp(region, &a.ff, &b.ff, &gamma)?;
        u.push(lanes.export(region, folded)?);
    }
    let g = fold_commitments(
        lanes,
        region,
        core::slice::from_ref(&acc.g),
        core::slice::from_ref(&claim.g),
        &gamma_limb,
    )?
    .pop()
    .ok_or(Error::Synthesis)?;
    Ok(ClaimAccumulatorCells { g, u, beta, e })
}

type W<C> = Word<<C as PastaCurve>::ScalarExt>;

/// A claim accumulator's field data in a field-side circuit: its digest
/// `kgwfcld1` as limbs (from the commitment side) and `u`, `beta`, `e` natively.
#[derive(Clone, Debug)]
pub struct ClaimAccumulatorFieldCells<C: PastaCurve> {
    /// `kgwfcld1` as `(lo, hi)` limbs.
    pub digest: [W<C>; 2],
    /// The folded challenges.
    pub u: Vec<W<C>>,
    /// The `pow` challenges.
    pub beta: Vec<W<C>>,
    /// The accumulated error.
    pub e: W<C>,
}

/// A restructuring sumcheck's messages in a field-side circuit.
#[derive(Clone, Debug)]
pub struct RestructureFieldCells<C: PastaCurve> {
    /// Each round polynomial at `0` and `2`.
    pub rounds: Vec<[W<C>; 2]>,
    /// The claimed `s~(rho)`.
    pub value: W<C>,
}

fn field_witnesses<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    values: Option<&[C::ScalarExt]>,
    count: usize,
) -> Result<Vec<W<C>>, Error> {
    if values.is_some_and(|v| v.len() != count) {
        return Err(Error::Synthesis);
    }
    (0..count)
        .map(|i| {
            lanes.glue.witness(
                region,
                values.map_or_else(Value::unknown, |v| Value::known(v[i])),
            )
        })
        .collect()
}

/// Assigns a claim accumulator's field data and digest limbs (`None`: key
/// generation); the wrap constrains them to the values crossing sides.
///
/// # Errors
/// Layout errors.
pub fn assign_claim_accumulator_field<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    domain: &[u8; 32],
    acc: Option<&ClaimAccumulator<C>>,
) -> Result<ClaimAccumulatorFieldCells<C>, Error> {
    let limbs = acc.map(|a| s6_limbs(&a.digest(domain)).map(C::ScalarExt::from_u128));
    let digest = field_witnesses::<C>(lanes, region, limbs.as_ref().map(<[_; 2]>::as_slice), 2)?;
    let u = field_witnesses::<C>(lanes, region, acc.map(|a| a.u.as_slice()), ROUNDS)?;
    let beta = field_witnesses::<C>(lanes, region, acc.map(|a| a.beta.as_slice()), ROUNDS)?;
    let e =
        field_witnesses::<C>(lanes, region, acc.map(|a| core::slice::from_ref(&a.e)), 1)?.remove(0);
    Ok(ClaimAccumulatorFieldCells {
        digest: [digest[0].clone(), digest[1].clone()],
        u,
        beta,
        e,
    })
}

/// Assigns a restructuring sumcheck's messages (`None`: key generation).
///
/// # Errors
/// Layout errors.
pub fn assign_restructure_field<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    proof: Option<&RestructureProof<C::ScalarExt>>,
) -> Result<RestructureFieldCells<C>, Error> {
    let rounds = (0..ROUNDS)
        .map(|j| {
            let round =
                field_witnesses::<C>(lanes, region, proof.map(|p| p.rounds[j].as_slice()), 2)?;
            Ok([round[0].clone(), round[1].clone()])
        })
        .collect::<Result<_, Error>>()?;
    let value = field_witnesses::<C>(
        lanes,
        region,
        proof.map(|p| core::slice::from_ref(&p.value)),
        1,
    )?
    .remove(0);
    Ok(RestructureFieldCells { rounds, value })
}

/// Verifies a restructuring sumcheck natively, bit for bit
/// [`crate::claims::restructure_verify`], and returns `rho` and `v` for the
/// committed-mask IPA. The lane is clear afterwards.
///
/// # Errors
/// Layout errors, messages of another shape, or another transcript live on
/// the lane.
pub fn verify_restructure_field<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    domain: &[u8; 32],
    acc: &ClaimAccumulatorFieldCells<C>,
    proof: &RestructureFieldCells<C>,
) -> Result<(Vec<W<C>>, W<C>), Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    if acc.u.len() != ROUNDS || acc.beta.len() != ROUNDS || proof.rounds.len() != ROUNDS {
        return Err(Error::Synthesis);
    }
    let (one, zero) = (S::<C>::ONE, S::<C>::ZERO);
    let half = S::<C>::from(2).invert().expect("two is invertible");
    lanes.start(TAG_CLAIM_RESTRUCTURE, domain)?;
    lanes.absorb(&acc.digest);
    lanes.absorb(&acc.u);
    lanes.absorb(&acc.beta);
    lanes.absorb(core::slice::from_ref(&acc.e));
    let mut claim = lanes.affine(region, &acc.e, one, one)?;
    let mut rho = Vec::with_capacity(ROUNDS);
    for round in &proof.rounds {
        lanes.absorb(round);
        let x = lanes.challenge(region)?;
        // The degree-2 polynomial through (0, s0), (1, claim - s0), (2, s2) at x.
        let s1 = lanes.sub(region, &claim, &round[0])?;
        let x_minus_one = lanes.affine(region, &x, one, -one)?;
        let x_minus_two = lanes.affine(region, &x, one, -S::<C>::from(2))?;
        let l0 = lanes.mul(region, &x_minus_one, &x_minus_two)?;
        let l1 = lanes.mul(region, &x, &x_minus_two)?;
        let l2 = lanes.mul(region, &x, &x_minus_one)?;
        let t0 = lanes.mul(region, &round[0], &l0)?;
        let t1 = lanes.mul(region, &s1, &l1)?;
        let t2 = lanes.mul(region, &round[1], &l2)?;
        let even = lanes.add(region, &t0, &t2)?;
        let even = lanes.affine(region, &even, half, zero)?;
        claim = lanes.sub(region, &even, &t1)?;
        rho.push(x);
    }
    // B~(rho) by the msb grouping of claims::restructured_weight_at.
    let mut suffix = vec![lanes.constant(region, zero)?; ROUNDS + 1];
    for m in (0..ROUNDS).rev() {
        let term = lanes.mul(region, &acc.beta[m], &acc.u[ROUNDS - 1 - m])?;
        suffix[m] = lanes.add(region, &suffix[m + 1], &term)?;
    }
    let mut above = vec![lanes.constant(region, one)?; ROUNDS + 1];
    for l in (0..ROUNDS).rev() {
        let not_rho = lanes.affine(region, &rho[l], -one, one)?;
        above[l] = lanes.mul(region, &above[l + 1], &not_rho)?;
    }
    let first = lanes.affine(region, &suffix[0], -one, one)?;
    let mut total = lanes.mul(region, &first, &above[0])?;
    let mut below = lanes.constant(region, one)?;
    for m in 0..ROUNDS {
        let weight = lanes.affine(region, &suffix[m + 1], -one, one)?;
        let term = lanes.mul(region, &weight, &acc.beta[m])?;
        let term = lanes.mul(region, &term, &rho[m])?;
        let term = lanes.mul(region, &term, &above[m + 1])?;
        let term = lanes.mul(region, &term, &below)?;
        total = lanes.add(region, &total, &term)?;
        let rho_beta = lanes.mul(region, &rho[m], &acc.beta[m])?;
        let not_rho = lanes.affine(region, &rho[m], -one, one)?;
        let factor = lanes.add(region, &not_rho, &rho_beta)?;
        below = lanes.mul(region, &below, &factor)?;
    }
    let expected = lanes.mul(region, &proof.value, &total)?;
    FieldLanes::assert_equal(region, &expected, &claim)?;
    lanes.absorb(core::slice::from_ref(&proof.value));
    lanes.finish(region)?;
    Ok((rho, proof.value.clone()))
}
