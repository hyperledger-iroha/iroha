// Musubi V1 OpenAPI schema construction.

fn musubi_closed_object(required: &[&str], properties: Vec<(&str, Value)>) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, schema)| (name.to_owned(), schema))
        .collect::<Map>();
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert(
            "required".to_owned(),
            Value::Array(
                required
                    .iter()
                    .map(|name| Value::String((*name).to_owned()))
                    .collect(),
            ),
        );
    }
    Value::Object(schema)
}

fn musubi_array(items: Value, minimum: usize, maximum: usize) -> Value {
    norito::json!({
        "type": "array",
        "minItems": (minimum),
        "maxItems": (maximum),
        "items": (items)
    })
}

fn musubi_nullable(schema: Value) -> Value {
    norito::json!({ "oneOf": [(schema), { "type": "null" }] })
}

fn musubi_string_newtype(maximum: usize, pattern: Option<&str>) -> Value {
    let mut item = Map::new();
    item.insert("type".to_owned(), Value::String("string".to_owned()));
    item.insert("minLength".to_owned(), Value::from(1_u64));
    item.insert(
        "maxLength".to_owned(),
        Value::from(u64::try_from(maximum).expect("Musubi string bound fits u64")),
    );
    if let Some(pattern) = pattern {
        item.insert("pattern".to_owned(), Value::String(pattern.to_owned()));
    }
    musubi_array(Value::Object(item), 1, 1)
}

fn musubi_tagged_variant(kind: &str, value: Value) -> Value {
    musubi_closed_object(
        &["kind", "value"],
        vec![
            ("kind", norito::json!({ "type": "string", "const": (kind) })),
            ("value", value),
        ],
    )
}

fn musubi_tagged_union(variants: Vec<(&str, Value)>) -> Value {
    Value::Object(Map::from([(
        "oneOf".to_owned(),
        Value::Array(
            variants
                .into_iter()
                .map(|(kind, value)| musubi_tagged_variant(kind, value))
                .collect(),
        ),
    )]))
}

fn musubi_tagged_units(kinds: &[&str]) -> Value {
    musubi_tagged_union(
        kinds
            .iter()
            .map(|kind| (*kind, norito::json!({ "type": "null" })))
            .collect(),
    )
}

fn musubi_page_schema(query: Value, item: Value) -> Value {
    musubi_closed_object(
        &["query", "items", "next_cursor", "snapshot"],
        vec![
            ("query", query),
            (
                "items",
                musubi_array(item, 0, iroha_data_model::musubi::MUSUBI_MAX_PAGE_SIZE_V1),
            ),
            (
                "next_cursor",
                musubi_nullable(schema_ref("MusubiFinalizedCursorV1")),
            ),
            ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
        ],
    )
}

