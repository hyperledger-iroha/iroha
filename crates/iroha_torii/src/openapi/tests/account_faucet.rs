#[cfg(feature = "app_api")]
#[test]
fn account_faucet_openapi_requires_exact_network_pow_for_mutation() {
    let document = generate_spec();
    let puzzle = openapi_operation(&document, "/v1/accounts/faucet/puzzle", "get");
    assert_eq!(
        operation_response_schema_ref(puzzle, "200", "/v1/accounts/faucet/puzzle"),
        "#/components/schemas/AccountFaucetPuzzle"
    );
    let claim = openapi_operation(&document, "/v1/accounts/faucet", "post");
    assert_eq!(
        operation_request_schema_ref(claim, "/v1/accounts/faucet"),
        "#/components/schemas/AccountFaucetRequest"
    );
    assert_eq!(
        operation_response_schema_ref(claim, "202", "/v1/accounts/faucet"),
        "#/components/schemas/AccountFaucetResponse"
    );
    let schemas = component_schemas(&document);
    let puzzle_schema = schemas
        .get("AccountFaucetPuzzle")
        .and_then(Value::as_object)
        .expect("AccountFaucetPuzzle schema");
    let puzzle_properties = puzzle_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("AccountFaucetPuzzle properties");
    assert_eq!(
        puzzle_properties
            .get("network_id")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NetworkId")
    );
    assert!(!puzzle_properties.contains_key("chain_id"));
    assert_eq!(
        puzzle_properties
            .get("difficulty_bits")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("minimum"))
            .and_then(Value::as_u64),
        Some(1)
    );
    let claim_required = schemas
        .get("AccountFaucetRequest")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .expect("AccountFaucetRequest required fields");
    for field in ["account_id", "pow_anchor_height", "pow_nonce_hex"] {
        assert!(
            claim_required
                .iter()
                .any(|value| value.as_str() == Some(field))
        );
    }
}
