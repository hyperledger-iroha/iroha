// PoP proof requests expose the same mandatory recipient binding as the native API.

#[test]
fn sorafs_pop_openapi_requires_closed_recipient_bound_requests() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    for (path, schema_name, first_field) in [
        (
            "/v1/sorafs/pop/wallet/prove",
            "SorafsPopMembershipRequestV1",
            "credential_commitment_hex",
        ),
        (
            "/v1/sorafs/pop/verify",
            "SorafsPopVerifyMembershipRequestV1",
            "canonical_proof_base64url",
        ),
    ] {
        let fields = [
            first_field,
            "challenge_digest_hex",
            "verifier_context",
            "presentation_binding_digest_hex",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let schema = schemas.get(schema_name).unwrap();
        assert_eq!(schema.get("type").and_then(Value::as_str), Some("object"));
        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            schema
                .get("required")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|field| field.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            fields
        );
        let properties = schema.get("properties").and_then(Value::as_object).unwrap();
        assert_eq!(
            properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            fields
        );
        assert_eq!(
            properties["presentation_binding_digest_hex"]
                .get("$ref")
                .and_then(Value::as_str),
            Some("#/components/schemas/SorafsPopNonzeroHex32V1")
        );
        for property in properties.values() {
            assert!(property.get("default").is_none());
            assert!(property.get("nullable").is_none());
        }
        if catalog_openapi_route_enabled(CatalogHttpMethod::Post, path) {
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation_request_schema_ref(operation, path),
                format!("#/components/schemas/{schema_name}")
            );
            assert_eq!(
                operation["requestBody"]
                    .get("required")
                    .and_then(Value::as_bool),
                Some(true)
            );
            assert!(
                operation["description"]
                    .as_str()
                    .unwrap()
                    .contains("presentation_binding_digest_hex")
            );
            assert!(
                operation["description"]
                    .as_str()
                    .unwrap()
                    .contains("nullifier")
            );
            assert!(
                operation_header_requirements(operation)
                    .contains(&("Sora-PoP-Authorization".to_owned(), true))
            );
        }
    }
}

#[cfg(feature = "app_api")]
#[test]
fn sorafs_pop_openapi_native_requests_match_advertised_fields() {
    use crate::sorafs::pop_api::{PopMembershipRequestV1, PopVerifyMembershipRequestV1};

    let wallet = PopMembershipRequestV1 {
        credential_commitment_hex: "11".repeat(32),
        challenge_digest_hex: "22".repeat(32),
        verifier_context: "moderation:appeal:7".to_owned(),
        presentation_binding_digest_hex: "33".repeat(32),
    };
    let verify = PopVerifyMembershipRequestV1 {
        canonical_proof_base64url: "AQ".to_owned(),
        challenge_digest_hex: wallet.challenge_digest_hex.clone(),
        verifier_context: wallet.verifier_context.clone(),
        presentation_binding_digest_hex: wallet.presentation_binding_digest_hex.clone(),
    };
    let schemas = openapi_schemas();
    for (name, native_json) in [
        (
            "SorafsPopMembershipRequestV1",
            norito::json::to_value(&wallet).unwrap(),
        ),
        (
            "SorafsPopVerifyMembershipRequestV1",
            norito::json::to_value(&verify).unwrap(),
        ),
    ] {
        assert_eq!(
            native_json
                .as_object()
                .unwrap()
                .keys()
                .collect::<BTreeSet<_>>(),
            schemas[name]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<BTreeSet<_>>()
        );
    }
}

#[test]
fn sorafs_pop_openapi_digest_context_and_proof_bounds_match_native_limits() {
    use sorafs_manifest::pop_credentials::{
        POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1, POP_MEMBERSHIP_PROOF_MAX_BYTES_V1,
    };

    let schemas = openapi_schemas();
    let digest = schemas.get("SorafsPopNonzeroHex32V1").unwrap();
    assert_eq!(
        digest.get("pattern").and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    for field in ["minLength", "maxLength"] {
        assert_eq!(digest.get(field).and_then(Value::as_u64), Some(64));
    }
    let context = schemas.get("SorafsPopVerifierContextV1").unwrap();
    for field in ["maxLength", "x-iroha-max-bytes"] {
        assert_eq!(
            context.get(field).and_then(Value::as_u64),
            Some(POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1 as u64)
        );
    }
    assert_eq!(context.get("minLength").and_then(Value::as_u64), Some(1));
    let proof = schemas.get("SorafsPopMembershipProofBase64urlV1").unwrap();
    assert_eq!(
        proof
            .get("x-iroha-max-decoded-bytes")
            .and_then(Value::as_u64),
        Some(POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 as u64)
    );
    assert_eq!(
        proof.get("maxLength").and_then(Value::as_u64),
        Some((POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 * 4).div_ceil(3) as u64)
    );
    assert_eq!(proof.get("minLength").and_then(Value::as_u64), Some(1));
    assert_eq!(
        proof.get("pattern").and_then(Value::as_str),
        Some(
            r"^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-][AQgw]|[A-Za-z0-9_-]{2}[AEIMQUYcgkosw048])?$(?![\s\S])"
        )
    );
}
