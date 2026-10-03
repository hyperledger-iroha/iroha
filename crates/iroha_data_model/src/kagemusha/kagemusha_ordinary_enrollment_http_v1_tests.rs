//! Pure transport controls over complete maintained Model originals. No Native owner or HTTP effect.
use super::*;
use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use norito::json::{self, Value};

fn requests(f: &Fixture) -> Vec<KagemushaOrdinaryEnrollmentHttpRequestV1> {
    use KagemushaOrdinaryEnrollmentHttpRequestV1 as Request;
    let c = &f.selection.preparation;
    let point = f
        .selection
        .issuance
        .credential
        .subject
        .app_public_key
        .as_sec1_bytes();
    let possession = match &f.proof.app_possession {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            KagemushaOrdinaryPossessionHttpV1 {
                platform: "android_keystore".into(),
                signature_der_base64: Some(STANDARD.encode(signature_der)),
                raw_assertion_base64: None,
            }
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            KagemushaOrdinaryPossessionHttpV1 {
                platform: "apple_app_attest".into(),
                signature_der_base64: None,
                raw_assertion_base64: Some(STANDARD.encode(raw_assertion)),
            }
        }
    };
    vec![
        Request::Prepare(KagemushaOrdinaryPreparationHttpRequestV1 {
            account_id: f.selection.owner.account_id.canonical_i105().unwrap(),
            client_nonce_hex: hex(&c.challenge.client_nonce),
            release_id_hex: hex(&c.challenge.release_id),
            profile_id_hex: hex(&c.challenge.hardware_profile_id),
            lane_id_hex: hex(&c.challenge.lane_id),
            financial_authority_commitment_hex: hex(&c.challenge.financial_authority_commitment),
        }),
        Request::RawAttestation(KagemushaOrdinaryRawAttestationHttpRequestV1 {
            schema: RAW_SCHEMA.into(),
            operation: "issue".into(),
            operation_id: hex(&c.challenge.attestation_challenge().unwrap()),
            signed_preparation_base64: STANDARD.encode(c.to_transport_bytes().unwrap()),
            attested_public_key_sec1_base64: STANDARD.encode(point),
            raw_attestation_base64: STANDARD.encode(&f.proof.raw_attestation),
        }),
        Request::Certificate(KagemushaOrdinaryCredentialHttpRequestV1 {
            schema: CREDENTIAL_SCHEMA.into(),
            operation: "issue".into(),
            operation_id: hex(&c.challenge.attestation_challenge().unwrap()),
            signed_preparation_base64: STANDARD.encode(c.to_transport_bytes().unwrap()),
            attested_public_key_sec1_base64: STANDARD.encode(point),
            raw_attestation_base64: STANDARD.encode(&f.proof.raw_attestation),
            app_possession: possession,
            play_integrity_token: None,
        }),
        Request::Start(start_originals(f)),
        Request::Finish(KagemushaOrdinaryRetailFinishHttpRequestV1 {
            challenge_id: hex(&c.challenge.attestation_challenge().unwrap()),
            account_signature_base64: STANDARD.encode(f.proof.account_signature.payload()),
        }),
    ]
}
fn raw_original(f: &Fixture) -> Vec<u8> {
    let c = &f.selection.preparation;
    let subject = &f.selection.issuance.credential.subject;
    let raw_subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
        version: 1,
        enrollment_challenge_digest: c.challenge.attestation_challenge().unwrap(),
        authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
        platform_class: subject.platform_class,
        security_level: subject.security_level,
        app_public_key: subject.app_public_key,
        attested_key_id: subject.attested_key_id,
        raw_platform_evidence_digest: hash(&f.proof.raw_attestation),
        app_signing_identity_digest: subject.app_signing_identity_digest,
        original_app_attest_counter: 0,
        issued_at_ms: c.challenge.issued_at_ms,
        expires_at_ms: c.challenge.expires_at_ms,
    };
    let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
    KagemushaRawAppAttestationAdmissionV1 {
        signature: Signature::new(
            key.private_key(),
            &raw_subject.canonical_signing_bytes().unwrap(),
        ),
        subject: raw_subject,
    }
    .to_transport_bytes()
    .unwrap()
}
fn start_originals(f: &Fixture) -> KagemushaOrdinaryRetailStartHttpRequestV1 {
    let c = &f.selection.preparation;
    let e = KagemushaAppEnrollmentPossessionV1 {
        challenge: KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            &c.challenge,
            &f.selection.issuance.credential.subject.app_public_key,
            hash(&f.proof.raw_attestation),
        )
        .unwrap(),
        evidence: f.proof.app_possession.clone(),
    };
    KagemushaOrdinaryRetailStartHttpRequestV1 {
        wallet: f.selection.owner.account_id.canonical_i105().unwrap(),
        signed_preparation_base64: STANDARD.encode(c.to_transport_bytes().unwrap()),
        raw_admission_original_base64: STANDARD.encode(raw_original(f)),
        platform_original_base64: STANDARD.encode(&f.proof.raw_attestation),
        core_possession_original_base64: STANDARD.encode(norito::encode_canonical(&e).unwrap()),
        app_certificate_base64: STANDARD
            .encode(f.selection.issuance.credential.canonical_bytes().unwrap()),
        selected_integrity: None,
    }
}
fn replies(f: &Fixture) -> Vec<KagemushaOrdinaryEnrollmentHttpReplyV1> {
    use KagemushaOrdinaryEnrollmentHttpReplyV1 as Reply;
    let c = &f.selection.preparation;
    let raw = raw_original(f);
    let credential = f.selection.issuance.credential.canonical_bytes().unwrap();
    vec![
        Reply::Prepare(KagemushaOrdinaryPreparationHttpReplyV1 {
            operation_id: hex(&c.challenge.attestation_challenge().unwrap()),
            signed_preparation_base64: STANDARD.encode(c.to_transport_bytes().unwrap()),
            attestation_challenge_base64: STANDARD
                .encode(c.challenge.attestation_challenge().unwrap()),
            expires_at_ms: c.challenge.expires_at_ms,
        }),
        Reply::RawAttestation(KagemushaOrdinaryRawAdmissionHttpReplyV1 {
            raw_admission_base64: STANDARD.encode(&raw),
            raw_admission_sha256_hex: hex(&hash(&raw)),
        }),
        Reply::Certificate(KagemushaOrdinaryCredentialHttpReplyV1 {
            certificate_base64: STANDARD.encode(&credential),
            certificate_sha256_hex: hex(&hash(&credential)),
        }),
        Reply::Start(KagemushaOrdinaryRetailStartHttpReplyV1 {
            challenge_id: hex(&c.challenge.attestation_challenge().unwrap()),
            canonical_challenge_base64: STANDARD.encode(f.challenge.canonical_bytes().unwrap()),
            account_signing_message_base64: STANDARD
                .encode(f.challenge.account_signing_message().unwrap()),
            expires_at_ms: f.challenge.expires_at_ms,
        }),
        Reply::Finish(KagemushaOrdinaryRetailFinishHttpReplyV1 {
            challenge_id: hex(&c.challenge.attestation_challenge().unwrap()),
            enrollment_id_hex: hex(&f.certificate.subject.enrollment_id),
            canonical_certificate_base64: STANDARD.encode(f.certificate.canonical_bytes().unwrap()),
        }),
    ]
}
fn mutate(raw: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value = json::from_slice(raw).unwrap();
    f(&mut value);
    json::to_vec(&value).unwrap()
}

