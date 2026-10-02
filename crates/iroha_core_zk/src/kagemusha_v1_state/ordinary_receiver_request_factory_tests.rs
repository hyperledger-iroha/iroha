//! Real cryptography over explicitly synthetic model originals; no Native owner is constructed.
use super::*;
use iroha_data_model::{
    kagemusha::KagemushaAppOperationApprovalEvidenceV1,
    testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
};
use p256::ecdsa::{SigningKey, signature::Signer as _};

fn body(
    enrolled: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
) -> KagemushaOrdinaryPaymentRequestBodyV1 {
    let subject = enrolled.app_credential().subject();
    let runtime = &enrolled.certificate().subject.owner.runtime;
    KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: subject.release_id,
        network_id: subject.network_id,
        normalized_asset_id: kagemusha_asset_identity_digest_v1(&runtime.asset).unwrap(),
        asset_incarnation: *runtime.asset_incarnation.as_bytes(),
        scale: runtime.scale,
        reserve_pool_id: [31; 32],
        recipient_account_binding: subject.account_binding,
        amount: 17,
        recipient_encryption_key: kagemusha_x25519_public_key_v1(&[32; 32]).unwrap(),
        recipient_credential_digest: enrolled.app_credential().digest(),
        recipient_lane_id: subject.lane_id,
        request_id: [33; 32],
        clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [34; 32],
            signed_observations_original_digest: [35; 32],
            lower_at_ms: 400,
            upper_at_ms: 401,
        },
        issued_at_ms: 400,
        expires_at_ms: 900,
    }
}
fn signed(body: KagemushaOrdinaryPaymentRequestBodyV1, seed: u8) -> Vec<u8> {
    let key = SigningKey::from_slice(&[seed; 32]).unwrap();
    let signature: p256::ecdsa::Signature = key.sign(&body.canonical_signing_bytes().unwrap());
    KagemushaOrdinaryPaymentRequestV1 {
        body,
        evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        },
    }
    .canonical_bytes()
    .unwrap()
}
#[test]
fn exact_reserved_request_refuses_new_validly_signed_body() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let original = body(&enrolled);
    assert_eq!(
        authenticate_selected_request_data(
            &original,
            &signed(original, 7),
            enrolled.app_credential(),
            None
        )
        .unwrap(),
        None
    );
    for change in 0..7 {
        let mut changed = original;
        match change {
            0 => changed.amount += 1,
            1 => changed.request_id[0] ^= 1,
            2 => {
                changed.recipient_encryption_key =
                    kagemusha_x25519_public_key_v1(&[36; 32]).unwrap()
            }
            3 => changed.clock_context.request_nonce[0] ^= 1,
            4 => changed.clock_context.signed_observations_original_digest[0] ^= 1,
            5 => changed.clock_context.upper_at_ms += 1,
            _ => changed.expires_at_ms += 1,
        }
        // This same fixture key genuinely signs the substituted body; equality must still refuse it.
        assert!(
            authenticate_selected_request_data(
                &original,
                &signed(changed, 7),
                enrolled.app_credential(),
                None
            )
            .is_err()
        );
    }
}
#[test]
fn request_original_refuses_wrong_platform_floor_signer_and_trailing_bytes() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let original = body(&enrolled);
    let raw = signed(original, 7);
    assert!(
        authenticate_selected_request_data(&original, &raw, enrolled.app_credential(), Some(0))
            .is_err()
    );
    assert!(
        authenticate_selected_request_data(
            &original,
            &signed(original, 8),
            enrolled.app_credential(),
            None
        )
        .is_err()
    );
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(
        authenticate_selected_request_data(&original, &trailing, enrolled.app_credential(), None)
            .is_err()
    );
    let mut evidence = KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&raw).unwrap();
    match &mut evidence.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            let last = signature_der.len() - 1;
            signature_der[last] ^= 1;
        }
        _ => unreachable!(),
    }
    assert!(
        authenticate_selected_request_data(
            &original,
            &evidence.canonical_bytes().unwrap(),
            enrolled.app_credential(),
            None
        )
        .is_err()
    );
}
#[test]
fn private_reservation_roundtrip_retains_same_key_and_full_originals_as_data() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let record = ReceiverRequestOriginals {
        publication_originals: [[37; 32]; 8],
        predecessor: [38; 32],
        private_key: ReceiverPrivateKey([32; 32]),
        body: body(&enrolled),
        fi_original: enrolled.certificate().canonical_bytes().unwrap(),
        credential_original: enrolled.app_credential().original().to_vec(),
        possession_original: enrolled.possession().original().to_vec(),
        integrity_lease_original: None,
        previous_app_attest_counter: None,
    };
    let bytes = Zeroizing::new(norito::to_bytes(&record).unwrap());
    let decoded: ReceiverRequestOriginals = norito::decode_from_bytes(&bytes).unwrap();
    assert!(record == decoded);
    assert_eq!(
        kagemusha_x25519_public_key_v1(&decoded.private_key.0).unwrap(),
        decoded.body.recipient_encryption_key
    );
    assert_eq!(
        record.body.canonical_signing_bytes().unwrap(),
        decoded.body.canonical_signing_bytes().unwrap()
    );
    // No Native owner, fsync token, consumption grant or balance is constructed by this decode.
}

