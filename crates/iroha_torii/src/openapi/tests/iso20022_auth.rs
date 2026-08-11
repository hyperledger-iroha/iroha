#[test]
fn iso20022_operations_require_fresh_operator_signatures() {
    let document = generate_spec();
    let expected = BTreeSet::from([
        "X-Iroha-Operator-Public-Key".to_owned(),
        "X-Iroha-Operator-Timestamp-Ms".to_owned(),
        "X-Iroha-Operator-Nonce".to_owned(),
        "X-Iroha-Operator-Signature".to_owned(),
    ]);

    for descriptor in iroha_torii_shared::route_catalog::iso20022::ROUTES {
        let method = match descriptor.method() {
            CatalogHttpMethod::Get => "get",
            CatalogHttpMethod::Post => "post",
            other => panic!("unexpected ISO 20022 method: {other:?}"),
        };
        let headers =
            operation_header_requirements(openapi_operation(&document, descriptor.path(), method))
                .into_iter()
                .filter_map(|(name, required)| {
                    name.starts_with("X-Iroha-Operator-").then(|| {
                        assert!(
                            required,
                            "{method} {} {name} must be required",
                            descriptor.path()
                        );
                        name
                    })
                })
                .collect::<BTreeSet<_>>();
        assert_eq!(headers, expected, "{method} {}", descriptor.path());
    }
}
