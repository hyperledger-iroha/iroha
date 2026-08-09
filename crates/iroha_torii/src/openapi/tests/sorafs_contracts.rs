#[test]
fn evidence_audit_openapi_requires_and_returns_exact_cursors() {
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/evidence/audit", "get");
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("evidence audit description");
    for required_phrase in [
        "only accepted genesis query wire",
        "expected_checkpoint_digest_hex then limit",
        "after_sequence and after_receipt_digest_hex must be supplied together",
        "sequence-only or digest-only continuation is rejected",
        "checkpoint change returns 409",
        "signed checkpoint_anchor",
        "digest-bound page_limit",
        "predecessor and next_cursor",
        "projection_digest_hex",
        "never emits a sequence-only continuation",
    ] {
        assert!(
            description.contains(required_phrase),
            "evidence audit description omitted `{required_phrase}`"
        );
    }

    let parameters = operation
        .get("parameters")
        .and_then(Value::as_array)
        .expect("evidence audit parameters");
    assert_eq!(parameters.len(), 9);
    let parameter = |name: &str| {
        parameters
            .iter()
            .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
            .unwrap_or_else(|| panic!("evidence audit `{name}` parameter"))
    };
    let expected_checkpoint = parameter("expected_checkpoint_digest_hex");
    assert_eq!(
        expected_checkpoint.get("required").and_then(Value::as_bool),
        Some(true)
    );
    let expected_checkpoint_schema = expected_checkpoint
        .get("schema")
        .and_then(Value::as_object)
        .expect("expected checkpoint digest schema");
    assert_eq!(
        expected_checkpoint_schema
            .get("pattern")
            .and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    let after_sequence = parameter("after_sequence");
    assert_eq!(
        after_sequence
            .get("schema")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("minimum"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        after_sequence
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| {
                description.contains("together with after_receipt_digest_hex")
            })
    );
    let after_digest = parameter("after_receipt_digest_hex");
    let after_digest_schema = after_digest
        .get("schema")
        .and_then(Value::as_object)
        .expect("exact receipt digest schema");
    assert_eq!(
        after_digest_schema.get("minLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        after_digest_schema.get("maxLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        after_digest_schema.get("pattern").and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    assert!(
        after_digest
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| description.contains("together with after_sequence"))
    );
    let limit = parameter("limit");
    assert_eq!(limit.get("required").and_then(Value::as_bool), Some(true));
    let limit_schema = limit
        .get("schema")
        .and_then(Value::as_object)
        .expect("audit limit schema");
    assert_eq!(limit_schema.get("minimum").and_then(Value::as_u64), Some(1));
    assert_eq!(
        limit_schema.get("maximum").and_then(Value::as_u64),
        Some(256)
    );
    let auth_headers = parameters
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        auth_headers,
        BTreeSet::from([
            "X-Iroha-Account",
            "X-Iroha-Signature",
            "X-Iroha-Timestamp-Ms",
            "X-Iroha-Nonce",
            "X-Iroha-Witness",
        ])
    );

    let success_description = operation
        .get("responses")
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("200"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("description"))
        .and_then(Value::as_str)
        .expect("evidence audit success description");
    for required_phrase in [
        "checkpoint_anchor",
        "digest-bound page_limit",
        "predecessor and next_cursor",
        "receipt_digest_hex",
        "projection_norito_b64",
        "projection_digest_hex",
        "No sequence-only continuation",
    ] {
        assert!(
            success_description.contains(required_phrase),
            "evidence audit success response omitted `{required_phrase}`"
        );
    }
    assert_eq!(
        operation_response_schema_ref(operation, "200", "/v1/evidence/audit"),
        "#/components/schemas/SorafsEvidenceAuditProjectionV1"
    );
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .expect("evidence audit responses");
    for status in ["400", "401", "403", "409", "503"] {
        assert!(
            responses.contains_key(status),
            "evidence audit omitted documented {status} response"
        );
        assert_eq!(
            operation_response_schema_ref(operation, status, "/v1/evidence/audit"),
            "#/components/schemas/SorafsEvidenceApiErrorV1"
        );
    }

    let status_operation = openapi_operation(&document, "/v1/evidence/status", "get");
    assert_eq!(
        operation_response_schema_ref(status_operation, "200", "/v1/evidence/status"),
        "#/components/schemas/SorafsEvidenceAuditStatusV1"
    );
    assert_eq!(
        status_operation
            .get("parameters")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(5)
    );
    let schemas = component_schemas(&document);
    for schema in [
        "SorafsEvidenceReceiptCursorV1",
        "SorafsEvidenceSignedCheckpointAnchorV1",
        "SorafsEvidenceSignedReceiptV1",
        "SorafsEvidenceAuditProjectionV1",
        "SorafsEvidenceAuditStatusV1",
        "SorafsEvidenceApiErrorV1",
    ] {
        assert!(schemas.contains_key(schema), "missing `{schema}` schema");
    }
}

#[test]
fn evidence_openapi_matches_authenticated_protocol_contract() {
    use iroha_torii_shared::route_catalog::AuthenticationPolicy;

    fn method_name(method: CatalogHttpMethod) -> &'static str {
        match method {
            CatalogHttpMethod::Get => "get",
            CatalogHttpMethod::Post => "post",
            CatalogHttpMethod::Put => "put",
            CatalogHttpMethod::Patch => "patch",
            CatalogHttpMethod::Delete => "delete",
            CatalogHttpMethod::Any => {
                panic!("ANY protocol gateways cannot enter the evidence OpenAPI contract")
            }
        }
    }

    fn assert_opaque_token_schema(schema: &Map, context: &str) {
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("string"),
            "{context} type"
        );
        assert_eq!(
            schema.get("minLength").and_then(Value::as_u64),
            Some(1),
            "{context} minimum length"
        );
        assert_eq!(
            schema.get("maxLength").and_then(Value::as_u64),
            Some(EVIDENCE_VIEWER_MAX_OPAQUE_TOKEN_BYTES_V1 as u64),
            "{context} maximum length"
        );
        assert_eq!(
            schema.get("pattern").and_then(Value::as_str),
            Some("^[!-~]+$"),
            "{context} printable-ASCII pattern"
        );
    }

    fn assert_nonzero_digest_schema(schema: &Map, context: &str) {
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("string"),
            "{context} type"
        );
        assert_eq!(
            schema.get("minLength").and_then(Value::as_u64),
            Some(64),
            "{context} minimum length"
        );
        assert_eq!(
            schema.get("maxLength").and_then(Value::as_u64),
            Some(64),
            "{context} maximum length"
        );
        assert_eq!(
            schema.get("pattern").and_then(Value::as_str),
            Some("^(?!0{64}$)[0-9a-f]{64}$"),
            "{context} canonical non-zero digest pattern"
        );
    }

    let document = generate_spec();
    let evidence_routes = RouteCatalog::new(CATALOGED_ROUTES)
        .project(
            CatalogProjection::OpenApi,
            crate::router::builder::compiled_route_features(),
        )
        .into_iter()
        .filter(|route| route.path().starts_with("/v1/evidence/"))
        .collect::<Vec<_>>();
    assert_eq!(
        evidence_routes.len(),
        12,
        "the evidence protocol must expose exactly twelve authenticated operations"
    );

    let expected_auth_headers = BTreeSet::from([
        ("X-Iroha-Account".to_owned(), false),
        ("X-Iroha-Signature".to_owned(), false),
        ("X-Iroha-Timestamp-Ms".to_owned(), false),
        ("X-Iroha-Nonce".to_owned(), false),
        ("X-Iroha-Witness".to_owned(), false),
    ]);

    for route in evidence_routes {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} {} catalog authentication policy",
            method_name(route.method()),
            route.path()
        );
        let method = method_name(route.method());
        let operation = openapi_operation(&document, route.path(), method);
        let auth_headers = operation_header_requirements(operation)
            .into_iter()
            .filter(|(name, _)| name.starts_with("X-Iroha-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            auth_headers,
            expected_auth_headers,
            "{method} {} canonical authentication headers",
            route.path()
        );

        let expected_secret_headers: BTreeSet<&str> = match (route.path(), method) {
            ("/v1/evidence/session", "post") => BTreeSet::from(["X-SoraFS-Evidence-Challenge"]),
            ("/v1/evidence/manifest/{session_id_hex}", "get")
            | ("/v1/evidence/segment/{session_id_hex}", "get")
            | ("/v1/evidence/log/{session_id_hex}", "post") => {
                BTreeSet::from(["X-SoraFS-Evidence-Grant"])
            }
            _ => BTreeSet::new(),
        };
        let parameters = operation
            .get("parameters")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{method} {} parameters", route.path()));
        let actual_secret_headers = parameters
            .iter()
            .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .filter(|name| name.starts_with("X-SoraFS-Evidence-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_secret_headers,
            expected_secret_headers,
            "{method} {} evidence request headers",
            route.path()
        );
        for name in expected_secret_headers {
            let parameter = parameters
                .iter()
                .find(|parameter| {
                    parameter.get("name").and_then(Value::as_str) == Some(name)
                        && parameter.get("in").and_then(Value::as_str) == Some("header")
                })
                .unwrap_or_else(|| panic!("{method} {} {name} request header", route.path()));
            assert_eq!(
                parameter.get("required").and_then(Value::as_bool),
                Some(true),
                "{method} {} {name} request requirement",
                route.path()
            );
            assert_opaque_token_schema(
                parameter
                    .get("schema")
                    .and_then(Value::as_object)
                    .unwrap_or_else(|| panic!("{method} {} {name} request schema", route.path())),
                &format!("{method} {} {name} request", route.path()),
            );
        }

        let (success_status, expected_response_headers): (&str, BTreeSet<&str>) =
            match (route.path(), method) {
                ("/v1/evidence/session/challenge", "post") => {
                    ("201", BTreeSet::from(["X-SoraFS-Evidence-Challenge"]))
                }
                ("/v1/evidence/session", "post") => {
                    ("201", BTreeSet::from(["X-SoraFS-Evidence-Grant"]))
                }
                ("/v1/evidence/manifest/{session_id_hex}", "get") => {
                    ("200", BTreeSet::from(["X-SoraFS-Evidence-Grant"]))
                }
                ("/v1/evidence/segment/{session_id_hex}", "get") => (
                    "206",
                    BTreeSet::from([
                        "X-SoraFS-Evidence-Grant",
                        "X-SoraFS-Evidence-Receipt-Digest",
                        "X-SoraFS-Evidence-Watermark-Digest",
                    ]),
                ),
                ("/v1/evidence/log/{session_id_hex}", "post") => {
                    ("202", BTreeSet::from(["X-SoraFS-Evidence-Grant"]))
                }
                ("/v1/evidence/legal-hold", "post") => ("201", BTreeSet::new()),
                _ => ("200", BTreeSet::new()),
            };
        let success_response = operation
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get(success_status))
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!(
                    "{method} {} {success_status} success response",
                    route.path()
                )
            });
        let response_headers = success_response.get("headers").and_then(Value::as_object);
        let actual_response_headers = response_headers
            .into_iter()
            .flat_map(|headers| headers.keys())
            .map(String::as_str)
            .filter(|name| name.starts_with("X-SoraFS-Evidence-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_response_headers,
            expected_response_headers,
            "{method} {} evidence success response headers",
            route.path()
        );
        for name in expected_response_headers {
            let header = response_headers
                .and_then(|headers| headers.get(name))
                .and_then(Value::as_object)
                .unwrap_or_else(|| {
                    panic!(
                        "{method} {} {success_status} {name} response header",
                        route.path()
                    )
                });
            assert_eq!(
                header.get("required").and_then(Value::as_bool),
                Some(true),
                "{method} {} {name} response requirement",
                route.path()
            );
            let schema = header
                .get("schema")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{method} {} {name} response schema", route.path()));
            match name {
                "X-SoraFS-Evidence-Challenge" | "X-SoraFS-Evidence-Grant" => {
                    assert_opaque_token_schema(
                        schema,
                        &format!("{method} {} {name} response", route.path()),
                    );
                }
                "X-SoraFS-Evidence-Receipt-Digest" => {
                    assert_eq!(
                        schema.get("$ref").and_then(Value::as_str),
                        Some("#/components/schemas/SorafsEvidenceNonzeroHex32V1")
                    );
                }
                "X-SoraFS-Evidence-Watermark-Digest" => {
                    assert_eq!(
                        schema.get("$ref").and_then(Value::as_str),
                        Some("#/components/schemas/SorafsEvidenceNonzeroHex32V1")
                    );
                }
                _ => panic!("unexpected evidence response header {name}"),
            }
        }
    }

    let manifest = openapi_operation(&document, "/v1/evidence/manifest/{session_id_hex}", "get");
    let manifest_queries = manifest
        .get("parameters")
        .and_then(Value::as_array)
        .expect("evidence manifest parameters")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .collect::<Vec<_>>();
    assert_eq!(manifest_queries.len(), 1);
    let idempotency_key = manifest_queries[0];
    assert_eq!(
        idempotency_key.get("name").and_then(Value::as_str),
        Some("idempotency_key_hex")
    );
    assert_eq!(
        idempotency_key.get("required").and_then(Value::as_bool),
        Some(true)
    );
    assert_nonzero_digest_schema(
        idempotency_key
            .get("schema")
            .and_then(Value::as_object)
            .expect("evidence manifest idempotency key schema"),
        "evidence manifest idempotency key",
    );

    let segment = openapi_operation(&document, "/v1/evidence/segment/{session_id_hex}", "get");
    let segment_queries = segment
        .get("parameters")
        .and_then(Value::as_array)
        .expect("evidence segment parameters")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .collect::<Vec<_>>();
    assert_eq!(
        segment_queries
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["start", "end", "idempotency_key_hex"])
    );
    let segment_query = |name: &str| {
        segment_queries
            .iter()
            .copied()
            .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
            .unwrap_or_else(|| panic!("evidence segment {name} query"))
    };
    for name in ["start", "end", "idempotency_key_hex"] {
        assert_eq!(
            segment_query(name).get("required").and_then(Value::as_bool),
            Some(true),
            "evidence segment {name} requirement"
        );
    }
    let start_schema = segment_query("start")
        .get("schema")
        .and_then(Value::as_object)
        .expect("evidence segment start schema");
    assert_eq!(
        start_schema.get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        start_schema.get("format").and_then(Value::as_str),
        Some("uint64")
    );
    assert_eq!(start_schema.get("minimum").and_then(Value::as_u64), Some(0));
    let end = segment_query("end");
    let end_schema = end
        .get("schema")
        .and_then(Value::as_object)
        .expect("evidence segment end schema");
    assert_eq!(
        end_schema.get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        end_schema.get("format").and_then(Value::as_str),
        Some("uint64")
    );
    assert_eq!(end_schema.get("minimum").and_then(Value::as_u64), Some(1));
    assert!(
        end.get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| description.contains("greater than start"))
    );
    assert_nonzero_digest_schema(
        segment_query("idempotency_key_hex")
            .get("schema")
            .and_then(Value::as_object)
            .expect("evidence segment idempotency key schema"),
        "evidence segment idempotency key",
    );
}

