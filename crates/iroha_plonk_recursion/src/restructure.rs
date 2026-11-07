//! Restructuring a relaxed claim accumulator into a structured claim.
//!
//! A claim accumulator folds deferred generator claims `G = <s(u), g>` as
//! committed tensor vectors, so after folding it holds a relaxed pair
//! `(G*, s*)`: `G* = <s*, g[0..2^16)>` with `s*` no longer of the form `s(u)`.
//! At compression its restructuring sumcheck reduces the relaxed tensor
//! constraints to the value `v = s*~(rho)` of the multilinear extension at a
//! point `rho` (little-endian index bits, as in [`iroha_plonk::pcs::mle`]).
//! This module opens that value with the committed-mask k16 fold of
//! [`crate::accumulation`], changed only in its public vector: `b = eq(rho, .)`
//! instead of powers of `z`. The output is the structured claim
//! `(G', u')`, `G' = <s(u'), g>`, which joins acc_P/acc_V like every other
//! claim ([`AccumulatorT::as_input`]).
//!
//! Binding and privacy follow PIPA-AS-v1 (spec C2) with the same transcript
//! function: the opened `D = h_0 g_0 + h_1 g_1 + rho_D W` commits the digest
//! `h` of `(G*, rho, v)` and of every mask before any challenge, and the
//! transcript sees only the public context, `D`, the masked `L_j`, `R_j`, `c`
//! and `G'`:
//!
//! ```text
//! context [params digest limbs, RESTRUCTURE_TAG, 1], D, zeta,
//! per round L_j, R_j, u_j (total nonzero map), then c and the unabsorbed G'
//! ```
//!
//! with `L_j = <s_hi, G_lo> + <s_hi, b_lo> zeta U + m_2j W` and
//! `R_j = <s_lo, G_hi> + <s_lo, b_hi> zeta U + m_2j+1 W`, folded top index bit
//! first. The verifier, which holds the tape like a fold verifier, checks
//!
//! ```text
//! G* + v zeta U + sum_j (u_j^-1 L_j + u_j R_j) - c b' zeta U - c G' - gamma W = 0,
//! b' = folded_eq(rho, u),   gamma = sum_j (m_2j u_j^-1 + m_2j+1 u_j).
//! ```
//!
//! The body and witness layouts are a fold's ([`FOLD_BODY_BYTES`],
//! [`FOLD_WITNESS_BYTES`]). As for folds, an Ok verification is not an
//! acceptance: the returned claim must be decided or forwarded as a bound
//! obligation.

use ff::{Field, PrimeField};
use group::CurveAffine;
use iroha_pasta::{
    PastaAffine, PastaCurve, PastaField,
    msm::msm_secret_cancellable,
    poseidon::{PoseidonField, Sponge},
};
use iroha_plonk::{
    pcs::{
        ipa::{PinnedParams, commit::msm_complete_cancellable},
        mle::{eq_table, folded_eq},
    },
    transcript::{
        BasePoseidonHash, Transcript, TranscriptError, TranscriptRead, TranscriptReader,
        TranscriptWrite, generator_coordinates,
        pipa_r::{nonzero_challenge, scalar_elements},
    },
};
use zeroize::Zeroizing;

use crate::{
    AccumulatorT, Error, FOLD_BODY_BYTES, FOLD_WITNESS_BYTES, FoldConfig, FoldTape, FoldWitness,
    GENERATORS, K, K_U32,
    accumulation::{fold_generators, opening},
    fold_tape::FieldGuard,
    private::{AffineGuard, AffineVec, FieldVec, PointGuard, PrivateWriter},
};

/// Context tag of a restructuring; no [`crate::FoldPurpose`] has it.
pub const RESTRUCTURE_TAG: [u8; 8] = *b"rstrclm1";

/// Domain of the restructuring input digest.
const DIGEST_DOMAIN: [u8; 8] = *b"pipa-rs1";

