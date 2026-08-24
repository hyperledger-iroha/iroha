// ISO 20022 MCP inner-target authentication regressions.
#[test]
fn iso20022_operator_auth_is_complete_and_profile_is_query_bound() {
    let key_pair =
        checked_torii_test_ed25519_keypair(0x75, "derive typed ISO 20022 operator-auth fixture");
    let public_key = key_pair
        .public_key()
        .try_to_multihash_string()
        .expect("canonical operator public key");
    let signature_bytes = iroha_crypto::Signature::new(
        key_pair.private_key(),
        b"typed ISO 20022 operator-auth fixture",
    );
    let signature = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        signature_bytes.payload(),
    );
    let arguments = norito::json!({
        "operator_auth": {
            "public_key": (public_key.clone()),
            "timestamp_ms": 42,
            "nonce": "fresh-nonce",
            "signature": (signature.clone())
        },
        "profile": "swift cbpr+"
    });
    let arguments = arguments.as_object().expect("arguments");
    let headers = iso20022_operator_auth_headers(arguments).expect("operator headers");
    let headers = headers.as_object().expect("operator header object");
    assert_eq!(headers.len(), 4);
    assert!(headers.contains_key("X-Iroha-Operator-Public-Key"));
    assert!(headers.contains_key("X-Iroha-Operator-Timestamp-Ms"));
    assert!(headers.contains_key("X-Iroha-Operator-Nonce"));
    assert!(headers.contains_key("X-Iroha-Operator-Signature"));
    assert_eq!(
        headers
            .get("X-Iroha-Operator-Public-Key")
            .and_then(Value::as_str),
        Some(public_key.as_str())
    );
    assert_eq!(
        headers
            .get("X-Iroha-Operator-Timestamp-Ms")
            .and_then(Value::as_str),
        Some("42")
    );
    assert_eq!(
        headers
            .get("X-Iroha-Operator-Signature")
            .and_then(Value::as_str),
        Some(signature.as_str())
    );
    assert_eq!(
        iso20022_route_with_profile("/v1/iso20022/pacs008", arguments)
            .expect("signed profile query"),
        "/v1/iso20022/pacs008?profile=swift%20cbpr%2B"
    );
}

#[test]
fn iso20022_profile_enforces_the_canonical_raw_query_limit() {
    const PREFIX_BYTES: usize = "profile=".len();
    let exact =
        "a".repeat(crate::app_auth::CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 - PREFIX_BYTES);
    let exact_arguments = norito::json!({ "profile": exact });
    let route = iso20022_route_with_profile(
        "/v1/iso20022/pacs008",
        exact_arguments.as_object().expect("arguments"),
    )
    .expect("exact profile limit");
    assert_eq!(
        route.len() - "/v1/iso20022/pacs008?".len(),
        crate::app_auth::CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1
    );

    let excessive =
        "a".repeat(crate::app_auth::CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 - PREFIX_BYTES + 1);
    let excessive_arguments = norito::json!({ "profile": excessive });
    iso20022_route_with_profile(
        "/v1/iso20022/pacs008",
        excessive_arguments.as_object().expect("arguments"),
    )
    .expect_err("profile above the raw-query limit");
}

#[test]
fn iso20022_operator_auth_rejects_noncanonical_or_unbounded_wire_values() {
    let key_pair = checked_torii_test_ed25519_keypair(
        0x76,
        "derive rejected typed ISO 20022 operator-auth fixture",
    );
    let public_key = key_pair
        .public_key()
        .try_to_multihash_string()
        .expect("canonical operator public key");
    let signature_bytes = iroha_crypto::Signature::new(
        key_pair.private_key(),
        b"rejected typed ISO 20022 operator-auth fixture",
    );
    let valid_signature = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        signature_bytes.payload(),
    );
    let auth = |public_key: Value, timestamp_ms: Value, nonce: Value, signature: Value| {
        norito::json!({
            "operator_auth": {
                "public_key": public_key,
                "timestamp_ms": timestamp_ms,
                "nonce": nonce,
                "signature": signature
            }
        })
    };
    let all_zero_signature =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0_u8; 64]);
    let short_signature =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0x11_u8; 63]);
    for invalid in [
        auth(
            Value::from(1_u64),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String(public_key.clone()),
            Value::String("42".to_owned()),
            Value::String("nonce".to_owned()),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String("A".repeat(OPERATOR_PUBLIC_KEY_MAX_LITERAL_BYTES + 1)),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String("ed0120AABB".to_owned()),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String(public_key.clone()),
            Value::from(42_u64),
            Value::String("contains space".to_owned()),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String(public_key.clone()),
            Value::from(42_u64),
            Value::String("n".repeat(257)),
            Value::String(valid_signature.clone()),
        ),
        auth(
            Value::String(public_key.clone()),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String("not-base64".to_owned()),
        ),
        auth(
            Value::String(public_key.clone()),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String(short_signature),
        ),
        auth(
            Value::String(public_key),
            Value::from(42_u64),
            Value::String("nonce".to_owned()),
            Value::String(all_zero_signature),
        ),
    ] {
        iso20022_operator_auth_headers(invalid.as_object().expect("arguments"))
            .expect_err("invalid typed operator wire values must fail before copying");
    }
}
#[test]
fn iso20022_mcp_schema_retires_raw_headers_and_requires_inner_operator_auth() {
    let config = iroha_config::parameters::actual::ToriiMcp::default();
    let tools = build_tool_specs(&config);
    let iso_tools = tools
        .iter()
        .filter(|tool| tool.name.starts_with("iroha.iso20022."))
        .collect::<Vec<_>>();
    assert_eq!(iso_tools.len(), 10);
    for tool in iso_tools {
        let schema = tool.input_schema.as_object().expect("ISO tool schema");
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("ISO required fields");
        assert!(
            required
                .iter()
                .any(|field| field.as_str() == Some("operator_auth")),
            "{} must require inner operator auth",
            tool.name
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("ISO properties");
        assert!(properties.contains_key("operator_auth"));
        if tool.name == "iroha.iso20022.status.get" {
            assert!(properties.contains_key("path"));
            assert!(!properties.contains_key("msg_id"));
            assert!(!properties.contains_key("message_id"));
            assert!(!properties.contains_key("id"));
        } else {
            assert!(properties.contains_key("body_base64"));
            assert!(!properties.contains_key("message_xml"));
            assert!(!properties.contains_key("xml"));
            assert!(!properties.contains_key("body"));
            assert!(
                required
                    .iter()
                    .any(|field| field.as_str() == Some("body_base64")),
                "{} must require canonical XML bytes",
                tool.name
            );
        }
        assert!(
            !properties.contains_key("headers"),
            "{} retains raw header injection",
            tool.name
        );
    }
    let retired = norito::json!({
        "message_xml": "<Document/>",
        "headers": { "X-Iroha-Iso-Profile": "swift-cbpr-plus" }
    });
    assert!(
        build_iso20022_payload_body(retired.as_object().expect("retired arguments"))
            .expect_err("retired raw headers")
            .contains("unexpected `headers`")
    );
}
