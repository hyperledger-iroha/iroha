use super::{ChallengeX1, ChallengeX2, ChallengeX3, ChallengeX4, construct_intermediate_sets};
use crate::arithmetic::{CurveAffine, eval_polynomial, parallelize};
use crate::poly::commitment::ParamsProver;
use crate::poly::commitment::{Blind, Prover};
use crate::poly::ipa::commitment::{self, IPACommitmentScheme, ParamsIPA};
use crate::poly::query::{PolynomialPointer, ProverQuery};
use crate::poly::{Coeff, Polynomial};
use crate::transcript::{EncodedChallenge, TranscriptWrite};

use ff::Field;
use group::Curve;
use rand_core::RngCore;
use std::io;
use std::marker::PhantomData;

/// IPA multi-open prover.
///
/// `QUERY_INSTANCE = false` selects the direct-instance transcript used by
/// recursive circuits: public values are absorbed verbatim and their queried
/// evaluations are reconstructed by the verifier, so they do not allocate an
/// instance commitment or enter the IPA opening set. When `QUERY_INSTANCE` is
/// true, each bit in `PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK` selects an
/// instance column whose commitment is written into the proof. Other instance
/// commitments retain the legacy verifier-computed behavior.
#[derive(Debug)]
pub struct ProverIPA<
    'params,
    C: CurveAffine,
    const QUERY_INSTANCE: bool = true,
    const PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK: u64 = 0,
> {
    pub(crate) params: &'params ParamsIPA<C>,
}

/// Direct-instance IPA multi-open prover.
pub type ProverIPADirect<'params, C> = ProverIPA<'params, C, false, 0>;

/// IPA multi-open prover with a per-column proof-supplied instance commitment
/// mask.
pub type ProverIPAHybrid<'params, C, const PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK: u64> =
    ProverIPA<'params, C, true, PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK>;

impl<
    'params,
    C: CurveAffine,
    const QUERY_INSTANCE: bool,
    const PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK: u64,
> Prover<'params, IPACommitmentScheme<C>>
    for ProverIPA<'params, C, QUERY_INSTANCE, PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK>
{
    const QUERY_INSTANCE: bool = QUERY_INSTANCE;
    const PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK: u64 = PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK;

    fn new(params: &'params ParamsIPA<C>) -> Self {
        assert!(
            QUERY_INSTANCE || PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK == 0,
            "proof-supplied instance commitments require QUERY_INSTANCE"
        );
        Self { params }
    }

    /// Create a multi-opening proof
    fn create_proof<'com, Z: EncodedChallenge<C>, T: TranscriptWrite<C, Z>, R, I>(
        &self,
        mut rng: R,
        transcript: &mut T,
        queries: I,
    ) -> io::Result<()>
    where
        I: IntoIterator<Item = ProverQuery<'com, C>> + Clone,
        R: RngCore,
    {
        let x_1: ChallengeX1<_> = transcript.squeeze_challenge_scalar();
        let x_2: ChallengeX2<_> = transcript.squeeze_challenge_scalar();

        let (poly_map, point_sets) = construct_intermediate_sets(queries).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "queries iterator contains mismatching evaluations",
            )
        })?;

        // Reconstruct one Q at a time in exactly the original commitment and
        // point-set orders. Keep only this reusable buffer and the quotient
        // accumulator; no polynomial bank grows with the number of point sets.
        let mut q_poly = Polynomial {
            values: Vec::with_capacity(self.params.n as usize),
            _marker: PhantomData,
        };
        let mut q_prime_poly: Option<Polynomial<C::Scalar, Coeff>> = None;
        for (set_idx, points) in point_sets.iter().enumerate() {
            reconstruct_q(&poly_map, set_idx, *x_1, &mut q_poly);
            for point in points {
                kate_division_in_place(&mut q_poly.values, *point);
            }
            q_poly
                .values
                .resize(self.params.n as usize, C::Scalar::ZERO);
            if let Some(accumulator) = &mut q_prime_poly {
                fold_polynomial_in_place(accumulator, *x_2, &q_poly);
            } else {
                q_prime_poly = Some(q_poly.clone());
            }
        }
        let mut q_prime_poly = q_prime_poly.unwrap();

        let q_prime_blind = Blind(C::Scalar::random(&mut rng));
        let q_prime_commitment = self.params.commit(&q_prime_poly, q_prime_blind).to_affine();

        transcript.write_point(q_prime_commitment)?;

        let x_3: ChallengeX3<_> = transcript.squeeze_challenge_scalar();

        // Prover sends u_i for all i, which correspond to the evaluation
        // of each Q polynomial commitment at x_3.
        for set_idx in 0..point_sets.len() {
            reconstruct_q(&poly_map, set_idx, *x_1, &mut q_poly);
            transcript.write_scalar(eval_polynomial(&q_poly, *x_3))?;
        }

        let x_4: ChallengeX4<_> = transcript.squeeze_challenge_scalar();

        let mut p_poly_blind = q_prime_blind;
        for set_idx in 0..point_sets.len() {
            let blind = reconstruct_q(&poly_map, set_idx, *x_1, &mut q_poly);
            fold_polynomial_in_place(&mut q_prime_poly, *x_4, &q_poly);
            p_poly_blind = Blind((p_poly_blind.0 * &(*x_4)) + &blind.0);
        }
        // Release reconstruction storage before the inner IPA allocates its
        // own work buffers. The quotient accumulator becomes the final p.
        drop(q_poly);
        drop(poly_map);
        let p_poly = q_prime_poly;

        commitment::create_proof(self.params, rng, transcript, &p_poly, p_poly_blind, *x_3)
    }
}