/// The opened statement: a relaxed claim accumulator's commitment and the
/// value of its tensor vector's multilinear extension at the restructuring
/// point. Private inside the wrap; cleared on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct RestructureStatement<C: PastaCurve> {
    /// `G* = <s*, g[0..2^16)>`, without a blind.
    pub commitment: C::AffineExt,
    /// `rho`, little-endian index bits.
    pub point: [C::ScalarExt; K],
    /// `v = sum_t eq(rho, t) s*_t`.
    pub value: C::ScalarExt,
}

impl<C: PastaCurve> core::fmt::Debug for RestructureStatement<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("RestructureStatement { private: [REDACTED] }")
    }
}

impl<C: PastaCurve> Drop for RestructureStatement<C> {
    fn drop(&mut self) {
        fn field<F: PastaField>(value: &mut F) {
            value.zeroize();
        }
        fn point<A: PastaAffine>(value: &mut A) {
            value.zeroize();
        }
        point(&mut self.commitment);
        self.point.iter_mut().for_each(field);
        field(&mut self.value);
    }
}

/// Consumes one private tape to open `statement` for the tensor vector
/// `witness` (`s*`, exactly `2^16` entries) and returns the private witness
/// and the structured claim `(G', u')`.
///
/// A false statement (a `G*` or `v` that `witness` does not have) is caught by
/// verifying the result before returning it. The consumed tape is cleared on
/// return and unwinding. Coefficient MSMs use full-width secret planning; the
/// generator fold is variable time over public challenges, as in a fold.
///
/// # Errors
///
/// Insufficient parameters, a witness that is not `2^16` long, cancellation,
/// identity `D`/`L`/`R` messages (negligible) or [`Error::FoldEquation`].
pub fn create_restructure<C: PastaCurve>(
    params: &PinnedParams<C>,
    statement: &RestructureStatement<C>,
    witness: &[C::ScalarExt],
    tape: FoldTape<C>,
    config: &FoldConfig,
) -> Result<(FoldWitness<C>, AccumulatorT<C>), Error>
where
    C::Base: PoseidonField,
{
    iroha_pasta::CancellationToken::checkpoint(config.cancellation.as_ref())?;
    params.require_k(K_U32).map_err(Error::Parameters)?;
    if witness.len() != GENERATORS {
        return Err(Error::Length {
            expected: GENERATORS,
            actual: witness.len(),
        });
    }
    let digest = FieldGuard(input_digest(params, statement, &tape));
    let d = AffineGuard::<C>(opening(params, digest.0, *tape.rho()).to_affine());
    let mut transcript = PrivateWriter::<C, _>::new(BasePoseidonHash::with_domain(
        iroha_plonk::contract::PIPA_AS_FOLD_DOMAIN,
    ));
    prelude(transcript.writer(), params)?;
    transcript.writer().write_point(&d.0)?;
    let zeta = transcript.writer().squeeze_challenge();

    let mut coefficients = FieldVec(witness.to_vec());
    let mut evaluations = FieldVec(eq_table(&statement.point));
    let mut generators = AffineVec::<C>(params.params().g()[..GENERATORS].to_vec());
    let auxiliary = params.params().u().to_curve();
    let blinding = params.params().w().to_curve();
    let mut challenges = [C::ScalarExt::ZERO; K];
    for (round, challenge) in challenges.iter_mut().enumerate() {
        iroha_pasta::CancellationToken::checkpoint(config.cancellation.as_ref())?;
        let half = coefficients.len() / 2;
        let left_value = FieldGuard(inner_product(&coefficients[half..], &evaluations[..half]));
        let right_value = FieldGuard(inner_product(&coefficients[..half], &evaluations[half..]));
        let left = AffineGuard::<C>(
            (msm_secret_cancellable::<C>(
                &coefficients[half..],
                &generators[..half],
                config.kernel_budget,
                &config.shared_budget,
                config.cancellation.as_ref(),
            )
            .map_err(Error::from)?
                + auxiliary * (left_value.0 * zeta)
                + blinding * tape.masks()[2 * round])
                .to_affine(),
        );
        let right = AffineGuard::<C>(
            (msm_secret_cancellable::<C>(
                &coefficients[..half],
                &generators[half..],
                config.kernel_budget,
                &config.shared_budget,
                config.cancellation.as_ref(),
            )
            .map_err(Error::from)?
                + auxiliary * (right_value.0 * zeta)
                + blinding * tape.masks()[2 * round + 1])
                .to_affine(),
        );
        transcript.writer().write_point(&left.0)?;
        transcript.writer().write_point(&right.0)?;
        *challenge = nonzero_challenge(transcript.writer().squeeze_challenge());
        let inverse = Option::<C::ScalarExt>::from(challenge.invert())
            .ok_or(Error::ZeroChallenge { round })?;
        for index in 0..half {
            let upper = FieldGuard(coefficients[index + half]);
            coefficients[index] += upper.0 * inverse;
            let upper = FieldGuard(evaluations[index + half]);
            evaluations[index] += upper.0 * *challenge;
        }
        coefficients.truncate(half);
        evaluations.truncate(half);
        fold_generators::<C>(&mut generators, *challenge, config)?;
        generators.truncate(half);
    }
    transcript.writer().write_scalar(&coefficients[0]);
    transcript
        .writer()
        .append_unabsorbed_generator(&generators[0]);
    let body = transcript.finish();
    let coins = tape.to_private_bytes();
    // The attempt consumes and clears its tape after the final encoding.
    drop(tape);
    let mut bytes = Zeroizing::new(vec![0; FOLD_WITNESS_BYTES].into_boxed_slice());
    bytes[..FOLD_BODY_BYTES].copy_from_slice(&body);
    bytes[FOLD_BODY_BYTES..].copy_from_slice(&coins);
    let proof = FoldWitness::from_private_bytes(&bytes)?;
    let output = AccumulatorT::new(generators[0], challenges)?;
    if verify_restructure(params, statement, &proof, config)? != output {
        return Err(Error::FoldEquation);
    }
    Ok((proof, output))
}

