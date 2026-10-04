//! Canonical API projection/query/binding controls without runtime authority construction.
use super::*;
fn catalog() -> GatewayComplianceCatalogStatusV1 {
    GatewayComplianceCatalogStatusV1 {
        digest_hex: hex::encode([0xab; 32]),
        sequence: 7,
        generated_at_unix: 100,
        valid_until_unix: 200,
    }
}
fn status() -> GatewayComplianceStatusResponseV1 {
    GatewayComplianceStatusResponseV1 {
        schema: GATEWAY_COMPLIANCE_STATUS_SCHEMA_V1.into(),
        checkpoint_version: 1,
        policy_digest_hex: hex::encode([1; 32]),
        observed_at_unix: 110,
        serving_ready: true,
        chain_head: Some(catalog()),
        serving: Some(catalog()),
        previous_serving: None,
        candidate: None,
        acknowledgement_count: 3,
        accepted_acknowledgement_count: 2,
        rejected_acknowledgement_count: 1,
        history_count: 1,
        idempotency_record_count: 4,
        latest_action: Some(GatewayComplianceLatestActionStatusV1 {
            operation_id_hex: hex::encode([2; 32]),
            action: "promotion".into(),
            previous_serving_digest_hex: None,
            serving_digest_hex: hex::encode([0xab; 32]),
            recorded_at_unix: 105,
            reason_code: "gateway-quorum".into(),
        }),
    }
}
fn roundtrip<
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + PartialEq + std::fmt::Debug,
>(
    value: &T,
) {
    let encoded =
        norito::json::to_json_bounded(value, GATEWAY_COMPLIANCE_RESPONSE_MAX_BYTES_V1).unwrap();
    let decoded: T = norito::json::from_str(&encoded).unwrap();
    assert_eq!(&decoded, value);
}
#[test]
fn all_six_api_shapes_roundtrip_and_status_nullable_fields_are_required() {
    let value = status();
    value.validate().unwrap();
    roundtrip(&value);
    roundtrip(&catalog());
    roundtrip(value.latest_action.as_ref().unwrap());
    let action = GatewayComplianceActionResponseV1 {
        schema: GATEWAY_COMPLIANCE_ACTION_SCHEMA_V1.into(),
        action: "stage".into(),
        catalog_digest_hex: hex::encode([3; 32]),
        idempotency_key: hex::encode([4; 32]),
        operation_timestamp_unix: 100,
    };
    action.validate().unwrap();
    roundtrip(&action);
    let error = GatewayComplianceErrorResponseV1 {
        schema: GATEWAY_COMPLIANCE_ERROR_SCHEMA_V1.into(),
        code: "controller_unavailable".into(),
        message: "controller unavailable".into(),
    };
    error.validate().unwrap();
    roundtrip(&error);
    roundtrip(&GatewayCompliancePromoteExpectationV1 {
        catalog_digest: [9; 32],
        sequence: 7,
    });
    for key in [
        "chain_head",
        "serving",
        "previous_serving",
        "candidate",
        "latest_action",
    ] {
        let mut json = norito::json::to_value(&value).unwrap();
        json.as_object_mut().unwrap().remove(key).unwrap();
        assert!(norito::json::from_value::<GatewayComplianceStatusResponseV1>(json).is_err());
    }
    let mut extra = norito::json::to_value(&value).unwrap();
    extra
        .as_object_mut()
        .unwrap()
        .insert("trusted".into(), norito::json::Value::Bool(true));
    assert!(norito::json::from_value::<GatewayComplianceStatusResponseV1>(extra).is_err());
}
#[test]
fn status_representation_refuses_wrong_schema_digests_counters_and_readiness_interval() {
    let value = status();
    for mutate in [
        |s: &mut GatewayComplianceStatusResponseV1| s.schema = "other".into(),
        |s: &mut GatewayComplianceStatusResponseV1| s.checkpoint_version = 2,
        |s: &mut GatewayComplianceStatusResponseV1| s.policy_digest_hex = "AB".repeat(32),
        |s: &mut GatewayComplianceStatusResponseV1| s.rejected_acknowledgement_count = u64::MAX,
        |s: &mut GatewayComplianceStatusResponseV1| s.accepted_acknowledgement_count = 1,
        |s: &mut GatewayComplianceStatusResponseV1| s.serving_ready = false,
        |s: &mut GatewayComplianceStatusResponseV1| s.observed_at_unix = 99,
        |s: &mut GatewayComplianceStatusResponseV1| s.observed_at_unix = 200,
        |s: &mut GatewayComplianceStatusResponseV1| s.serving.as_mut().unwrap().sequence = 0,
        |s: &mut GatewayComplianceStatusResponseV1| {
            s.latest_action.as_mut().unwrap().reason_code = "x".repeat(129)
        },
        |s: &mut GatewayComplianceStatusResponseV1| {
            s.latest_action.as_mut().unwrap().action = "promote".into()
        },
    ] {
        let mut changed = value.clone();
        mutate(&mut changed);
        assert!(changed.validate().is_err());
    }
    for observed in [99, 200] {
        let mut expired = value.clone();
        expired.observed_at_unix = observed;
        expired.serving_ready = false;
        expired.validate().unwrap();
    }
    let mut at_start = value;
    at_start.observed_at_unix = 100;
    at_start.validate().unwrap();
}
#[test]
fn promote_query_has_exact_canonical_shape_and_no_missing_or_alternate_spellings() {
    let value = GatewayCompliancePromoteExpectationV1 {
        catalog_digest: [0xab; 32],
        sequence: 7,
    };
    let query = value.canonical_query().unwrap();
    assert_eq!(
        GatewayCompliancePromoteExpectationV1::parse_query(&query).unwrap(),
        value
    );
    assert!(
        GatewayCompliancePromoteExpectationV1 {
            sequence: 0,
            ..value
        }
        .canonical_query()
        .is_err()
    );
    for invalid in [
        String::new(),
        query.replace("ab", "AB"),
        query.replace("=7", "=07"),
        query.replace("=7", "=0"),
        query.replace("=7", "=+7"),
        format!("{query}&expected_sequence=8"),
        format!(
            "expected_sequence=7&expected_catalog_digest={}",
            hex::encode(value.catalog_digest)
        ),
        query.replace("ab", "%61b"),
        query.replace("=7", "=18446744073709551616"),
    ] {
        assert!(
            GatewayCompliancePromoteExpectationV1::parse_query(&invalid).is_err(),
            "{invalid}"
        );
    }
    // The parser preserves existing zero-digest syntax; actual native promotion owns matching.
    let zero = GatewayCompliancePromoteExpectationV1 {
        catalog_digest: [0; 32],
        sequence: 1,
    };
    assert_eq!(
        GatewayCompliancePromoteExpectationV1::parse_query(&zero.canonical_query().unwrap())
            .unwrap(),
        zero
    );
}
#[test]
fn request_binding_separates_every_original_byte_boundary_without_uri_rewriting() {
    let target =
        "/v1/sorafs/gateway/compliance/promote?expected_catalog_digest=ab&expected_sequence=7";
    let original = request_idempotency_binding("promote", target, b"{}");
    assert_eq!(
        original,
        request_idempotency_binding("promote", target, b"{}")
    );
    for changed in [
        request_idempotency_binding("stage", target, b"{}"),
        request_idempotency_binding("promote", target, b"{ }"),
        request_idempotency_binding("promote", &format!("{target}&x=1"), b"{}"),
        request_idempotency_binding("promote", &target.replace("ab", "%61b"), b"{}"),
        request_idempotency_binding("promote", target, b""),
    ] {
        assert_ne!(original, changed);
    }
    assert_ne!(
        request_idempotency_binding("ab", "c", b"d"),
        request_idempotency_binding("a", "bc", b"d")
    );
    assert_eq!(decode_lower_hex_32(&hex::encode(original)), Some(original));
    assert!(decode_lower_hex_32(&"FF".repeat(32)).is_none());
}