#[test]
fn sorafs_pin_register_openapi_is_caller_signed_transaction_transport() {
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/sorafs/pin/register", "post");
    assert_eq!(
        operation_request_schema_ref(operation, "/v1/sorafs/pin/register"),
        "#/components/schemas/VersionedSignedTransactionJson"
    );
    assert_eq!(
        operation_response_schema_ref(operation, "202", "/v1/sorafs/pin/register"),
        "#/components/schemas/SorafsPinRegisterResponseV1"
    );
    let request_content = operation
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .expect("pin-register request content");
    assert_eq!(
        request_content
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["application/json", "application/x-norito"])
    );
    assert_eq!(
        request_content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("x-iroha-norito-schema"))
            .and_then(Value::as_str),
        Some("SignedTransaction")
    );
    let success_content = operation
        .get("responses")
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("202"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .expect("pin-register success content");
    assert_eq!(
        success_content
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/json"]
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("pin-register operation description");
    assert!(
        description.contains("caller-signed")
            && description.contains("exactly one native `RegisterPinManifest`")
            && description.contains("queues the original transaction unchanged")
            && description.contains("Submitted never means committed or finalized")
            && description.contains("not a finality, fee, custody, or pin-status receipt")
            && description.contains("never accepts or handles a private key"),
        "pin-register operation must document the signature-bound transport contract"
    );

    let schemas = component_schemas(&document);
    assert!(
        !schemas.contains_key("SorafsPinRegisterRequestV1"),
        "the secret-bearing pin-register request DTO must not remain in OpenAPI"
    );
    assert_strict_object_schema(
        schemas,
        "SorafsPinRegisterResponseV1",
        &["status", "tx_hash_hex", "manifest_digest_hex"],
        &[],
    );
    assert!(!schemas.contains_key("SorafsPinAliasV1"));
    assert!(!schemas.contains_key("SorafsPinSuccessorDigestV1"));
}

#[test]
fn sorafs_storage_token_openapi_requires_the_credential_and_diagnostic_headers() {
    use iroha_torii_shared::route_catalog::AuthenticationPolicy;

    assert_eq!(
        iroha_torii_shared::route_catalog::sorafs::STORAGE_TOKEN.authentication(),
        AuthenticationPolicy::RequiredApiToken
    );
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/sorafs/storage/token", "post");
    assert_eq!(
        operation_header_requirements(operation)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ("X-API-Token".to_owned(), true),
            ("X-SoraFS-Client".to_owned(), true),
            ("X-SoraFS-Nonce".to_owned(), true),
        ])
    );
    assert!(
        operation
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| {
                description.contains("listener-wide API-token enforcement is disabled")
                    && description.contains("client label is diagnostic")
            })
    );
}