/// Verifies a restructuring of `statement` and returns its **undecided**
/// structured claim. Complete arithmetic is used for every group operation.
///
/// # Errors
///
/// Insufficient parameters, cancellation, malformed proof messages, a `D` that
/// does not open to the statement and tape, or a false equation
/// ([`Error::FoldEquation`]).
pub fn verify_restructure<C: PastaCurve>(
    params: &PinnedParams<C>,
    statement: &RestructureStatement<C>,
    proof: &FoldWitness<C>,
    config: &FoldConfig,
) -> Result<AccumulatorT<C>, Error>
where
    C::Base: PoseidonField,
{
    iroha_pasta::CancellationToken::checkpoint(config.cancellation.as_ref())?;
    params.require_k(K_U32).map_err(Error::Parameters)?;
    let bytes = proof.to_private_bytes();
    let tape = FoldTape::<C>::from_private_bytes(&bytes.0[FOLD_BODY_BYTES..])
        .ok_or(Error::Encoding(TranscriptError::NonCanonicalScalar))?;
    let mut transcript = TranscriptReader::<C, _>::new(
        BasePoseidonHash::with_domain(iroha_plonk::contract::PIPA_AS_FOLD_DOMAIN),
        &bytes.0[..FOLD_BODY_BYTES],
    );
    let digest = FieldGuard(input_digest(params, statement, &tape));
    prelude(&mut transcript, params)?;
    let d = AffineGuard::<C>(transcript.read_point()?);
    if d.0.to_curve() != opening(params, digest.0, *tape.rho()) {
        return Err(Error::FoldEquation);
    }
    let zeta = transcript.squeeze_challenge();
    let mut gamma = FieldGuard(C::ScalarExt::ZERO);
    let mut points = AffineVec::<C>(Vec::with_capacity(2 * K + 4));
    let mut scalars = FieldVec(Vec::with_capacity(2 * K + 4));
    let mut challenges = [C::ScalarExt::ZERO; K];
    for (round, challenge) in challenges.iter_mut().enumerate() {
        iroha_pasta::CancellationToken::checkpoint(config.cancellation.as_ref())?;
        let left = AffineGuard::<C>(transcript.read_point()?);
        let right = AffineGuard::<C>(transcript.read_point()?);
        *challenge = nonzero_challenge(transcript.squeeze_challenge());
        let inverse = Option::<C::ScalarExt>::from(challenge.invert())
            .ok_or(Error::ZeroChallenge { round })?;
        gamma.0 += tape.masks()[2 * round] * inverse + tape.masks()[2 * round + 1] * *challenge;
        points.extend([left.0, right.0]);
        scalars.extend([inverse, *challenge]);
    }
    let final_coefficient = FieldGuard(transcript.read_scalar()?);
    let generator = AffineGuard::<C>(transcript.read_unabsorbed_generator()?);
    transcript.finish()?;
    let folded = FieldGuard(folded_eq(&statement.point, &challenges).ok_or(Error::FoldEquation)?);
    points.extend([
        statement.commitment,
        params.params().u(),
        generator.0,
        params.params().w(),
    ]);
    scalars.extend([
        C::ScalarExt::ONE,
        (statement.value - final_coefficient.0 * folded.0) * zeta,
        -final_coefficient.0,
        -gamma.0,
    ]);
    let equation = PointGuard(msm_complete_cancellable::<C>(
        &scalars,
        &points,
        config.kernel_budget,
        &config.shared_budget,
        config.cancellation.as_ref(),
    )?);
    if !bool::from(equation.0.is_identity()) {
        return Err(Error::FoldEquation);
    }
    AccumulatorT::new(generator.0, challenges)
}

