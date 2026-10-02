//! Real Ed/BLS/complete-World fixtures for the purpose-specific HTTP grammar.
//! No fixture is an authenticated FI customer, installed owner or issuer key.
use super::*;
use crate::participant_enrollment_request::http::{
    ParticipantEnrollmentHttpOwnerContextV1, decode_participant_enrollment_http_original_v1,
    encode_participant_enrollment_http_headers_v1,
};

fn owner<'a>(
    request: &ParticipantEnrollmentRequestV1<'a>,
) -> ParticipantEnrollmentHttpOwnerContextV1<'a> {
    ParticipantEnrollmentHttpOwnerContextV1 {
        network_id: request.network_id,
        authentication_namespace: request.authentication_namespace,
        actor_id: request.actor_id,
        operation: request.operation,
        http_method: "POST",
        request_target: request.target.path(),
        target: request.target,
    }
}
fn borrowed<'h>(headers: &'h [(&'static str, Vec<u8>)]) -> impl Iterator<Item = (&'h str, &'h [u8])> + 'h {
    headers
        .iter()
        .map(|(name, value)| -> (&'h str, &'h [u8]) { (*name, value.as_slice()) })
}

#[test]
fn enrollment_http_original_retains_exact_body_signature_and_fresh_receiver_cut() {
    let fixture = Fixture::new();
    let network = fixture.native.network_id();
    let target = valid_target();
    let mut request = request(&fixture, &network, &target, b"{ \"attempt\" : 1 }");
    let nonce = "ab".repeat(32);
    request.nonce = &nonce;
    let authorization = b"unit-test-only-not-an-authenticated-session";
    request.session_sha256 = Sha256::digest(authorization).into();
    let signature = Signature::new(
        fixture.signer.private_key(),
        &request.signing_message().unwrap(),
    );
    let headers =
        encode_participant_enrollment_http_headers_v1(&request, &signature, authorization).unwrap();
    let original = decode_participant_enrollment_http_original_v1(
        owner(&request),
        request.body,
        borrowed(&headers),
    )
    .unwrap();
    assert!(std::ptr::eq(
        original.request().body.as_ptr(),
        request.body.as_ptr()
    ));
    assert_eq!(
        original.request().signing_message().unwrap(),
        request.signing_message().unwrap()
    );
    assert_eq!(original.signature().payload(), signature.payload());
    assert_ne!(original.request().signatory, original.request().wallet);
    assert_eq!(original.request().target.as_str(), target.as_str());
    // The decoder grants no current-read capability. Obtain another actual challenged
    // four-statement fixture cut, then invoke the same real receiving verification primitive.
    let received = original.request();
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&received).unwrap();
    let nodes = Fixture::nodes();
    let statements = fixture.statements(challenge.bytes(), &nodes);
    let current = fixture.admit(challenge, &nodes, &statements).unwrap();
    let verified = current
        .verify_request(&received, original.signature())
        .unwrap();
    verified.verify_original_body(request.body).unwrap();
    assert_eq!(verified.request_id(), request.request_id);
    assert_eq!(verified.idempotency_key(), request.idempotency_key);
}

#[test]
fn enrollment_http_original_refuses_duplicates_missing_values_and_all_signed_substitutions() {
    let fixture = Fixture::new();
    let network = fixture.native.network_id();
    let target = valid_target();
    let mut request = request(&fixture, &network, &target, b"original exact JSON");
    let nonce = "ab".repeat(32);
    request.nonce = &nonce;
    let authorization = b"unit-test-only-not-an-authenticated-session";
    request.session_sha256 = Sha256::digest(authorization).into();
    let signature = Signature::new(
        fixture.signer.private_key(),
        &request.signing_message().unwrap(),
    );
    let headers =
        encode_participant_enrollment_http_headers_v1(&request, &signature, authorization).unwrap();
    for index in 0..headers.len() {
        let mut missing = headers.clone();
        missing.remove(index);
        assert!(
            decode_participant_enrollment_http_original_v1(
                owner(&request),
                request.body,
                borrowed(&missing)
            )
            .is_err(),
            "missing field {index}"
        );
        let mut duplicate = headers.clone();
        duplicate.push(headers[index].clone());
        assert!(
            decode_participant_enrollment_http_original_v1(
                owner(&request),
                request.body,
                borrowed(&duplicate)
            )
            .is_err(),
            "duplicate field {index}"
        );
    }
    for (index, changed) in [
        (0, b"01".as_slice()),
        (1, headers[2].1.as_slice()),
        (2, headers[1].1.as_slice()),
        (3, b"another-request".as_slice()),
        (4, b"another-business-attempt".as_slice()),
        (5, b"1000001".as_slice()),
        (
            6,
            b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd".as_slice(),
        ),
        (7, b"00".as_slice()),
        (8, b"different-test-only-session".as_slice()),
    ] {
        let mut substituted = headers.clone();
        substituted[index].1 = changed.to_vec();
        assert!(
            decode_participant_enrollment_http_original_v1(
                owner(&request),
                request.body,
                borrowed(&substituted)
            )
            .is_err(),
            "changed field {index}"
        );
    }
    for malformed in [
        b"01000000".as_slice(),
        b"+1000000".as_slice(),
        b"1000000 ".as_slice(),
    ] {
        let mut substituted = headers.clone();
        substituted[5].1 = malformed.to_vec();
        assert!(
            decode_participant_enrollment_http_original_v1(
                owner(&request),
                request.body,
                borrowed(&substituted)
            )
            .is_err()
        );
    }
    for index in 0..headers.len() {
        for malformed in [
            vec![0xff],
            [b" ".as_slice(), headers[index].1.as_slice()].concat(),
            [headers[index].1.as_slice(), b" ".as_slice()].concat(),
            vec![b'x'; 8193],
        ] {
            let mut substituted = headers.clone();
            substituted[index].1 = malformed;
            assert!(
                decode_participant_enrollment_http_original_v1(
                    owner(&request),
                    request.body,
                    borrowed(&substituted)
                )
                .is_err(),
                "non-UTF8/whitespace/size field {index}"
            );
        }
    }
    let mut uppercase_nonce = headers.clone();
    uppercase_nonce[6].1.make_ascii_uppercase();
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&uppercase_nonce)
        )
        .is_err()
    );
    let mut base64_nonce = headers.clone();
    base64_nonce[6].1 = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_vec();
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&base64_nonce)
        )
        .is_err()
    );
    let mut changed_ed64 = headers.clone();
    changed_ed64[7].1[0] = if changed_ed64[7].1[0] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&changed_ed64)
        )
        .is_err()
    );
    let mut uppercase_signature = headers.clone();
    uppercase_signature[7].1.make_ascii_uppercase();
    assert_ne!(uppercase_signature[7].1, headers[7].1);
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&uppercase_signature)
        )
        .is_err()
    );
    let mut base64_signature = headers.clone();
    base64_signature[7].1 =
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
            .to_vec();
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&base64_signature)
        )
        .is_err()
    );
    for noncanonical in [b"alias@leumi.is2".as_slice(), b"0x000001".as_slice()] {
        let mut substituted = headers.clone();
        substituted[1].1 = noncanonical.to_vec();
        assert!(
            decode_participant_enrollment_http_original_v1(
                owner(&request),
                request.body,
                borrowed(&substituted)
            )
            .is_err()
        );
    }
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            &[0xff, 0xfe],
            borrowed(&headers)
        )
        .is_err()
    );
    let huge_body = vec![b'x'; MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES + 1];
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            &huge_body,
            borrowed(&headers)
        )
        .is_err()
    );
    let mut unknown_case_variant = headers.clone();
    unknown_case_variant.push(("X-Iroha-Enrollment-Offered-Target", b"test-only".to_vec()));
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&unknown_case_variant)
        )
        .is_err()
    );
    let mut changed = headers.clone();
    changed.push((
        "x-iroha-enrollment-offered-target",
        b"https://untrusted.example.invalid".to_vec(),
    ));
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&changed)
        )
        .is_err()
    );
    let mut duplicate_authorization = headers.clone();
    duplicate_authorization.push(("Authorization", authorization.to_vec()));
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&duplicate_authorization)
        )
        .is_err()
    );
    let another_target = Url::parse(
        "https://another.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare",
    )
    .unwrap();
    for field in 0..4 {
        let mut changed_owner = owner(&request);
        match field {
            0 => changed_owner.authentication_namespace = "leumi.is",
            1 => changed_owner.actor_id = "another-customer",
            2 => changed_owner.target = &another_target,
            _ => changed_owner.operation = ParticipantEnrollmentOperationV1::Certificate,
        }
        assert!(
            decode_participant_enrollment_http_original_v1(
                changed_owner,
                request.body,
                borrowed(&headers)
            )
            .is_err()
        );
    }
    let mut wrong_method = owner(&request);
    wrong_method.http_method = "GET";
    assert!(
        decode_participant_enrollment_http_original_v1(
            wrong_method,
            request.body,
            borrowed(&headers)
        )
        .is_err()
    );
    let mut removed_mount = owner(&request);
    removed_mount.request_target = "/v1/kagemusha/enrollment/ordinary/prepare";
    assert!(
        decode_participant_enrollment_http_original_v1(
            removed_mount,
            request.body,
            borrowed(&headers)
        )
        .is_err()
    );
    let mut query = owner(&request);
    query.request_target = "/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare?redirect=1";
    assert!(
        decode_participant_enrollment_http_original_v1(query, request.body, borrowed(&headers))
            .is_err()
    );
    assert!(
        decode_participant_enrollment_http_original_v1(
            owner(&request),
            b"reserialized body",
            borrowed(&headers)
        )
        .is_err()
    );
}