#[test]
fn sorafs_pin_list_openapi_is_finalized_bounded_keyset_readback() {
    const PATH: &str = "/v1/sorafs/pin";
    let document = generate_spec();
    let operation = openapi_operation(&document, PATH, "get");
    assert_eq!(
        operation_response_schema_ref(operation, "200", PATH),
        "#/components/schemas/PinManifestPageV1"
    );
    assert_eq!(
        operation
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .map(|content| content.keys().cloned().collect::<BTreeSet<_>>()),
        Some(BTreeSet::from([
            "application/json".to_owned(),
            "application/x-norito".to_owned(),
        ])),
        "pin-list OpenAPI response must advertise both supported representations"
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("pin-list operation description");
    assert!(
        description.contains("exclusive keyset cursor")
            && description.contains("O(1) consensus-maintained")
            && description.contains("Offset pagination")
            && description.contains("bounded detail route"),
        "pin-list operation must document the finalized bounded hard cut"
    );

    let parameters = operation
        .get("parameters")
        .and_then(Value::as_array)
        .expect("pin-list parameters");
    assert_eq!(
        parameters
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "after_digest_hex",
            "expected_finalized_block_hash_hex",
            "expected_finalized_height",
            "limit",
            "max_bytes",
            "status",
        ])
    );
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.get("name").and_then(Value::as_str) != Some("offset"))
    );

    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "PinManifestPageV1",
        &["finalized_cursor", "charged_usage", "manifests", "has_more"],
        &["next_after_digest"],
    );
    assert_strict_object_schema(
        schemas,
        "PinManifestSummaryV1",
        &[
            "digest",
            "submitted_by",
            "submitted_epoch",
            "content_length",
            "retention_epoch",
            "status",
        ],
        &["successor_of"],
    );
    assert_strict_object_schema(
        schemas,
        "PinResourceUsage",
        &["manifest_count", "content_bytes"],
        &[],
    );
    assert_eq!(
        property_ref(schemas, "PinManifestPageV1", "finalized_cursor"),
        "#/components/schemas/PinManifestFinalizedCursorV1"
    );
    assert_eq!(
        property_ref(schemas, "PinManifestPageV1", "charged_usage"),
        "#/components/schemas/PinResourceUsage"
    );
}

