#[test]
fn kaigi_typed_routes_document_dual_responses() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    let required_headers = BTreeSet::from([
        "X-Iroha-Operator-Public-Key".to_owned(),
        "X-Iroha-Operator-Timestamp-Ms".to_owned(),
        "X-Iroha-Operator-Nonce".to_owned(),
        "X-Iroha-Operator-Signature".to_owned(),
    ]);
    for path in [
        "/v1/kaigi/relays",
        "/v1/kaigi/relays/{relay_id}",
        "/v1/kaigi/relays/health",
    ] {
        let operation = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|path_item| path_item.get("get"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} GET operation"));
        assert_eq!(
            operation_header_requirements(operation)
                .into_iter()
                .map(|(name, required)| {
                    assert!(required, "{path} operator header must be required");
                    name
                })
                .collect::<BTreeSet<_>>(),
            required_headers,
            "{path} exact operator headers"
        );
        let contract = operation
            .get("x-iroha-operator-signature-v1")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} operator signature contract"));
        for exactness in [
            "exact_network_id",
            "exact_method",
            "exact_path_and_sorted_query",
            "empty_body_hash",
            "fresh_timestamp_and_nonce",
        ] {
            assert_eq!(
                contract.get(exactness).and_then(Value::as_bool),
                Some(true),
                "{path} {exactness}"
            );
        }
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("Kaigi operator responses");
        assert!(responses.contains_key("401"), "{path}");
        assert!(responses.contains_key("403"), "{path}");
        if path != "/v1/kaigi/relays/{relay_id}" {
            assert!(responses.contains_key("422"), "{path}");
        }
        let content = responses
            .get("200")
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} 200 response content"));
        assert!(content.contains_key("application/json"));
        assert!(content.contains_key("application/x-norito"));
    }
    let schemas = doc
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("Kaigi schemas");
    for (schema, property) in [
        ("KaigiRelaySummaryList", "items"),
        ("KaigiRelayHealthSnapshot", "domains"),
    ] {
        assert_eq!(
            schemas
                .get(schema)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(property))
                .and_then(Value::as_object)
                .and_then(|property| property.get("maxItems"))
                .and_then(Value::as_u64),
            Some(crate::routing::KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS as u64),
            "{schema}.{property} hard bound"
        );
    }
    assert_eq!(
        schemas
            .get("KaigiRelaySummaryList")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("total"))
            .and_then(Value::as_object)
            .and_then(|total| total.get("maximum"))
            .and_then(Value::as_u64),
        Some(crate::routing::KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS as u64),
        "KaigiRelaySummaryList.total hard bound"
    );
}
