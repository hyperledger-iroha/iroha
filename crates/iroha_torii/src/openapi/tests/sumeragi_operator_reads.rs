#[test]
fn sumeragi_operator_reads_document_exact_signature_authentication() {
    use iroha_torii_shared::route_catalog::sumeragi;

    let document = generate_spec();
    let required_headers = BTreeSet::from([
        "X-Iroha-Operator-Public-Key".to_owned(),
        "X-Iroha-Operator-Timestamp-Ms".to_owned(),
        "X-Iroha-Operator-Nonce".to_owned(),
        "X-Iroha-Operator-Signature".to_owned(),
    ]);
    for route in [
        sumeragi::STATUS,
        sumeragi::DIAGNOSTICS,
        sumeragi::LEADER,
        sumeragi::BLS_KEYS,
        sumeragi::QC,
        sumeragi::CHECKPOINTS,
        sumeragi::COMMIT_CERTIFICATES,
        sumeragi::VALIDATOR_SETS,
        sumeragi::VALIDATOR_SET_BY_HEIGHT,
        sumeragi::CONSENSUS_KEYS,
        sumeragi::KEY_LIFECYCLE,
        sumeragi::TELEMETRY,
        sumeragi::PARAMETERS,
        sumeragi::COMMIT_QC,
        sumeragi::EVIDENCE_COUNT,
        sumeragi::EVIDENCE_LIST,
        sumeragi::VRF_PENALTIES,
        sumeragi::VRF_EPOCH,
    ] {
        if !catalog_openapi_route_enabled(CatalogHttpMethod::Get, route.path()) {
            continue;
        }
        let operation = openapi_operation(&document, route.path(), "get");
        assert_eq!(
            operation_header_requirements(operation)
                .into_iter()
                .map(|(name, required)| {
                    assert!(
                        required,
                        "{} signature headers must be required",
                        route.path()
                    );
                    name
                })
                .collect::<BTreeSet<_>>(),
            required_headers,
            "{}",
            route.path()
        );
        let contract = operation
            .get("x-iroha-operator-signature-v1")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{} operator signature contract", route.path()));
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
                "{} {exactness}",
                route.path()
            );
        }
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("Sumeragi operator responses");
        assert!(responses.contains_key("401"), "{}", route.path());
        assert!(responses.contains_key("403"), "{}", route.path());
    }

    if catalog_openapi_route_enabled(CatalogHttpMethod::Get, sumeragi::EVIDENCE_LIST.path()) {
        let evidence = openapi_operation(&document, sumeragi::EVIDENCE_LIST.path(), "get");
        let evidence_parameters = evidence
            .get("parameters")
            .and_then(Value::as_array)
            .expect("Sumeragi evidence parameters");
        for (name, maximum) in [("limit", 1_000_u64), ("offset", 10_000_u64)] {
            let schema = evidence_parameters
                .iter()
                .find(|parameter| {
                    parameter.get("name").and_then(Value::as_str) == Some(name)
                        && parameter.get("in").and_then(Value::as_str) == Some("query")
                })
                .and_then(|parameter| parameter.get("schema"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("Sumeragi evidence {name} query schema"));
            assert_eq!(schema.get("maximum"), Some(&Value::from(maximum)));
        }
    }
    if catalog_openapi_route_enabled(CatalogHttpMethod::Get, sumeragi::CONSENSUS_KEYS.path()) {
        let consensus_keys = openapi_operation(&document, sumeragi::CONSENSUS_KEYS.path(), "get");
        assert!(
            consensus_keys
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| description.contains("newest 128")),
            "consensus-key response cap must be explicit"
        );
    }
    if catalog_openapi_route_enabled(CatalogHttpMethod::Get, sumeragi::BLS_KEYS.path()) {
        let bls_keys = openapi_operation(&document, sumeragi::BLS_KEYS.path(), "get");
        assert!(
            bls_keys
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| {
                    description.contains("protocol maximum of 31 validators")
                        && description.contains("global peer registry is not cloned")
                }),
            "BLS-key response cap and bounded source must be explicit"
        );
    }

    if catalog_openapi_route_enabled(CatalogHttpMethod::Get, sumeragi::STATUS_SSE.path()) {
        let stream = openapi_operation(&document, sumeragi::STATUS_SSE.path(), "get");
        assert!(
            operation_header_requirements(stream)
                .into_iter()
                .all(|(name, _)| !name.starts_with("X-Iroha-Operator-")),
            "validator-roster SSE handshake must remain distinct from operator request auth"
        );
    }
}
