//! In-circuit fresh-instance transcripts, accumulator digests and folds,
//! bit for bit [`crate::fold`].

use ff::Field;
use iroha_pasta::PastaCurve;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{
    Word,
    ecc::{AssignedPoint, EccChip, ScalarLimbs},
    ff::FfValue,
};

use super::{FoldVerifierChips, ScalarCell, transcript as t};
use crate::{
    fold::{Accumulator, FoldProof, Instance},
    relation::FoldedRelation,
    transcript::{TAG_ACCUMULATOR_DIGEST, TAG_FOLD, TAG_INSTANCE},
};

/// The commitments and public columns of an instance. A fresh instance's
/// `LogUp` challenges are recomputed in circuit, never witnessed.
#[derive(Clone, Debug)]
pub struct InstanceCells<C: PastaCurve> {
    /// One commitment per witness column, in relation order.
    pub commitments: Vec<AssignedPoint<C::Base>>,
    /// The public columns.
    pub public: Vec<Vec<ScalarCell<C>>>,
}

/// A relaxed accumulator in circuit.
#[derive(Clone, Debug)]
pub struct AccumulatorCells<C: PastaCurve> {
    /// Commitments and public columns.
    pub instance: InstanceCells<C>,
    /// `theta` powers.
    pub theta: Vec<ScalarCell<C>>,
    /// One `LogUp` challenge per group.
    pub r: Vec<ScalarCell<C>>,
    /// `pow` challenges.
    pub beta: Vec<ScalarCell<C>>,
    /// The accumulated error.
    pub e: ScalarCell<C>,
}

/// The prover's messages of one fold in circuit.
#[derive(Clone, Debug)]
pub struct FoldProofCells<C: PastaCurve> {
    /// Coefficients `1..=t` of `F(X)`.
    pub f: Vec<ScalarCell<C>>,
    /// Coefficients of `K(X)`.
    pub k: Vec<ScalarCell<C>>,
}

/// The challenges and digest of a fresh instance's transcript.
#[derive(Clone, Debug)]
pub struct FreshTranscript<C: PastaCurve> {
    /// `theta^1 .. theta^L`.
    pub theta: Vec<FfValue<C::Base>>,
    /// One `r` per group.
    pub r: Vec<ScalarCell<C>>,
    /// The instance digest.
    pub digest: Word<C::Base>,
}

fn value_of<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// Assigns private scalars (`None`: key generation, `count` unknown cells).
fn assign_scalars<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    values: Option<&[C::ScalarExt]>,
    count: usize,
) -> Result<Vec<ScalarCell<C>>, Error> {
    if values.is_some_and(|v| v.len() != count) {
        return Err(Error::Synthesis);
    }
    (0..count)
        .map(|i| chips.assign_scalar(region, value_of(values.map(|v| &v[i]))))
        .collect()
}

/// Assigns the commitments and public columns of an instance of `relation`.
///
/// # Errors
/// Layout errors, or a value of another shape.
pub fn assign_instance<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    relation: &FoldedRelation<C::ScalarExt>,
    value: Option<&Instance<C>>,
) -> Result<InstanceCells<C>, Error> {
    if let Some(v) = value
        && (v.commitments.len() != relation.columns()
            || v.public.iter().map(Vec::len).ne(relation.instance_lengths().iter().copied()))
    {
        return Err(Error::Synthesis);
    }
    let mut commitments = Vec::with_capacity(relation.columns());
    for c in 0..relation.columns() {
        let point = value.map(|v| C::from(v.commitments[c]));
        commitments.push(chips.assign_point(region, value_of(point.as_ref()))?);
    }
    let mut public = Vec::with_capacity(relation.instance_lengths().len());
    for (column, len) in relation.instance_lengths().iter().enumerate() {
        public.push(assign_scalars(chips, region, value.map(|v| v.public[column].as_slice()), *len)?);
    }
    Ok(InstanceCells { commitments, public })
}

/// Assigns an accumulator of `relation`.
///
/// # Errors
/// Layout errors, or a value of another shape.
pub fn assign_accumulator<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    relation: &FoldedRelation<C::ScalarExt>,
    value: Option<&Accumulator<C>>,
) -> Result<AccumulatorCells<C>, Error> {
    let instance = assign_instance(chips, region, relation, value.map(|v| &v.instance))?;
    let theta = assign_scalars(chips, region, value.map(|v| v.instance.theta.as_slice()), relation.theta_len())?;
    let r = assign_scalars(chips, region, value.map(|v| v.instance.r.as_slice()), relation.groups())?;
    let beta = assign_scalars(chips, region, value.map(|v| v.beta.as_slice()), relation.index_bits())?;
    let e = chips.assign_scalar(region, value_of(value.map(|v| &v.e)))?;
    Ok(AccumulatorCells { instance, theta, r, beta, e })
}