#[test]
fn sorafs_pin_manifest_openapi_is_finalized_native_readback() {
    const PATH: &str = "/v1/sorafs/pin/{digest_hex}";
    const RETIRED_TOP_LEVEL_FIELDS: [&str; 10] = [
        "limit",
        "attestation",
        "aliases",
        "alias_count",
        "aliases_returned",
        "aliases_truncated",
        "replication_orders",
        "replication_order_count",
        "replication_orders_returned",
        "replication_orders_truncated",
    ];

    let document = generate_spec();
    let operation = openapi_operation(&document, PATH, "get");
    assert_eq!(
        operation_response_schema_ref(operation, "200", PATH),
        "#/components/schemas/PinManifestFinalizedRecordV1"
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("pin-manifest operation description");
    assert!(
        description.contains("exact native Norito JSON `PinManifestFinalizedRecordV1`")
            && description.contains("must be supplied together")
            && description.contains("stale anchor returns 409")
            && description.contains("retired projection `limit`"),
        "pin-manifest operation must document finalized native readback and paired-anchor semantics"
    );

    let parameters = operation
        .get("parameters")
        .and_then(Value::as_array)
        .expect("pin-manifest parameters");
    assert_eq!(parameters.len(), 3);
    assert_eq!(
        parameters
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "digest_hex",
            "expected_finalized_height",
            "expected_finalized_block_hash_hex",
        ])
    );
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.get("name").and_then(Value::as_str) != Some("limit")),
        "the retired projection limit must not remain in the operation"
    );

    let height = parameters
        .iter()
        .find(|parameter| {
            parameter.get("name").and_then(Value::as_str) == Some("expected_finalized_height")
        })
        .and_then(Value::as_object)
        .expect("expected finalized height parameter");
    assert_eq!(height.get("in").and_then(Value::as_str), Some("query"));
    assert_eq!(height.get("required").and_then(Value::as_bool), Some(false));
    assert_eq!(
        height
            .get("schema")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("minimum"))
            .and_then(Value::as_u64),
        Some(1)
    );

    let block_hash = parameters
        .iter()
        .find(|parameter| {
            parameter.get("name").and_then(Value::as_str)
                == Some("expected_finalized_block_hash_hex")
        })
        .and_then(Value::as_object)
        .expect("expected finalized block hash parameter");
    assert_eq!(block_hash.get("in").and_then(Value::as_str), Some("query"));
    assert_eq!(
        block_hash.get("required").and_then(Value::as_bool),
        Some(false)
    );
    let block_hash_schema = block_hash
        .get("schema")
        .and_then(Value::as_object)
        .expect("expected finalized block hash schema");
    assert_eq!(
        block_hash_schema.get("minLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        block_hash_schema.get("maxLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        block_hash_schema.get("pattern").and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );

    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "PinManifestFinalizedRecordV1",
        &["finalized_cursor", "manifest"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "PinManifestFinalizedCursorV1",
        &["height", "block_hash"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "PinManifestRecord",
        &[
            "digest",
            "root_cid",
            "chunker",
            "chunk_digest_sha3_256",
            "por_root",
            "content_length",
            "policy",
            "submitted_by",
            "submitted_epoch",
            "alias",
            "metadata",
            "status",
            "council_envelope_digest",
        ],
        &["successor_of", "retirement_reason", "pin_fee_payment"],
    );
    assert_eq!(
        property_ref(schemas, "PinManifestFinalizedRecordV1", "finalized_cursor"),
        "#/components/schemas/PinManifestFinalizedCursorV1"
    );
    assert_eq!(
        property_ref(schemas, "PinManifestFinalizedRecordV1", "manifest"),
        "#/components/schemas/PinManifestRecord"
    );
    assert_eq!(
        property_ref(schemas, "PinManifestRecord", "por_root"),
        "#/components/schemas/PinManifestBytes32V1"
    );
    assert_eq!(
        property_ref(schemas, "PinManifestRecord", "status"),
        "#/components/schemas/PinStatus"
    );
    assert_eq!(
        nullable_property_ref(schemas, "PinManifestRecord", "alias"),
        "#/components/schemas/ManifestAliasBinding"
    );
    assert_eq!(
        nullable_property_ref(schemas, "PinManifestRecord", "council_envelope_digest"),
        "#/components/schemas/PinManifestBytes32V1"
    );

    let content_length = component_properties(schemas, "PinManifestRecord")
        .get("content_length")
        .and_then(Value::as_object)
        .expect("native manifest content length schema");
    assert_eq!(
        content_length.get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        content_length.get("format").and_then(Value::as_str),
        Some("uint64")
    );

    let response_properties = component_properties(schemas, "PinManifestFinalizedRecordV1");
    for retired in RETIRED_TOP_LEVEL_FIELDS {
        assert!(
            !response_properties.contains_key(retired),
            "retired pin-manifest projection field `{retired}` must remain absent"
        );
    }
    let bytes32 = schemas
        .get("PinManifestBytes32V1")
        .and_then(Value::as_object)
        .expect("pin-manifest bytes32 schema");
    assert_eq!(bytes32.get("minItems").and_then(Value::as_u64), Some(32));
    assert_eq!(bytes32.get("maxItems").and_then(Value::as_u64), Some(32));
    let statuses = schemas
        .get("PinStatus")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("oneOf"))
        .and_then(Value::as_array)
        .expect("native pin status variants");
    assert_eq!(
        statuses
            .iter()
            .filter_map(|variant| {
                variant
                    .get("properties")
                    .and_then(Value::as_object)
                    .and_then(|properties| properties.get("status"))
                    .and_then(Value::as_object)
                    .and_then(|status| status.get("const"))
                    .and_then(Value::as_str)
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Pending", "Approved", "Retired"])
    );
}

#[test]
fn sorafs_replication_openapi_is_a_strict_chain_authoritative_v1_projection() {
    const PATH: &str = "/v1/sorafs/replication";

    let document = generate_spec();
    let operation = openapi_operation(&document, PATH, "get");
    assert_eq!(
        operation_response_schema_ref(operation, "200", PATH),
        "#/components/schemas/SorafsReplicationListResponseV1"
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("replication operation description");
    assert!(
        description.contains("fresh committed registry projection")
            && description.contains("assignment revision")
            && description.contains("provider-owner signer-policy")
            && description.contains("finalized block anchor")
            && description.contains("unknown, duplicate, empty, aliased, or out-of-range"),
        "replication operation must document the V1 hard-cut projection and selectors"
    );

    let parameters = operation
        .get("parameters")
        .and_then(Value::as_array)
        .expect("replication query parameters");
    assert_eq!(
        parameters
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["limit", "offset", "status", "manifest_digest"])
    );
    let status_parameter = parameters
        .iter()
        .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some("status"))
        .and_then(Value::as_object)
        .and_then(|parameter| parameter.get("schema"))
        .and_then(Value::as_object)
        .expect("replication status parameter schema");
    assert_eq!(
        status_parameter
            .get("enum")
            .and_then(Value::as_array)
            .expect("replication status values")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["pending", "completed", "expired"])
    );
    let digest_parameter = parameters
        .iter()
        .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some("manifest_digest"))
        .and_then(Value::as_object)
        .and_then(|parameter| parameter.get("schema"))
        .and_then(Value::as_object)
        .expect("replication digest parameter schema");
    assert_eq!(
        digest_parameter.get("pattern").and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );

    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "SorafsRegistryAttestationV1",
        &["block_height", "block_hash_hex", "chain_id"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationAssignmentV1",
        &["provider_id_hex", "slice_gib", "lane"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationSlaV1",
        &[
            "ingest_deadline_secs",
            "min_availability_percent_milli",
            "min_por_success_percent_milli",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationMetadataEntryV1",
        &["key", "value"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationCanonicalOrderV1",
        &[
            "version",
            "order_id_hex",
            "manifest_cid_b64",
            "manifest_digest_hex",
            "chunking_profile",
            "target_replicas",
            "assignments",
            "issued_at",
            "deadline_at",
            "sla",
            "metadata",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsProviderIngestCompletionAuthorityV1",
        &["provider_owner", "signer_policy"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsProviderIngestFinalizedAnchorV1",
        &["height", "block_hash_hex"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationCompletionV1",
        &[
            "provider_hex",
            "completed_by",
            "completion_epoch",
            "assignment_revision",
            "completion_authority",
            "finalized_anchor",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationOrderProjectionV1",
        &[
            "order_id_hex",
            "manifest_digest_hex",
            "issued_by",
            "issued_epoch",
            "deadline_epoch",
            "status",
            "canonical_order_b64",
            "assignment_revision",
            "order",
            "provider_completions",
            "providers",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsReplicationListResponseV1",
        &[
            "attestation",
            "total_count",
            "returned_count",
            "offset",
            "limit",
            "replication_orders",
        ],
        &[],
    );

    assert_eq!(
        property_ref(
            schemas,
            "SorafsReplicationCompletionV1",
            "completion_authority"
        ),
        "#/components/schemas/SorafsProviderIngestCompletionAuthorityV1"
    );
    assert_eq!(
        property_ref(schemas, "SorafsReplicationCompletionV1", "finalized_anchor"),
        "#/components/schemas/SorafsProviderIngestFinalizedAnchorV1"
    );
    assert_eq!(
        property_ref(
            schemas,
            "SorafsProviderIngestCompletionAuthorityV1",
            "signer_policy"
        ),
        "#/components/schemas/SorafsProviderIngestSignerPolicyV1"
    );
    assert_eq!(
        property_ref(schemas, "SorafsReplicationOrderProjectionV1", "order"),
        "#/components/schemas/SorafsReplicationCanonicalOrderV1"
    );
    assert_eq!(
        property_ref(schemas, "SorafsReplicationOrderProjectionV1", "status"),
        "#/components/schemas/SorafsReplicationOrderStatusV1"
    );

    let status_variants = schemas
        .get("SorafsReplicationOrderStatusV1")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("oneOf"))
        .and_then(Value::as_array)
        .expect("replication lifecycle variants");
    assert_eq!(status_variants.len(), 3);
    for variant in status_variants {
        assert_eq!(
            variant.get("additionalProperties").and_then(Value::as_bool),
            Some(false)
        );
    }
    assert_eq!(
        status_variants
            .iter()
            .filter_map(|variant| {
                variant
                    .get("properties")
                    .and_then(Value::as_object)
                    .and_then(|properties| properties.get("state"))
                    .and_then(Value::as_object)
                    .and_then(|state| state.get("const"))
                    .and_then(Value::as_str)
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["pending", "completed", "expired"])
    );
    assert_eq!(
        status_variants[0]
            .get("properties")
            .and_then(Value::as_object)
            .expect("pending status properties")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["state"]),
        "pending must not carry a compatibility epoch"
    );

    let policy_variants = schemas
        .get("SorafsProviderIngestSignerPolicyV1")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("oneOf"))
        .and_then(Value::as_array)
        .expect("signer-policy chain variants");
    assert_eq!(policy_variants.len(), 2);
    assert_eq!(
        policy_variants[0]
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("revision"))
            .and_then(Value::as_object)
            .and_then(|revision| revision.get("const"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        policy_variants[0]
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("predecessor_digest_hex"))
            .and_then(Value::as_object)
            .and_then(|predecessor| predecessor.get("type"))
            .and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        policy_variants[1]
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("revision"))
            .and_then(Value::as_object)
            .and_then(|revision| revision.get("minimum"))
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        policy_variants[1]
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("predecessor_digest_hex"))
            .and_then(Value::as_object)
            .and_then(|predecessor| predecessor.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationNonzeroHex32V1")
    );

    let projection_properties = component_properties(schemas, "SorafsReplicationOrderProjectionV1");
    assert_eq!(
        projection_properties
            .get("provider_completions")
            .and_then(Value::as_object)
            .and_then(|array| array.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationCompletionV1")
    );
    assert_eq!(
        component_properties(schemas, "SorafsReplicationListResponseV1")
            .get("replication_orders")
            .and_then(Value::as_object)
            .and_then(|array| array.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationOrderProjectionV1")
    );
    let canonical_order = projection_properties
        .get("canonical_order_b64")
        .and_then(Value::as_object)
        .expect("canonical order payload schema");
    assert_eq!(
        canonical_order.get("maxLength").and_then(Value::as_u64),
        Some(349_528)
    );
}

#[test]
fn moderation_dead_letter_openapi_is_typed_bounded_and_dual_control() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "SorafsModerationDeadLetterPrepareRequestV1",
        &["identity_hex", "kind", "action", "authorized_at_unix_ms"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsModerationDeadLetterPrepareResponseV1",
        &[
            "schema",
            "status",
            "resolution_norito_b64",
            "signing_message_hex",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsModerationDeadLetterApplyRequestV1",
        &["resolution_norito_b64", "signature_hex"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsModerationDeadLetterApplyResponseV1",
        &["schema", "status", "identity_hex", "kind", "action"],
        &[],
    );

    let resolution_schema = schemas
        .get("SorafsModerationDeadLetterResolutionNoritoBase64V1")
        .and_then(Value::as_object)
        .expect("moderation resolution base64 schema");
    assert_eq!(
        resolution_schema.get("maxLength").and_then(Value::as_u64),
        Some(
            u64::try_from(SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1)
                .expect("moderation resolution base64 bound fits uint64")
        )
    );
    assert_eq!(
        schemas
            .get("SorafsModerationDeadLetterKindV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        schemas
            .get("SorafsModerationDeadLetterResolutionActionV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let expected_auth_headers = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);
    for (path, route, request_schema, response_schema, request_max_bytes) in [
            (
                "/v1/sorafs/moderation/dead-letters/prepare",
                iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_PREPARE_POST,
                "#/components/schemas/SorafsModerationDeadLetterPrepareRequestV1",
                "#/components/schemas/SorafsModerationDeadLetterPrepareResponseV1",
                SORAFS_MODERATION_DEAD_LETTER_PREPARE_REQUEST_MAX_BYTES_V1,
            ),
            (
                "/v1/sorafs/moderation/dead-letters/apply",
                iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_APPLY_POST,
                "#/components/schemas/SorafsModerationDeadLetterApplyRequestV1",
                "#/components/schemas/SorafsModerationDeadLetterApplyResponseV1",
                SORAFS_MODERATION_DEAD_LETTER_APPLY_REQUEST_MAX_BYTES_V1,
            ),
        ] {
            assert!(catalog_openapi_route_enabled(CatalogHttpMethod::Post, path));
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation.get("operationId").and_then(Value::as_str),
                Some(route.stable_route_id())
            );
            assert_eq!(operation_request_schema_ref(operation, path), request_schema);
            assert_eq!(
                operation_response_schema_ref(operation, "200", path),
                response_schema
            );
            assert_eq!(
                operation
                    .get("x-iroha-max-request-bytes")
                    .and_then(Value::as_u64),
                Some(
                    u64::try_from(request_max_bytes)
                        .expect("moderation request bound fits uint64")
                )
            );
            assert_eq!(
                operation_header_requirements(operation)
                    .into_iter()
                    .map(|(name, required)| {
                        assert!(!required, "{path} canonical auth uses alternative proof sets");
                        name
                    })
                    .collect::<BTreeSet<_>>(),
                expected_auth_headers
            );
            assert_eq!(
                operation.get("security").and_then(Value::as_array).map(Vec::len),
                Some(2)
            );
            let description = operation
                .get("description")
                .and_then(Value::as_str)
                .expect("moderation recovery description");
            assert!(description.contains("independent"));
            let responses = operation
                .get("responses")
                .and_then(Value::as_object)
                .expect("moderation recovery responses");
            for status in ["200", "400", "401", "403", "404", "409", "429", "503"] {
                assert!(responses.contains_key(status), "{path} missing HTTP {status}");
            }
        }
}

#[test]
fn hedging_billing_openapi_is_authenticated_bounded_and_private() {
    let document = generate_spec();
    let expected_auth_headers = BTreeSet::from([
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ]);
    for (path, method, catalog_method) in [
        ("/v1/sorafs/billing/status", "get", CatalogHttpMethod::Get),
        (
            "/v1/sorafs/billing/statements",
            "get",
            CatalogHttpMethod::Get,
        ),
        (
            "/v1/sorafs/billing/statements/{statement_id}",
            "get",
            CatalogHttpMethod::Get,
        ),
        (
            "/v1/sorafs/billing/statements/{statement_id}/acknowledgements",
            "post",
            CatalogHttpMethod::Post,
        ),
        (
            "/v1/sorafs/billing/reconciliation",
            "get",
            CatalogHttpMethod::Get,
        ),
        ("/v1/sorafs/hedging/exposure", "get", CatalogHttpMethod::Get),
        ("/v1/sorafs/hedging/intents", "get", CatalogHttpMethod::Get),
    ] {
        assert!(
            catalog_openapi_route_enabled(catalog_method, path),
            "{method} {path} must be projected by the canonical route catalog"
        );
        let operation = openapi_operation(&document, path, method);
        let auth_headers = operation_header_requirements(operation)
            .into_iter()
            .map(|(name, required)| {
                assert!(
                    !required,
                    "{method} {path} canonical auth headers are alternative proof sets"
                );
                name
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            auth_headers,
            expected_auth_headers
                .iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>(),
            "{method} {path} canonical auth inventory"
        );
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("hedging/billing responses");
        for (status, response) in responses {
            let headers = response
                .get("headers")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{method} {path} HTTP {status} private headers"));
            assert_eq!(
                headers
                    .get("Cache-Control")
                    .and_then(Value::as_object)
                    .and_then(|header| header.get("schema"))
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("const"))
                    .and_then(Value::as_str),
                Some("private, no-store")
            );
            assert_eq!(
                headers
                    .get("Vary")
                    .and_then(Value::as_object)
                    .and_then(|header| header.get("schema"))
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("const"))
                    .and_then(Value::as_str),
                Some(
                    "X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, X-Iroha-Nonce, X-Iroha-Witness"
                )
            );
        }
    }

    for path in [
        "/v1/sorafs/billing/statements",
        "/v1/sorafs/hedging/exposure",
        "/v1/sorafs/hedging/intents",
    ] {
        let parameters = openapi_operation(&document, path, "get")
            .get("parameters")
            .and_then(Value::as_array)
            .expect("bounded page parameters");
        let limit = parameters
            .iter()
            .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some("limit"))
            .expect("required page limit");
        assert_eq!(limit.get("required").and_then(Value::as_bool), Some(true));
        assert_eq!(
            limit
                .get("schema")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(100)
        );
        let checkpoint = parameters
            .iter()
            .find(|parameter| {
                parameter.get("name").and_then(Value::as_str)
                    == Some("expected_checkpoint_fingerprint")
            })
            .expect("required checkpoint fingerprint");
        assert_eq!(
            checkpoint.get("required").and_then(Value::as_bool),
            Some(true)
        );
    }

    let statement_content = openapi_operation(
        &document,
        "/v1/sorafs/billing/statements/{statement_id}",
        "get",
    )
    .get("responses")
    .and_then(Value::as_object)
    .and_then(|responses| responses.get("200"))
    .and_then(Value::as_object)
    .and_then(|response| response.get("content"))
    .and_then(Value::as_object)
    .expect("exact statement response content");
    assert_eq!(
        statement_content
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/x-norito"]
    );
    assert_eq!(
        statement_content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("x-iroha-norito-schema"))
            .and_then(Value::as_str),
        Some("BillingPublishedStatementV1")
    );

    let acknowledgement_content = openapi_operation(
        &document,
        "/v1/sorafs/billing/statements/{statement_id}/acknowledgements",
        "post",
    )
    .get("requestBody")
    .and_then(Value::as_object)
    .and_then(|body| body.get("content"))
    .and_then(Value::as_object)
    .expect("acknowledgement request content");
    assert_eq!(
        acknowledgement_content
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/x-norito"]
    );
    let acknowledgement_schema = acknowledgement_content
        .get("application/x-norito")
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .expect("acknowledgement Norito schema");
    assert_eq!(
        acknowledgement_schema
            .get("x-iroha-norito-schema")
            .and_then(Value::as_str),
        Some(BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1)
    );
    assert_eq!(
        acknowledgement_schema
            .get("x-iroha-norito-schema-hash")
            .and_then(Value::as_str),
        Some(BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_HEX_V1)
    );
    assert_eq!(
        acknowledgement_schema
            .get("maxLength")
            .and_then(Value::as_u64),
        Some(69_632)
    );

    let schemas = component_schemas(&document);
    for (schema_name, tag, variants) in [
        (
            "HedgingBillingRetentionScopeV1",
            "scope",
            &["active_epoch_only"][..],
        ),
        (
            "BillingStatementOwnerStatusV1",
            "status",
            &["published", "acknowledged"][..],
        ),
        ("HedgeIntentDirectionV1", "direction", &["sell_xor"][..]),
        (
            "HedgeIntentDispositionV1",
            "disposition",
            &["executable", "governed_overflow"][..],
        ),
    ] {
        let actual = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("oneOf"))
            .and_then(Value::as_array)
            .expect("tagged hedging/billing enum")
            .iter()
            .filter_map(|variant| {
                variant
                    .get("properties")
                    .and_then(Value::as_object)
                    .and_then(|properties| properties.get(tag))
                    .and_then(Value::as_object)
                    .and_then(|tag_schema| tag_schema.get("const"))
                    .and_then(Value::as_str)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, variants.iter().copied().collect::<BTreeSet<_>>());
    }
}

#[test]
fn proof_stream_openapi_matches_the_closed_canonical_envelope() {
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/sorafs/proof/stream", "post");
    assert_eq!(
        operation_request_schema_ref(operation, "/v1/sorafs/proof/stream"),
        "#/components/schemas/SorafsProofStreamHttpRequestV1"
    );

    let success_content = operation
        .get("responses")
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("200"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .expect("proof-stream success content");
    assert_eq!(
        success_content
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/x-ndjson"]
    );
    assert_eq!(
        success_content
            .get("application/x-ndjson")
            .and_then(Value::as_object)
            .and_then(|media| media.get("x-iroha-ndjson-item-schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsProofStreamItemV1")
    );

    let schemas = component_schemas(&document);
    assert!(
        document
            .get("paths")
            .and_then(Value::as_object)
            .and_then(|paths| paths.get("/v1/sorafs/storage/por-sample"))
            .is_none(),
        "the unauthenticated local PoR sampling route must remain retired"
    );
    assert!(
        !schemas.contains_key("SorafsStoragePorSampleRequestV1"),
        "the retired route's request schema must not remain in generated OpenAPI"
    );
    let aggregate = schemas
        .get("SorafsProofStreamHttpRequestV1")
        .and_then(Value::as_object)
        .expect("canonical proof-stream request schema");
    let variants = aggregate
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("proof-stream request variants")
        .iter()
        .map(|variant| {
            variant
                .get("$ref")
                .and_then(Value::as_str)
                .expect("proof-stream request variant reference")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        [
            "#/components/schemas/SorafsProofStreamPorRequestV1",
            "#/components/schemas/SorafsProofStreamPdpRequestV1",
            "#/components/schemas/SorafsProofStreamPotrRequestV1",
        ]
    );

    for (name, kind, required_field, allowed_kind_fields, forbidden_kind_fields) in [
        (
            "SorafsProofStreamPorRequestV1",
            "por",
            "sample_count",
            &["sample_count", "sample_seed"][..],
            &["challenge_id_hex", "deadline_ms", "orchestrator_job_id_hex"][..],
        ),
        (
            "SorafsProofStreamPdpRequestV1",
            "pdp",
            "challenge_id_hex",
            &["challenge_id_hex"][..],
            &[
                "sample_count",
                "sample_seed",
                "deadline_ms",
                "orchestrator_job_id_hex",
            ][..],
        ),
        (
            "SorafsProofStreamPotrRequestV1",
            "potr",
            "deadline_ms",
            &["deadline_ms", "orchestrator_job_id_hex"][..],
            &["challenge_id_hex", "sample_count", "sample_seed"][..],
        ),
    ] {
        let schema = schemas
            .get(name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name} schema"));
        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{name} must reject unknown and alias fields"
        );
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("proof request required fields");
        assert!(
            required
                .iter()
                .any(|field| field.as_str() == Some(required_field)),
            "{name} must require {required_field}"
        );
        if kind == "potr" {
            assert!(
                required
                    .iter()
                    .any(|field| field.as_str() == Some("orchestrator_job_id_hex")),
                "PoTR must require the request-scope job id"
            );
        }
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("proof request properties");
        for field in [
            "expected_finalized_height",
            "expected_finalized_block_hash_hex",
        ] {
            assert!(
                properties.contains_key(field),
                "{name} must publish the finalized cursor field {field}"
            );
        }
        if kind == "por" {
            for field in [
                "expected_finalized_height",
                "expected_finalized_block_hash_hex",
            ] {
                assert!(
                    required
                        .iter()
                        .any(|required| required.as_str() == Some(field)),
                    "PoR must require finalized cursor field {field}"
                );
            }
        } else {
            let dependencies = schema
                .get("dependentRequired")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{name} finalized cursor dependencies"));
            assert_eq!(
                dependencies
                    .get("expected_finalized_height")
                    .and_then(Value::as_array)
                    .and_then(|fields| fields.first())
                    .and_then(Value::as_str),
                Some("expected_finalized_block_hash_hex")
            );
            assert_eq!(
                dependencies
                    .get("expected_finalized_block_hash_hex")
                    .and_then(Value::as_array)
                    .and_then(|fields| fields.first())
                    .and_then(Value::as_str),
                Some("expected_finalized_height")
            );
        }
        assert_eq!(
            properties
                .get("proof_kind")
                .and_then(Value::as_object)
                .and_then(|kind_schema| kind_schema.get("const"))
                .and_then(Value::as_str),
            Some(kind)
        );
        for field in allowed_kind_fields {
            assert!(
                properties.contains_key(*field),
                "{name} must publish {field}"
            );
        }
        for field in forbidden_kind_fields {
            assert!(
                !properties.contains_key(*field),
                "{name} must not publish incompatible field {field}"
            );
        }
        assert_eq!(
            properties
                .get("nonce_b64")
                .and_then(Value::as_object)
                .and_then(|nonce| nonce.get("pattern"))
                .and_then(Value::as_str),
            Some("^(?!A{22}==$)[A-Za-z0-9+/]{21}[AQgw]==$")
        );
        assert_eq!(
            properties
                .get("expected_finalized_height")
                .and_then(Value::as_object)
                .and_then(|height| height.get("minimum"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            properties
                .get("expected_finalized_block_hash_hex")
                .and_then(Value::as_object)
                .and_then(|hash| hash.get("pattern"))
                .and_then(Value::as_str),
            Some("^(?!0{64}$)[0-9a-f]{64}$")
        );
    }

    let por_properties = schemas
        .get("SorafsProofStreamPorRequestV1")
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("PoR request properties");
    assert_eq!(
        por_properties
            .get("sample_count")
            .and_then(Value::as_object)
            .and_then(|count| count.get("maximum"))
            .and_then(Value::as_u64),
        Some(500)
    );

    let por_proof = schemas
        .get("SorafsPorProofV1")
        .and_then(Value::as_object)
        .expect("closed PoR proof schema");
    assert_eq!(
        por_proof
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        por_proof
            .get("required")
            .and_then(Value::as_array)
            .expect("PoR proof required fields")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        [
            "payload_len",
            "chunk_count",
            "chunk_index",
            "chunk_offset",
            "chunk_length",
            "chunk_digest_hex",
            "chunk_root_hex",
            "segment_index",
            "segment_offset",
            "segment_length",
            "segment_digest_hex",
            "leaf_index",
            "leaf_offset",
            "leaf_length",
            "leaf_bytes_hex",
            "leaf_digest_hex",
            "segment_leaves_hex",
            "chunk_segments_hex",
            "chunk_merkle_path_hex",
        ]
    );
    let proof_properties = por_proof
        .get("properties")
        .and_then(Value::as_object)
        .expect("PoR proof properties");
    for digest_field in [
        "chunk_digest_hex",
        "chunk_root_hex",
        "segment_digest_hex",
        "leaf_digest_hex",
    ] {
        assert_eq!(
            proof_properties
                .get(digest_field)
                .and_then(Value::as_object)
                .and_then(|field| field.get("pattern"))
                .and_then(Value::as_str),
            Some("^[0-9a-f]{64}$"),
            "{digest_field} must require canonical lowercase digest hex"
        );
    }
    let leaf_bytes = proof_properties
        .get("leaf_bytes_hex")
        .and_then(Value::as_object)
        .expect("PoR leaf bytes schema");
    assert_eq!(
        leaf_bytes.get("pattern").and_then(Value::as_str),
        Some("^(?:[0-9a-f]{2})+$")
    );
    assert_eq!(
        leaf_bytes.get("maxLength").and_then(Value::as_u64),
        Some(8_192)
    );
    for (field, maximum) in [
        ("chunk_count", 4_194_304),
        ("chunk_index", 4_194_303),
        ("chunk_length", 4_194_304),
        ("segment_index", 63),
        ("segment_length", 65_536),
        ("leaf_index", 15),
        ("leaf_length", 4_096),
    ] {
        assert_eq!(
            proof_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(maximum),
            "{field} must publish the self-verifying runtime bound"
        );
    }
    for (field, maximum) in [("segment_leaves_hex", 16), ("chunk_segments_hex", 64)] {
        let array = proof_properties
            .get(field)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{field} schema"));
        assert_eq!(
            array.get("minItems").and_then(Value::as_u64),
            Some(1),
            "{field} must not accept an empty Merkle level"
        );
        assert_eq!(
            array.get("maxItems").and_then(Value::as_u64),
            Some(maximum),
            "{field} must publish the runtime allocation bound"
        );
        assert_eq!(
            array
                .get("items")
                .and_then(Value::as_object)
                .and_then(|items| items.get("pattern"))
                .and_then(Value::as_str),
            Some("^[0-9a-f]{64}$"),
            "{field} entries must use canonical lowercase digest hex"
        );
    }
    let chunk_path = proof_properties
        .get("chunk_merkle_path_hex")
        .and_then(Value::as_object)
        .expect("chunk Merkle path schema");
    assert_eq!(chunk_path.get("minItems").and_then(Value::as_u64), Some(0));
    assert_eq!(chunk_path.get("maxItems").and_then(Value::as_u64), Some(22));

    let item = schemas
        .get("SorafsProofStreamItemV1")
        .and_then(Value::as_object)
        .expect("proof-stream item schema");
    let item_properties = item
        .get("properties")
        .and_then(Value::as_object)
        .expect("proof-stream item properties");
    assert_eq!(
        item_properties
            .get("proof")
            .and_then(Value::as_object)
            .and_then(|proof| proof.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsPorProofV1")
    );
    for field in ["deadline_ms", "recorded_at_ms"] {
        assert_eq!(
            item_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("minimum"))
                .and_then(Value::as_u64),
            Some(1),
            "{field} must reject the zero value forbidden by signed PoTR receipts"
        );
    }
    let receipt = item_properties
        .get("receipt_b64")
        .and_then(Value::as_object)
        .expect("PoTR receipt schema");
    assert_eq!(
        receipt.get("pattern").and_then(Value::as_str),
        Some("^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$")
    );
    assert_eq!(
        receipt
            .get("x-iroha-runtime-validation")
            .and_then(Value::as_object)
            .and_then(|validation| validation.get("requireByteIdenticalCanonicalReencode"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        receipt
            .get("x-iroha-runtime-validation")
            .and_then(Value::as_object)
            .and_then(|validation| validation.get("requireValidatedReceipt"))
            .and_then(Value::as_bool),
        Some(true)
    );

    let kind_variants = item
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|all_of| all_of.get(1))
        .and_then(|kind_constraint| kind_constraint.get("oneOf"))
        .and_then(Value::as_array)
        .expect("proof-kind response variants");
    for (kind, expected_reasons) in [
        (
            "pdp",
            &[
                "deadline_expired",
                "submission_late",
                "future_timestamp",
                "invalid_proof",
                "admission_revoked",
                "admission_inactive",
                "storage_unavailable",
            ][..],
        ),
        (
            "potr",
            &[
                "missed_deadline",
                "provider_error",
                "gateway_error",
                "client_cancelled",
            ][..],
        ),
    ] {
        let variant = kind_variants
            .iter()
            .find(|variant| {
                variant
                    .get("properties")
                    .and_then(|properties| properties.get("proof_kind"))
                    .and_then(|proof_kind| proof_kind.get("const"))
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .unwrap_or_else(|| panic!("{kind} response variant"));
        assert_eq!(
            variant
                .get("properties")
                .and_then(|properties| properties.get("failure_reason"))
                .and_then(|reason| reason.get("enum"))
                .and_then(Value::as_array)
                .expect("kind-specific failure reasons")
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            expected_reasons,
            "{kind} must expose only its canonical terminal failure statuses"
        );
    }
}
