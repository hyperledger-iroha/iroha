#[test]
fn node_local_core_and_pipeline_reads_document_operator_signatures() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    let expected_headers = [
        "X-Iroha-Operator-Public-Key",
        "X-Iroha-Operator-Timestamp-Ms",
        "X-Iroha-Operator-Nonce",
        "X-Iroha-Operator-Signature",
    ];
    let expected_schemes = [
        ("IrohaOperatorPublicKey", "X-Iroha-Operator-Public-Key"),
        ("IrohaOperatorTimestampMs", "X-Iroha-Operator-Timestamp-Ms"),
        ("IrohaOperatorNonce", "X-Iroha-Operator-Nonce"),
        ("IrohaOperatorSignature", "X-Iroha-Operator-Signature"),
    ];
    let security_schemes = doc
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("securitySchemes"))
        .and_then(Value::as_object)
        .expect("operator security schemes");
    for (scheme, header) in expected_schemes {
        let definition = security_schemes
            .get(scheme)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing {scheme}"));
        assert_eq!(
            definition.get("type").and_then(Value::as_str),
            Some("apiKey")
        );
        assert_eq!(definition.get("in").and_then(Value::as_str), Some("header"));
        assert_eq!(definition.get("name").and_then(Value::as_str), Some(header));
    }

    for path in [
        "/v1/peers",
        "/v1/time/status",
        "/v1/pipeline/preflight",
        "/v1/policy",
        "/v1/proofs/retention",
        "/v1/pipeline/recovery/{height}",
    ] {
        let get = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|methods| methods.get("get"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("GET operation for {path}"));
        let description = get
            .get("description")
            .and_then(Value::as_str)
            .expect("operator description");
        assert!(description.contains("operator-only"), "{path}");
        assert!(description.contains("exact NetworkId"), "{path}");

        let parameters = get
            .get("parameters")
            .and_then(Value::as_array)
            .expect("operator signature parameters");
        for expected in expected_headers {
            let parameter = parameters.iter().find_map(|parameter| {
                let parameter = parameter.as_object()?;
                (parameter.get("name").and_then(Value::as_str) == Some(expected))
                    .then_some(parameter)
            });
            let parameter = parameter.unwrap_or_else(|| panic!("{path} missing {expected}"));
            assert_eq!(parameter.get("in").and_then(Value::as_str), Some("header"));
            assert_eq!(
                parameter.get("required").and_then(Value::as_bool),
                Some(true)
            );
        }
        let security = get
            .get("security")
            .and_then(Value::as_array)
            .and_then(|requirements| requirements.first())
            .and_then(Value::as_object)
            .expect("operator security requirement");
        for (scheme, _) in expected_schemes {
            assert!(security.contains_key(scheme), "{path} missing {scheme}");
        }
        let contract = get
            .get("x-iroha-operator-signature-v1")
            .and_then(Value::as_object)
            .expect("operator extension");
        assert_eq!(
            contract.get("replay_rejected").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            contract.get("redirects").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            contract.get("retries").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            contract.get("token_fallback").and_then(Value::as_bool),
            Some(false)
        );
    }

    let retention_get = paths
        .get("/v1/proofs/retention")
        .and_then(Value::as_object)
        .and_then(|methods| methods.get("get"))
        .and_then(Value::as_object)
        .expect("proof retention GET operation");
    let retention_description = retention_get
        .get("description")
        .and_then(Value::as_str)
        .expect("proof retention description");
    assert!(retention_description.contains(&format!(
        "more than {} distinct backend summaries",
        iroha_torii_shared::PROOF_RETENTION_STATUS_MAX_BACKENDS
    )));
    let retention_responses = retention_get
        .get("responses")
        .and_then(Value::as_object)
        .expect("proof retention responses");
    assert!(retention_responses.contains_key("422"));
}
