//! Fixed pairing scratch checked against the original independently prepared relation.

use super::*;
use crate::test_allocations::without_allocations;
use blstrs::G2Prepared;
use pairing::{MillerLoopResult as _, MultiMillerLoop as _};

// This intentionally retains the former allocating relation only in tests.
// The reference uses the complete serialized signing message, not the borrowed
// message point or either one-point loop from the production implementation.
fn prepared_reference<P: ThresholdBlsPurpose>(
    session: &ThresholdBlsSession<P>,
    payload: &[u8],
    key: &ThresholdBlsPublicKey<P>,
    signature: &ThresholdBlsSignature<P>,
) -> Result<(), ThresholdBlsError> {
    if key.session_id != *session.session_id() || signature.session_id != *session.session_id() {
        return Err(ThresholdBlsError::SessionMismatch);
    }
    let message = session.signing_message(payload)?;
    let public_key = decode_g2(&key.bytes)?;
    let signature = decode_g1(&signature.bytes)?;
    let message_point = G1Projective::hash_to_curve(&message, P::SIGNATURE_DST, &[]);
    let generator = G2Prepared::from(G2Affine::generator());
    let public_key = G2Prepared::from(public_key);
    let negated = (-message_point).to_affine();
    let relation =
        blstrs::Bls12::multi_miller_loop(&[(&signature, &generator), (&negated, &public_key)])
            .final_exponentiation();
    if bool::from(relation.is_identity()) {
        Ok(())
    } else {
        Err(ThresholdBlsError::SignatureMismatch)
    }
}

fn compare<P: ThresholdBlsPurpose>(
    session: &ThresholdBlsSession<P>,
    payload: &[u8],
    key: &ThresholdBlsPublicKey<P>,
    signature: &ThresholdBlsSignature<P>,
) -> Result<(), ThresholdBlsError> {
    let expected = prepared_reference(session, payload, key, signature);
    let actual = without_allocations(|| key.verify_payload(session, payload, signature));
    assert_eq!(actual, expected);
    actual
}

