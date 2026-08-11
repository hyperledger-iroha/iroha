fn space_directory_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/space-directory/manifests".to_owned(),
        Value::Object(canonical_account_operations::json_post(
            "SpaceDirectory",
            "Build a manifest-publication transaction draft.",
            "Validate and quote a manifest publication and return a canonical unsigned transaction payload for local signing. Torii does not accept a private key or submit the draft. The exact-NetworkId authenticated account must equal the body authority.",
            "#/components/schemas/SpaceDirectoryManifestPublishDraftRequestV1",
            "#/components/schemas/AppApiTransactionDraftV1",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/space-directory/manifests/revoke".to_owned(),
        Value::Object(canonical_account_operations::json_post(
            "SpaceDirectory",
            "Build a manifest-revocation transaction draft.",
            "Validate and quote a manifest revocation and return a canonical unsigned transaction payload for local signing. Torii does not accept a private key or submit the draft. The exact-NetworkId authenticated account must equal the body authority.",
            "#/components/schemas/SpaceDirectoryManifestRevokeDraftRequestV1",
            "#/components/schemas/AppApiTransactionDraftV1",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/space-directory/uaids/{uaid}".to_owned(),
        Value::Object(json_get_operation(
            "SpaceDirectory",
            "Fetch space directory bindings.",
            "Fetch bindings for a user account identifier.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("uaid", "User account identifier.")],
        )),
    );
    paths.insert(
        "/v1/space-directory/uaids/{uaid}/manifests".to_owned(),
        Value::Object(json_get_operation(
            "SpaceDirectory",
            "Fetch space directory manifests.",
            "Fetch manifests registered for a user account identifier.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("uaid", "User account identifier.")],
        )),
    );
    paths
}
