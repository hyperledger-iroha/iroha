#[test]
fn sns_name_absence_openapi_is_typed_and_selector_bound() {
    use iroha_data_model::sns::NameSelectorV1;
    use iroha_torii_shared::sns::{SNS_REGISTRATION_NOT_FOUND_CODE, SnsRegistrationNotFoundV1};

    let document = canonical_document();
    let operation = openapi_operation(&document, "/v1/sns/names/{namespace}/{literal}", "get");
    let missing = operation
        .get("responses")
        .and_then(|responses| responses.get("404"))
        .expect("SNS missing-registration response");
    let content = missing
        .get("content")
        .and_then(Value::as_object)
        .expect("SNS not-found media types");
    assert_eq!(
        content.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["application/json", "text/plain"])
    );
    assert_eq!(
        content["application/json"].get("schema"),
        Some(&schema_ref("SnsRegistrationNotFoundV1"))
    );
    assert_eq!(
        content["text/plain"]
            .get("schema")
            .and_then(|schema| schema.get("type"))
            .and_then(Value::as_str),
        Some("string")
    );
    let description = missing.get("description").and_then(Value::as_str).unwrap();
    for distinction in ["exact requested", "text/plain", "404 alone"] {
        assert!(description.contains(distinction));
    }

    let schemas = component_schemas(&document);
    let schema = &schemas["SnsRegistrationNotFoundV1"];
    assert_eq!(schema.get("type").and_then(Value::as_str), Some("object"));
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false)
    );
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let selector = NameSelectorV1::new(4099, "dpn").unwrap();
    let absence = SnsRegistrationNotFoundV1::new(selector.suffix_id, selector.label.clone());
    let encoded = norito::json::to_vec(&absence).unwrap();
    let native: Value = norito::json::from_slice(&encoded).unwrap();
    let native_keys = native
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(required, native_keys);
    let properties = component_properties(schemas, "SnsRegistrationNotFoundV1");
    assert_eq!(
        properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        native_keys
    );
    assert_eq!(
        properties["code"].get("const").and_then(Value::as_str),
        Some(SNS_REGISTRATION_NOT_FOUND_CODE)
    );
    assert_eq!(
        properties["suffix_id"].get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        properties["suffix_id"]
            .get("minimum")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        properties["suffix_id"]
            .get("maximum")
            .and_then(Value::as_u64),
        Some(u64::from(u16::MAX))
    );
    assert_eq!(
        properties["label"].get("type").and_then(Value::as_str),
        Some("string")
    );
    assert!(absence.matches_selector(&selector));
    assert!(!absence.matches_selector(&NameSelectorV1::new(4099, "other").unwrap()));
}