#[test]
fn enrollment_http_encoder_refuses_foreign_session_and_retail_operation_signature() {
    let fixture = Fixture::new();
    let network = fixture.native.network_id();
    let target = valid_target();
    let mut request = request(&fixture, &network, &target, b"exact original JSON");
    let nonce = "ab".repeat(32);
    request.nonce = &nonce;
    let authorization = b"unit-test-only-not-an-authenticated-session";
    request.session_sha256 = Sha256::digest(authorization).into();
    let signature = Signature::new(
        fixture.signer.private_key(),
        &request.signing_message().unwrap(),
    );
    assert!(
        encode_participant_enrollment_http_headers_v1(
            &request,
            &signature,
            b"another-test-only-session"
        )
        .is_err()
    );
    // Retail phase10 signs a different retained raw32 subject. Even the correct real S
    // signature over that operation cannot authenticate the HTTP signing purpose.
    let retail_signature = Signature::new(fixture.signer.private_key(), &[71; 32]);
    assert!(
        encode_participant_enrollment_http_headers_v1(&request, &retail_signature, authorization)
            .is_err()
    );
}

#[test]
fn enrollment_http_all_three_purposes_retain_real_originals_and_refuse_cross_purpose_signatures() {
    use ParticipantEnrollmentOperationV1::{Certificate, Prepare, RawAttestation};
    let fixture = Fixture::new();
    let network = fixture.native.network_id();
    let purposes = [Prepare, RawAttestation, Certificate];
    for operation in purposes {
        let mut target = valid_target();
        let mount = target
            .path()
            .strip_suffix(Prepare.path_suffix())
            .unwrap()
            .to_owned();
        target.set_path(&format!("{}{}", mount, operation.path_suffix()));
        let mut request = request(&fixture, &network, &target, b"{ \"attempt\" : 1 }");
        request.operation = operation;
        let nonce = "ab".repeat(32);
        request.nonce = &nonce;
        let authorization = b"unit-test-only-not-an-authenticated-session";
        request.session_sha256 = Sha256::digest(authorization).into();
        let signature = Signature::new(
            fixture.signer.private_key(),
            &request.signing_message().unwrap(),
        );
        let headers =
            encode_participant_enrollment_http_headers_v1(&request, &signature, authorization)
                .unwrap();
        let original = decode_participant_enrollment_http_original_v1(
            owner(&request),
            request.body,
            borrowed(&headers),
        )
        .unwrap();
        let received = original.request();
        assert_eq!(
            received.signing_message().unwrap(),
            request.signing_message().unwrap()
        );
        assert!(std::ptr::eq(received.body.as_ptr(), request.body.as_ptr()));
        assert_eq!(original.signature().payload(), signature.payload());
        let challenge = EnrollmentWalletReadChallengeV1::for_request(&received).unwrap();
        let nodes = Fixture::nodes();
        let statements = fixture.statements(challenge.bytes(), &nodes);
        let current = fixture.admit(challenge, &nodes, &statements).unwrap();
        let verified = current
            .verify_request(&received, original.signature())
            .unwrap();
        verified.verify_original_body(request.body).unwrap();
        for other in purposes.into_iter().filter(|other| *other != operation) {
            let mut other_target = target.clone();
            other_target.set_path(&format!("{}{}", mount, other.path_suffix()));
            // The other owner's method and exact target are internally valid. Its purpose
            // and canonical request differ from the original real Ed signing subject.
            let mut other_owner = owner(&request);
            other_owner.operation = other;
            other_owner.target = &other_target;
            other_owner.request_target = other_target.path();
            assert!(
                decode_participant_enrollment_http_original_v1(
                    other_owner,
                    request.body,
                    borrowed(&headers),
                )
                .is_err(),
                "original {operation:?} signature accepted for {other:?}"
            );
        }
    }
}