/// Assigns the messages of a fold of `relation`.
///
/// # Errors
/// Layout errors, or messages of another length.
pub fn assign_proof<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    relation: &FoldedRelation<C::ScalarExt>,
    value: Option<&FoldProof<C::ScalarExt>>,
) -> Result<FoldProofCells<C>, Error> {
    let f = assign_scalars(chips, region, value.map(|v| v.f.as_slice()), relation.index_bits())?;
    let k = assign_scalars(chips, region, value.map(|v| v.k.as_slice()), relation.degree() - 1)?;
    Ok(FoldProofCells { f, k })
}

fn absorb_public<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    public: &[Vec<ScalarCell<C>>],
) -> Result<(), Error> {
    t::absorb_u64(chips, public.len() as u64);
    for column in public {
        t::absorb_u64(chips, column.len() as u64);
        t::absorb_scalars(chips, region, column)?;
    }
    Ok(())
}

/// The fresh instance's transcript: its `theta` powers, `r` and digest.
///
/// # Errors
/// Layout errors, or an instance of another shape.
pub fn fresh_transcript<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    instance: &InstanceCells<C>,
) -> Result<FreshTranscript<C>, Error> {
    if instance.commitments.len() != relation.columns() {
        return Err(Error::Synthesis);
    }
    t::start(chips, TAG_INSTANCE, domain)?;
    absorb_public(chips, region, &instance.public)?;
    for point in &instance.commitments[..relation.advice_columns] {
        t::absorb_point(chips, point);
    }
    for group in &relation.groups {
        t::absorb_point(chips, &instance.commitments[group.m_column()]);
    }
    let theta = t::challenge(chips, region)?;
    let mut powers = Vec::with_capacity(relation.theta_len());
    let mut power = theta.ff.clone();
    for _ in 0..relation.theta_len() {
        powers.push(power.clone());
        power = chips.mul(region, &power, &theta.ff)?;
    }
    let mut r = Vec::with_capacity(relation.groups());
    for _ in &relation.groups {
        r.push(t::challenge(chips, region)?);
    }
    for group in &relation.groups {
        for c in (group.m_column() + 1)..=group.g_column() {
            t::absorb_point(chips, &instance.commitments[c]);
        }
    }
    let digest = t::finish(chips, region)?;
    Ok(FreshTranscript { theta: powers, r, digest })
}

/// The accumulator digest.
///
/// # Errors
/// Layout errors.
pub fn accumulator_digest<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    acc: &AccumulatorCells<C>,
) -> Result<Word<C::Base>, Error> {
    t::start(chips, TAG_ACCUMULATOR_DIGEST, domain)?;
    t::absorb_u64(chips, acc.instance.commitments.len() as u64);
    for point in &acc.instance.commitments {
        t::absorb_point(chips, point);
    }
    absorb_public(chips, region, &acc.instance.public)?;
    t::absorb_scalars(chips, region, &acc.theta)?;
    t::absorb_scalars(chips, region, &acc.r)?;
    t::absorb_scalars(chips, region, &acc.beta)?;
    t::absorb_scalar(chips, region, &acc.e)?;
    t::finish(chips, region)
}

/// `sum_i c_i x^i` by Horner's rule over FF values (`c_0` first).
fn horner<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    coefficients: &[&FfValue<C::Base>],
    x: &FfValue<C::Base>,
) -> Result<FfValue<C::Base>, Error> {
    let (last, rest) = coefficients.split_last().ok_or(Error::Synthesis)?;
    let mut acc = (*last).clone();
    for c in rest.iter().rev() {
        let scaled = chips.mul(region, &acc, x)?;
        acc = chips.add(region, &scaled, c)?;
    }
    Ok(acc)
}

/// Folds every commitment: `C* = C_acc + gamma (C_new - C_acc)`, one GLV split
/// of `gamma` shared by all columns.
fn fold_commitments<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    acc: &[AssignedPoint<C::Base>],
    new: &[AssignedPoint<C::Base>],
    gamma: &iroha_plonk_gadgets::U128<C::Base>,
) -> Result<Vec<AssignedPoint<C::Base>>, Error> {
    let zero = chips.uint().constant::<127>(region, 0)?;
    let mut split = None;
    let mut out = Vec::with_capacity(acc.len());
    for (a, b) in acc.iter().zip(new) {
        let negated = EccChip::<C>::neg(&mut chips.glue, region, a)?;
        let difference = chips.ecc.add(region, b, &negated)?;
        let scaled = match &split {
            Some(split) => chips.ecc.mul_with(region, split, &difference)?,
            None => {
                let (scaled, glv) = chips.ecc.mul(region, &mut chips.range, ScalarLimbs::new(gamma, &zero), &difference)?;
                split = Some(glv);
                scaled
            }
        };
        out.push(chips.ecc.add(region, a, &scaled)?);
    }
    Ok(out)
}