#[test]
fn actual_five_stage_sdk_shapes_roundtrip_full_model_originals_for_both_platforms() {
    for apple in [false, true] {
        let f = Fixture::new(apple);
        let requests = requests(&f);
        let replies = replies(&f);
        for (request, reply) in requests.iter().zip(&replies) {
            let raw = request.canonical_bytes().unwrap();
            assert_eq!(
                *request,
                KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &raw).unwrap()
            );
            assert_eq!(
                *request,
                KagemushaOrdinaryEnrollmentHttpRequestV1::parse_http_data(
                    request.stage().path(),
                    &raw
                )
                .unwrap()
            );
            assert_eq!(
                KagemushaOrdinaryEnrollmentHttpStageV1::from_path(request.stage().path()),
                Some(request.stage())
            );
            let suffix = request.stage().path().rsplit('/').next().unwrap();
            for retired_or_changed in [
                format!("/v1/offline/enrollment/ordinary/{suffix}"),
                format!("/v1/kagemusha/ordinary/enrollment/{suffix}"),
                format!("/v1/retail/kagemusha/ordinary/enrollment/{suffix}"),
                format!("{}/", request.stage().path()),
                format!("{}?current=true", request.stage().path()),
                format!("{}/current", request.stage().path()),
            ] {
                assert_eq!(
                    KagemushaOrdinaryEnrollmentHttpStageV1::from_path(&retired_or_changed),
                    None
                );
                assert!(
                    KagemushaOrdinaryEnrollmentHttpRequestV1::parse_http_data(
                        &retired_or_changed,
                        &raw
                    )
                    .is_err()
                );
            }
            let body = reply.canonical_bytes(request).unwrap();
            assert_eq!(
                *reply,
                KagemushaOrdinaryEnrollmentHttpReplyV1::parse(request, &body).unwrap()
            );
            assert_eq!(
                request.stage().path(),
                format!(
                    "/v1/kagemusha/enrollment/ordinary/{}",
                    match request.stage() {
                        KagemushaOrdinaryEnrollmentHttpStageV1::Prepare => "prepare",
                        KagemushaOrdinaryEnrollmentHttpStageV1::RawAttestation => "raw-attestation",
                        KagemushaOrdinaryEnrollmentHttpStageV1::Certificate => "certificate",
                        KagemushaOrdinaryEnrollmentHttpStageV1::Start => "start",
                        KagemushaOrdinaryEnrollmentHttpStageV1::Finish => "finish",
                    }
                )
            );
        }
        let keys: Value = json::from_slice(&requests[0].canonical_bytes().unwrap()).unwrap();
        assert_eq!(keys.as_object().unwrap().len(), 6);
        assert!(keys.get("enrollment_id_hex").is_none() && keys.get("platform").is_none());
    }
}

