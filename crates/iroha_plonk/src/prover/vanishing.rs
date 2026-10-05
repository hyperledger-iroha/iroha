//! The vanishing argument (spec section 2 "Degree and vanishing", section 7
//! rows 5 and 6, `BlindingScheduleV1` items 5 and 6).
//!
//! - `R`: `n` random coefficients and a random blind, committed in
//!   coefficient form before `y`; `R(x)` masks `h(x_3)` in the multiopen
//!   because both are opened at `x`.
//! - `h`: the `(d - 1) n` coefficients from [`super::quotient`] split into
//!   `d - 1` pieces of `n` coefficients, each with its own random blind,
//!   committed in coefficient form and written in order before `x`.
//!
//! The polynomial opened at `x` is `h_0 + x^n h_1 + ... + x^{(d-2) n}
//! h_{d-2}` with the blinds combined the same way, which the verifier
//! commits to as `sum_i x^{n i} H_i`.

use ff::Field;
use iroha_pasta::{PastaCurve, msm::MemoryBudget};
use rand_core_06::RngCore;

use super::{ProverError, random_values, write_point};
use crate::{
    keys::{KeyError, ProvingKey},
    pcs::ipa::{PinnedParams, commit::Secrecy},
    protocol::Shape,
    transcript::TranscriptWrite,
};

/// The committed random polynomial `R`.
pub(super) struct RandomPoly<F> {
    /// `n` coefficients.
    pub(super) coeffs: Vec<F>,
    /// Its blind.
    pub(super) blind: F,
}

/// The committed quotient pieces.
pub(super) struct QuotientPieces<F> {
    pieces: Vec<Vec<F>>,
    blinds: Vec<F>,
}

/// The quotient combined at `x^n` for its opening at `x`.
pub(super) struct CombinedQuotient<F> {
    /// `sum_i x^{n i} h_i` (`n` coefficients).
    pub(super) coeffs: Vec<F>,
    /// `sum_i x^{n i} blind_i`.
    pub(super) blind: F,
}

/// Draws, commits and writes `R` (`BlindingScheduleV1` item 5).
///
/// # Errors
///
/// MSM and transcript errors.
pub(super) fn commit_random<C, T, R>(
    params: &PinnedParams<C>,
    pk: &ProvingKey<C>,
    shape: &Shape,
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<RandomPoly<C::ScalarExt>, ProverError>
where
    C: PastaCurve,
    T: TranscriptWrite<C>,
    R: RngCore,
{
    let coeffs: Vec<C::ScalarExt> = random_values(rng, shape.n);
    let blind = C::ScalarExt::random(&mut *rng);
    let commitment = pk
        .commitment_tables()
        .commit(params.params(), &coeffs, &blind, Secrecy::Secret, budget)?
        .to_affine();
    write_point(transcript, &commitment)?;
    Ok(RandomPoly { coeffs, blind })
}

/// Splits `h` into `d - 1` pieces, draws their blinds (`BlindingScheduleV1`
/// item 6), commits them and writes the commitments in order.
///
/// # Errors
///
/// [`ProverError::Key`] when `h` does not have `(d - 1) n` coefficients, or
/// MSM and transcript errors.
pub(super) fn commit_quotient<C, T, R>(
    params: &PinnedParams<C>,
    pk: &ProvingKey<C>,
    shape: &Shape,
    h: &[C::ScalarExt],
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<QuotientPieces<C::ScalarExt>, ProverError>
where
    C: PastaCurve,
    T: TranscriptWrite<C>,
    R: RngCore,
{
    let expected = shape
        .quotient_pieces
        .checked_mul(shape.n)
        .ok_or(ProverError::Protocol(
            crate::protocol::ProtocolError::Overflow,
        ))?;
    if h.len() != expected {
        return Err(ProverError::Key(KeyError::Shape {
            what: "quotient coefficients",
            expected,
            actual: h.len(),
        }));
    }
    let pieces: Vec<Vec<C::ScalarExt>> = h.chunks_exact(shape.n).map(<[_]>::to_vec).collect();
    let blinds: Vec<C::ScalarExt> = random_values(rng, shape.quotient_pieces);
    let tables = pk.commitment_tables();
    for (piece, blind) in pieces.iter().zip(&blinds) {
        let commitment = tables
            .commit(params.params(), piece, blind, Secrecy::Secret, budget)?
            .to_affine();
        write_point(transcript, &commitment)?;
    }
    Ok(QuotientPieces { pieces, blinds })
}

impl<F: Field> QuotientPieces<F> {
    /// `h_0 + xn h_1 + ...` and the matching blind (Horner from the last
    /// piece).
    pub(super) fn combine(&self, xn: F) -> CombinedQuotient<F> {
        let n = self.pieces.first().map_or(0, Vec::len);
        let mut coeffs = vec![F::ZERO; n];
        for piece in self.pieces.iter().rev() {
            for (acc, value) in coeffs.iter_mut().zip(piece) {
                *acc = *acc * xn + value;
            }
        }
        let blind = self
            .blinds
            .iter()
            .rev()
            .fold(F::ZERO, |acc, blind| acc * xn + blind);
        CombinedQuotient { coeffs, blind }
    }
}

#[cfg(test)]
mod tests {
    use iroha_pasta::Fq;

    use super::*;
    use crate::pcs::ipa::evaluate_polynomial;

    #[test]
    fn combined_pieces_evaluate_like_the_long_quotient() {
        let n = 4;
        let h: Vec<Fq> = (1..=12_u64).map(Fq::from).collect();
        let pieces = QuotientPieces {
            pieces: h.chunks_exact(n).map(<[_]>::to_vec).collect(),
            blinds: vec![Fq::from(2), Fq::from(3), Fq::from(5)],
        };
        let x = Fq::from(7);
        let xn = x.pow_vartime([n as u64]);
        let combined = pieces.combine(xn);
        assert_eq!(
            evaluate_polynomial(&combined.coeffs, x),
            evaluate_polynomial(&h, x)
        );
        assert_eq!(
            combined.blind,
            Fq::from(2) + xn * Fq::from(3) + xn.square() * Fq::from(5)
        );
    }
}
