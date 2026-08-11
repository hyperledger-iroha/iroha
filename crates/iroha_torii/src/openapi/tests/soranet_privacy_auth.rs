/// OpenAPI assertions for exact SoraNet collector request authentication.

#[test]
fn soranet_privacy_ingest_requires_exact_operator_headers() {
    let document = generate_spec();
    let expected = std::collections::BTreeSet::from([
        "X-Iroha-Operator-Public-Key".to_owned(),
        "X-Iroha-Operator-Timestamp-Ms".to_owned(),
        "X-Iroha-Operator-Nonce".to_owned(),
        "X-Iroha-Operator-Signature".to_owned(),
    ]);

    for path in ["/v1/soranet/privacy/event", "/v1/soranet/privacy/share"] {
        let operation = openapi_operation(&document, path, "post");
        let headers = operation_header_requirements(operation)
            .into_iter()
            .map(|(name, required)| {
                assert!(required, "{path} must require `{name}`");
                name
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(headers, expected, "unexpected auth surface for {path}");
        let description = operation
            .get("description")
            .and_then(Value::as_str)
            .expect("SoraNet operation description");
        assert!(description.contains("exact NetworkId-bound operator signature"));
        assert!(!description.contains("token"));
    }
}