fn insert_musubi_v1_schemas(schemas: &mut Map) {
    use iroha_data_model::musubi::{
        MUSUBI_MAX_ALIAS_BYTES_V1, MUSUBI_MAX_CURSOR_KEY_BYTES_V1, MUSUBI_MAX_NAMESPACE_BYTES_V1,
        MUSUBI_MAX_PACKAGE_NAME_BYTES_V1, MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1,
        MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1, MUSUBI_MAX_VERSION_COMPARATORS_V1,
    };

    let byte =
        norito::json!({ "type": "integer", "format": "uint8", "minimum": 0, "maximum": 255 });
    let fixed_32 = musubi_array(byte.clone(), 32, 32);
    let digest_32 = musubi_array(schema_ref("MusubiFixed32BytesV1"), 1, 1);
    let u64_schema = norito::json!({ "type": "integer", "format": "uint64", "minimum": 0 });
    let positive_u64 = norito::json!({ "type": "integer", "format": "uint64", "minimum": 1 });
    let account = norito::json!({
        "type": "string",
        "minLength": 1,
        "description": "Canonical domainless I105 AccountId; native multisignature policies are bounded by the enclosing request or response body rather than a single-key text limit."
    });

    schemas.insert("MusubiByteV1".to_owned(), byte);
    schemas.insert("MusubiFixed32BytesV1".to_owned(), fixed_32);
    schemas.insert("MusubiDigest32V1".to_owned(), digest_32);
    schemas.insert("MusubiU64V1".to_owned(), u64_schema.clone());
    schemas.insert("MusubiPositiveU64V1".to_owned(), positive_u64.clone());
    schemas.insert("MusubiAccountIdV1".to_owned(), account.clone());
    schemas.insert(
        "MusubiChainIdV1".to_owned(),
        norito::json!({
            "type": "string",
            "minLength": 1,
            "maxLength": (iroha_data_model::id::MAX_CHAIN_ID_BYTES),
            "pattern": "^[A-Za-z0-9](?:[A-Za-z0-9._:-]*[A-Za-z0-9])?$"
        }),
    );
    schemas.insert(
        "MusubiNamespaceV1".to_owned(),
        musubi_string_newtype(MUSUBI_MAX_NAMESPACE_BYTES_V1, None),
    );
    schemas.insert(
        "MusubiPackageNameV1".to_owned(),
        musubi_string_newtype(
            MUSUBI_MAX_PACKAGE_NAME_BYTES_V1,
            Some("^[a-z0-9]+(?:-[a-z0-9]+)*$"),
        ),
    );
    schemas.insert(
        "MusubiAliasNameV1".to_owned(),
        musubi_string_newtype(
            MUSUBI_MAX_ALIAS_BYTES_V1,
            Some("^[a-z0-9]+(?:-[a-z0-9]+)*$"),
        ),
    );
    schemas.insert(
        "MusubiReasonV1".to_owned(),
        musubi_string_newtype(1_024, None),
    );
    schemas.insert(
        "MusubiDescriptionV1".to_owned(),
        musubi_string_newtype(4_096, None),
    );
    schemas.insert(
        "MusubiDocumentRefV1".to_owned(),
        musubi_string_newtype(2_048, None),
    );
    schemas.insert(
        "MusubiKeywordV1".to_owned(),
        musubi_string_newtype(64, Some("^[a-z0-9]+(?:-[a-z0-9]+)*$")),
    );
    schemas.insert(
        "MusubiPackageScopeV1".to_owned(),
        musubi_tagged_union(vec![
            ("DataspaceRoot", norito::json!({ "type": "null" })),
            (
                "Domain",
                norito::json!({ "type": "string", "minLength": 1, "maxLength": 255 }),
            ),
        ]),
    );
    schemas.insert(
        "MusubiPackageIdV1".to_owned(),
        musubi_closed_object(
            &["home_dataspace", "scope", "name"],
            vec![
                ("home_dataspace", u64_schema.clone()),
                ("scope", schema_ref("MusubiPackageScopeV1")),
                ("name", schema_ref("MusubiPackageNameV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiPackageSelectorV1".to_owned(),
        musubi_closed_object(
            &["namespace", "name"],
            vec![
                ("namespace", schema_ref("MusubiNamespaceV1")),
                ("name", schema_ref("MusubiPackageNameV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiNamespaceBindingV1".to_owned(),
        musubi_closed_object(
            &["namespace", "home_dataspace", "scope", "generation"],
            vec![
                ("namespace", schema_ref("MusubiNamespaceV1")),
                ("home_dataspace", u64_schema.clone()),
                ("scope", schema_ref("MusubiPackageScopeV1")),
                ("generation", positive_u64.clone()),
            ],
        ),
    );

    let prerelease_identifier = musubi_tagged_union(vec![
        ("Numeric", u64_schema.clone()),
        (
            "AlphaNumeric",
            norito::json!({
                "type": "string",
                "minLength": 1,
                "maxLength": (MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1),
                "pattern": "^[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*$"
            }),
        ),
    ]);
    schemas.insert(
        "MusubiPrereleaseIdentifierV1".to_owned(),
        prerelease_identifier,
    );
    schemas.insert(
        "MusubiVersionV1".to_owned(),
        musubi_closed_object(
            &["major", "minor", "patch", "prerelease"],
            vec![
                ("major", u64_schema.clone()),
                ("minor", u64_schema.clone()),
                ("patch", u64_schema.clone()),
                (
                    "prerelease",
                    musubi_array(
                        schema_ref("MusubiPrereleaseIdentifierV1"),
                        0,
                        MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1,
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiReleaseIdV1".to_owned(),
        musubi_closed_object(
            &["package", "version"],
            vec![
                ("package", schema_ref("MusubiPackageIdV1")),
                ("version", schema_ref("MusubiVersionV1")),
            ],
        ),
    );
    let comparator_operator =
        musubi_tagged_units(&["Greater", "GreaterOrEqual", "Less", "LessOrEqual", "Equal"]);
    schemas.insert("MusubiComparatorOpV1".to_owned(), comparator_operator);
    schemas.insert(
        "MusubiVersionComparatorV1".to_owned(),
        musubi_closed_object(
            &["op", "version"],
            vec![
                ("op", schema_ref("MusubiComparatorOpV1")),
                ("version", schema_ref("MusubiVersionV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiMinorWildcardV1".to_owned(),
        musubi_closed_object(
            &["major", "minor"],
            vec![("major", u64_schema.clone()), ("minor", u64_schema.clone())],
        ),
    );
    schemas.insert(
        "MusubiVersionReqV1".to_owned(),
        musubi_tagged_union(vec![
            ("Any", norito::json!({ "type": "null" })),
            ("Caret", schema_ref("MusubiVersionV1")),
            ("Tilde", schema_ref("MusubiVersionV1")),
            ("MajorWildcard", u64_schema.clone()),
            ("MinorWildcard", schema_ref("MusubiMinorWildcardV1")),
            ("Exact", schema_ref("MusubiVersionV1")),
            (
                "Comparators",
                musubi_array(
                    schema_ref("MusubiVersionComparatorV1"),
                    1,
                    MUSUBI_MAX_VERSION_COMPARATORS_V1,
                ),
            ),
        ]),
    );

    schemas.insert(
        "MusubiRegistrySnapshotV1".to_owned(),
        musubi_closed_object(
            &["finalized_height", "finalized_block_hash", "index_revision"],
            vec![
                ("finalized_height", positive_u64.clone()),
                ("finalized_block_hash", schema_ref("MusubiFixed32BytesV1")),
                ("index_revision", positive_u64.clone()),
            ],
        ),
    );
    schemas.insert(
        "MusubiFinalizedCursorV1".to_owned(),
        musubi_closed_object(
            &["snapshot", "query_hash", "last_key", "caller"],
            vec![
                ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ("query_hash", schema_ref("MusubiDigest32V1")),
                (
                    "last_key",
                    norito::json!({
                        "type": "string",
                        "minLength": 1,
                        "maxLength": (MUSUBI_MAX_CURSOR_KEY_BYTES_V1)
                    }),
                ),
                ("caller", musubi_nullable(account.clone())),
            ],
        ),
    );
    schemas.insert(
        "MusubiPageRequestV1".to_owned(),
        musubi_closed_object(
            &["limit", "cursor"],
            vec![
                (
                    "limit",
                    norito::json!({
                        "type": "integer",
                        "format": "uint32",
                        "minimum": 0,
                        "maximum": (iroha_data_model::musubi::MUSUBI_MAX_PAGE_SIZE_V1)
                    }),
                ),
                (
                    "cursor",
                    musubi_nullable(schema_ref("MusubiFinalizedCursorV1")),
                ),
            ],
        ),
    );

    // The remaining schemas deliberately describe the canonical Norito JSON shape rather than
    // treating a typed Musubi payload as an arbitrary JSON value. This also gives MCP a closed
    // request-body root for every V1 query and unsigned instruction builder.
    insert_musubi_release_and_archive_schemas(schemas);
    insert_musubi_governance_and_route_schemas(schemas);
}

fn insert_musubi_release_and_archive_schemas(schemas: &mut Map) {
    use iroha_data_model::musubi::{
        MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1,
        MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_CHUNKS_V1, MUSUBI_MAX_DEPENDENCIES_V1,
        MUSUBI_MAX_EXPORTS_V1, MUSUBI_MAX_FILES_V1, MUSUBI_MAX_KEYWORDS_V1,
        MUSUBI_MAX_LOCATION_PROVIDERS_V1, MUSUBI_MAX_NAMESPACE_DELEGATION_APPROVALS_V1,
        MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1, MUSUBI_MAX_RESOLUTION_NODES_V1,
    };

    schemas.insert(
        "MusubiAbiBindingV1".to_owned(),
        musubi_closed_object(
            &["abi_version", "abi_hash"],
            vec![
                (
                    "abi_version",
                    norito::json!({ "type": "integer", "format": "uint16", "const": 1 }),
                ),
                ("abi_hash", schema_ref("MusubiFixed32BytesV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiKotodamaEditionV1".to_owned(),
        musubi_tagged_units(&["V1"]),
    );
    schemas.insert(
        "MusubiDependencyKindV1".to_owned(),
        musubi_tagged_units(&["Normal", "Development"]),
    );
    schemas.insert(
        "MusubiDependencyReqV1".to_owned(),
        musubi_closed_object(
            &["alias", "package", "requirement"],
            vec![
                (
                    "alias",
                    norito::json!({ "type": "string", "minLength": 1, "maxLength": 255 }),
                ),
                ("package", schema_ref("MusubiPackageIdV1")),
                ("requirement", schema_ref("MusubiVersionReqV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiReleaseMetadataV1".to_owned(),
        musubi_closed_object(
            &["description", "readme", "license", "repository", "keywords"],
            vec![
                (
                    "description",
                    musubi_nullable(schema_ref("MusubiDescriptionV1")),
                ),
                ("readme", musubi_nullable(schema_ref("MusubiDocumentRefV1"))),
                (
                    "license",
                    musubi_nullable(schema_ref("MusubiDocumentRefV1")),
                ),
                (
                    "repository",
                    musubi_nullable(schema_ref("MusubiDocumentRefV1")),
                ),
                (
                    "keywords",
                    musubi_array(schema_ref("MusubiKeywordV1"), 0, MUSUBI_MAX_KEYWORDS_V1),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiReleaseManifestV1".to_owned(),
        musubi_closed_object(
            &[
                "release",
                "edition",
                "abi",
                "dependencies",
                "exports",
                "interface_digest",
                "metadata",
                "archive_id",
                "verification_lock_digest",
            ],
            vec![
                ("release", schema_ref("MusubiReleaseIdV1")),
                ("edition", schema_ref("MusubiKotodamaEditionV1")),
                ("abi", schema_ref("MusubiAbiBindingV1")),
                (
                    "dependencies",
                    musubi_array(
                        schema_ref("MusubiDependencyReqV1"),
                        0,
                        MUSUBI_MAX_DEPENDENCIES_V1,
                    ),
                ),
                (
                    "exports",
                    musubi_array(
                        norito::json!({ "type": "string", "minLength": 1, "maxLength": 255 }),
                        0,
                        MUSUBI_MAX_EXPORTS_V1,
                    ),
                ),
                ("interface_digest", schema_ref("MusubiDigest32V1")),
                ("metadata", schema_ref("MusubiReleaseMetadataV1")),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("verification_lock_digest", schema_ref("MusubiDigest32V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiExactDependencyEdgeV1".to_owned(),
        musubi_closed_object(
            &["alias", "kind", "package", "requirement", "selected"],
            vec![
                (
                    "alias",
                    norito::json!({ "type": "string", "minLength": 1, "maxLength": 255 }),
                ),
                ("kind", schema_ref("MusubiDependencyKindV1")),
                ("package", schema_ref("MusubiPackageIdV1")),
                ("requirement", schema_ref("MusubiVersionReqV1")),
                ("selected", schema_ref("MusubiReleaseIdV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiVerificationNodeV1".to_owned(),
        musubi_closed_object(
            &[
                "release",
                "release_digest",
                "archive_id",
                "source_digest",
                "interface_digest",
                "abi",
                "dependencies",
            ],
            vec![
                ("release", schema_ref("MusubiReleaseIdV1")),
                ("release_digest", schema_ref("MusubiDigest32V1")),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("source_digest", schema_ref("MusubiDigest32V1")),
                ("interface_digest", schema_ref("MusubiDigest32V1")),
                ("abi", schema_ref("MusubiAbiBindingV1")),
                (
                    "dependencies",
                    musubi_array(
                        schema_ref("MusubiExactDependencyEdgeV1"),
                        0,
                        MUSUBI_MAX_DEPENDENCIES_V1,
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiVerificationLockV1".to_owned(),
        musubi_closed_object(
            &["schema", "version", "root", "root_dependencies", "nodes"],
            vec![
                (
                    "schema",
                    norito::json!({ "type": "string", "const": "musubi-verification-lock" }),
                ),
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                ("root", schema_ref("MusubiReleaseIdV1")),
                (
                    "root_dependencies",
                    musubi_array(
                        schema_ref("MusubiExactDependencyEdgeV1"),
                        0,
                        MUSUBI_MAX_DEPENDENCIES_V1,
                    ),
                ),
                (
                    "nodes",
                    musubi_array(
                        schema_ref("MusubiVerificationNodeV1"),
                        0,
                        MUSUBI_MAX_RESOLUTION_NODES_V1,
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiResolutionProofV1".to_owned(),
        musubi_closed_object(
            &["snapshot", "lock"],
            vec![
                ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ("lock", schema_ref("MusubiVerificationLockV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiPublicationV1".to_owned(),
        musubi_closed_object(
            &["manifest", "resolution"],
            vec![
                ("manifest", schema_ref("MusubiReleaseManifestV1")),
                ("resolution", schema_ref("MusubiResolutionProofV1")),
            ],
        ),
    );

    schemas.insert(
        "MusubiManifestRootCidV1".to_owned(),
        musubi_array(
            schema_ref("MusubiByteV1"),
            iroha_data_model::sorafs::pin_registry::MANIFEST_ROOT_CID_LENGTH,
            iroha_data_model::sorafs::pin_registry::MANIFEST_ROOT_CID_LENGTH,
        ),
    );
    schemas.insert(
        "MusubiChunkerProfileHandleV1".to_owned(),
        musubi_closed_object(
            &[
                "profile_id",
                "namespace",
                "name",
                "semver",
                "multihash_code",
            ],
            vec![
                (
                    "profile_id",
                    norito::json!({ "type": "integer", "format": "uint32", "minimum": 0 }),
                ),
                (
                    "namespace",
                    norito::json!({ "type": "string", "minLength": 1, "maxLength": 128 }),
                ),
                (
                    "name",
                    norito::json!({ "type": "string", "minLength": 1, "maxLength": 128 }),
                ),
                (
                    "semver",
                    norito::json!({ "type": "string", "minLength": 1, "maxLength": 128 }),
                ),
                (
                    "multihash_code",
                    norito::json!({ "type": "integer", "format": "uint64", "minimum": 0 }),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiArchiveCommitmentV1".to_owned(),
        musubi_closed_object(
            &[
                "root_cid",
                "chunker",
                "chunk_plan_digest",
                "por_root",
                "content_length",
                "car_digest",
                "car_size",
                "bundle_digest",
                "source_tree_digest",
                "descriptor_digest",
                "file_count",
                "chunk_count",
            ],
            vec![
                ("root_cid", schema_ref("MusubiManifestRootCidV1")),
                ("chunker", schema_ref("MusubiChunkerProfileHandleV1")),
                ("chunk_plan_digest", schema_ref("MusubiDigest32V1")),
                ("por_root", schema_ref("MusubiDigest32V1")),
                (
                    "content_length",
                    norito::json!({
                        "type": "integer",
                        "format": "uint64",
                        "minimum": 1,
                        "maximum": (MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1)
                    }),
                ),
                ("car_digest", schema_ref("MusubiDigest32V1")),
                (
                    "car_size",
                    norito::json!({
                        "type": "integer",
                        "format": "uint64",
                        "minimum": 1,
                        "maximum": (MUSUBI_MAX_CAR_BYTES_V1)
                    }),
                ),
                ("bundle_digest", schema_ref("MusubiDigest32V1")),
                ("source_tree_digest", schema_ref("MusubiDigest32V1")),
                ("descriptor_digest", schema_ref("MusubiDigest32V1")),
                (
                    "file_count",
                    norito::json!({
                        "type": "integer",
                        "format": "uint32",
                        "minimum": 1,
                        "maximum": (MUSUBI_MAX_FILES_V1)
                    }),
                ),
                (
                    "chunk_count",
                    norito::json!({
                        "type": "integer",
                        "format": "uint32",
                        "minimum": 1,
                        "maximum": (MUSUBI_MAX_CHUNKS_V1)
                    }),
                ),
            ],
        ),
    );

    schemas.insert(
        "MusubiProviderIdV1".to_owned(),
        musubi_array(
            norito::json!({
                "type": "string",
                "minLength": 64,
                "maxLength": 64,
                "pattern": "^[0-9A-Fa-f]{64}$"
            }),
            1,
            1,
        ),
    );
    schemas.insert(
        "MusubiControllerApprovalV1".to_owned(),
        musubi_closed_object(
            &["public_key", "signature"],
            vec![
                (
                    "public_key",
                    norito::json!({
                        "type": "string",
                        "minLength": 1,
                        "description": "Canonical multihash public-key literal; native post-quantum keys are bounded by the enclosing body."
                    }),
                ),
                (
                    "signature",
                    norito::json!({
                        "type": "string",
                        "minLength": 2,
                        "pattern": "^(?:[0-9A-Fa-f]{2})+$",
                        "description": "Canonical hexadecimal signature; native post-quantum signatures are bounded by the enclosing body."
                    }),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiSeedIngressReceiptBindingV1".to_owned(),
        musubi_closed_object(
            &[
                "chain_id",
                "genesis_block_hash",
                "publisher",
                "ingress_broker",
                "seed_provider",
                "semantic_release_manifest_digest",
                "archive_id",
                "car_body_digest",
                "car_body_length",
                "nonce",
            ],
            vec![
                ("chain_id", schema_ref("MusubiChainIdV1")),
                ("genesis_block_hash", schema_ref("MusubiFixed32BytesV1")),
                ("publisher", schema_ref("MusubiAccountIdV1")),
                ("ingress_broker", schema_ref("MusubiAccountIdV1")),
                ("seed_provider", schema_ref("MusubiProviderIdV1")),
                (
                    "semantic_release_manifest_digest",
                    schema_ref("MusubiDigest32V1"),
                ),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("car_body_digest", schema_ref("MusubiDigest32V1")),
                (
                    "car_body_length",
                    norito::json!({
                        "type": "integer",
                        "format": "uint64",
                        "minimum": 1,
                        "maximum": (MUSUBI_MAX_CAR_BYTES_V1)
                    }),
                ),
                ("nonce", schema_ref("MusubiFixed32BytesV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiSeedIngressReceiptPayloadV1".to_owned(),
        musubi_closed_object(
            &["version", "binding", "issued_at_ms", "expires_at_ms"],
            vec![
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                ("binding", schema_ref("MusubiSeedIngressReceiptBindingV1")),
                ("issued_at_ms", schema_ref("MusubiPositiveU64V1")),
                ("expires_at_ms", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiSeedIngressReceiptV1".to_owned(),
        musubi_closed_object(
            &["payload", "approvals"],
            vec![
                ("payload", schema_ref("MusubiSeedIngressReceiptPayloadV1")),
                (
                    "approvals",
                    musubi_array(
                        schema_ref("MusubiControllerApprovalV1"),
                        1,
                        MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1,
                    ),
                ),
            ],
        ),
    );

    schemas.insert(
        "MusubiNamespaceDelegationPayloadV1".to_owned(),
        musubi_closed_object(
            &[
                "version",
                "namespace_binding",
                "owner_generation",
                "owner",
                "delegate",
                "expires_at_height",
            ],
            vec![
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                ("namespace_binding", schema_ref("MusubiDigest32V1")),
                ("owner_generation", schema_ref("MusubiPositiveU64V1")),
                ("owner", schema_ref("MusubiAccountIdV1")),
                ("delegate", schema_ref("MusubiAccountIdV1")),
                ("expires_at_height", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiNamespaceDelegationV1".to_owned(),
        musubi_closed_object(
            &["payload", "approvals"],
            vec![
                ("payload", schema_ref("MusubiNamespaceDelegationPayloadV1")),
                (
                    "approvals",
                    musubi_array(
                        schema_ref("MusubiControllerApprovalV1"),
                        1,
                        MUSUBI_MAX_NAMESPACE_DELEGATION_APPROVALS_V1,
                    ),
                ),
            ],
        ),
    );

    schemas.insert(
        "MusubiProviderCompletionSignerPolicyV1".to_owned(),
        musubi_closed_object(
            &[
                "policy_id",
                "revision",
                "predecessor_digest",
                "policy_digest",
            ],
            vec![
                ("policy_id", schema_ref("MusubiFixed32BytesV1")),
                ("revision", schema_ref("MusubiPositiveU64V1")),
                (
                    "predecessor_digest",
                    musubi_nullable(schema_ref("MusubiFixed32BytesV1")),
                ),
                ("policy_digest", schema_ref("MusubiFixed32BytesV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderCompletionAuthorityV1".to_owned(),
        musubi_closed_object(
            &["provider_owner", "signer_policy"],
            vec![
                ("provider_owner", schema_ref("MusubiAccountIdV1")),
                (
                    "signer_policy",
                    schema_ref("MusubiProviderCompletionSignerPolicyV1"),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderFinalizedAnchorV1".to_owned(),
        musubi_closed_object(
            &["height", "block_hash"],
            vec![
                ("height", schema_ref("MusubiPositiveU64V1")),
                ("block_hash", schema_ref("MusubiFixed32BytesV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderBundleVerificationBindingV1".to_owned(),
        musubi_closed_object(
            &[
                "chain_id",
                "genesis_block_hash",
                "provider_id",
                "completed_by",
                "completion_authority",
                "replication_order",
                "assignment_revision",
                "completion_epoch",
                "finalized_anchor",
                "archive_id",
                "bundle_digest",
                "descriptor_digest",
                "semantic_release_manifest_digest",
                "verification_lock_digest",
                "source_tree_digest",
            ],
            vec![
                ("chain_id", schema_ref("MusubiChainIdV1")),
                ("genesis_block_hash", schema_ref("MusubiFixed32BytesV1")),
                ("provider_id", schema_ref("MusubiProviderIdV1")),
                ("completed_by", schema_ref("MusubiAccountIdV1")),
                (
                    "completion_authority",
                    schema_ref("MusubiProviderCompletionAuthorityV1"),
                ),
                ("replication_order", schema_ref("MusubiDigest32V1")),
                ("assignment_revision", schema_ref("MusubiPositiveU64V1")),
                ("completion_epoch", schema_ref("MusubiPositiveU64V1")),
                (
                    "finalized_anchor",
                    schema_ref("MusubiProviderFinalizedAnchorV1"),
                ),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("bundle_digest", schema_ref("MusubiDigest32V1")),
                ("descriptor_digest", schema_ref("MusubiDigest32V1")),
                (
                    "semantic_release_manifest_digest",
                    schema_ref("MusubiDigest32V1"),
                ),
                ("verification_lock_digest", schema_ref("MusubiDigest32V1")),
                ("source_tree_digest", schema_ref("MusubiDigest32V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderBundleVerificationPayloadV1".to_owned(),
        musubi_closed_object(
            &["version", "binding"],
            vec![
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                (
                    "binding",
                    schema_ref("MusubiProviderBundleVerificationBindingV1"),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderBundleVerificationAttestationV1".to_owned(),
        musubi_closed_object(
            &["payload", "approvals"],
            vec![
                (
                    "payload",
                    schema_ref("MusubiProviderBundleVerificationPayloadV1"),
                ),
                (
                    "approvals",
                    musubi_array(
                        schema_ref("MusubiControllerApprovalV1"),
                        1,
                        MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1,
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderBundleAttestationKeyV1".to_owned(),
        musubi_closed_object(
            &["archive_id", "replication_order", "provider_id"],
            vec![
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("replication_order", schema_ref("MusubiDigest32V1")),
                ("provider_id", schema_ref("MusubiProviderIdV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiProviderBundleAttestationRecordV1".to_owned(),
        musubi_closed_object(
            &[
                "key",
                "attestation_digest",
                "attestation",
                "registered_by",
                "registered_at_height",
            ],
            vec![
                ("key", schema_ref("MusubiProviderBundleAttestationKeyV1")),
                ("attestation_digest", schema_ref("MusubiDigest32V1")),
                (
                    "attestation",
                    schema_ref("MusubiProviderBundleVerificationAttestationV1"),
                ),
                ("registered_by", schema_ref("MusubiAccountIdV1")),
                ("registered_at_height", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );

    schemas.insert(
        "MusubiArchiveLocationStateV1".to_owned(),
        musubi_tagged_units(&["Pending", "Healthy", "Degraded", "Retired"]),
    );
    schemas.insert(
        "MusubiStorageAvailabilityV1".to_owned(),
        musubi_tagged_units(&["Selectable", "BelowQuorum", "Unavailable"]),
    );
    schemas.insert(
        "MusubiArchiveAvailabilityV1".to_owned(),
        musubi_closed_object(
            &[
                "archive_id",
                "availability",
                "healthy_replicas",
                "active_locations",
                "finalized_height",
                "finalized_block_hash",
                "index_revision",
            ],
            vec![
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("availability", schema_ref("MusubiStorageAvailabilityV1")),
                (
                    "healthy_replicas",
                    norito::json!({ "type": "integer", "format": "uint16", "minimum": 0 }),
                ),
                (
                    "active_locations",
                    norito::json!({
                        "type": "integer",
                        "format": "uint8",
                        "minimum": 0,
                        "maximum": (MUSUBI_MAX_ARCHIVE_LOCATIONS_V1)
                    }),
                ),
                ("finalized_height", schema_ref("MusubiPositiveU64V1")),
                ("finalized_block_hash", schema_ref("MusubiFixed32BytesV1")),
                ("index_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiArchiveRecordV1".to_owned(),
        musubi_closed_object(
            &[
                "archive_id",
                "commitment",
                "staging_receipt",
                "registered_by",
                "registered_at_height",
                "location_revision",
                "location_ids",
            ],
            vec![
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("commitment", schema_ref("MusubiArchiveCommitmentV1")),
                ("staging_receipt", schema_ref("MusubiSeedIngressReceiptV1")),
                ("registered_by", schema_ref("MusubiAccountIdV1")),
                ("registered_at_height", schema_ref("MusubiPositiveU64V1")),
                ("location_revision", schema_ref("MusubiPositiveU64V1")),
                (
                    "location_ids",
                    musubi_array(
                        schema_ref("MusubiDigest32V1"),
                        0,
                        MUSUBI_MAX_ARCHIVE_LOCATIONS_V1,
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiArchiveLocationV1".to_owned(),
        musubi_closed_object(
            &[
                "location_id",
                "archive_id",
                "pin_manifest",
                "replication_order",
                "providers",
                "provider_attestation_set_digest",
                "renew_after_epoch",
                "expires_at_epoch",
                "finalized_height",
                "revision",
                "state",
            ],
            vec![
                ("location_id", schema_ref("MusubiDigest32V1")),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("pin_manifest", schema_ref("MusubiDigest32V1")),
                ("replication_order", schema_ref("MusubiDigest32V1")),
                (
                    "providers",
                    musubi_array(
                        schema_ref("MusubiProviderIdV1"),
                        1,
                        MUSUBI_MAX_LOCATION_PROVIDERS_V1,
                    ),
                ),
                (
                    "provider_attestation_set_digest",
                    schema_ref("MusubiDigest32V1"),
                ),
                ("renew_after_epoch", schema_ref("MusubiU64V1")),
                ("expires_at_epoch", schema_ref("MusubiPositiveU64V1")),
                ("finalized_height", schema_ref("MusubiPositiveU64V1")),
                ("revision", schema_ref("MusubiPositiveU64V1")),
                ("state", schema_ref("MusubiArchiveLocationStateV1")),
            ],
        ),
    );

    schemas.insert(
        "MusubiReleaseYankV1".to_owned(),
        musubi_closed_object(
            &[
                "release",
                "yanked",
                "reason",
                "changed_by",
                "changed_at_height",
                "revision",
            ],
            vec![
                ("release", schema_ref("MusubiReleaseIdV1")),
                ("yanked", norito::json!({ "type": "boolean" })),
                ("reason", schema_ref("MusubiReasonV1")),
                ("changed_by", schema_ref("MusubiAccountIdV1")),
                ("changed_at_height", schema_ref("MusubiPositiveU64V1")),
                ("revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiArtifactTakedownV1".to_owned(),
        musubi_closed_object(
            &["action_digest", "reason", "applied_at_height"],
            vec![
                ("action_digest", schema_ref("MusubiDigest32V1")),
                ("reason", schema_ref("MusubiReasonV1")),
                ("applied_at_height", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiArtifactGovernanceStateV1".to_owned(),
        musubi_tagged_union(vec![
            ("Available", norito::json!({ "type": "null" })),
            ("TakenDown", schema_ref("MusubiArtifactTakedownV1")),
        ]),
    );
    schemas.insert(
        "MusubiReleaseSelectionStateV1".to_owned(),
        musubi_closed_object(
            &["yank", "storage", "governance"],
            vec![
                ("yank", schema_ref("MusubiReleaseYankV1")),
                ("storage", schema_ref("MusubiArchiveAvailabilityV1")),
                ("governance", schema_ref("MusubiArtifactGovernanceStateV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiResolverReleaseRowV1".to_owned(),
        musubi_closed_object(
            &[
                "release",
                "release_digest",
                "archive_id",
                "source_digest",
                "interface_digest",
                "abi",
                "dependencies",
                "selection",
                "index_revision",
            ],
            vec![
                ("release", schema_ref("MusubiReleaseIdV1")),
                ("release_digest", schema_ref("MusubiDigest32V1")),
                ("archive_id", schema_ref("MusubiDigest32V1")),
                ("source_digest", schema_ref("MusubiDigest32V1")),
                ("interface_digest", schema_ref("MusubiDigest32V1")),
                ("abi", schema_ref("MusubiAbiBindingV1")),
                (
                    "dependencies",
                    musubi_array(
                        schema_ref("MusubiDependencyReqV1"),
                        0,
                        MUSUBI_MAX_DEPENDENCIES_V1,
                    ),
                ),
                ("selection", schema_ref("MusubiReleaseSelectionStateV1")),
                ("index_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiReleaseRecordV1".to_owned(),
        musubi_closed_object(
            &[
                "manifest",
                "release_digest",
                "published_by",
                "published_at_height",
                "yank",
                "artifact_governance",
                "revisions",
            ],
            vec![
                ("manifest", schema_ref("MusubiReleaseManifestV1")),
                ("release_digest", schema_ref("MusubiDigest32V1")),
                ("published_by", schema_ref("MusubiAccountIdV1")),
                ("published_at_height", schema_ref("MusubiPositiveU64V1")),
                ("yank", schema_ref("MusubiReleaseYankV1")),
                (
                    "artifact_governance",
                    schema_ref("MusubiArtifactGovernanceStateV1"),
                ),
                (
                    "revisions",
                    musubi_closed_object(
                        &["yank", "artifact_governance"],
                        vec![
                            ("yank", schema_ref("MusubiPositiveU64V1")),
                            ("artifact_governance", schema_ref("MusubiPositiveU64V1")),
                        ],
                    ),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiExactReleaseSnapshotV1".to_owned(),
        musubi_closed_object(
            &[
                "chain_id",
                "genesis_hash",
                "snapshot",
                "home_release",
                "universal_release",
            ],
            vec![
                ("chain_id", schema_ref("MusubiChainIdV1")),
                ("genesis_hash", schema_ref("MusubiFixed32BytesV1")),
                ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ("home_release", schema_ref("MusubiReleaseRecordV1")),
                (
                    "universal_release",
                    schema_ref("MusubiResolverReleaseRowV1"),
                ),
            ],
        ),
    );
}

fn insert_musubi_governance_and_route_schemas(schemas: &mut Map) {
    use iroha_data_model::musubi::{
        MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1, MUSUBI_MAX_PACKAGE_MEMBERS_V1,
        MUSUBI_MAX_PACKAGE_OWNERS_V1, MUSUBI_MAX_RESOLUTION_NODES_V1,
    };

    schemas.insert(
        "MusubiMaintainerPermissionsV1".to_owned(),
        musubi_closed_object(
            &["publish", "yank", "metadata", "archive_locations"],
            vec![
                ("publish", norito::json!({ "type": "boolean" })),
                ("yank", norito::json!({ "type": "boolean" })),
                ("metadata", norito::json!({ "type": "boolean" })),
                ("archive_locations", norito::json!({ "type": "boolean" })),
            ],
        ),
    );
    schemas.insert(
        "MusubiPackageRoleV1".to_owned(),
        musubi_tagged_union(vec![
            ("Owner", norito::json!({ "type": "null" })),
            ("Maintainer", schema_ref("MusubiMaintainerPermissionsV1")),
        ]),
    );
    schemas.insert(
        "MusubiInvitationStateV1".to_owned(),
        musubi_tagged_units(&["Pending", "Accepted", "Revoked", "Expired"]),
    );
    schemas.insert(
        "MusubiPackageMemberV1".to_owned(),
        musubi_closed_object(
            &[
                "package",
                "account",
                "role",
                "accepted_at_height",
                "governance_revision",
            ],
            vec![
                ("package", schema_ref("MusubiPackageIdV1")),
                ("account", schema_ref("MusubiAccountIdV1")),
                ("role", schema_ref("MusubiPackageRoleV1")),
                ("accepted_at_height", schema_ref("MusubiPositiveU64V1")),
                ("governance_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiMaintainerInvitationV1".to_owned(),
        musubi_closed_object(
            &[
                "invite_id",
                "package",
                "invited_by",
                "invited_account",
                "role",
                "expected_governance_revision",
                "expires_at_height",
                "state",
            ],
            vec![
                ("invite_id", schema_ref("MusubiDigest32V1")),
                ("package", schema_ref("MusubiPackageIdV1")),
                ("invited_by", schema_ref("MusubiAccountIdV1")),
                ("invited_account", schema_ref("MusubiAccountIdV1")),
                ("role", schema_ref("MusubiPackageRoleV1")),
                (
                    "expected_governance_revision",
                    schema_ref("MusubiPositiveU64V1"),
                ),
                ("expires_at_height", schema_ref("MusubiPositiveU64V1")),
                ("state", schema_ref("MusubiInvitationStateV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiMaintainerDirectoryEntryV1".to_owned(),
        musubi_tagged_union(vec![
            ("Accepted", schema_ref("MusubiPackageMemberV1")),
            (
                "PendingInvitation",
                schema_ref("MusubiMaintainerInvitationV1"),
            ),
        ]),
    );
    schemas.insert(
        "MusubiPackageRevisionsV1".to_owned(),
        musubi_closed_object(
            &["governance", "metadata", "archive_locations"],
            vec![
                ("governance", schema_ref("MusubiPositiveU64V1")),
                ("metadata", schema_ref("MusubiPositiveU64V1")),
                ("archive_locations", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiPackageRecordV1".to_owned(),
        musubi_closed_object(
            &[
                "package",
                "claimed_namespace",
                "claimed_namespace_binding",
                "owners",
                "member_accounts",
                "claimed_at_height",
                "revisions",
            ],
            vec![
                ("package", schema_ref("MusubiPackageIdV1")),
                ("claimed_namespace", schema_ref("MusubiNamespaceV1")),
                ("claimed_namespace_binding", schema_ref("MusubiDigest32V1")),
                (
                    "owners",
                    musubi_array(
                        schema_ref("MusubiAccountIdV1"),
                        1,
                        MUSUBI_MAX_PACKAGE_OWNERS_V1,
                    ),
                ),
                (
                    "member_accounts",
                    musubi_array(
                        schema_ref("MusubiAccountIdV1"),
                        1,
                        MUSUBI_MAX_PACKAGE_MEMBERS_V1,
                    ),
                ),
                ("claimed_at_height", schema_ref("MusubiPositiveU64V1")),
                ("revisions", schema_ref("MusubiPackageRevisionsV1")),
            ],
        ),
    );

    schemas.insert(
        "MusubiAliasPricingPolicyV1".to_owned(),
        musubi_closed_object(
            &[
                "revision",
                "length_1_xor",
                "length_2_xor",
                "length_3_xor",
                "length_4_xor",
                "length_5_to_32_xor",
            ],
            vec![
                ("revision", schema_ref("MusubiPositiveU64V1")),
                ("length_1_xor", schema_ref("MusubiPositiveU64V1")),
                ("length_2_xor", schema_ref("MusubiPositiveU64V1")),
                ("length_3_xor", schema_ref("MusubiPositiveU64V1")),
                ("length_4_xor", schema_ref("MusubiPositiveU64V1")),
                ("length_5_to_32_xor", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiRegistryAdmissionModeV1".to_owned(),
        musubi_tagged_units(&["Closed", "Allowlisted", "Open"]),
    );
    schemas.insert(
        "MusubiRegistryPolicyV1".to_owned(),
        musubi_closed_object(
            &[
                "version",
                "revision",
                "mode",
                "allowlisted_dataspaces",
                "alias_pricing",
            ],
            vec![
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                ("revision", schema_ref("MusubiPositiveU64V1")),
                ("mode", schema_ref("MusubiRegistryAdmissionModeV1")),
                (
                    "allowlisted_dataspaces",
                    musubi_array(schema_ref("MusubiU64V1"), 0, MUSUBI_MAX_RESOLUTION_NODES_V1),
                ),
                ("alias_pricing", schema_ref("MusubiAliasPricingPolicyV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiGovernanceDecisionV1".to_owned(),
        musubi_closed_object(
            &[
                "decision_id",
                "action_digest",
                "enacted_at_height",
                "execute_after_height",
            ],
            vec![
                ("decision_id", schema_ref("MusubiFixed32BytesV1")),
                ("action_digest", schema_ref("MusubiDigest32V1")),
                ("enacted_at_height", schema_ref("MusubiPositiveU64V1")),
                ("execute_after_height", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiAliasRecordV1".to_owned(),
        musubi_closed_object(
            &[
                "alias",
                "target",
                "registered_by",
                "pricing_revision",
                "paid_xor",
                "registered_at_height",
                "history_revision",
            ],
            vec![
                ("alias", schema_ref("MusubiAliasNameV1")),
                ("target", schema_ref("MusubiPackageIdV1")),
                ("registered_by", schema_ref("MusubiAccountIdV1")),
                ("pricing_revision", schema_ref("MusubiPositiveU64V1")),
                ("paid_xor", schema_ref("MusubiPositiveU64V1")),
                ("registered_at_height", schema_ref("MusubiPositiveU64V1")),
                ("history_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiAliasHistoryActionV1".to_owned(),
        musubi_tagged_units(&["Registered", "ParliamentRetarget"]),
    );
    schemas.insert(
        "MusubiAliasHistoryEntryV1".to_owned(),
        musubi_closed_object(
            &[
                "alias",
                "revision",
                "action",
                "previous_target",
                "target",
                "governance_action",
                "finalized_height",
            ],
            vec![
                ("alias", schema_ref("MusubiAliasNameV1")),
                ("revision", schema_ref("MusubiPositiveU64V1")),
                ("action", schema_ref("MusubiAliasHistoryActionV1")),
                (
                    "previous_target",
                    musubi_nullable(schema_ref("MusubiPackageIdV1")),
                ),
                ("target", schema_ref("MusubiPackageIdV1")),
                (
                    "governance_action",
                    musubi_nullable(schema_ref("MusubiDigest32V1")),
                ),
                ("finalized_height", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );

    schemas.insert(
        "MusubiArchiveRetentionDispositionV1".to_owned(),
        musubi_tagged_units(&[
            "RetainUnknown",
            "RetainReferenced",
            "PruneUnreferenced",
            "PruneGovernedTakedown",
        ]),
    );
    schemas.insert(
        "MusubiArchiveRetentionDecisionV1".to_owned(),
        musubi_closed_object(
            &[
                "archive_id",
                "disposition",
                "active_releases",
                "yanked_releases",
                "taken_down_releases",
                "storage",
            ],
            vec![
                ("archive_id", schema_ref("MusubiDigest32V1")),
                (
                    "disposition",
                    schema_ref("MusubiArchiveRetentionDispositionV1"),
                ),
                (
                    "active_releases",
                    norito::json!({ "type": "integer", "format": "uint16", "minimum": 0 }),
                ),
                (
                    "yanked_releases",
                    norito::json!({ "type": "integer", "format": "uint16", "minimum": 0 }),
                ),
                (
                    "taken_down_releases",
                    norito::json!({ "type": "integer", "format": "uint16", "minimum": 0 }),
                ),
                (
                    "storage",
                    musubi_nullable(schema_ref("MusubiArchiveAvailabilityV1")),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiOrderedPrefixV1".to_owned(),
        musubi_string_newtype(MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1, None),
    );
    schemas.insert(
        "MusubiOrderedPackageEntryV1".to_owned(),
        musubi_closed_object(
            &[
                "selector",
                "package",
                "latest_selectable",
                "metadata_revision",
                "index_revision",
            ],
            vec![
                ("selector", schema_ref("MusubiPackageSelectorV1")),
                ("package", schema_ref("MusubiPackageIdV1")),
                (
                    "latest_selectable",
                    musubi_nullable(schema_ref("MusubiVersionV1")),
                ),
                ("metadata_revision", schema_ref("MusubiPositiveU64V1")),
                ("index_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiSearchSnapshotV1".to_owned(),
        musubi_closed_object(
            &[
                "finalized_height",
                "finalized_block_hash",
                "projection_revision",
            ],
            vec![
                ("finalized_height", schema_ref("MusubiPositiveU64V1")),
                ("finalized_block_hash", schema_ref("MusubiFixed32BytesV1")),
                ("projection_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiSearchCursorV1".to_owned(),
        musubi_closed_object(
            &["snapshot", "query_hash", "last_package"],
            vec![
                ("snapshot", schema_ref("MusubiSearchSnapshotV1")),
                ("query_hash", schema_ref("MusubiDigest32V1")),
                ("last_package", schema_ref("MusubiPackageIdV1")),
            ],
        ),
    );
    schemas.insert(
        "MusubiSearchPageRequestV1".to_owned(),
        musubi_closed_object(
            &["limit", "cursor"],
            vec![
                (
                    "limit",
                    norito::json!({
                        "type": "integer",
                        "format": "uint32",
                        "minimum": 0,
                        "maximum": (iroha_data_model::musubi::MUSUBI_MAX_PAGE_SIZE_V1)
                    }),
                ),
                (
                    "cursor",
                    musubi_nullable(schema_ref("MusubiSearchCursorV1")),
                ),
            ],
        ),
    );
    schemas.insert(
        "MusubiSearchHitV1".to_owned(),
        musubi_closed_object(
            &[
                "package",
                "claimed_namespace",
                "description",
                "keywords",
                "metadata_revision",
            ],
            vec![
                ("package", schema_ref("MusubiPackageIdV1")),
                ("claimed_namespace", schema_ref("MusubiNamespaceV1")),
                (
                    "description",
                    musubi_nullable(schema_ref("MusubiDescriptionV1")),
                ),
                (
                    "keywords",
                    musubi_array(
                        schema_ref("MusubiKeywordV1"),
                        0,
                        iroha_data_model::musubi::MUSUBI_MAX_KEYWORDS_V1,
                    ),
                ),
                ("metadata_revision", schema_ref("MusubiPositiveU64V1")),
            ],
        ),
    );

    insert_musubi_instruction_request_schemas(schemas);
    insert_musubi_query_request_and_response_schemas(schemas);
    insert_musubi_instruction_envelope_schema(schemas);
}

fn insert_musubi_instruction_request_schemas(schemas: &mut Map) {
    use iroha_data_model::musubi::MUSUBI_MAX_PACKAGE_OWNERS_V1;

    for (name, schema) in [
        (
            "RegisterMusubiNamespaceBindingV1",
            musubi_closed_object(
                &["binding", "expected_policy_revision"],
                vec![
                    ("binding", schema_ref("MusubiNamespaceBindingV1")),
                    (
                        "expected_policy_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RegisterMusubiArchiveV1",
            musubi_closed_object(
                &["commitment", "staging_receipt", "expected_policy_revision"],
                vec![
                    ("commitment", schema_ref("MusubiArchiveCommitmentV1")),
                    ("staging_receipt", schema_ref("MusubiSeedIngressReceiptV1")),
                    (
                        "expected_policy_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RegisterMusubiProviderBundleAttestationV1",
            musubi_closed_object(
                &["attestation", "expected_location_revision"],
                vec![
                    (
                        "attestation",
                        schema_ref("MusubiProviderBundleVerificationAttestationV1"),
                    ),
                    (
                        "expected_location_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "AddMusubiArchiveLocationV1",
            musubi_closed_object(
                &[
                    "archive_id",
                    "location_id",
                    "pin_manifest",
                    "replication_order",
                    "provider_attestation_set_digest",
                    "renew_after_epoch",
                    "expires_at_epoch",
                    "expected_location_revision",
                ],
                vec![
                    ("archive_id", schema_ref("MusubiDigest32V1")),
                    ("location_id", schema_ref("MusubiDigest32V1")),
                    ("pin_manifest", schema_ref("MusubiDigest32V1")),
                    ("replication_order", schema_ref("MusubiDigest32V1")),
                    (
                        "provider_attestation_set_digest",
                        schema_ref("MusubiDigest32V1"),
                    ),
                    ("renew_after_epoch", schema_ref("MusubiU64V1")),
                    ("expires_at_epoch", schema_ref("MusubiPositiveU64V1")),
                    (
                        "expected_location_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RetireMusubiArchiveLocationV1",
            musubi_closed_object(
                &[
                    "archive_id",
                    "location_id",
                    "expected_location_revision",
                    "reason",
                ],
                vec![
                    ("archive_id", schema_ref("MusubiDigest32V1")),
                    ("location_id", schema_ref("MusubiDigest32V1")),
                    (
                        "expected_location_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                    ("reason", schema_ref("MusubiReasonV1")),
                ],
            ),
        ),
        (
            "PublishMusubiReleaseV1",
            musubi_closed_object(
                &[
                    "namespace",
                    "publication",
                    "namespace_delegation",
                    "expected_policy_revision",
                    "expected_governance_revision",
                ],
                vec![
                    ("namespace", schema_ref("MusubiNamespaceV1")),
                    ("publication", schema_ref("MusubiPublicationV1")),
                    (
                        "namespace_delegation",
                        musubi_nullable(schema_ref("MusubiNamespaceDelegationV1")),
                    ),
                    (
                        "expected_policy_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                    (
                        "expected_governance_revision",
                        musubi_nullable(schema_ref("MusubiPositiveU64V1")),
                    ),
                ],
            ),
        ),
        (
            "SetMusubiReleaseYankV1",
            musubi_closed_object(
                &["release", "yanked", "reason", "expected_yank_revision"],
                vec![
                    ("release", schema_ref("MusubiReleaseIdV1")),
                    ("yanked", norito::json!({ "type": "boolean" })),
                    ("reason", schema_ref("MusubiReasonV1")),
                    ("expected_yank_revision", schema_ref("MusubiPositiveU64V1")),
                ],
            ),
        ),
        (
            "SetMusubiPackageMetadataV1",
            musubi_closed_object(
                &["package", "metadata", "expected_metadata_revision"],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    ("metadata", schema_ref("MusubiReleaseMetadataV1")),
                    (
                        "expected_metadata_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "InviteMusubiPackageMaintainerV1",
            musubi_closed_object(
                &[
                    "package",
                    "invite_id",
                    "invited_account",
                    "role",
                    "expires_at_height",
                    "expected_governance_revision",
                ],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    ("invite_id", schema_ref("MusubiDigest32V1")),
                    ("invited_account", schema_ref("MusubiAccountIdV1")),
                    ("role", schema_ref("MusubiPackageRoleV1")),
                    ("expires_at_height", schema_ref("MusubiPositiveU64V1")),
                    (
                        "expected_governance_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "AcceptMusubiPackageMaintainerV1",
            musubi_package_invitation_revision_schema(),
        ),
        (
            "RevokeMusubiPackageMaintainerInvitationV1",
            musubi_package_invitation_revision_schema(),
        ),
        (
            "SetMusubiPackageMaintainerRoleV1",
            musubi_closed_object(
                &["package", "account", "role", "expected_governance_revision"],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    ("account", schema_ref("MusubiAccountIdV1")),
                    ("role", schema_ref("MusubiPackageRoleV1")),
                    (
                        "expected_governance_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RemoveMusubiPackageMaintainerV1",
            musubi_closed_object(
                &["package", "account", "expected_governance_revision"],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    ("account", schema_ref("MusubiAccountIdV1")),
                    (
                        "expected_governance_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RegisterMusubiAliasV1",
            musubi_closed_object(
                &["alias", "target", "expected_pricing_revision"],
                vec![
                    ("alias", schema_ref("MusubiAliasNameV1")),
                    ("target", schema_ref("MusubiPackageIdV1")),
                    (
                        "expected_pricing_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RecoverMusubiPackageV1",
            musubi_closed_object(
                &[
                    "decision",
                    "package",
                    "owners",
                    "expected_governance_revision",
                ],
                vec![
                    ("decision", schema_ref("MusubiGovernanceDecisionV1")),
                    ("package", schema_ref("MusubiPackageIdV1")),
                    (
                        "owners",
                        musubi_array(
                            schema_ref("MusubiAccountIdV1"),
                            1,
                            MUSUBI_MAX_PACKAGE_OWNERS_V1,
                        ),
                    ),
                    (
                        "expected_governance_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "RetargetMusubiAliasV1",
            musubi_closed_object(
                &["decision", "alias", "target", "expected_history_revision"],
                vec![
                    ("decision", schema_ref("MusubiGovernanceDecisionV1")),
                    ("alias", schema_ref("MusubiAliasNameV1")),
                    ("target", schema_ref("MusubiPackageIdV1")),
                    (
                        "expected_history_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "SetMusubiArtifactTakedownV1",
            musubi_closed_object(
                &[
                    "decision",
                    "release",
                    "reason",
                    "expected_artifact_governance_revision",
                ],
                vec![
                    ("decision", schema_ref("MusubiGovernanceDecisionV1")),
                    ("release", schema_ref("MusubiReleaseIdV1")),
                    ("reason", schema_ref("MusubiReasonV1")),
                    (
                        "expected_artifact_governance_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "SetMusubiRegistryPolicyV1",
            musubi_closed_object(
                &["decision", "policy", "expected_policy_revision"],
                vec![
                    ("decision", schema_ref("MusubiGovernanceDecisionV1")),
                    ("policy", schema_ref("MusubiRegistryPolicyV1")),
                    (
                        "expected_policy_revision",
                        schema_ref("MusubiPositiveU64V1"),
                    ),
                ],
            ),
        ),
        (
            "AssertMusubiReleaseDigestV1",
            musubi_closed_object(
                &["release", "expected_digest"],
                vec![
                    ("release", schema_ref("MusubiReleaseIdV1")),
                    ("expected_digest", schema_ref("MusubiDigest32V1")),
                ],
            ),
        ),
    ] {
        schemas.insert(name.to_owned(), schema);
    }
}

fn musubi_package_invitation_revision_schema() -> Value {
    musubi_closed_object(
        &["package", "invite_id", "expected_governance_revision"],
        vec![
            ("package", schema_ref("MusubiPackageIdV1")),
            ("invite_id", schema_ref("MusubiDigest32V1")),
            (
                "expected_governance_revision",
                schema_ref("MusubiPositiveU64V1"),
            ),
        ],
    )
}

fn insert_musubi_query_request_and_response_schemas(schemas: &mut Map) {
    use iroha_data_model::musubi::{
        MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1,
        MUSUBI_MAX_PAGE_SIZE_V1, MUSUBI_MAX_SEARCH_QUERY_BYTES_V1,
    };

    for (name, schema) in [
        (
            "MusubiExactPackageQueryV1",
            musubi_closed_object(
                &["package"],
                vec![("package", schema_ref("MusubiPackageIdV1"))],
            ),
        ),
        (
            "MusubiExactReleaseQueryV1",
            musubi_closed_object(
                &["release"],
                vec![("release", schema_ref("MusubiReleaseIdV1"))],
            ),
        ),
        (
            "MusubiResolverIndexQueryV1",
            musubi_closed_object(
                &["package", "requirement", "page"],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    (
                        "requirement",
                        musubi_nullable(schema_ref("MusubiVersionReqV1")),
                    ),
                    ("page", schema_ref("MusubiPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiPackagePageQueryV1",
            musubi_closed_object(
                &["package", "page"],
                vec![
                    ("package", schema_ref("MusubiPackageIdV1")),
                    ("page", schema_ref("MusubiPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiArchiveLocationQueryV1",
            musubi_closed_object(
                &["archive_id", "page"],
                vec![
                    ("archive_id", schema_ref("MusubiDigest32V1")),
                    ("page", schema_ref("MusubiPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiArchiveRetentionQueryV1",
            musubi_closed_object(
                &["archive_ids", "expected_snapshot"],
                vec![
                    (
                        "archive_ids",
                        musubi_array(
                            schema_ref("MusubiDigest32V1"),
                            1,
                            MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1,
                        ),
                    ),
                    (
                        "expected_snapshot",
                        musubi_nullable(schema_ref("MusubiRegistrySnapshotV1")),
                    ),
                ],
            ),
        ),
        (
            "MusubiAliasQueryV1",
            musubi_closed_object(
                &["alias", "page"],
                vec![
                    ("alias", schema_ref("MusubiAliasNameV1")),
                    ("page", schema_ref("MusubiPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiOrderedPrefixQueryV1",
            musubi_closed_object(
                &["prefix", "page"],
                vec![
                    ("prefix", schema_ref("MusubiOrderedPrefixV1")),
                    ("page", schema_ref("MusubiPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiSearchQueryV1",
            musubi_closed_object(
                &["query", "page"],
                vec![
                    (
                        "query",
                        norito::json!({
                            "type": "string",
                            "minLength": 1,
                            "maxLength": (MUSUBI_MAX_SEARCH_QUERY_BYTES_V1)
                        }),
                    ),
                    ("page", schema_ref("MusubiSearchPageRequestV1")),
                ],
            ),
        ),
        (
            "MusubiResolverIndexPageV1",
            musubi_closed_object(
                &[
                    "query",
                    "chain_id",
                    "genesis_hash",
                    "items",
                    "next_cursor",
                    "snapshot",
                ],
                vec![
                    ("query", schema_ref("MusubiResolverIndexQueryV1")),
                    ("chain_id", schema_ref("MusubiChainIdV1")),
                    ("genesis_hash", schema_ref("MusubiFixed32BytesV1")),
                    (
                        "items",
                        musubi_array(
                            schema_ref("MusubiResolverReleaseRowV1"),
                            0,
                            MUSUBI_MAX_PAGE_SIZE_V1,
                        ),
                    ),
                    (
                        "next_cursor",
                        musubi_nullable(schema_ref("MusubiFinalizedCursorV1")),
                    ),
                    ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ],
            ),
        ),
        (
            "MusubiVersionPageV1",
            musubi_page_schema(
                schema_ref("MusubiPackagePageQueryV1"),
                schema_ref("MusubiVersionV1"),
            ),
        ),
        (
            "MusubiMaintainerPageV1",
            musubi_page_schema(
                schema_ref("MusubiPackagePageQueryV1"),
                schema_ref("MusubiMaintainerDirectoryEntryV1"),
            ),
        ),
        (
            "MusubiArchiveLocationPageV1",
            musubi_closed_object(
                &[
                    "chain_id",
                    "genesis_hash",
                    "archive",
                    "items",
                    "next_cursor",
                    "snapshot",
                ],
                vec![
                    ("chain_id", schema_ref("MusubiChainIdV1")),
                    ("genesis_hash", schema_ref("MusubiFixed32BytesV1")),
                    ("archive", schema_ref("MusubiArchiveRecordV1")),
                    (
                        "items",
                        musubi_array(
                            schema_ref("MusubiArchiveLocationV1"),
                            0,
                            MUSUBI_MAX_ARCHIVE_LOCATIONS_V1,
                        ),
                    ),
                    (
                        "next_cursor",
                        musubi_nullable(schema_ref("MusubiFinalizedCursorV1")),
                    ),
                    ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ],
            ),
        ),
        (
            "MusubiArchiveRetentionPageV1",
            musubi_closed_object(
                &[
                    "chain_id",
                    "genesis_hash",
                    "items",
                    "snapshot",
                    "finalized_time_ms",
                ],
                vec![
                    ("chain_id", schema_ref("MusubiChainIdV1")),
                    ("genesis_hash", schema_ref("MusubiFixed32BytesV1")),
                    (
                        "items",
                        musubi_array(
                            schema_ref("MusubiArchiveRetentionDecisionV1"),
                            1,
                            MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1,
                        ),
                    ),
                    ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                    ("finalized_time_ms", schema_ref("MusubiU64V1")),
                ],
            ),
        ),
        (
            "MusubiAliasHistoryPageV1",
            musubi_page_schema(
                schema_ref("MusubiAliasQueryV1"),
                schema_ref("MusubiAliasHistoryEntryV1"),
            ),
        ),
        (
            "MusubiOrderedPackagePageV1",
            musubi_closed_object(
                &[
                    "query",
                    "chain_id",
                    "genesis_hash",
                    "namespace_binding",
                    "items",
                    "next_cursor",
                    "snapshot",
                ],
                vec![
                    ("query", schema_ref("MusubiOrderedPrefixQueryV1")),
                    ("chain_id", schema_ref("MusubiChainIdV1")),
                    ("genesis_hash", schema_ref("MusubiFixed32BytesV1")),
                    ("namespace_binding", schema_ref("MusubiNamespaceBindingV1")),
                    (
                        "items",
                        musubi_array(
                            schema_ref("MusubiOrderedPackageEntryV1"),
                            0,
                            MUSUBI_MAX_PAGE_SIZE_V1,
                        ),
                    ),
                    (
                        "next_cursor",
                        musubi_nullable(schema_ref("MusubiFinalizedCursorV1")),
                    ),
                    ("snapshot", schema_ref("MusubiRegistrySnapshotV1")),
                ],
            ),
        ),
        (
            "MusubiSearchPageV1",
            musubi_closed_object(
                &["query", "items", "next_cursor", "snapshot"],
                vec![
                    ("query", schema_ref("MusubiSearchQueryV1")),
                    (
                        "items",
                        musubi_array(schema_ref("MusubiSearchHitV1"), 0, MUSUBI_MAX_PAGE_SIZE_V1),
                    ),
                    (
                        "next_cursor",
                        musubi_nullable(schema_ref("MusubiSearchCursorV1")),
                    ),
                    ("snapshot", schema_ref("MusubiSearchSnapshotV1")),
                ],
            ),
        ),
    ] {
        schemas.insert(name.to_owned(), schema);
    }
}

fn insert_musubi_instruction_envelope_schema(schemas: &mut Map) {
    use iroha_data_model::isi::musubi::{
        AcceptMusubiPackageMaintainerV1, AddMusubiArchiveLocationV1, AssertMusubiReleaseDigestV1,
        InviteMusubiPackageMaintainerV1, PublishMusubiReleaseV1, RecoverMusubiPackageV1,
        RegisterMusubiAliasV1, RegisterMusubiArchiveV1, RegisterMusubiNamespaceBindingV1,
        RegisterMusubiProviderBundleAttestationV1, RemoveMusubiPackageMaintainerV1,
        RetargetMusubiAliasV1, RetireMusubiArchiveLocationV1,
        RevokeMusubiPackageMaintainerInvitationV1, SetMusubiArtifactTakedownV1,
        SetMusubiPackageMaintainerRoleV1, SetMusubiPackageMetadataV1, SetMusubiRegistryPolicyV1,
        SetMusubiReleaseYankV1,
    };

    let payload_variants = [
        (
            RegisterMusubiNamespaceBindingV1::WIRE_ID,
            "RegisterMusubiNamespaceBindingV1",
        ),
        (RegisterMusubiArchiveV1::WIRE_ID, "RegisterMusubiArchiveV1"),
        (
            RegisterMusubiProviderBundleAttestationV1::WIRE_ID,
            "RegisterMusubiProviderBundleAttestationV1",
        ),
        (
            AddMusubiArchiveLocationV1::WIRE_ID,
            "AddMusubiArchiveLocationV1",
        ),
        (
            RetireMusubiArchiveLocationV1::WIRE_ID,
            "RetireMusubiArchiveLocationV1",
        ),
        (PublishMusubiReleaseV1::WIRE_ID, "PublishMusubiReleaseV1"),
        (SetMusubiReleaseYankV1::WIRE_ID, "SetMusubiReleaseYankV1"),
        (
            SetMusubiPackageMetadataV1::WIRE_ID,
            "SetMusubiPackageMetadataV1",
        ),
        (
            InviteMusubiPackageMaintainerV1::WIRE_ID,
            "InviteMusubiPackageMaintainerV1",
        ),
        (
            AcceptMusubiPackageMaintainerV1::WIRE_ID,
            "AcceptMusubiPackageMaintainerV1",
        ),
        (
            RevokeMusubiPackageMaintainerInvitationV1::WIRE_ID,
            "RevokeMusubiPackageMaintainerInvitationV1",
        ),
        (
            SetMusubiPackageMaintainerRoleV1::WIRE_ID,
            "SetMusubiPackageMaintainerRoleV1",
        ),
        (
            RemoveMusubiPackageMaintainerV1::WIRE_ID,
            "RemoveMusubiPackageMaintainerV1",
        ),
        (RegisterMusubiAliasV1::WIRE_ID, "RegisterMusubiAliasV1"),
        (RecoverMusubiPackageV1::WIRE_ID, "RecoverMusubiPackageV1"),
        (RetargetMusubiAliasV1::WIRE_ID, "RetargetMusubiAliasV1"),
        (
            SetMusubiArtifactTakedownV1::WIRE_ID,
            "SetMusubiArtifactTakedownV1",
        ),
        (
            SetMusubiRegistryPolicyV1::WIRE_ID,
            "SetMusubiRegistryPolicyV1",
        ),
        (
            AssertMusubiReleaseDigestV1::WIRE_ID,
            "AssertMusubiReleaseDigestV1",
        ),
    ];
    let preview_variants = payload_variants
        .iter()
        .map(|(wire_id, payload_schema)| {
            musubi_closed_object(
                &["wire_id", "payload"],
                vec![
                    (
                        "wire_id",
                        norito::json!({ "type": "string", "const": (*wire_id) }),
                    ),
                    ("payload", schema_ref(payload_schema)),
                ],
            )
        })
        .collect::<Vec<_>>();
    let wire_ids = payload_variants
        .iter()
        .map(|(wire_id, _)| Value::String((*wire_id).to_owned()))
        .collect::<Vec<_>>();
    schemas.insert(
        "MusubiInstructionPreviewV1".to_owned(),
        norito::json!({ "oneOf": (preview_variants) }),
    );
    schemas.insert(
        "MusubiInstructionEnvelopeV1".to_owned(),
        musubi_closed_object(
            &[
                "schema",
                "version",
                "wire_id",
                "instruction_base64",
                "instruction_hex",
                "instruction_json",
            ],
            vec![
                (
                    "schema",
                    norito::json!({ "type": "string", "const": "musubi-instruction-envelope" }),
                ),
                (
                    "version",
                    norito::json!({ "type": "integer", "format": "uint8", "const": 1 }),
                ),
                (
                    "wire_id",
                    norito::json!({ "type": "string", "enum": (wire_ids) }),
                ),
                (
                    "instruction_base64",
                    norito::json!({
                        "type": "string",
                        "minLength": 1,
                        "pattern": "^[A-Za-z0-9+/]+={0,2}$"
                    }),
                ),
                (
                    "instruction_hex",
                    norito::json!({
                        "type": "string",
                        "minLength": 2,
                        "pattern": "^(?:[0-9a-f]{2})+$"
                    }),
                ),
                ("instruction_json", schema_ref("MusubiInstructionPreviewV1")),
            ],
        ),
    );
}