#[test]
fn requests_reject_schema_operation_platform_correlation_unknown_duplicate_and_complete_bound() {
    let f = Fixture::new(false);
    let requests = requests(&f);
    for request in &requests {
        let raw = request.canonical_bytes().unwrap();
        let unknown = mutate(&raw, |v| {
            v.as_object_mut()
                .unwrap()
                .insert("authority_verdict".into(), Value::Bool(true));
        });
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &unknown).is_err()
        );
        let value: Value = json::from_slice(&raw).unwrap();
        let (key, item) = value.as_object().unwrap().iter().next().unwrap();
        let mut duplicate = raw[..raw.len() - 1].to_vec();
        duplicate.push(b',');
        duplicate.extend(json::to_vec(key).unwrap());
        duplicate.push(b':');
        duplicate.extend(json::to_vec(item).unwrap());
        duplicate.push(b'}');
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &duplicate).is_err()
        );
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(
                request.stage(),
                &vec![b' '; KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_MAX_BYTES_V1 + 1]
            )
            .is_err()
        );
    }
    for (i, key) in [
        (1, "schema"),
        (1, "operation"),
        (1, "operation_id"),
        (2, "schema"),
        (2, "operation_id"),
    ] {
        let raw = requests[i].canonical_bytes().unwrap();
        let changed = mutate(&raw, |v| {
            v.as_object_mut()
                .unwrap()
                .insert(key.into(), Value::from("substituted"));
        });
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(requests[i].stage(), &changed).is_err()
        );
    }
    let raw = requests[2].canonical_bytes().unwrap();
    for field in ["raw_assertion_base64", "signature_der_base64"] {
        let changed = mutate(&raw, |v| {
            v.get_mut("app_possession")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), Value::Null);
        });
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(requests[2].stage(), &changed).is_err()
        );
    }
    let changed = mutate(&raw, |v| {
        v.as_object_mut().unwrap().remove("play_integrity_token");
    });
    assert!(
        KagemushaOrdinaryEnrollmentHttpRequestV1::parse(requests[2].stage(), &changed).is_err()
    );
}

