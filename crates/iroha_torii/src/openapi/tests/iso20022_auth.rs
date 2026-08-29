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
        let all_headers =
            operation_header_requirements(openapi_operation(&document, descriptor.path(), method));
        assert!(
            all_headers
                .iter()
                .all(|(name, _)| name != "X-Iroha-Iso-Profile"),
            "{method} {} retains the unsigned profile selector",
            descriptor.path()
        );
        let headers = all_headers
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

#[test]
fn iso20022_openapi_documents_party_scope_durable_admission_and_signed_xml() {
    let document = generate_spec();
    let component_headers = document["components"]["headers"]
        .as_object()
        .expect("OpenAPI component headers");
    assert_eq!(
        component_headers["IsoOutboundSignatureDomainV2"]["schema"]["const"].as_str(),
        Some("iroha.iso20022.outbound.v2")
    );
    assert_eq!(
        component_headers["IsoOutboundSignatureV2"]["schema"]["pattern"].as_str(),
        Some("^[A-Za-z0-9+/]+={0,2}$")
    );
    assert_eq!(
        component_headers["IsoOutboundSignerV2"]["required"].as_bool(),
        Some(true)
    );

    let expected_signature_headers = [
        (
            "X-Iroha-Iso-Signature-Domain",
            "#/components/headers/IsoOutboundSignatureDomainV2",
        ),
        (
            "X-Iroha-Iso-Signature",
            "#/components/headers/IsoOutboundSignatureV2",
        ),
        (
            "X-Iroha-Iso-Signer",
            "#/components/headers/IsoOutboundSignerV2",
        ),
    ];
    for path in [
        "/v1/iso20022/messages/{msg_id}/camt029",
        "/v1/iso20022/messages/{msg_id}/pacs002",
        "/v1/iso20022/messages/{msg_id}/pacs004",
        "/v1/iso20022/messages/{msg_id}/sese024",
        "/v1/iso20022/messages/{msg_id}/sese025",
    ] {
        let operation = openapi_operation(&document, path, "get");
        let description = operation["description"]
            .as_str()
            .expect("ISO XML operation description");
        assert!(description.contains("party-scoped"), "{path}");
        assert!(description.contains("exact response bytes"), "{path}");
        let response = &operation["responses"]["200"];
        assert_eq!(
            response["content"]["application/xml"]["schema"]["$ref"].as_str(),
            Some("#/components/schemas/XmlText"),
            "{path}"
        );
        let headers = response["headers"]
            .as_object()
            .expect("signed XML response headers");
        for (name, expected_ref) in expected_signature_headers {
            assert_eq!(headers[name]["$ref"].as_str(), Some(expected_ref), "{path}");
        }
    }

    let message_description =
        openapi_operation(&document, "/v1/iso20022/messages/{msg_id}", "get")
            ["description"]
            .as_str()
            .expect("ISO record description");
    assert!(message_description.contains("either original participant"));
    assert!(message_description.contains("audit-admin"));
    let audit_description =
        openapi_operation(&document, "/v1/iso20022/audit/messages", "get")["description"]
            .as_str()
            .expect("ISO audit description");
    assert!(audit_description.contains("originator or counterparty"));
    assert!(audit_description.contains("global read-only"));

    for path in [
        "/v1/iso20022/pacs008",
        "/v1/iso20022/pacs009",
        "/v1/iso20022/pacs002",
        "/v1/iso20022/pacs004",
        "/v1/iso20022/camt056",
        "/v1/iso20022/sese023",
        "/v1/iso20022/sese024",
        "/v1/iso20022/sese025",
        "/v1/iso20022/colr012",
    ] {
        let responses = openapi_operation(&document, path, "post")["responses"]
            .as_object()
            .expect("ISO submission responses");
        assert!(responses.contains_key("202"), "{path}");
        assert!(responses.contains_key("503"), "{path}");
        assert!(!responses.contains_key("200"), "{path}");
    }
}
