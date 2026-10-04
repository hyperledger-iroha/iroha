//! Exact initialized dealer coefficients without retained or cloning heap storage.

use super::*;
use crate::test_allocations::without_allocations;

// Independently preserve the previous parsed-vector equation and error sequence.
fn reference<P: ThresholdBlsPurpose>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    index: u16,
    bytes: &[[u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES]],
    commitment: [u8; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
    response: [u8; 32],
) -> Result<(), ThresholdBlsError> {
    validate_participant_index(parameters.session(), index)?;
    if bytes.len() != usize::from(parameters.session().threshold()) {
        return Err(ThresholdBlsError::InvalidCoefficientCommitment);
    }
    let coefficients = bytes
        .iter()
        .map(|bytes| DasRenCoefficientCommitment::from_bytes(parameters, *bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let proof = DasRenSchnorrPok::<P>::from_bytes(commitment, response)?;
    let challenge = dealer_pok_challenge(parameters, index, &coefficients, &proof)?;
    let lhs = (G2Projective::generator() * decode_scalar(&response)?).to_affine();
    let rhs = (G2Projective::from(decode_g2(&commitment)?)
        + G2Projective::from(coefficients[0].point()?) * challenge)
        .to_affine();
    if lhs == rhs {
        Ok(())
    } else {
        Err(ThresholdBlsError::InvalidDealerProof)
    }
}

#[test]
fn initialized_dealer_coefficients_verify_and_clone_without_allocations_at_both_bounds() {
    fn check<P: ThresholdBlsPurpose>() {
        for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
            let session = ThresholdBlsSession::<P>::new(
                binding(1),
                binding(2),
                binding(3),
                n,
                (n - 1) / 3 + 1,
            )
            .unwrap();
            let parameters = AdaptiveThresholdBlsParameters::derive(&session).unwrap();
            let mut rng = ChaCha20Rng::from_seed([0x6d; 32]);
            let (_, original) =
                DasRenDealerSecret::generate_with_rng(&parameters, n, &mut rng).unwrap();
            let bytes: Vec<_> = original
                .coefficients()
                .iter()
                .map(|c| *c.as_bytes())
                .collect();
            let proof = original.constant_proof();
            reference(
                &parameters,
                n,
                &bytes,
                *proof.commitment_bytes(),
                *proof.response_bytes(),
            )
            .unwrap();
            let verified = without_allocations(|| {
                DasRenDealerCommitment::verify(
                    &parameters,
                    n,
                    &bytes,
                    *proof.commitment_bytes(),
                    *proof.response_bytes(),
                )
            })
            .unwrap();
            assert_eq!(verified, original);
            assert_eq!(
                verified.coefficients().len(),
                usize::from(session.threshold())
            );
            assert_eq!(verified.coefficients.capacity(), MAX_DEALER_COEFFICIENTS_V1);
            let cloned = without_allocations(|| verified.clone());
            assert_eq!(cloned, verified);
            assert_ne!(
                cloned.coefficients().as_ptr(),
                verified.coefficients().as_ptr()
            );
            assert_eq!(
                cloned
                    .coefficients()
                    .iter()
                    .map(|c| *c.as_bytes())
                    .collect::<Vec<_>>(),
                bytes
            );
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn inline_dealer_verification_preserves_index_length_point_scalar_and_proof_precedence() {
    fn check<P: ThresholdBlsPurpose>() {
        let parameters = AdaptiveThresholdBlsParameters::derive(&session::<P>()).unwrap();
        let mut rng = ChaCha20Rng::from_seed([0x6e; 32]);
        let (_, original) =
            DasRenDealerSecret::generate_with_rng(&parameters, 1, &mut rng).unwrap();
        let bytes: Vec<_> = original
            .coefficients()
            .iter()
            .map(|c| *c.as_bytes())
            .collect();
        let proof = original.constant_proof();
        let commitment = *proof.commitment_bytes();
        let response = *proof.response_bytes();
        let compare = |index, bytes: &[[u8; 96]], commitment, response| {
            let expected = reference(&parameters, index, bytes, commitment, response);
            let actual = without_allocations(|| {
                DasRenDealerCommitment::verify(&parameters, index, bytes, commitment, response)
            })
            .map(|_| ());
            assert_eq!(actual, expected);
            actual
        };
        assert!(compare(0, &[], [0; 96], [0xff; 32]).is_err());
        assert_eq!(
            compare(1, &bytes[..1], [0; 96], [0xff; 32]),
            Err(ThresholdBlsError::InvalidCoefficientCommitment)
        );
        let mut malformed = bytes.clone();
        malformed[0] = [0; 96];
        assert_eq!(
            compare(1, &malformed, [0; 96], [0xff; 32]),
            Err(ThresholdBlsError::InvalidCoefficientCommitment)
        );
        assert_eq!(
            compare(1, &bytes, [0; 96], [0xff; 32]),
            Err(ThresholdBlsError::InvalidDealerProof)
        );
        assert_eq!(
            compare(1, &bytes, commitment, [0xff; 32]),
            Err(ThresholdBlsError::InvalidScalar)
        );
        let changed = (decode_scalar(&response).unwrap() + Scalar::from(1)).to_bytes_be();
        assert_eq!(
            compare(1, &bytes, commitment, changed),
            Err(ThresholdBlsError::InvalidDealerProof)
        );
        compare(1, &bytes, commitment, response).unwrap();
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}