#[test]
fn replies_reject_substituted_full_original_digest_c_nonce_release_lane_message_and_enrollment() {
    let f = Fixture::new(false);
    let requests = requests(&f);
    let replies = replies(&f);
    for (request, reply) in requests.iter().zip(&replies) {
        let raw = reply.canonical_bytes(request).unwrap();
        let changed = mutate(&raw, |v| {
            v.as_object_mut()
                .unwrap()
                .insert("current_authority".into(), Value::Bool(true));
        });
        assert!(KagemushaOrdinaryEnrollmentHttpReplyV1::parse(request, &changed).is_err());
    }
    for (i, field) in [
        (0, "operation_id"),
        (1, "raw_admission_sha256_hex"),
        (2, "certificate_sha256_hex"),
        (4, "enrollment_id_hex"),
    ] {
        let raw = replies[i].canonical_bytes(&requests[i]).unwrap();
        let changed = mutate(&raw, |v| {
            v.as_object_mut()
                .unwrap()
                .insert(field.into(), Value::from(hex(&[99; 32])));
        });
        assert!(KagemushaOrdinaryEnrollmentHttpReplyV1::parse(&requests[i], &changed).is_err());
    }
    for field in [
        "client_nonce_hex",
        "release_id_hex",
        "profile_id_hex",
        "lane_id_hex",
        "financial_authority_commitment_hex",
    ] {
        let changed = mutate(&requests[0].canonical_bytes().unwrap(), |v| {
            v.as_object_mut()
                .unwrap()
                .insert(field.into(), Value::from(hex(&[99; 32])));
        });
        let foreign =
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(requests[0].stage(), &changed).unwrap();
        assert!(replies[0].require_request_data(&foreign).is_err());
    }
    let changed = mutate(&replies[3].canonical_bytes(&requests[3]).unwrap(), |v| {
        v.as_object_mut().unwrap().insert(
            "account_signing_message_base64".into(),
            Value::from(STANDARD.encode([99; 32])),
        );
    });
    assert!(KagemushaOrdinaryEnrollmentHttpReplyV1::parse(&requests[3], &changed).is_err());
}

#[test]
fn minimal_sorted_json_preserves_full_original_slash_bytes_and_maximum_uint64() {
    let value = norito::json!({"z": "/+==", "a": (u64::MAX)});
    assert_eq!(
        encode(&value).unwrap(),
        br#"{"a":18446744073709551615,"z":"/+=="}"#
    );
    let decoded: Value = decode(&encode(&value).unwrap()).unwrap();
    assert_eq!(decoded.get("a").and_then(Value::as_u64), Some(u64::MAX));
}

