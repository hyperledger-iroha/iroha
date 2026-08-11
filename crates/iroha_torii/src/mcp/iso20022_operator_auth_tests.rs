// ISO 20022 MCP inner-target authentication regressions.

#[test]
fn iso20022_operator_auth_is_complete_and_profile_is_query_bound() {
    let arguments = norito::json!({
        "operator_auth": {
            "public_key": "ed0120AABB",
            "timestamp_ms": 42,
            "nonce": "fresh-nonce",
            "signature": "c2lnbmF0dXJl"
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
        iso20022_route_with_profile("/v1/iso20022/pacs008", arguments)
            .expect("signed profile query"),
        "/v1/iso20022/pacs008?profile=swift%20cbpr%2B"
    );
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