fn reservation(
    enrolled: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
) -> ReceiverRequestOriginals {
    ReceiverRequestOriginals {
        publication_originals: [[37; 32]; 8],
        predecessor: [38; 32],
        private_key: ReceiverPrivateKey([32; 32]),
        body: body(enrolled),
        fi_original: enrolled.certificate().canonical_bytes().unwrap(),
        credential_original: enrolled.app_credential().original().to_vec(),
        possession_original: enrolled.possession().original().to_vec(),
        integrity_lease_original: None,
        previous_app_attest_counter: None,
    }
}
fn capture(
    enrolled: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
) -> CapturedReceiverRequestOriginals {
    let reservation = reservation(enrolled);
    let original = signed(reservation.body, 7);
    CapturedReceiverRequestOriginals {
        reservation,
        signed_request_original: original,
        signature_admission_clock: KagemushaOrdinaryCashClockContextV1 {
            lower_at_ms: 402,
            upper_at_ms: 403,
            ..body(enrolled).clock_context
        },
        accepted_app_attest_counter: None,
    }
}
#[test]
fn captured_request_checks_original_signature_clock_window_and_counter_data() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let captured = capture(&enrolled);
    captured.recheck_capture_data(&enrolled, None).unwrap();
    for change in 0..7 {
        let mut altered = captured.clone();
        match change {
            0 => altered.signature_admission_clock.lower_at_ms = 399,
            1 => altered.signature_admission_clock.upper_at_ms = 400,
            2 => altered.signature_admission_clock.upper_at_ms = 900,
            3 => altered.accepted_app_attest_counter = Some(1),
            4 => altered.signed_request_original.push(0),
            5 => {
                let mut new = altered.reservation.body;
                new.amount += 1;
                altered.signed_request_original = signed(new, 7);
            }
            _ => {
                altered
                    .signature_admission_clock
                    .signed_observations_original_digest = [0; 32]
            }
        }
        assert!(altered.recheck_capture_data(&enrolled, None).is_err());
    }
}
#[test]
fn same_main_fence_guards_forbid_wrong_id_dispatch_retry_or_fenced_cancel() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let mut pending = super::super::PendingReceiverRequest {
        originals: reservation(&enrolled),
        financial_control: super::super::CapturedFinancialControlIdentity {
            original_sha256: [50; 32],
            lower_ms: 399,
            upper_ms: 400,
        },
        lease: None,
        fenced: false,
    };
    let id = pending.originals.request_id();
    pending.require_unfenced(id).unwrap();
    assert!(pending.require_fenced(id).is_err());
    assert!(pending.require_unfenced([51; 32]).is_err());
    pending.fenced = true;
    pending.require_fenced(id).unwrap();
    assert!(pending.require_unfenced(id).is_err());
    assert!(pending.require_fenced([51; 32]).is_err());
    // This privately constructed synthetic data is not a Native holder or a durable fence.
}
#[test]
fn receiver_capture_roundtrip_and_capacity_charge_hold_full_secret_sources_as_private_data() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrolled = f.verify(400).unwrap();
    let captured = capture(&enrolled);
    let bytes = Zeroizing::new(norito::encode_canonical(&captured).unwrap());
    let decoded: CapturedReceiverRequestOriginals = norito::decode_canonical(&bytes).unwrap();
    assert!(decoded == captured);
    decoded.recheck_capture_data(&enrolled, None).unwrap();
    let charge = decoded.reservation.capacity_charge_bytes().unwrap();
    assert!(charge >= bytes.len() as u64);
    let record = super::super::Record::ReceiverCapture(decoded);
    let maximum_payload_bytes =
        u64::try_from(norito::canonical_frame_len(&record).unwrap()).unwrap();
    let frame = super::super::encode(&record, maximum_payload_bytes).unwrap();
    assert_eq!(frame.len() as u64, maximum_payload_bytes);
    assert_eq!(
        super::super::decode(&frame, maximum_payload_bytes).unwrap(),
        record
    );
    assert!(super::super::encode(&record, maximum_payload_bytes - 1).is_err());
    assert!(super::super::decode(&frame, maximum_payload_bytes - 1).is_err());
    assert!(!format!("{record:?}").contains("private_key"));
}
