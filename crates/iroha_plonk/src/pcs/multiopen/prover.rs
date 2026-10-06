//! The multiopen prover (vendored `poly/ipa/multiopen/prover.rs` with static
//! grouping).
//!
//! Each set polynomial `q_t` is rebuilt when it is needed (three passes), as
//! in the vendored prover, so memory does not grow with the number of sets.
//! RNG draws: the `q'` blind, then the IPA's (`BlindingScheduleV1` items 7
//! and 8).

use ff::Field;
use iroha_pasta::{PastaCurve, msm::MemoryBudget, params::ParamsIpa};
use rand_core_06::{CryptoRng, RngCore};

use super::{MultiopenError, OpeningPlan, ShapeItem};
use crate::{
    pcs::ipa::{
        commit::{Secrecy, commit},
        evaluate_polynomial,
        prover::create_proof_with_claim as ipa_create_proof,
    },
    transcript::TranscriptWrite,
};

/// A slot's polynomial: `n` coefficients and the blind of its commitment.
#[derive(Clone, Copy, Debug)]
pub struct SlotPolynomial<'a, F> {
    /// The coefficients (exactly `n`).
    pub coeffs: &'a [F],
    /// The commitment blind.
    pub blind: F,
}

/// `acc <- acc * challenge + addend`, coefficient-wise.
fn fold_into<F: Field>(acc: &mut [F], challenge: F, addend: &[F]) {
    for (value, add) in acc.iter_mut().zip(addend) {
        *value = *value * challenge + add;
    }
}

/// Divides by `X - point` in place, dropping the remainder (the vendored
/// Kate-division recurrence); the result has one coefficient fewer.
pub(crate) fn divide_by_linear<F: Field>(values: &mut Vec<F>, point: F) {
    let Some(mut original) = values.last().copied() else {
        return;
    };
    let b = -point;
    let mut carry = F::ZERO;
    for index in (0..values.len() - 1).rev() {
        let next_original = values[index];
        let lead = original - carry;
        values[index] = lead;
        carry = lead * b;
        original = next_original;
    }
    values.pop();
}

/// The set polynomial `q_t = sum_{slots in t} x_1^e p_slot` (Horner in slot
/// order) and its blind.
fn reconstruct<F: Field>(
    plan: &OpeningPlan,
    polys: &[SlotPolynomial<'_, F>],
    set: usize,
    x_1: F,
    n: usize,
) -> (Vec<F>, F) {
    let mut q = vec![F::ZERO; n];
    let mut blind = F::ZERO;
    for (slot, poly) in plan.slots().iter().zip(polys) {
        if slot.set == set {
            fold_into(&mut q, x_1, poly.coeffs);
            blind = blind * x_1 + poly.blind;
        }
    }
    (q, blind)
}

/// Writes a multiopen proof for `plan` and returns the folded generator of
/// the final IPA.
///
/// - `points`: the opening point of each plan rotation (distinct);
/// - `polys`: each plan slot's polynomial (exactly `n` coefficients) and
///   blind.
///
/// # Errors
///
/// [`MultiopenError::Shape`] or [`MultiopenError::PointCollision`] for inputs
/// that do not match the plan; [`MultiopenError::Ipa`] for MSM, transcript
/// or IPA failures.
pub fn create_proof<C, T, R>(
    params: &ParamsIpa<C>,
    plan: &OpeningPlan,
    points: &[C::ScalarExt],
    polys: &[SlotPolynomial<'_, C::ScalarExt>],
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<C::AffineExt, MultiopenError>
where
    C: PastaCurve,
    T: TranscriptWrite<C> + ?Sized,
    R: RngCore + CryptoRng,
{
    Ok(*create_proof_with_claim(params, plan, points, polys, rng, transcript, budget)?.g())
}

/// Creates the same opening and returns its generator obligation.
///
/// # Errors
/// As [`create_proof`], including a degenerate folded generator.
pub fn create_proof_with_claim<C, T, R>(
    params: &ParamsIpa<C>,
    plan: &OpeningPlan,
    points: &[C::ScalarExt],
    polys: &[SlotPolynomial<'_, C::ScalarExt>],
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<crate::pcs::ipa::GeneratorClaim<C>, MultiopenError>
where
    C: PastaCurve,
    T: TranscriptWrite<C> + ?Sized,
    R: RngCore + CryptoRng,
{
    let n = params.n();
    plan.check_points(points)?;
    OpeningPlan::check_len(ShapeItem::Slots, plan.slots().len(), polys.len())?;
    for poly in polys {
        OpeningPlan::check_len(ShapeItem::Coefficients, n, poly.coeffs.len())?;
    }

    let x_1 = transcript.squeeze_challenge();
    let x_2 = transcript.squeeze_challenge();

    // q' = sum_t x_2^{n_s-1-t} floor(q_t / prod_{p in t} (X - p)).
    let mut q_prime = vec![C::ScalarExt::ZERO; n];
    for (set, set_points) in plan.sets().iter().enumerate() {
        let (mut q, _) = reconstruct(plan, polys, set, x_1, n);
        for point in set_points {
            divide_by_linear(&mut q, points[*point]);
        }
        q.resize(n, C::ScalarExt::ZERO);
        fold_into(&mut q_prime, x_2, &q);
    }
    let q_prime_blind = C::ScalarExt::random(&mut *rng);
    let q_prime_commitment = commit(params, &q_prime, &q_prime_blind, Secrecy::Secret, budget)
        .map_err(|error| MultiopenError::Ipa(error.into()))?
        .to_affine();
    transcript.write_point(&q_prime_commitment)?;

    let x_3 = transcript.squeeze_challenge();
    for set in 0..plan.sets().len() {
        let (q, _) = reconstruct(plan, polys, set, x_1, n);
        transcript.write_scalar(&evaluate_polynomial(&q, x_3));
    }

    let x_4 = transcript.squeeze_challenge();
    let mut p_blind = q_prime_blind;
    for set in 0..plan.sets().len() {
        let (q, blind) = reconstruct(plan, polys, set, x_1, n);
        fold_into(&mut q_prime, x_4, &q);
        p_blind = p_blind * x_4 + blind;
    }
    Ok(ipa_create_proof(
        params, rng, transcript, &q_prime, &p_blind, &x_3, budget,
    )?)
}

#[cfg(test)]
mod tests {
    use iroha_pasta::Fq;

    use super::*;

    #[test]
    fn linear_division_drops_the_remainder() {
        // (X^2 + 3X + 5) = (X - 2)(X + 5) + 15.
        let mut values = vec![Fq::from(5), Fq::from(3), Fq::ONE];
        divide_by_linear(&mut values, Fq::from(2));
        assert_eq!(values, vec![Fq::from(5), Fq::ONE]);
        let mut empty: Vec<Fq> = Vec::new();
        divide_by_linear(&mut empty, Fq::ONE);
        assert!(empty.is_empty());
        let mut constant = vec![Fq::from(9)];
        divide_by_linear(&mut constant, Fq::ONE);
        assert!(constant.is_empty());
    }

    #[test]
    fn fold_into_is_horner() {
        let mut acc = vec![Fq::from(1), Fq::from(2)];
        fold_into(&mut acc, Fq::from(10), &[Fq::from(3), Fq::from(4)]);
        assert_eq!(acc, vec![Fq::from(13), Fq::from(24)]);
    }
}
