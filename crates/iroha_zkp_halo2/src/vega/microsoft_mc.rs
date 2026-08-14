//! First-party Microsoft Vega-MC compatibility implementation.
use super::{
    VegaMdlProofDimensionsV1, VegaT256ScalarV1 as Scalar,
    engine::VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
};
type ValidatedFixture = (
    [u8; 32],
    VegaMdlProofDimensionsV1,
    Vec<Vec<Scalar>>,
    Vec<Scalar>,
);
#[path = "microsoft_mc/sha256.rs"]
mod sha256;
#[cfg(test)]
/// Hash bounded test input with the crate-owned dependency-free implementation.
pub(super) fn dependency_free_sha256_for_tests(input: &[u8]) -> [u8; 32] {
    sha256::sha256(input).expect("bounded unit-test input")
}
#[path = "microsoft_mc/verifier_key.rs"]
mod verifier_key;
#[path = "microsoft_mc/verify.rs"]
mod verify;
#[path = "microsoft_mc/wire.rs"]
mod wire;
/// Return the released Figure 9 dimensions independently of runtime setup.
///
/// These values were derived from the canonical Microsoft verifier key and
/// are governed by the compiled-profile manifest. Keeping the owned values
/// here makes proof admission deterministic and lockfile-independent.
pub(super) fn canonical_figure9_dimensions() -> VegaMdlProofDimensionsV1 {
    let mut verifier_challenges_per_round = vec![1; 47];
    for index in [3, 44, 45, 46] {
        verifier_challenges_per_round[index] = 0;
    }
    VegaMdlProofDimensionsV1 {
        num_steps: 8,
        shared_variables: 524_288,
        step_precommitted_variables: 2_048,
        step_rest_variables: 522_240,
        core_precommitted_variables: 2_048,
        core_rest_variables: 522_240,
        step_constraints: 262_144,
        step_variables: 1_048_576,
        core_constraints: 262_144,
        core_variables: 1_048_576,
        shared_commitment_points: 256,
        step_precommitted_points: 1,
        step_rest_points: 255,
        step_public_values: 1,
        step_challenges: 0,
        core_precommitted_points: 1,
        core_rest_points: 255,
        core_public_values: 18,
        core_challenges: 0,
        evaluation_response_scalars: 2_048,
        verifier_round_commitment_points: vec![1; 47],
        verifier_public_values: 6,
        verifier_challenges_per_round,
        nova_cross_term_points: 16,
        random_witness_commitment_points: 47,
        random_error_commitment_points: 16,
        random_public_values: 49,
        verifier_constraints: 512,
        verifier_variables: 1_504,
        relaxed_outer_rounds: 9,
        relaxed_outer_coefficients: 3,
        relaxed_inner_rounds: 12,
        relaxed_inner_coefficients: 2,
        relaxed_opening_scalars: 32,
    }
}
/// Return the governed digest of the canonical Figure 9 Microsoft key.
pub(super) const fn canonical_figure9_verifier_digest() -> [u8; 32] {
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
}
/// Validate only the canonical structure of one released Figure 9 proof.
///
/// This does not verify equations without the governed Figure 9 verifier key;
/// callers must never treat success here as proof acceptance.
pub(super) fn scan_canonical_figure9_proof(proof: &[u8]) -> Result<(), wire::McCodecError> {
    let decoded = wire::McProofWire::decode(proof, &canonical_figure9_dimensions())?;
    if decoded.encode()? != proof {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    Ok(())
}
/// Decode, re-encode, and verify an independent Microsoft fixture pair.
pub(super) fn validate_fixture(
    verifier_key: &[u8],
    proof: &[u8],
) -> Result<ValidatedFixture, wire::McCodecError> {
    let key = verifier_key::McVerifierKeyWire::decode(verifier_key)?;
    if key.encode()? != verifier_key {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    let dimensions = key.proof_dimensions()?;
    let decoded = wire::McProofWire::decode(proof, &dimensions)?;
    if decoded.encode()? != proof {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    let (steps, core) = verify::verify(&decoded, &key, dimensions.num_steps)?;
    Ok((key.digest()?, dimensions, steps, core))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn governed_figure9_dimensions_match_the_compiled_profile() {
        let dimensions = canonical_figure9_dimensions();
        assert_eq!(dimensions.num_steps, 8);
        assert_eq!(dimensions.shared_variables, 524_288);
        assert_eq!(dimensions.verifier_round_commitment_points, [1; 47]);
        assert_eq!(dimensions.verifier_challenges_per_round[3], 0);
        assert_eq!(dimensions.verifier_challenges_per_round[44..], [0; 3]);
        assert_eq!(dimensions.relaxed_outer_rounds, 9);
        assert_eq!(dimensions.relaxed_inner_rounds, 12);
        assert_eq!(
            canonical_figure9_verifier_digest(),
            VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
        );
    }
}