#[test]
fn initial_start_complete_originals_have_bounded_json_and_norito_roundtrips() {
    for apple in [false, true] {
        let f = Fixture::new(apple);
        let start = start_originals(&f);
        let norito = start.canonical_norito_bytes().unwrap();
        assert_eq!(
            start,
            KagemushaOrdinaryRetailStartHttpRequestV1::decode_canonical_norito_exact(&norito)
                .unwrap()
        );
        let request = KagemushaOrdinaryEnrollmentHttpRequestV1::Start(start);
        let json = request.canonical_bytes().unwrap();
        let shape: Value = json::from_slice(&json).unwrap();
        assert_eq!(shape.as_object().unwrap().len(), 7);
        assert_eq!(shape.get("selected_integrity"), Some(&Value::Null));
        assert_eq!(
            request,
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse_http_data(
                "/v1/kagemusha/enrollment/ordinary/start",
                &json
            )
            .unwrap()
        );
        let mut tail = norito.clone();
        tail.push(0);
        assert!(
            KagemushaOrdinaryRetailStartHttpRequestV1::decode_canonical_norito_exact(&tail)
                .is_err()
        );
        assert!(
            KagemushaOrdinaryRetailStartHttpRequestV1::decode_canonical_norito_exact(&vec![
                0;
                KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        // First release: each full original and explicit null are mandatory; old2-field refuses.
        for key in [
            "wallet",
            "signed_preparation_base64",
            "raw_admission_original_base64",
            "platform_original_base64",
            "core_possession_original_base64",
            "app_certificate_base64",
            "selected_integrity",
        ] {
            let missing = mutate(&json, |v| {
                v.as_object_mut().unwrap().remove(key);
            });
            assert!(
                KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &missing).is_err()
            );
        }
        for (key, limit) in [
            ("raw_admission_original_base64", 315usize),
            ("platform_original_base64", 131_073),
            ("core_possession_original_base64", 5121),
            ("app_certificate_base64", 16385),
        ] {
            let oversized = mutate(&json, |v| {
                v.as_object_mut()
                    .unwrap()
                    .insert(key.into(), Value::String(STANDARD.encode(vec![1; limit])));
            });
            assert!(
                KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &oversized)
                    .is_err()
            );
        }
        let selected = mutate(&json, |v| {
            v.as_object_mut().unwrap().insert(
                "selected_integrity".into(),
                norito::json!({"challenge":"AA==","lease":"AA=="}),
            );
        });
        assert!(
            KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &selected).is_err()
        );
        let foreign = start_originals(&Fixture::new(!apple));
        for (key, value) in [
            ("wallet", foreign.wallet),
            (
                "raw_admission_original_base64",
                foreign.raw_admission_original_base64,
            ),
            ("platform_original_base64", foreign.platform_original_base64),
            (
                "core_possession_original_base64",
                foreign.core_possession_original_base64,
            ),
            ("app_certificate_base64", foreign.app_certificate_base64),
        ] {
            if shape.get(key) == Some(&Value::String(value.clone())) {
                continue;
            }
            let changed = mutate(&json, |v| {
                v.as_object_mut()
                    .unwrap()
                    .insert(key.into(), Value::String(value));
            });
            assert!(
                KagemushaOrdinaryEnrollmentHttpRequestV1::parse(request.stage(), &changed).is_err()
            );
        }
    }
}

#[test]
fn selected_integrity_public_carrier_roundtrips_complete_original_pair_without_start_authority() {
    let f = Fixture::android_with_integrity();
    let (challenge, lease) = f.integrity_refresh_originals();
    let challenge_original = challenge.to_transport_bytes().unwrap();
    let lease_original = lease.canonical_bytes().unwrap();
    let pair = KagemushaOrdinaryStartIntegrityHttpV1 {
        challenge: STANDARD.encode(&challenge_original),
        lease: STANDARD.encode(&lease_original),
    };
    let binary = norito::encode_canonical(&pair).unwrap();
    let decoded: KagemushaOrdinaryStartIntegrityHttpV1 = norito::decode_canonical_with_limits(
        &binary,
        norito::canonical_decode_limits(binary.len()),
    )
    .unwrap();
    assert_eq!(decoded, pair);
    assert_eq!(
        STANDARD.decode(&decoded.challenge).unwrap(),
        challenge_original
    );
    assert_eq!(STANDARD.decode(&decoded.lease).unwrap(), lease_original);
    let json_original = json::to_vec(&pair).unwrap();
    assert_eq!(
        json::from_slice::<KagemushaOrdinaryStartIntegrityHttpV1>(&json_original).unwrap(),
        pair
    );
    let unknown = mutate(&json_original, |value| {
        value
            .as_object_mut()
            .unwrap()
            .insert("current_authority".into(), Value::Bool(true));
    });
    assert!(json::from_slice::<KagemushaOrdinaryStartIntegrityHttpV1>(&unknown).is_err());
    // A decoded refresh pair remains DATA. It cannot enter the initial Start ceremony.
    let mut initial = start_originals(&f);
    initial.selected_integrity = Some(pair);
    assert!(
        KagemushaOrdinaryEnrollmentHttpRequestV1::Start(initial)
            .canonical_bytes()
            .is_err()
    );
}