/// Verifies one fold of `fresh` into `acc` and returns the folded accumulator,
/// as [`crate::fold::verify_fold`]: the fresh instance's `LogUp` challenges and
/// digest, the accumulator digest, `delta`, `alpha`, `gamma`, then every folded
/// value. Nothing is accepted here; a decider accepts the result or not.
///
/// # Errors
/// Layout errors, or inputs of another shape.
pub fn verify_fold<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    acc: &AccumulatorCells<C>,
    fresh: &InstanceCells<C>,
    proof: &FoldProofCells<C>,
) -> Result<AccumulatorCells<C>, Error> {
    let bits = relation.index_bits();
    if proof.f.len() != bits
        || proof.k.len() != relation.degree() - 1
        || acc.beta.len() != bits
        || acc.instance.commitments.len() != relation.columns()
        || acc.theta.len() != relation.theta_len()
        || acc.r.len() != relation.groups()
    {
        return Err(Error::Synthesis);
    }
    let fresh_t = fresh_transcript(chips, region, relation, domain, fresh)?;
    let acc_digest = accumulator_digest(chips, region, domain, acc)?;
    t::start(chips, TAG_FOLD, domain)?;
    t::absorb_word(chips, &acc_digest);
    t::absorb_word(chips, &fresh_t.digest);
    let delta = t::challenge(chips, region)?;
    t::absorb_scalars(chips, region, &proof.f)?;
    let alpha = t::challenge(chips, region)?;
    t::absorb_scalars(chips, region, &proof.k)?;
    let (gamma_limb, gamma) = t::challenge_128_last(chips, region)?;

    // F(alpha) over (e, F_1, .., F_t).
    let mut f_coefficients = vec![&acc.e.ff];
    f_coefficients.extend(proof.f.iter().map(|c| &c.ff));
    let f_alpha = horner(chips, region, &f_coefficients, &alpha.ff)?;
    // beta*_j = beta_j + alpha delta^(2^j).
    let mut beta = Vec::with_capacity(bits);
    let mut power = delta.ff.clone();
    for (j, b) in acc.beta.iter().enumerate() {
        if j > 0 {
            power = chips.mul(region, &power, &power)?;
        }
        let scaled = chips.mul(region, &alpha.ff, &power)?;
        let folded = chips.add(region, &b.ff, &scaled)?;
        beta.push(chips.export(region, folded)?);
    }
    // e* = F(alpha) (1 - gamma) + gamma (gamma - 1) K(gamma).
    let k_coefficients: Vec<_> = proof.k.iter().map(|c| &c.ff).collect();
    let k_gamma = horner(chips, region, &k_coefficients, &gamma.ff)?;
    let one = chips.constant(region, <C::ScalarExt as Field>::ONE)?;
    let one_minus_gamma = chips.sub(region, &one, &gamma.ff)?;
    let left = chips.mul(region, &f_alpha, &one_minus_gamma)?;
    let gamma_minus_one = chips.sub(region, &gamma.ff, &one)?;
    let z = chips.mul(region, &gamma.ff, &gamma_minus_one)?;
    let right = chips.mul(region, &z, &k_gamma)?;
    let e = chips.add(region, &left, &right)?;
    let e = chips.export(region, e)?;
    // Linear parts.
    let mut public = Vec::with_capacity(acc.instance.public.len());
    for (a_column, b_column) in acc.instance.public.iter().zip(&fresh.public) {
        if a_column.len() != b_column.len() {
            return Err(Error::Synthesis);
        }
        let mut column = Vec::with_capacity(a_column.len());
        for (a, b) in a_column.iter().zip(b_column) {
            let folded = chips.lerp(region, &a.ff, &b.ff, &gamma.ff)?;
            column.push(chips.export(region, folded)?);
        }
        public.push(column);
    }
    let mut theta = Vec::with_capacity(acc.theta.len());
    for (a, b) in acc.theta.iter().zip(&fresh_t.theta) {
        let folded = chips.lerp(region, &a.ff, b, &gamma.ff)?;
        theta.push(chips.export(region, folded)?);
    }
    let mut r = Vec::with_capacity(acc.r.len());
    for (a, b) in acc.r.iter().zip(&fresh_t.r) {
        let folded = chips.lerp(region, &a.ff, &b.ff, &gamma.ff)?;
        r.push(chips.export(region, folded)?);
    }
    let commitments = fold_commitments(chips, region, &acc.instance.commitments, &fresh.commitments, &gamma_limb)?;
    Ok(AccumulatorCells { instance: InstanceCells { commitments, public }, theta, r, beta, e })
}
