//! Borrowed threshold signing preimages retain the exact canonical point and seed bytes.

use super::*;
use crate::test_allocations::without_allocations;

fn serialized<P: ThresholdBlsPurpose>(session: &ThresholdBlsSession<P>, payload: &[u8]) -> Vec<u8> {
    let mut bytes = MESSAGE_DOMAIN_V1.to_vec();
    bytes.extend_from_slice(SESSION_DOMAIN_V1);
    bytes.extend_from_slice(&THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    bytes.push(P::ROLE_TAG);
    bytes.extend_from_slice(session.network_id());
    bytes.extend_from_slice(session.session_id());
    bytes.extend_from_slice(session.roster_hash());
    bytes.extend_from_slice(&session.committee_size().to_be_bytes());
    bytes.extend_from_slice(&session.threshold().to_be_bytes());
    bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn borrowed_message_matches_serialized_points_and_digest_for_both_roles_and_payload_bounds() {
    fn check<P: ThresholdBlsPurpose>() {
        let session = session::<P>();
        for length in [
            0,
            1,
            31,
            32,
            63,
            64,
            65,
            255,
            THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1,
        ] {
            let payload: Vec<_> = (0..length).map(|offset| (offset % 251) as u8).collect();
            let expected = serialized(&session, &payload);
            let mut role_bound = vec![P::ROLE_TAG];
            role_bound.extend_from_slice(&expected);
            let expected_h0 =
                G1Projective::hash_to_curve(&expected, P::SIGNATURE_DST, &[]).to_affine();
            let expected_h1 =
                G1Projective::hash_to_curve(&role_bound, PARTIAL_H1_DST_V1, &[]).to_affine();
            let expected_digest: [u8; 32] = Sha256::digest(&expected).into();
            let (h0, h1, digest) = without_allocations(|| {
                let message = session.borrowed_signing_message(&payload).unwrap();
                assert_eq!(message.payload.as_ptr(), payload.as_ptr());
                assert_eq!(
                    message.prefix.as_slice(),
                    &expected[..MESSAGE_PREFIX_BYTES_V1]
                );
                (message.h0(), message.h1(), message.digest())
            });
            assert_eq!(h0, expected_h0);
            assert_eq!(h1, expected_h1);
            assert_eq!(digest, expected_digest);
            assert_eq!(session.signing_message(&payload).unwrap(), expected);
        }
        let oversized = vec![0; THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1 + 1];
        without_allocations(|| {
            assert!(matches!(
                session.borrowed_signing_message(&oversized),
                Err(ThresholdBlsError::MessageTooLarge)
            ));
            assert_eq!(
                session.signing_message(&oversized),
                Err(ThresholdBlsError::MessageTooLarge)
            );
        });
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn actual_partial_sign_verify_and_beacon_seed_use_no_message_backing() {
    let fixture = adaptive_fixture();
    let secret = adaptive_secret(&fixture, 1);
    let payload = b"borrowed-message-real-proof";
    let mut rng = ChaCha20Rng::from_seed([0x53; 32]);
    let partial = without_allocations(|| {
        secret.sign_payload_with_rng(&fixture.transcript, payload, &mut rng)
    })
    .unwrap();
    without_allocations(|| {
        fixture
            .transcript
            .verify_partial_signature(payload, &partial)
    })
    .unwrap();

    let signature = sign(fixture.transcript.session(), 36, payload);
    let message = serialized(fixture.transcript.session(), payload);
    let mut salt = Sha256::new();
    salt.update(BEACON_SEED_SALT_V1);
    salt.update(fixture.transcript.transcript_hash());
    salt.update(Sha256::digest(&message));
    let salt: [u8; 32] = salt.finalize().into();
    let mut expected = [0; 32];
    Hkdf::<Sha256>::new(Some(&salt), signature.as_bytes())
        .expand(BEACON_SEED_INFO_V1, &mut expected)
        .unwrap();
    let seed = without_allocations(|| {
        derive_beacon_seed(
            fixture.transcript.session(),
            fixture.transcript.transcript_hash(),
            payload,
            &signature,
        )
    })
    .unwrap();
    assert_eq!(seed, expected);
}

#[test]
fn borrowed_message_preserves_partial_rejection_precedence() {
    let fixture = adaptive_fixture();
    let secret = adaptive_secret(&fixture, 1);
    let mut rng = ChaCha20Rng::from_seed([0x54; 32]);
    let partial = secret
        .sign_payload_with_rng(&fixture.transcript, b"valid", &mut rng)
        .unwrap();
    let oversized = vec![0; THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1 + 1];
    let mut wrong = partial.clone();
    wrong.session_id = binding(87);
    assert_eq!(
        fixture
            .transcript
            .verify_partial_signature(&oversized, &wrong),
        Err(ThresholdBlsError::SessionMismatch)
    );
    wrong = partial.clone();
    wrong.index = 0;
    assert_eq!(
        fixture
            .transcript
            .verify_partial_signature(&oversized, &wrong),
        Err(ThresholdBlsError::UnknownParticipant)
    );
    wrong = partial;
    wrong.sigma = [0; THRESHOLD_BLS_SIGNATURE_BYTES];
    assert_eq!(
        fixture
            .transcript
            .verify_partial_signature(&oversized, &wrong),
        Err(ThresholdBlsError::MessageTooLarge)
    );
    assert_eq!(
        fixture
            .transcript
            .verify_partial_signature(b"valid", &wrong),
        Err(ThresholdBlsError::InvalidSignature)
    );
}
