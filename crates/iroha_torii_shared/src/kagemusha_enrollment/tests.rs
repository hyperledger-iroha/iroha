//! Framing/shape tests only; synthetic bytes are never treated as signed native originals.
use super::*;
#[test]
fn request_actions_roundtrip_exactly_with_full_original_caps() {
    for action in [
        EnrollmentServiceActionV1::PreKey,
        EnrollmentServiceActionV1::Evidence,
        EnrollmentServiceActionV1::Issue,
        EnrollmentServiceActionV1::Deliver,
    ] {
        let value = EnrollmentServiceRequestV1 {
            version: 1,
            action,
            dispatch_original: vec![1; ENROLLMENT_DISPATCH_MAX_BYTES_V1],
            evidence_original: if action == EnrollmentServiceActionV1::Evidence {
                vec![2; ENROLLMENT_EVIDENCE_REQUEST_MAX_BYTES_V1]
            } else {
                vec![]
            },
        };
        let original = value.canonical_wire().unwrap();
        assert!(original.len() <= ENROLLMENT_SERVICE_REQUEST_MAX_BYTES_V1);
        assert_eq!(
            EnrollmentServiceRequestV1::decode_canonical(&original).unwrap(),
            value
        );
        let mut trailing = original;
        trailing.push(0);
        assert!(EnrollmentServiceRequestV1::decode_canonical(&trailing).is_err());
    }
}
#[test]
fn request_rejects_cross_action_inputs_versions_and_overflow() {
    let original = EnrollmentServiceRequestV1 {
        version: 1,
        action: EnrollmentServiceActionV1::PreKey,
        dispatch_original: vec![1],
        evidence_original: vec![],
    };
    for mutation in 0..6 {
        let mut bad = original.clone();
        match mutation {
            0 => bad.version = 2,
            1 => bad.dispatch_original.clear(),
            2 => bad.dispatch_original = vec![1; ENROLLMENT_DISPATCH_MAX_BYTES_V1 + 1],
            3 => bad.evidence_original = vec![1],
            4 => bad.action = EnrollmentServiceActionV1::Evidence,
            _ => {
                bad.action = EnrollmentServiceActionV1::Evidence;
                bad.evidence_original = vec![1; ENROLLMENT_EVIDENCE_REQUEST_MAX_BYTES_V1 + 1];
            }
        }
        assert!(bad.canonical_wire().is_err());
    }
    assert!(EnrollmentServiceRequestV1::decode_canonical(&[]).is_err());
    assert!(
        EnrollmentServiceRequestV1::decode_canonical(&vec![
            0;
            ENROLLMENT_SERVICE_REQUEST_MAX_BYTES_V1
                + 1
        ])
        .is_err()
    );
}
#[test]
fn response_roundtrips_each_status_and_keeps_exact_original_boundaries() {
    for value in [
        EnrollmentServiceResponseV1::Permit(vec![1; 2048]),
        EnrollmentServiceResponseV1::EvidenceReady,
        EnrollmentServiceResponseV1::Pending,
        EnrollmentServiceResponseV1::CredentialReady,
        EnrollmentServiceResponseV1::Credential(vec![2; ENROLLMENT_RESULT_MAX_BYTES_V1]),
    ] {
        let original = value.canonical_wire().unwrap();
        assert_eq!(
            EnrollmentServiceResponseV1::decode_canonical(&original).unwrap(),
            value
        );
        let mut trailing = original;
        trailing.push(0);
        assert!(EnrollmentServiceResponseV1::decode_canonical(&trailing).is_err());
    }
    for value in [
        EnrollmentServiceResponseV1::Permit(vec![]),
        EnrollmentServiceResponseV1::Permit(vec![0; 2049]),
        EnrollmentServiceResponseV1::Credential(vec![]),
        EnrollmentServiceResponseV1::Credential(vec![0; ENROLLMENT_RESULT_MAX_BYTES_V1 + 1]),
    ] {
        assert!(value.canonical_wire().is_err());
    }
    assert!(EnrollmentServiceResponseV1::decode_canonical(&[]).is_err());
}
