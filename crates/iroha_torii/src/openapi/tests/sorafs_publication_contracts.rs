// Publication routes expose bounded canonical wire formats and state their verification boundary.

#[test]
fn sorafs_publication_contracts_bind_canonical_account_auth_and_evidence_bounds() {
    let document = compiled_spec();
    let routes = [
        (
            "source",
            "sorafs_car::publisher::PublisherSourceRequestV1",
            sorafs_car::publisher::PUBLISHER_SOURCE_REQUEST_MAX_BYTES_V1 as u64,
        ),
        (
            "prepare",
            "iroha_data_model::sorafs::pin_registry::ManifestDigest",
            4096,
        ),
        (
            "proof",
            "iroha_data_model::sorafs::publication::SorafsPublicationProofRequestV1",
            4096,
        ),
    ];
    for (name, schema, maximum) in routes {
        let path = format!("/v1/sorafs/publish/{name}");
        let operation = &document["paths"][path.as_str()]["post"];
        assert_eq!(
            operation["x-iroha-route-auth"]["authentication"].as_str(),
            Some("canonical_account_signature")
        );
        assert_eq!(
            operation["x-iroha-route-auth"]["admission"].as_str(),
            Some("authenticated_account")
        );
        let body = &operation["requestBody"]["content"]["application/x-norito"]["schema"];
        assert_eq!(body["x-iroha-norito-schema"].as_str(), Some(schema));
        assert_eq!(body["maxLength"].as_u64(), Some(maximum));
        assert!(operation["responses"]["401"].is_object());
        assert!(operation["responses"]["403"].is_object());
    }
    let source = &document["paths"]["/v1/sorafs/publish/source"]["post"];
    let description = source["description"].as_str().unwrap();
    for requirement in [
        "Metadata variant",
        "Chunk variant",
        "canonical header digest",
        "without repeating the complete metadata",
        "durable file identity",
    ] {
        assert!(
            description.contains(requirement),
            "missing source boundary: {requirement}"
        );
    }
    let proof = &document["paths"]["/v1/sorafs/publish/proof"]["post"];
    let description = proof["description"].as_str().unwrap();
    for requirement in [
        "CanReadAllLedgerData",
        "independent checkpoint",
        "only lineage",
        "SumeragiFinalityProof",
        "SumeragiFinalityCheckpoint",
        "configured network and chain label",
        "embedded current commit certificate",
        "cannot select its own trust root",
        "successful execution",
        "monotonic deadline",
    ] {
        assert!(
            description.contains(requirement),
            "missing proof boundary: {requirement}"
        );
    }
    assert_eq!(
        proof["responses"]["200"]["content"]["application/x-norito"]["schema"]["maxLength"]
            .as_u64(),
        Some(32 * 1024 * 1024)
    );
}

#[test]
fn repair_source_contract_requires_native_lease_and_bounded_authenticated_payload() {
    let document = compiled_spec();
    let route = &document["paths"]["/v1/sorafs/repair/source"]["post"];
    assert_eq!(
        route["x-iroha-route-auth"]["authentication"].as_str(),
        Some("canonical_account_signature")
    );
    assert_eq!(
        route["requestBody"]["content"]["application/x-norito"]["schema"]["maxLength"].as_u64(),
        Some(4096)
    );
    assert_eq!(
        route["responses"]["200"]["content"]["application/octet-stream"]["schema"]["maxLength"]
            .as_u64(),
        Some(4 * 1024 * 1024)
    );
    let description = route["description"].as_str().unwrap();
    for condition in [
        "active native repair lease",
        "before and after payload I/O",
        "worker permission",
        "revoked providers",
    ] {
        assert!(description.contains(condition));
    }
}

#[test]
fn provider_source_contract_binds_assignment_authority_and_metadata_chunk_bounds() {
    let document = compiled_spec();
    let operation = &document["paths"]["/v1/sorafs/provider/source"]["post"];
    let auth = &operation["x-iroha-route-auth"];
    assert_eq!(
        auth["authentication"].as_str(),
        Some("canonical_account_signature")
    );
    assert_eq!(auth["admission"].as_str(), Some("authenticated_account"));
    assert_eq!(
        auth["stableRouteId"].as_str(),
        Some("sorafs.provider.source")
    );
    assert_eq!(operation["x-iroha-tool-effect"].as_str(), Some("expensive"));
    let request = &operation["requestBody"]["content"]["application/x-norito"]["schema"];
    assert_eq!(request["maxLength"].as_u64(), Some(4096));
    assert_eq!(
        request["x-iroha-norito-schema"].as_str(),
        Some("iroha_data_model::sorafs::publication::SorafsAssignedSourceRequestV1")
    );
    let response = &operation["responses"]["200"]["content"]["application/x-norito"]["schema"];
    assert_eq!(
        response["maxLength"].as_u64(),
        Some(sorafs_car::publisher::PROVIDER_SOURCE_RESPONSE_MAX_BYTES_V1 as u64)
    );
    assert_eq!(
        response["x-iroha-norito-schema"].as_str(),
        Some("sorafs_car::publisher::ProviderSourceResponseV1")
    );
    assert_eq!(
        operation["responses"]["200"]["headers"]["Cache-Control"]["schema"]["enum"][0].as_str(),
        Some("private, no-store")
    );
    for status in ["400", "401", "403", "404", "413", "429", "503"] {
        assert!(
            operation["responses"][status].is_object(),
            "missing status {status}"
        );
    }
    let description = operation["description"].as_str().unwrap();
    for condition in [
        "target provider owner",
        "same-State durable finality",
        "both provider admissions",
        "assignment revision",
        "CanCompleteSorafsReplicationOrder",
        "before and after payload I/O",
        "healthy admitted payload",
        "Metadata variant once",
        "only the Chunk variant",
        "without repeating the complete metadata",
    ] {
        assert!(
            description.contains(condition),
            "missing authority boundary: {condition}"
        );
    }
}