/// Rebuild a point set's Horner combination without retaining other sets.
fn reconstruct_q<C: CurveAffine>(
    poly_map: &[super::CommitmentData<C::Scalar, PolynomialPointer<'_, C>>],
    set_idx: usize,
    challenge: C::Scalar,
    output: &mut Polynomial<C::Scalar, Coeff>,
) -> Blind<C::Scalar> {
    let mut initialized = false;
    let mut blind = Blind(C::Scalar::ZERO);
    for data in poly_map.iter().filter(|data| data.set_index == set_idx) {
        if initialized {
            fold_polynomial_in_place(output, challenge, data.commitment.poly);
        } else {
            output.values.clear();
            output
                .values
                .extend_from_slice(&data.commitment.poly.values);
            initialized = true;
        }
        blind *= challenge;
        blind += data.commitment.blind;
    }
    assert!(initialized, "every point set has a polynomial");
    blind
}

/// Preserve the original polynomial multiply-then-add operations in place.
fn fold_polynomial_in_place<F: Field>(
    accumulator: &mut Polynomial<F, Coeff>,
    challenge: F,
    addend: &Polynomial<F, Coeff>,
) {
    if challenge == F::ZERO {
        accumulator.values.fill(F::ZERO);
    } else if challenge != F::ONE {
        parallelize(&mut accumulator.values, |values, _| {
            for value in values {
                *value *= challenge;
            }
        });
    }
    parallelize(&mut accumulator.values, |values, start| {
        for (value, addend) in values.iter_mut().zip(addend.values[start..].iter()) {
            *value += *addend;
        }
    });
}

/// Apply the original Kate-division recurrence without allocating a quotient.
fn kate_division_in_place<F: Field>(values: &mut Vec<F>, point: F) {
    let b = -point;
    let mut original = *values.last().expect("Kate division requires a coefficient");
    let mut tmp = F::ZERO;
    for index in (0..values.len() - 1).rev() {
        // Writing q[index] would overwrite the next original coefficient.
        // Carry it before the write, preserving the reference recurrence.
        let next_original = values[index];
        let mut lead_coeff = original;
        lead_coeff -= tmp;
        values[index] = lead_coeff;
        tmp = lead_coeff;
        tmp *= b;
        original = next_original;
    }
    values.pop();
}

#[cfg(test)]
mod bounded_tests;
