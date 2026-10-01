//! Canonical gateway model framing, strict JSON, and original-policy lease validation.
use super::*;
use crate::sorafs::reputation::StreamTokenValidationBindingV1;
const VALIDATED_AT_MS: u64 = 1_800_000_000_000;
fn qualification() -> StreamTokenGatewayAdmissionQualificationV1 {
    StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id: [0x31; 32],
        revision: 7,
        policy_digest: [0x32; 32],
        max_pending: 64,
        max_tracked_tokens: 64,
        lease_ttl_ms: 120_000,
    }
}
fn request(
    nonce: &str,
    validated_at_unix_ms: u64,
    expires_at_epoch: u64,
    max_streams: u16,
) -> StreamTokenGatewayAdmissionRequestV1 {
    StreamTokenGatewayAdmissionRequestV1 {
        serving_attempt_id: [0x61; 32],
        context: StreamTokenValidationRequestContextV1::try_new(
            ProviderId::new([0x41; 32]),
            [0x42; 32],
            sorafs_manifest::canonical_manifest_root_cid([0x43; 32]),
            "sorafs.sf1@1.0.0".to_owned(),
            nonce,
            Some(b"Q2Fub25pY2FsVG9rZW4="),
            StreamTokenRequestRouteV1::car_range(64, 1_023).expect("canonical route"),
        )
        .expect("canonical request context"),
        token_body_digest: Some([0x44; 32]),
        token_key_version: Some(3),
        validated_at_unix_ms,
        status: StreamTokenValidationStatusV1::Accepted,
        quota: Some(StreamTokenGatewayQuotaRequestV1 {
            token_id: "11".repeat(16),
            max_streams,
            requests_per_minute: 120,
            rate_limit_bytes: 1_048_576,
            requested_bytes: 960,
            expires_at_epoch,
            observed_at_epoch: validated_at_unix_ms / 1_000,
        }),
    }
}
fn record_for_request(
    request: &StreamTokenGatewayAdmissionRequestV1,
    sequence: u64,
    status: StreamTokenValidationStatusV1,
) -> StreamTokenGatewayAdmissionRecordV1 {
    let admitted = status == StreamTokenValidationStatusV1::Accepted;
    let token_expiry = admitted.then(|| {
        request
            .quota
            .as_ref()
            .expect("accepted request quota")
            .expires_at_epoch
    });
    StreamTokenGatewayAdmissionRecordV1 {
        serving_attempt_id: request.serving_attempt_id,
        admitted_under: qualification(),
        provider_id: request.context.provider_id(),
        outcome: StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: qualification().gateway_id,
                gateway_sequence: sequence,
                request_context_digest: request.context.digest().expect("request digest"),
            },
            token_body_digest: request.token_body_digest,
            token_key_version: request.token_key_version,
            validated_at_unix_ms: request.validated_at_unix_ms,
            status,
        },
        retry_after_secs: None,
        lease_id: admitted.then(|| [u8::try_from(sequence).expect("test sequence"); 32]),
        lease_expires_at_unix_ms: token_expiry.map(|expires| {
            stream_token_gateway_lease_expiry_unix_ms_v1(
                request.validated_at_unix_ms,
                expires,
                qualification().lease_ttl_ms,
            )
            .expect("canonical lease expiry")
        }),
        lease_token_expires_at_epoch: token_expiry,
    }
}
#[test]
fn lease_deadline_is_exact_and_rejects_early_late_expired_and_overflow_values() {
    let short = request(
        "nonce-lease-boundary",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 30,
        2,
    );
    let admission = StreamTokenGatewayAdmissionResultV1 {
        record: record_for_request(&short, 1, StreamTokenValidationStatusV1::Accepted),
        delivery_state: StreamTokenGatewayAdmissionDeliveryStateV1::Pending {
            predecessor_sequence: 0,
        },
    };
    let expected = VALIDATED_AT_MS + 30_000;
    assert_eq!(admission.record.lease_expires_at_unix_ms, Some(expected));
    admission
        .validate_for_request(&short, qualification())
        .expect("exact lease deadline");
    let mut early = admission;
    early.record.lease_expires_at_unix_ms = Some(expected - 1);
    assert_eq!(
        early.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut late = admission;
    late.record.lease_expires_at_unix_ms = Some(expected + 1);
    assert_eq!(
        late.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut expired = admission;
    expired.record.lease_token_expires_at_epoch = Some(VALIDATED_AT_MS / 1_000);
    expired.record.lease_expires_at_unix_ms = Some(VALIDATED_AT_MS);
    assert_eq!(
        expired.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut overflowing_token = short.clone();
    overflowing_token
        .quota
        .as_mut()
        .expect("quota")
        .expires_at_epoch = u64::MAX / 1_000 + 1;
    assert_eq!(
        overflowing_token.validate(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
    let mut mismatched_bytes = short.clone();
    mismatched_bytes
        .quota
        .as_mut()
        .expect("quota")
        .requested_bytes = 959;
    assert_eq!(
        mismatched_bytes.validate(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
    assert_eq!(
        stream_token_gateway_lease_expiry_unix_ms_v1(u64::MAX - 5, u64::MAX / 1_000, 10),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
}

fn assert_current_model<T>(value: &T, owner: &str)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize
        + IntoSchema
        + std::fmt::Debug
        + PartialEq,
{
    assert_eq!(T::nominal_name(), owner);
    assert_eq!(T::frame_name(), owner);
    assert!(T::schema().get::<T>().is_some());
    let frame = norito::encode_canonical(value).expect("model frame");
    assert_eq!(frame[6..22], norito::schema::identity::frame_hash::<T>());
    assert_eq!(&norito::decode_canonical::<T>(&frame).unwrap(), value);
    let mut foreign = frame.clone();
    foreign[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&foreign),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let json = norito::json::to_json(value).unwrap();
    assert_eq!(&norito::json::from_str::<T>(&json).unwrap(), value);
    let foreign = json.replacen('{', "{\"unknown\":1,", 1);
    assert_ne!(foreign, json);
    assert!(norito::json::from_str::<T>(&foreign).is_err());
}

#[test]
fn gateway_models_have_current_owner_schema_and_strict_json_roundtrips() {
    macro_rules! check {
        ($value:expr, $name:literal) => {
            assert_current_model(
                &$value,
                concat!("iroha_data_model::sorafs::stream_token_gateway::", $name),
            );
        };
    }
    let request = request(
        "nonce-model",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let record = record_for_request(&request, 1, StreamTokenValidationStatusV1::Accepted);
    check!(
        qualification(),
        "StreamTokenGatewayAdmissionQualificationV1"
    );
    check!(
        request.quota.clone().unwrap(),
        "StreamTokenGatewayQuotaRequestV1"
    );
    check!(request, "StreamTokenGatewayAdmissionRequestV1");
    check!(record, "StreamTokenGatewayAdmissionRecordV1");
    for delivery_state in [
        StreamTokenGatewayAdmissionDeliveryStateV1::Pending {
            predecessor_sequence: 0,
        },
        StreamTokenGatewayAdmissionDeliveryStateV1::AcknowledgedExactReplay {
            acknowledged_through_sequence: 1,
        },
    ] {
        check!(delivery_state, "StreamTokenGatewayAdmissionDeliveryStateV1");
        check!(
            StreamTokenGatewayAdmissionResultV1 {
                record,
                delivery_state
            },
            "StreamTokenGatewayAdmissionResultV1"
        );
    }
    check!(
        StreamTokenGatewayAdmissionReadbackV1 {
            acknowledged_through_sequence: 0,
            high_water_sequence: 1,
            records: vec![record],
        },
        "StreamTokenGatewayAdmissionReadbackV1"
    );
    for ack in [
        StreamTokenGatewayAdmissionAckV1::Acknowledged,
        StreamTokenGatewayAdmissionAckV1::ExactReplay,
    ] {
        check!(ack, "StreamTokenGatewayAdmissionAckV1");
    }
}

#[test]
fn original_admission_policy_survives_rotation_without_reinterpreting_leases() {
    let request = request(
        "nonce-rotation",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let record = record_for_request(&request, 1, StreamTokenValidationStatusV1::Accepted);
    let current = StreamTokenGatewayAdmissionQualificationV1 {
        revision: qualification().revision + 1,
        policy_digest: [0x55; 32],
        max_pending: 8,
        max_tracked_tokens: 8,
        lease_ttl_ms: 30_000,
        ..qualification()
    };
    record
        .validate_for_request(&request, current)
        .expect("original policy remains bound");
    assert_eq!(
        record.lease_expires_at_unix_ms,
        Some(VALIDATED_AT_MS + 120_000)
    );
    let mut reinterpreted = record;
    reinterpreted.lease_expires_at_unix_ms = Some(VALIDATED_AT_MS + current.lease_ttl_ms);
    assert_eq!(
        reinterpreted.validate_for_request(&request, current),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );

    for original in [
        StreamTokenGatewayAdmissionQualificationV1 {
            revision: current.revision + 1,
            ..qualification()
        },
        StreamTokenGatewayAdmissionQualificationV1 {
            gateway_id: [0x56; 32],
            ..qualification()
        },
        StreamTokenGatewayAdmissionQualificationV1 {
            revision: current.revision,
            ..qualification()
        },
        StreamTokenGatewayAdmissionQualificationV1 {
            revision: 0,
            ..qualification()
        },
    ] {
        let rebound = StreamTokenGatewayAdmissionRecordV1 {
            admitted_under: original,
            ..record
        };
        assert_eq!(
            rebound.validate_shape(current),
            Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
        );
    }
    let equal_revision_substitution = StreamTokenGatewayAdmissionQualificationV1 {
        policy_digest: [0x57; 32],
        ..qualification()
    };
    assert_eq!(
        record.validate_shape(equal_revision_substitution),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );

    let pending = StreamTokenGatewayAdmissionReadbackV1 {
        acknowledged_through_sequence: 0,
        high_water_sequence: 1,
        records: vec![record],
    };
    pending
        .validate(8, current)
        .expect("rotated pending prefix remains recoverable");
    let replay = StreamTokenGatewayAdmissionResultV1 {
        record,
        delivery_state: StreamTokenGatewayAdmissionDeliveryStateV1::AcknowledgedExactReplay {
            acknowledged_through_sequence: 1,
        },
    };
    replay
        .validate_for_request(&request, current)
        .expect("rotated exact replay remains recoverable");
}

#[test]
fn gateway_quota_enforces_the_canonical_signed_token_limits() {
    let mut request = request(
        "nonce-quota",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let quota = request.quota.as_mut().unwrap();
    quota.max_streams = STREAM_TOKEN_MAX_STREAMS_V1;
    quota.requests_per_minute = STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1;
    quota.rate_limit_bytes = STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1;
    request
        .validate()
        .expect("exact signed-token maxima are admissible");
    for max_streams in [0, STREAM_TOKEN_MAX_STREAMS_V1 + 1] {
        let mut invalid = request.clone();
        invalid.quota.as_mut().unwrap().max_streams = max_streams;
        assert_eq!(
            invalid.validate(),
            Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
        );
    }
    for requests_per_minute in [0, STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1 + 1] {
        let mut invalid = request.clone();
        invalid.quota.as_mut().unwrap().requests_per_minute = requests_per_minute;
        assert_eq!(
            invalid.validate(),
            Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
        );
    }
    for rate_limit_bytes in [0, STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1 + 1] {
        let mut invalid = request.clone();
        invalid.quota.as_mut().unwrap().rate_limit_bytes = rate_limit_bytes;
        assert_eq!(
            invalid.validate(),
            Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
        );
    }
}

#[test]
fn gateway_serving_attempt_is_mandatory_and_cannot_be_substituted() {
    let request = request(
        "nonce-attempt",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let record = record_for_request(&request, 1, StreamTokenValidationStatusV1::Accepted);
    record
        .validate_for_request(&request, qualification())
        .expect("exact physical attempt");
    let mut zero_request = request.clone();
    zero_request.serving_attempt_id = [0; 32];
    assert_eq!(
        zero_request.validate(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
    let zero_record = StreamTokenGatewayAdmissionRecordV1 {
        serving_attempt_id: [0; 32],
        ..record
    };
    assert_eq!(
        zero_record.validate_shape(qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut independent = request.clone();
    independent.serving_attempt_id = [0x62; 32];
    independent
        .validate()
        .expect("new physical attempt has valid shape");
    assert_eq!(independent.context.digest(), request.context.digest());
    assert_eq!(
        independent.validated_at_unix_ms,
        request.validated_at_unix_ms
    );
    assert_ne!(
        norito::encode_canonical(&independent).unwrap(),
        norito::encode_canonical(&request).unwrap()
    );
    assert_eq!(
        record.validate_for_request(&independent, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut request_json = norito::json::to_value(&request).unwrap();
    let bytes = request_json
        .get("serving_attempt_id")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(bytes.len(), 32);
    assert!(bytes.iter().all(|byte| byte.as_u64() == Some(0x61)));
    request_json
        .as_object_mut()
        .unwrap()
        .remove("serving_attempt_id");
    assert!(
        norito::json::from_value::<StreamTokenGatewayAdmissionRequestV1>(request_json).is_err()
    );
    let mut record_json = norito::json::to_value(&record).unwrap();
    record_json
        .as_object_mut()
        .unwrap()
        .remove("serving_attempt_id");
    assert!(norito::json::from_value::<StreamTokenGatewayAdmissionRecordV1>(record_json).is_err());
}

#[test]
fn gateway_empty_readback_still_requires_a_valid_binding_and_bounded_request() {
    let readback = StreamTokenGatewayAdmissionReadbackV1 {
        acknowledged_through_sequence: 0,
        high_water_sequence: 0,
        records: Vec::new(),
    };
    readback
        .validate(1, qualification())
        .expect("empty bound gateway");
    readback
        .validate(STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1, qualification())
        .expect("maximum bounded request");
    assert!(readback.validate(0, qualification()).is_err());
    assert!(
        readback
            .validate(
                STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1 + 1,
                qualification()
            )
            .is_err()
    );
    let mut inert = qualification();
    inert.policy_digest = [0; 32];
    assert!(readback.validate(1, inert).is_err());
    let missing = StreamTokenGatewayAdmissionReadbackV1 {
        high_water_sequence: 1,
        ..readback
    };
    assert!(missing.validate(1, qualification()).is_err());
}