/// The public context words: two parameter-digest limbs, the tag and the
/// count of opened claims (one).
fn context<C: PastaCurve>(params: &PinnedParams<C>) -> [C::Base; 4] {
    let d = params.digest();
    let (low, high) = d.split_at(16);
    [
        C::Base::from_u128(u128::from_le_bytes(low.try_into().expect("16 bytes"))),
        C::Base::from_u128(u128::from_le_bytes(high.try_into().expect("16 bytes"))),
        C::Base::from(u64::from_le_bytes(RESTRUCTURE_TAG)),
        C::Base::ONE,
    ]
}

/// Absorbs the public context; the digest and opening never enter it.
fn prelude<C: PastaCurve, T: Transcript<C>>(
    transcript: &mut T,
    params: &PinnedParams<C>,
) -> Result<(), Error> {
    for word in context(params) {
        transcript.common_base(&word)?;
    }
    Ok(())
}

/// RP57 digest of the context, `G*`, `rho`, `v` and every mask.
fn input_digest<C: PastaCurve>(
    params: &PinnedParams<C>,
    statement: &RestructureStatement<C>,
    tape: &FoldTape<C>,
) -> C::Base
where
    C::Base: PoseidonField,
{
    let mut words = FieldVec(Vec::with_capacity(6 + 2 * (K + 1 + 2 * K)));
    words.extend(context(params));
    let (x, y) = generator_coordinates::<C>(&statement.commitment);
    words.extend([x, y]);
    let scalars = statement
        .point
        .iter()
        .chain(core::iter::once(&statement.value))
        .chain(tape.masks());
    for value in scalars {
        let encoded = FieldVec(scalar_elements::<C>(value));
        words.extend_from_slice(&encoded);
    }
    let mut sponge = Sponge::new();
    sponge.update(&[
        C::Base::from(u64::from_le_bytes(DIGEST_DOMAIN)),
        C::Base::from(words.len() as u64),
    ]);
    sponge.update(&words);
    sponge.squeeze()
}

fn inner_product<F: PastaField>(left: &[F], right: &[F]) -> F {
    let mut sum = FieldGuard(F::ZERO);
    for (left, right) in left.iter().zip(right) {
        sum.0 += *left * right;
    }
    sum.0
}

#[cfg(test)]
mod tests;
