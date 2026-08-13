//! Public response-bound checks for decoded Lantern presentations.
use thiserror::Error;
use super::{
    codec::BootleLanternPresentationProofV1,
    compression::{center_proof_residue_v1, use_gamma_hint_v1},
    params::{Z1_NORM_SQUARED_BOUND_V1, Z3_NORM_SQUARED_BOUND_V1, Z4_INFINITY_NORM_BOUND_V1},
};
/// Validate all proof components whose bounds are independently public.
///
/// This does not replace the ABDLOP commitment equation or the linear/norm
/// subprotocol checks. It is the fail-fast layer for `z1`, reconciliation
/// hints, `z3`, and `z4`.
///
/// # Errors
///
/// Returns the first fixed bound or hint-domain violation.
pub fn validate_public_response_bounds_v1(
    proof: &BootleLanternPresentationProofV1,
) -> Result<(), ResponseBoundErrorV1> {
    let z1_norm = squared_centered_norm(proof.z1())?;
    if z1_norm > u128::from(Z1_NORM_SQUARED_BOUND_V1) {
        return Err(ResponseBoundErrorV1::Z1NormExceeded);
    }
    for residue in proof.hint() {
        let centered = center_proof_residue_v1(*residue)
            .map_err(|_| ResponseBoundErrorV1::InternalInvariant)?;
        use_gamma_hint_v1(0, centered).map_err(|_| ResponseBoundErrorV1::HintOutOfRange)?;
    }
    let z3_norm = squared_centered_norm(proof.z3())?;
    if z3_norm > u128::from(Z3_NORM_SQUARED_BOUND_V1) {
        return Err(ResponseBoundErrorV1::Z3NormExceeded);
    }
    for residue in proof.z4() {
        let centered = center_proof_residue_v1(*residue)
            .map_err(|_| ResponseBoundErrorV1::InternalInvariant)?;
        if centered.unsigned_abs() > Z4_INFINITY_NORM_BOUND_V1 {
            return Err(ResponseBoundErrorV1::Z4InfinityNormExceeded);
        }
    }
    Ok(())
}
fn squared_centered_norm(residues: &[u64]) -> Result<u128, ResponseBoundErrorV1> {
    let mut norm = 0_u128;
    for residue in residues {
        let centered = center_proof_residue_v1(*residue)
            .map_err(|_| ResponseBoundErrorV1::InternalInvariant)?;
        let magnitude = u128::from(centered.unsigned_abs());
        norm = norm
            .checked_add(magnitude * magnitude)
            .ok_or(ResponseBoundErrorV1::InternalInvariant)?;
    }
    Ok(norm)
}
/// Public response-bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ResponseBoundErrorV1 {
    /// `z1` exceeded its theorem-derived squared-norm bound.
    #[error("Bootle/Lantern z1 squared norm exceeds its fixed bound")]
    Z1NormExceeded,
    /// A reconciliation hint was outside `(-m/2,m/2]`.
    #[error("Bootle/Lantern reconciliation hint is outside its fixed range")]
    HintOutOfRange,
    /// `z3` exceeded its fixed squared-norm bound.
    #[error("Bootle/Lantern z3 squared norm exceeds its fixed bound")]
    Z3NormExceeded,
    /// `z4` exceeded its fixed infinity-norm bound.
    #[error("Bootle/Lantern z4 infinity norm exceeds its fixed bound")]
    Z4InfinityNormExceeded,
    /// A decoded canonical-residue invariant unexpectedly failed.
    #[error("Bootle/Lantern response-bound internal invariant failed")]
    InternalInvariant,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::bootle_lantern::{
        codec::{
            CHALLENGE_POLYNOMIALS_V1, H_POLYNOMIALS_V1, HINT_POLYNOMIALS_V1, PROOF_COEFFICIENTS_V1,
            T_A1_POLYNOMIALS_V1, T_B_POLYNOMIALS_V1, Z1_POLYNOMIALS_V1, Z3_POLYNOMIALS_V1,
            Z21_POLYNOMIALS_V1,
        },
        compression::proof_residue_from_centered_v1,
        params::{APPLICATION_RING_DEGREE_V1, COMPRESSION_MODULUS_V1},
    };
    const T_B_START: usize = 0;
    const H_START: usize = T_B_START + T_B_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const T_A1_START: usize = H_START + H_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const CHALLENGE_START: usize = T_A1_START + T_A1_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const HINT_START: usize =
        CHALLENGE_START + CHALLENGE_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z1_START: usize = HINT_START + HINT_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z21_START: usize = Z1_START + Z1_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z3_START: usize = Z21_START + Z21_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    const Z4_START: usize = Z3_START + Z3_POLYNOMIALS_V1 * APPLICATION_RING_DEGREE_V1;
    fn proof_with(index: usize, centered: i64) -> BootleLanternPresentationProofV1 {
        let mut coefficients = vec![0_u64; PROOF_COEFFICIENTS_V1];
        coefficients[index] = proof_residue_from_centered_v1(centered);
        BootleLanternPresentationProofV1::from_coefficients(coefficients.into_boxed_slice())
            .expect("synthetic canonical proof")
    }
    #[test]
    fn zero_responses_and_every_exact_boundary_are_accepted() {
        let zero = proof_with(0, 0);
        validate_public_response_bounds_v1(&zero).expect("zero responses");
        let z1 = proof_with(Z1_START, 1_040_728_451);
        validate_public_response_bounds_v1(&z1).expect("z1 below exact bound");
        let hint = proof_with(
            HINT_START,
            i64::try_from(COMPRESSION_MODULUS_V1 / 2).expect("fits"),
        );
        validate_public_response_bounds_v1(&hint).expect("positive half hint");
        let z3 = proof_with(Z3_START, 10_661_920);
        validate_public_response_bounds_v1(&z3).expect("z3 below exact bound");
        let z4 = proof_with(
            Z4_START,
            i64::try_from(Z4_INFINITY_NORM_BOUND_V1).expect("fits"),
        );
        validate_public_response_bounds_v1(&z4).expect("z4 exact bound");
    }
    #[test]
    fn one_coefficient_over_each_bound_fails_closed() {
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(Z1_START, 1_040_728_452)),
            Err(ResponseBoundErrorV1::Z1NormExceeded)
        );
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(
                HINT_START,
                -i64::try_from(COMPRESSION_MODULUS_V1 / 2).expect("fits")
            )),
            Err(ResponseBoundErrorV1::HintOutOfRange)
        );
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(
                HINT_START,
                i64::try_from(COMPRESSION_MODULUS_V1 / 2 + 1).expect("fits")
            )),
            Err(ResponseBoundErrorV1::HintOutOfRange)
        );
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(Z3_START, 10_661_921)),
            Err(ResponseBoundErrorV1::Z3NormExceeded)
        );
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(
                Z4_START,
                i64::try_from(Z4_INFINITY_NORM_BOUND_V1 + 1).expect("fits")
            )),
            Err(ResponseBoundErrorV1::Z4InfinityNormExceeded)
        );
        assert_eq!(
            validate_public_response_bounds_v1(&proof_with(
                Z4_START,
                -i64::try_from(Z4_INFINITY_NORM_BOUND_V1 + 1).expect("fits")
            )),
            Err(ResponseBoundErrorV1::Z4InfinityNormExceeded)
        );
    }
    #[test]
    fn multi_coefficient_norm_overflow_is_detected_not_just_linf() {
        let mut coefficients = vec![0_u64; PROOF_COEFFICIENTS_V1];
        coefficients[Z3_START] = proof_residue_from_centered_v1(8_000_000);
        coefficients[Z3_START + 1] = proof_residue_from_centered_v1(8_000_000);
        let proof =
            BootleLanternPresentationProofV1::from_coefficients(coefficients.into_boxed_slice())
                .expect("canonical proof");
        assert_eq!(
            validate_public_response_bounds_v1(&proof),
            Err(ResponseBoundErrorV1::Z3NormExceeded)
        );
    }
}
