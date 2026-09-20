// The documented absence contract matches the typed query error envelope.
#[test]
fn query_asset_absence_openapi_requires_the_exact_selector() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let details = component_properties(schemas, "ErrorDetails");
    let selector = &details["query_asset_not_found"];
    assert_eq!(selector.get("type").and_then(Value::as_str), Some("string"));
    let operation = openapi_operation(&document, "/v1/query", "post");
    let missing = &operation["responses"]["404"];
    for media in ["application/json", "application/x-norito"] {
        assert_eq!(
            missing["content"][media].get("schema"),
            Some(&schema_ref("ErrorEnvelope"))
        );
    }
    let description = missing["description"].as_str().expect("absence contract");
    for required in [
        "query_asset_not_found",
        "details.query_asset_not_found",
        "exact requested AssetId",
        "Every selected dataspace route",
        "unavailable routes remain errors",
        "404 alone",
    ] {
        assert!(description.contains(required), "missing contract: {required}");
    }
}