#[test]
fn fixed_pairing_matches_prepared_reference_for_both_roles_and_canonical_failures() {
    fn check<P: ThresholdBlsPurpose>() {
        let session = session::<P>();
        for length in [0, 1, 64, THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1] {
            let payload: Vec<_> = (0..length)
                .map(|i| u8::try_from(i % 251).unwrap())
                .collect();
            for scalar in [1, 19, 0xffff_ffff] {
                let public_key = key(&session, scalar);
                let signature = sign(&session, scalar, &payload);
                compare(&session, &payload, &public_key, &signature).unwrap();
                assert_eq!(
                    compare(&session, b"different payload", &public_key, &signature),
                    Err(ThresholdBlsError::SignatureMismatch)
                );
                assert_eq!(
                    compare(&session, &payload, &key(&session, scalar + 1), &signature),
                    Err(ThresholdBlsError::SignatureMismatch)
                );
            }
        }
        let public_key = key(&session, 7);
        let signature = sign(&session, 7, b"canonical");
        for bytes in [
            [0; THRESHOLD_BLS_PUBLIC_KEY_BYTES],
            G2Affine::identity().to_compressed(),
        ] {
            let malformed = ThresholdBlsPublicKey {
                bytes,
                ..public_key
            };
            assert_eq!(
                compare(&session, b"canonical", &malformed, &signature),
                Err(ThresholdBlsError::InvalidPublicKey)
            );
        }
        for bytes in [
            [0; THRESHOLD_BLS_SIGNATURE_BYTES],
            G1Affine::identity().to_compressed(),
        ] {
            let malformed = ThresholdBlsSignature { bytes, ..signature };
            assert_eq!(
                compare(&session, b"canonical", &public_key, &malformed),
                Err(ThresholdBlsError::InvalidSignature)
            );
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

fn transcript_and_partials<P: ThresholdBlsPurpose>(
    payload: &[u8],
) -> (
    AdaptiveThresholdBlsPublicTranscript<P>,
    [DasRenPartialSignature<P>; 4],
) {
    let parameters = AdaptiveThresholdBlsParameters::derive(&session::<P>()).unwrap();
    let mut rng = ChaCha20Rng::from_seed([0x7b; 32]);
    let dealers: Vec<_> = (1..=3_u16)
        .map(|index| {
            let offset = u64::from(index);
            let coefficients = Zeroizing::new(vec![
                [Scalar::from(10 + offset).to_bytes_be(), [0; 32], [0; 32]],
                [
                    Scalar::from(20 + offset).to_bytes_be(),
                    Scalar::from(30 + offset).to_bytes_be(),
                    Scalar::from(40 + offset).to_bytes_be(),
                ],
            ]);
            let (secret, dealer) = DasRenDealerSecret::from_coefficients_with_rng(
                &parameters,
                index,
                coefficients,
                &mut rng,
            )
            .unwrap();
            drop(secret);
            dealer
        })
        .collect();
    let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
        &parameters,
        &dealers,
        &[1, 2, 3],
        binding(90),
    )
    .unwrap();
    let partials = core::array::from_fn(|offset| {
        let index = u16::try_from(offset + 1).unwrap();
        let x = Scalar::from(u64::from(index));
        let secret = AdaptiveThresholdBlsSecretShare::from_components(
            &transcript,
            index,
            (Scalar::from(36_u64) + Scalar::from(66_u64) * x).to_bytes_be(),
            (Scalar::from(96_u64) * x).to_bytes_be(),
            (Scalar::from(126_u64) * x).to_bytes_be(),
        )
        .unwrap();
        secret
            .sign_payload_with_rng(&transcript, payload, &mut rng)
            .unwrap()
    });
    (transcript, partials)
}

#[test]
fn complete_threshold_verification_and_combination_allocate_no_heap_for_either_role() {
    fn check<P: ThresholdBlsPurpose>() {
        let payload = b"full verifier and final pairing allocation boundary";
        let (transcript, partials) = transcript_and_partials::<P>(payload);
        let expected = sign(transcript.session(), 36, payload);
        prepared_reference(
            transcript.session(),
            payload,
            transcript.group_public_key(),
            &expected,
        )
        .unwrap();
        let mut checked = 0;
        for mask in 1_u8..16 {
            if mask.count_ones() < u32::from(transcript.session().threshold()) {
                continue;
            }
            let subset: Vec<_> = partials
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, partial)| *partial)
                .collect();
            let combined = without_allocations(|| {
                for partial in &subset {
                    transcript
                        .verify_partial_signature(payload, partial)
                        .unwrap();
                }
                let combined = transcript
                    .combine_partial_signatures(payload, &subset)
                    .unwrap();
                transcript
                    .verify_final_signature(payload, &combined)
                    .unwrap();
                combined
            });
            assert_eq!(combined.as_bytes(), expected.as_bytes());
            assert_eq!(
                without_allocations(|| transcript.combine_partial_signatures(b"wrong", &subset)),
                Err(ThresholdBlsError::InvalidPartialSignatureProof)
            );
            checked += 1;
        }
        assert_eq!(
            checked, 11,
            "all accepted subsets remain allocation-free end to end"
        );
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn verified_beacon_seed_and_pairing_error_precedence_use_fixed_scratch() {
    fn check<P: ThresholdBlsPurpose>() {
        let session = session::<P>();
        let mut public_key = key(&session, 7);
        let mut signature = sign(&session, 7, b"valid");
        let oversized = vec![0; THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1 + 1];
        public_key.bytes = [0; THRESHOLD_BLS_PUBLIC_KEY_BYTES];
        signature.bytes = [0; THRESHOLD_BLS_SIGNATURE_BYTES];
        public_key.session_id = binding(88);
        assert_eq!(
            compare(&session, &oversized, &public_key, &signature),
            Err(ThresholdBlsError::SessionMismatch)
        );
        public_key.session_id = *session.session_id();
        signature.session_id = binding(88);
        assert_eq!(
            compare(&session, &oversized, &public_key, &signature),
            Err(ThresholdBlsError::SessionMismatch)
        );
        signature.session_id = *session.session_id();
        assert_eq!(
            compare(&session, &oversized, &public_key, &signature),
            Err(ThresholdBlsError::MessageTooLarge)
        );
        assert_eq!(
            compare(&session, b"valid", &public_key, &signature),
            Err(ThresholdBlsError::InvalidPublicKey)
        );
        public_key = key(&session, 7);
        assert_eq!(
            compare(&session, b"valid", &public_key, &signature),
            Err(ThresholdBlsError::InvalidSignature)
        );
        let (transcript, partials) = transcript_and_partials::<P>(b"valid");
        let mut invalid = partials[0];
        invalid.z_s = [0xff; 32];
        assert_eq!(
            without_allocations(|| transcript.combine_partial_signatures(b"valid", &[invalid])),
            Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
        );
        assert_eq!(
            without_allocations(
                || transcript.combine_partial_signatures(b"valid", &[invalid, partials[1]])
            ),
            Err(ThresholdBlsError::InvalidScalar)
        );
        invalid.sigma = [0; THRESHOLD_BLS_SIGNATURE_BYTES];
        assert_eq!(
            without_allocations(
                || transcript.combine_partial_signatures(b"valid", &[invalid, partials[1]])
            ),
            Err(ThresholdBlsError::InvalidSignature)
        );
    }
    let payload = b"verified seed with fixed final pairing scratch";
    let (transcript, partials) = transcript_and_partials::<BeaconPurpose>(payload);
    let expected_signature = sign(transcript.session(), 36, payload);
    let expected_seed = derive_beacon_seed(
        transcript.session(),
        transcript.transcript_hash(),
        payload,
        &expected_signature,
    )
    .unwrap();
    let actual = without_allocations(|| {
        let signature = transcript
            .combine_partial_signatures(payload, &partials[..2])
            .unwrap();
        transcript.finalized_seed(payload, &signature).unwrap()
    });
    assert_eq!(actual, expected_seed);
    assert_eq!(
        without_allocations(|| transcript.finalized_seed(b"wrong", &expected_signature)),
        Err(ThresholdBlsError::SignatureMismatch)
    );

    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}
