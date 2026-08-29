const OPENAPI_CONTRACT_ASSET_VERSION: u64 = 1;
const OPENAPI_CONTRACT_ASSET_LEN: usize = 15_417;
const OPENAPI_CONTRACT_ASSET_SHA256: &str =
    "7a02dfd132a9e26da1201a79bd70f3a1f19437075326300aab63485b9e517a68";
const OPENAPI_CONTRACT_SECTION_ORDER: &[&str] = &[
    "evidence.audit.description",
    "evidence.audit.success",
    "evidence.schemas",
    "pin.register.description",
    "pin.list.description",
    "pin.manifest.description",
    "pin.manifest.retired",
    "replication.description",
    "proof.por.required",
    "proof.pdp.failures",
    "proof.potr.failures",
    "sumeragi.da.required",
    "bridge.proof.required",
    "bridge.attestation.required",
    "finality.artifact.required",
    "height.context.required",
    "height.context.nullable",
    "validator.power.required",
    "dual.quorum.required",
    "block.subject.required",
    "block.subject.nullable",
    "merge.carrier.required",
    "execution.required",
    "execution.nullable",
    "qc.required",
    "snapshot.bootstrap.required",
    "next.epoch.required",
    "bridge.commitment.required",
    "bridge.bundle.required",
    "block.header.required",
    "block.header.nullable",
    "ledger.state_finality.required",
    "ledger.state_finality.retired",
    "ledger.state_finality.retired_paths",
    "ledger.state_finality.retired_schemas",
    "bridge.components",
    "bridge.retired",
    "fixture.header.required",
    "fixture.artifact.fields",
    "fixture.execution.fields",
    "fixture.retired",
    "lifecycle.required",
    "status.present",
    "status.absent",
    "native.receipt.required",
    "native.leg.required",
    "native.proposal.required",
    "native.body.required",
    "hf.headers",
    "app.page.required",
    "app.page.properties",
    "repo.agreement.fields",
    "repo.query.fields",
    "contract.alias.request.required",
    "contract.alias.binding.required",
    "contract.alias.binding.optional",
    "contract.alias.response.required",
    "governed.found.fields",
    "governed.inactive.fields",
    "governed.missing.fields",
];
const OPENAPI_CONTRACT_ASSET: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/openapi/tests/openapi_contracts_v1.json"
));

struct SchemaShape {
    name: &'static str,
    required: &'static str,
    optional: Option<&'static str>,
}

struct PropertyRefContract {
    owner: &'static str,
    property: &'static str,
    expected: &'static str,
}

struct OperationResponseContract {
    path: &'static str,
    method: &'static str,
    status: &'static str,
    schema_ref: &'static str,
}

fn contract_asset() -> &'static Map {
    use sha2::{Digest as _, Sha256};
    static ASSET: LazyLock<Map> = LazyLock::new(|| {
        assert_eq!(OPENAPI_CONTRACT_ASSET.len(), OPENAPI_CONTRACT_ASSET_LEN);
        assert_eq!(
            hex::encode(Sha256::digest(OPENAPI_CONTRACT_ASSET)),
            OPENAPI_CONTRACT_ASSET_SHA256,
            "OpenAPI contract asset digest drift"
        );
        let value: Value = norito::json::from_slice(OPENAPI_CONTRACT_ASSET)
            .expect("OpenAPI contract V1 asset must be valid Norito JSON");
        let root = value
            .as_object()
            .expect("OpenAPI contract asset root object");
        assert_eq!(
            root.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["sections", "version"]),
            "OpenAPI contract asset root keys"
        );
        assert_eq!(
            root.get("version").and_then(Value::as_u64),
            Some(OPENAPI_CONTRACT_ASSET_VERSION),
            "unsupported OpenAPI contract asset version"
        );
        let sections = root
            .get("sections")
            .and_then(Value::as_array)
            .expect("OpenAPI contract asset sections");
        assert_eq!(sections.len(), OPENAPI_CONTRACT_SECTION_ORDER.len());
        let mut indexed = Map::new();
        for (section, expected_id) in sections.iter().zip(OPENAPI_CONTRACT_SECTION_ORDER) {
            let section = section.as_object().expect("contract section object");
            assert_eq!(
                section.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                BTreeSet::from(["id", "values"]),
                "contract section keys"
            );
            let id = section
                .get("id")
                .and_then(Value::as_str)
                .expect("section id");
            assert_eq!(id, *expected_id, "OpenAPI contract section order drift");
            let values = section
                .get("values")
                .and_then(Value::as_array)
                .expect("contract section string inventory");
            assert!(
                !values.is_empty(),
                "contract section `{id}` must not be empty"
            );
            assert!(
                values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|item| !item.is_empty())),
                "contract section `{id}` must contain only non-empty strings"
            );
            assert_eq!(
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
                    .len(),
                values.len(),
                "contract section `{id}` must not contain duplicates"
            );
            assert!(
                indexed
                    .insert(id.to_owned(), Value::Array(values.clone()))
                    .is_none()
            );
        }
        indexed
    });
    LazyLock::force(&ASSET)
}

fn contract_strings(id: &str) -> Vec<&'static str> {
    contract_asset()
        .get(id)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing contract inventory `{id}`"))
        .iter()
        .map(|value| value.as_str().expect("validated contract string"))
        .collect()
}

fn contract_schema<'a>(schemas: &'a Map, name: &str) -> &'a Map {
    schemas
        .get(name)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{name} schema"))
}

fn contract_property<'a>(schemas: &'a Map, owner: &str, property: &str) -> &'a Map {
    contract_schema(schemas, owner)
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(property))
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{owner}.{property} schema"))
}

fn operation_parameters<'a>(operation: &'a Map, context: &str) -> &'a Vec<Value> {
    operation
        .get("parameters")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context} parameters"))
}

fn operation_parameter<'a>(operation: &'a Map, name: &str, context: &str) -> &'a Value {
    operation_parameters(operation, context)
        .iter()
        .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("{context} parameter `{name}`"))
}

fn parameter_schema<'a>(parameter: &'a Value, context: &str) -> &'a Map {
    parameter
        .get("schema")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} parameter schema"))
}

fn operation_responses<'a>(operation: &'a Map, context: &str) -> &'a Map {
    operation
        .get("responses")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} responses"))
}

fn response_content<'a>(operation: &'a Map, status: &str, context: &str) -> &'a Map {
    operation_responses(operation, context)
        .get(status)
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} HTTP {status} content"))
}

fn request_content<'a>(operation: &'a Map, context: &str) -> &'a Map {
    operation
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} request content"))
}

fn value_strings<'a>(value: &'a Value, context: &str) -> Vec<&'a str> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{context} string array"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("{context} string entry"))
        })
        .collect()
}

fn variant_property<'a>(variants: &'a [Value], index: usize, field: &str) -> &'a Map {
    variants[index]
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(field))
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("variant {index} property {field}"))
}

fn assert_description_inventory(description: &str, inventory: &str, context: &str) {
    for phrase in contract_strings(inventory) {
        assert!(description.contains(phrase), "{context} omitted `{phrase}`");
    }
}

fn assert_schema_shapes(schemas: &Map, contracts: &[SchemaShape]) {
    for contract in contracts {
        let required = contract_strings(contract.required);
        let optional = contract.optional.map(contract_strings).unwrap_or_default();
        assert_strict_object_schema(schemas, contract.name, &required, &optional);
    }
}

fn assert_property_refs(schemas: &Map, contracts: &[PropertyRefContract]) {
    for contract in contracts {
        assert_eq!(
            property_ref(schemas, contract.owner, contract.property),
            contract.expected,
            "{}.{} reference drift",
            contract.owner,
            contract.property
        );
    }
}

fn assert_operation_response_contracts(document: &Value, contracts: &[OperationResponseContract]) {
    for contract in contracts {
        let operation = openapi_operation(document, contract.path, contract.method);
        assert_eq!(
            operation_response_schema_ref(operation, contract.status, contract.path),
            contract.schema_ref,
            "{} {} HTTP {} schema",
            contract.method,
            contract.path,
            contract.status
        );
    }
}

fn assert_required_inventory(schema: &Map, inventory: &str, context: &str) {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context} required fields"));
    for field in contract_strings(inventory) {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "{context} must require {field}"
        );
    }
}

fn canonical_account_headers(required: bool) -> BTreeSet<(String, bool)> {
    canonical_account_header_requirements(required)
        .into_iter()
        .collect()
}

fn canonical_account_header_names() -> BTreeSet<&'static str> {
    [
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ]
    .into_iter()
    .collect()
}

fn canonical_account_header_requirements(required: bool) -> Vec<(String, bool)> {
    [
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), required))
    .collect()
}

fn assert_opaque_evidence_token(schema: &Map, context: &str) {
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

fn assert_nonzero_digest(schema: &Map, context: &str) {
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

fn catalog_method_name(method: CatalogHttpMethod) -> &'static str {
    match method {
        CatalogHttpMethod::Get => "get",
        CatalogHttpMethod::Post => "post",
        CatalogHttpMethod::Put => "put",
        CatalogHttpMethod::Patch => "patch",
        CatalogHttpMethod::Delete => "delete",
        CatalogHttpMethod::Any => panic!("ANY gateway cannot enter this OpenAPI contract"),
    }
}

#[test]
fn evidence_audit_openapi_requires_and_returns_exact_cursors() {
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/evidence/audit", "get");
    assert_description_inventory(
        operation
            .get("description")
            .and_then(Value::as_str)
            .expect("evidence audit description"),
        "evidence.audit.description",
        "evidence audit description",
    );
    let parameters = operation_parameters(operation, "evidence audit");
    assert_eq!(parameters.len(), 9);
    let checkpoint = operation_parameter(
        operation,
        "expected_checkpoint_digest_hex",
        "evidence audit",
    );
    assert_eq!(
        checkpoint.get("required").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        parameter_schema(checkpoint, "expected checkpoint")
            .get("pattern")
            .and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    let after_sequence = operation_parameter(operation, "after_sequence", "evidence audit");
    assert_eq!(
        parameter_schema(after_sequence, "after sequence")
            .get("minimum")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        after_sequence
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("together with after_receipt_digest_hex"))
    );
    let after_digest = operation_parameter(operation, "after_receipt_digest_hex", "evidence audit");
    let digest_schema = parameter_schema(after_digest, "after receipt digest");
    assert_eq!(
        digest_schema.get("minLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        digest_schema.get("maxLength").and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        digest_schema.get("pattern").and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    assert!(
        after_digest
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("together with after_sequence"))
    );
    let limit = operation_parameter(operation, "limit", "evidence audit");
    assert_eq!(limit.get("required").and_then(Value::as_bool), Some(true));
    assert_eq!(
        parameter_schema(limit, "audit limit")
            .get("minimum")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        parameter_schema(limit, "audit limit")
            .get("maximum")
            .and_then(Value::as_u64),
        Some(256)
    );
    let auth_headers = parameters
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(auth_headers, canonical_account_header_names());
    let success = operation_responses(operation, "evidence audit")
        .get("200")
        .and_then(Value::as_object)
        .and_then(|response| response.get("description"))
        .and_then(Value::as_str)
        .expect("evidence audit success description");
    assert_description_inventory(
        success,
        "evidence.audit.success",
        "evidence audit success response",
    );
    assert_operation_response_contracts(
        &document,
        &[
            OperationResponseContract {
                path: "/v1/evidence/audit",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/SorafsEvidenceAuditProjectionV1",
            },
            OperationResponseContract {
                path: "/v1/evidence/status",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/SorafsEvidenceAuditStatusV1",
            },
        ],
    );
    let responses = operation_responses(operation, "evidence audit");
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
    assert_eq!(
        operation_parameters(
            openapi_operation(&document, "/v1/evidence/status", "get"),
            "evidence status"
        )
        .len(),
        5
    );
    let schemas = component_schemas(&document);
    for schema in contract_strings("evidence.schemas") {
        assert!(schemas.contains_key(schema), "missing `{schema}` schema");
    }
}

#[test]
fn evidence_openapi_matches_authenticated_protocol_contract() {
    use iroha_torii_shared::route_catalog::AuthenticationPolicy;
    let document = generate_spec();
    let routes = RouteCatalog::new(CATALOGED_ROUTES)
        .project(
            CatalogProjection::OpenApi,
            crate::router::builder::compiled_route_features(),
        )
        .into_iter()
        .filter(|route| route.path().starts_with("/v1/evidence/"))
        .collect::<Vec<_>>();
    assert_eq!(
        routes.len(),
        12,
        "the evidence protocol must expose exactly twelve authenticated operations"
    );
    for route in routes {
        let method = catalog_method_name(route.method());
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{method} {} catalog authentication policy",
            route.path()
        );
        let operation = openapi_operation(&document, route.path(), method);
        let auth = operation_header_requirements(operation)
            .into_iter()
            .filter(|(name, _)| name.starts_with("X-Iroha-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            auth,
            canonical_account_headers(false),
            "{method} {} canonical authentication headers",
            route.path()
        );
        let secret: BTreeSet<&str> = match (route.path(), method) {
            ("/v1/evidence/session", "post") => BTreeSet::from(["X-SoraFS-Evidence-Challenge"]),
            ("/v1/evidence/manifest/{session_id_hex}", "get")
            | ("/v1/evidence/segment/{session_id_hex}", "get")
            | ("/v1/evidence/log/{session_id_hex}", "post") => {
                BTreeSet::from(["X-SoraFS-Evidence-Grant"])
            }
            _ => BTreeSet::new(),
        };
        let parameters = operation_parameters(operation, route.path());
        let actual_secret = parameters
            .iter()
            .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .filter(|name| name.starts_with("X-SoraFS-Evidence-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_secret,
            secret,
            "{method} {} evidence request headers",
            route.path()
        );
        for name in secret {
            let parameter = operation_parameter(operation, name, route.path());
            assert_eq!(
                parameter.get("required").and_then(Value::as_bool),
                Some(true),
                "{method} {} {name} request requirement",
                route.path()
            );
            assert_opaque_evidence_token(
                parameter_schema(parameter, name),
                &format!("{method} {} {name} request", route.path()),
            );
        }
        let (status, expected_headers): (&str, BTreeSet<&str>) = match (route.path(), method) {
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
        let response = operation_responses(operation, route.path())
            .get(status)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{method} {} {status} success response", route.path()));
        let headers = response.get("headers").and_then(Value::as_object);
        let actual = headers
            .into_iter()
            .flat_map(|headers| headers.keys())
            .map(String::as_str)
            .filter(|name| name.starts_with("X-SoraFS-Evidence-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual,
            expected_headers,
            "{method} {} evidence success response headers",
            route.path()
        );
        for name in expected_headers {
            let header = headers
                .and_then(|headers| headers.get(name))
                .and_then(Value::as_object)
                .unwrap_or_else(|| {
                    panic!("{method} {} {status} {name} response header", route.path())
                });
            assert_eq!(header.get("required").and_then(Value::as_bool), Some(true));
            let schema = header
                .get("schema")
                .and_then(Value::as_object)
                .expect("evidence response header schema");
            if matches!(
                name,
                "X-SoraFS-Evidence-Challenge" | "X-SoraFS-Evidence-Grant"
            ) {
                assert_opaque_evidence_token(schema, name);
            } else {
                assert_eq!(
                    schema.get("$ref").and_then(Value::as_str),
                    Some("#/components/schemas/SorafsEvidenceNonzeroHex32V1")
                );
            }
        }
    }
    let manifest = openapi_operation(&document, "/v1/evidence/manifest/{session_id_hex}", "get");
    let queries = operation_parameters(manifest, "evidence manifest")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .collect::<Vec<_>>();
    assert_eq!(queries.len(), 1);
    assert_eq!(
        queries[0].get("name").and_then(Value::as_str),
        Some("idempotency_key_hex")
    );
    assert_eq!(
        queries[0].get("required").and_then(Value::as_bool),
        Some(true)
    );
    assert_nonzero_digest(
        parameter_schema(queries[0], "manifest idempotency key"),
        "evidence manifest idempotency key",
    );
    let segment = openapi_operation(&document, "/v1/evidence/segment/{session_id_hex}", "get");
    let queries = operation_parameters(segment, "evidence segment")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .collect::<Vec<_>>();
    assert_eq!(
        queries
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["start", "end", "idempotency_key_hex"])
    );
    for name in ["start", "end", "idempotency_key_hex"] {
        assert_eq!(
            operation_parameter(segment, name, "evidence segment")
                .get("required")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
    for (name, minimum) in [("start", 0), ("end", 1)] {
        let schema = parameter_schema(operation_parameter(segment, name, "evidence segment"), name);
        assert_eq!(schema.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(schema.get("format").and_then(Value::as_str), Some("uint64"));
        assert_eq!(schema.get("minimum").and_then(Value::as_u64), Some(minimum));
    }
    assert!(
        operation_parameter(segment, "end", "evidence segment")
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("greater than start"))
    );
    assert_nonzero_digest(
        parameter_schema(
            operation_parameter(segment, "idempotency_key_hex", "evidence segment"),
            "segment idempotency key",
        ),
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
    let request = request_content(operation, "pin-register");
    assert_eq!(
        request.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["application/json", "application/x-norito"])
    );
    assert_eq!(
        request
            .get("application/x-norito")
            .and_then(|media| media.get("schema"))
            .and_then(|schema| schema.get("x-iroha-norito-schema"))
            .and_then(Value::as_str),
        Some("SignedTransaction")
    );
    assert_eq!(
        response_content(operation, "202", "pin-register")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/json"]
    );
    assert_description_inventory(
        operation
            .get("description")
            .and_then(Value::as_str)
            .expect("pin-register description"),
        "pin.register.description",
        "pin-register operation",
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
fn sorafs_storage_token_openapi_requires_operator_and_diagnostic_headers() {
    use iroha_torii_shared::route_catalog::AuthenticationPolicy;
    assert_eq!(
        iroha_torii_shared::route_catalog::sorafs::STORAGE_TOKEN.authentication(),
        AuthenticationPolicy::OperatorSignature
    );
    let document = generate_spec();
    let operation = openapi_operation(&document, "/v1/sorafs/storage/token", "post");
    assert_eq!(
        operation_header_requirements(operation)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ("X-Iroha-Operator-Nonce".to_owned(), true),
            ("X-Iroha-Operator-Public-Key".to_owned(), true),
            ("X-Iroha-Operator-Signature".to_owned(), true),
            ("X-Iroha-Operator-Timestamp-Ms".to_owned(), true),
            ("X-SoraFS-Client".to_owned(), true),
            ("X-SoraFS-Nonce".to_owned(), true),
        ])
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("storage-token description");
    assert!(
        description.contains("listener-wide API-token enforcement is disabled")
            && description.contains("client label is diagnostic")
    );
}

#[test]
fn sorafs_storage_and_inventory_openapi_matches_authenticated_catalog() {
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    assert!(!paths.contains_key("/v1/sorafs/storage/state"));
    assert!(!paths.contains_key("/v1/sorafs/storage/fetch"));
    let canonical = canonical_account_headers(false)
        .into_iter()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    for path in ["/v1/sorafs/aliases", "/v1/sorafs/replication"] {
        let operation = openapi_operation(&document, path, "get");
        let headers = operation_header_requirements(operation)
            .into_iter()
            .map(|(name, required)| {
                assert!(
                    !required,
                    "{path} canonical auth uses alternative proof sets"
                );
                name
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(headers, canonical, "{path} canonical auth inventory");
        assert_eq!(
            operation
                .get("security")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert!(
            operation
                .get("x-iroha-canonical-auth-v1")
                .and_then(Value::as_object)
                .is_some(),
            "{path} canonical auth contract"
        );
    }
    let aliases = openapi_operation(&document, "/v1/sorafs/aliases", "get");
    assert_eq!(
        operation_response_schema_ref(aliases, "200", "aliases"),
        "#/components/schemas/SorafsAliasListResponseV1"
    );
    let alias_queries = operation_parameters(aliases, "aliases")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        alias_queries,
        BTreeSet::from(["limit", "offset", "namespace", "manifest_digest"])
    );
    let namespace = parameter_schema(
        operation_parameter(aliases, "namespace", "aliases"),
        "alias namespace",
    );
    assert_eq!(namespace.get("minLength").and_then(Value::as_u64), Some(1));
    assert_eq!(
        namespace.get("maxLength").and_then(Value::as_u64),
        Some(128)
    );
    assert_eq!(
        namespace.get("pattern").and_then(Value::as_str),
        Some("^[a-z0-9._-]+$")
    );
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "SorafsAliasListResponseV1",
        &[
            "attestation",
            "total_count",
            "returned_count",
            "offset",
            "limit",
            "aliases",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SorafsAliasProjectionV1",
        &[
            "alias",
            "namespace",
            "name",
            "manifest_digest_hex",
            "bound_by",
            "bound_epoch",
            "expiry_epoch",
            "proof_b64",
            "cache_state",
            "status_label",
            "lineage",
            "cache_rotation_due",
            "cache_age_seconds",
            "proof_generated_at_unix",
            "proof_expires_at_unix",
            "policy_positive_ttl_secs",
            "policy_refresh_window_secs",
            "policy_hard_expiry_secs",
            "policy_rotation_max_age_secs",
            "policy_successor_grace_secs",
            "policy_governance_grace_secs",
            "cache_evaluation",
            "cache_decision",
            "cache_reasons",
        ],
        &["proof_expires_in_seconds"],
    );
    for (name, required) in [
        (
            "SorafsAliasCacheEvaluationV1",
            &[
                "decision",
                "reasons",
                "ttl_expires_at",
                "ttl_expires_at_unix",
                "serve_until",
                "serve_until_unix",
                "successor",
                "governance",
                "policy_successor_grace_secs",
                "policy_governance_grace_secs",
            ][..],
        ),
        (
            "SorafsAliasGovernanceAssessmentV1",
            &[
                "ref_ids",
                "revoked",
                "frozen",
                "rotated",
                "flags",
                "effective_at_unix",
                "effective_at",
            ],
        ),
        (
            "SorafsAliasGovernanceFlagsV1",
            &["revoked", "frozen", "rotated"],
        ),
        (
            "SorafsAliasLineageV1",
            &[
                "successor_of_hex",
                "head_hex",
                "depth_to_head",
                "is_head",
                "superseded_by",
                "immediate_successor",
                "anomalies",
            ],
        ),
        (
            "SorafsAliasLineageSuccessorV1",
            &[
                "digest_hex",
                "status",
                "approved_epoch",
                "approved_at",
                "status_timestamp_unix",
            ],
        ),
        (
            "SorafsAliasSuccessorAssessmentV1",
            &[
                "exists",
                "head_hex",
                "approved",
                "approved_at",
                "approved_at_unix",
                "depth_to_head",
                "anomalies",
            ],
        ),
    ] {
        assert_strict_object_schema(schemas, name, required, &[]);
    }
    assert_eq!(
        contract_schema(schemas, "SorafsAliasManifestStatusV1")
            .get("oneOf")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        contract_schema(schemas, "SorafsAliasCacheReasonV1")
            .get("enum")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(17)
    );
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "SorafsAliasListResponseV1",
                property: "attestation",
                expected: "#/components/schemas/SorafsRegistryAttestationV1",
            },
            PropertyRefContract {
                owner: "SorafsAliasProjectionV1",
                property: "manifest_digest_hex",
                expected: "#/components/schemas/SorafsReplicationNonzeroHex32V1",
            },
            PropertyRefContract {
                owner: "SorafsAliasProjectionV1",
                property: "lineage",
                expected: "#/components/schemas/SorafsAliasLineageV1",
            },
            PropertyRefContract {
                owner: "SorafsAliasProjectionV1",
                property: "cache_evaluation",
                expected: "#/components/schemas/SorafsAliasCacheEvaluationV1",
            },
        ],
    );
    let replication = openapi_operation(&document, "/v1/sorafs/replication", "get");
    let replication_queries = operation_parameters(replication, "replication")
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        replication_queries,
        BTreeSet::from(["limit", "offset", "status", "manifest_digest"])
    );
    for (operation, context) in [(aliases, "aliases"), (replication, "replication")] {
        let digest = parameter_schema(
            operation_parameter(operation, "manifest_digest", context),
            context,
        );
        assert_nonzero_digest(digest, context);
        for (name, allow_zero) in [("limit", false), ("offset", true)] {
            let parameter = operation_parameter(operation, name, context);
            let canonical_decimal = parameter
                .get("x-iroha-canonical-unsigned-decimal-v1")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{context} {name} canonical decimal contract"));
            assert_eq!(
                canonical_decimal.get("allow_zero").and_then(Value::as_bool),
                Some(allow_zero)
            );
            for flag in ["allow_leading_zero", "allow_percent_encoding", "allow_sign"] {
                assert_eq!(
                    canonical_decimal.get(flag).and_then(Value::as_bool),
                    Some(false),
                    "{context} {name} {flag}"
                );
            }
        }
    }
    let statuses = parameter_schema(
        operation_parameter(replication, "status", "replication"),
        "replication status",
    )
    .get("enum")
    .and_then(Value::as_array)
    .expect("replication status enum")
    .iter()
    .filter_map(Value::as_str)
    .collect::<Vec<_>>();
    assert_eq!(statuses, ["pending", "completed", "cancelled", "expired"]);
    for (path, expected) in [
        (
            "/v1/sorafs/storage/car/{manifest_id}",
            &[
                "Range",
                "Sora-Dag-Scope",
                "X-SoraFS-Chunker",
                "X-SoraFS-Nonce",
                "X-SoraFS-Stream-Token",
            ][..],
        ),
        (
            "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}",
            &["X-SoraFS-Nonce", "X-SoraFS-Stream-Token"][..],
        ),
    ] {
        let headers = operation_header_requirements(openapi_operation(&document, path, "get"))
            .into_iter()
            .collect::<BTreeSet<_>>();
        for name in expected {
            assert!(
                headers.contains(&(name.to_string(), true)),
                "{path} must require {name}"
            );
        }
    }
    let car = openapi_operation(&document, "/v1/sorafs/storage/car/{manifest_id}", "get");
    assert!(
        car.get("responses")
            .and_then(Value::as_object)
            .is_some_and(|responses| {
                !responses.contains_key("200")
                    && responses
                        .get("206")
                        .and_then(|response| response.get("content"))
                        .and_then(Value::as_object)
                        .is_some_and(|content| content.contains_key("application/vnd.ipld.car"))
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
        response_content(operation, "200", PATH)
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["application/json", "application/x-norito"])
    );
    assert_description_inventory(
        operation
            .get("description")
            .and_then(Value::as_str)
            .expect("pin-list description"),
        "pin.list.description",
        "pin-list operation",
    );
    let names = operation_parameters(operation, PATH)
        .iter()
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from([
            "after_digest_hex",
            "expected_finalized_block_hash_hex",
            "expected_finalized_height",
            "limit",
            "max_bytes",
            "status"
        ])
    );
    assert!(!names.contains("offset"));
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
            "approved_epoch",
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
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "PinManifestPageV1",
                property: "finalized_cursor",
                expected: "#/components/schemas/PinManifestFinalizedCursorV1",
            },
            PropertyRefContract {
                owner: "PinManifestPageV1",
                property: "charged_usage",
                expected: "#/components/schemas/PinResourceUsage",
            },
        ],
    );
}

#[test]
fn sorafs_pin_manifest_openapi_is_finalized_native_readback() {
    const PATH: &str = "/v1/sorafs/pin/{digest_hex}";
    let document = generate_spec();
    let operation = openapi_operation(&document, PATH, "get");
    assert_eq!(
        operation_response_schema_ref(operation, "200", PATH),
        "#/components/schemas/PinManifestFinalizedRecordV1"
    );
    assert_description_inventory(
        operation
            .get("description")
            .and_then(Value::as_str)
            .expect("pin-manifest description"),
        "pin.manifest.description",
        "pin-manifest operation",
    );
    let parameters = operation_parameters(operation, PATH);
    assert_eq!(parameters.len(), 3);
    let names = parameters
        .iter()
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from([
            "digest_hex",
            "expected_finalized_height",
            "expected_finalized_block_hash_hex"
        ])
    );
    assert!(
        !names.contains("limit"),
        "the retired projection limit must not remain in the operation"
    );
    assert_nonzero_digest(
        parameter_schema(
            operation_parameter(operation, "digest_hex", PATH),
            "manifest digest",
        ),
        "manifest digest",
    );
    let height = operation_parameter(operation, "expected_finalized_height", PATH);
    assert_eq!(height.get("in").and_then(Value::as_str), Some("query"));
    assert_eq!(height.get("required").and_then(Value::as_bool), Some(false));
    assert_eq!(
        parameter_schema(height, "expected finalized height")
            .get("minimum")
            .and_then(Value::as_u64),
        Some(1)
    );
    let block_hash = operation_parameter(operation, "expected_finalized_block_hash_hex", PATH);
    assert_eq!(block_hash.get("in").and_then(Value::as_str), Some("query"));
    assert_eq!(
        block_hash.get("required").and_then(Value::as_bool),
        Some(false)
    );
    assert_nonzero_digest(
        parameter_schema(block_hash, "expected finalized block hash"),
        "expected finalized block hash",
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
            "approved_epoch",
            "alias",
            "metadata",
            "status",
            "council_envelope_digest",
        ],
        &["successor_of", "retirement_reason", "pin_fee_payment"],
    );
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "PinManifestFinalizedRecordV1",
                property: "finalized_cursor",
                expected: "#/components/schemas/PinManifestFinalizedCursorV1",
            },
            PropertyRefContract {
                owner: "PinManifestFinalizedRecordV1",
                property: "manifest",
                expected: "#/components/schemas/PinManifestRecord",
            },
            PropertyRefContract {
                owner: "PinManifestRecord",
                property: "por_root",
                expected: "#/components/schemas/PinManifestBytes32V1",
            },
            PropertyRefContract {
                owner: "PinManifestRecord",
                property: "status",
                expected: "#/components/schemas/PinStatus",
            },
        ],
    );
    assert_eq!(
        nullable_property_ref(schemas, "PinManifestRecord", "alias"),
        "#/components/schemas/ManifestAliasBinding"
    );
    assert_eq!(
        nullable_property_ref(schemas, "PinManifestRecord", "council_envelope_digest"),
        "#/components/schemas/PinManifestBytes32V1"
    );
    let approved_epoch = contract_property(schemas, "PinManifestRecord", "approved_epoch");
    let approved_epoch_variants = approved_epoch
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("required nullable approval epoch schema");
    assert!(approved_epoch_variants.iter().any(|variant| {
        variant.get("type").and_then(Value::as_str) == Some("integer")
            && variant.get("format").and_then(Value::as_str) == Some("uint64")
    }));
    assert!(
        approved_epoch_variants
            .iter()
            .any(|variant| { variant.get("type").and_then(Value::as_str) == Some("null") })
    );
    let content_length = contract_property(schemas, "PinManifestRecord", "content_length");
    assert_eq!(
        content_length.get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        content_length.get("format").and_then(Value::as_str),
        Some("uint64")
    );
    let response_properties = component_properties(schemas, "PinManifestFinalizedRecordV1");
    for retired in contract_strings("pin.manifest.retired") {
        assert!(
            !response_properties.contains_key(retired),
            "retired pin-manifest projection field `{retired}` must remain absent"
        );
    }
    let bytes32 = contract_schema(schemas, "PinManifestBytes32V1");
    assert_eq!(bytes32.get("minItems").and_then(Value::as_u64), Some(32));
    assert_eq!(bytes32.get("maxItems").and_then(Value::as_u64), Some(32));
    let statuses = contract_schema(schemas, "PinStatus")
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("native pin status variants");
    let values = statuses
        .iter()
        .filter_map(|variant| {
            variant
                .get("properties")
                .and_then(|properties| properties.get("status"))
                .and_then(|status| status.get("const"))
                .and_then(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(values, BTreeSet::from(["Pending", "Approved", "Retired"]));
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
    assert_description_inventory(
        operation
            .get("description")
            .and_then(Value::as_str)
            .expect("replication description"),
        "replication.description",
        "replication operation",
    );
    assert_eq!(
        operation_header_requirements(operation)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        canonical_account_headers(false)
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    let names = operation_parameters(operation, PATH)
        .iter()
        .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("query"))
        .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from(["limit", "offset", "status", "manifest_digest"])
    );
    let status = parameter_schema(
        operation_parameter(operation, "status", PATH),
        "replication status",
    );
    assert_eq!(
        value_strings(
            status.get("enum").expect("replication status enum"),
            "replication statuses"
        )
        .into_iter()
        .collect::<BTreeSet<_>>(),
        BTreeSet::from(["pending", "completed", "cancelled", "expired"])
    );
    assert_eq!(
        parameter_schema(
            operation_parameter(operation, "manifest_digest", PATH),
            "replication digest"
        )
        .get("pattern")
        .and_then(Value::as_str),
        Some("^(?!0{64}$)[0-9a-f]{64}$")
    );
    let schemas = component_schemas(&document);
    for (name, required) in [
        (
            "SorafsRegistryAttestationV1",
            &["block_height", "block_hash_hex", "chain_id"][..],
        ),
        (
            "SorafsReplicationAssignmentV1",
            &["provider_id_hex", "slice_gib", "lane"],
        ),
        (
            "SorafsReplicationSlaV1",
            &[
                "ingest_deadline_secs",
                "min_availability_percent_milli",
                "min_por_success_percent_milli",
            ],
        ),
        ("SorafsReplicationMetadataEntryV1", &["key", "value"]),
        (
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
        ),
        (
            "SorafsProviderIngestCompletionAuthorityV1",
            &["provider_owner", "signer_policy"],
        ),
        (
            "SorafsProviderIngestFinalizedAnchorV1",
            &["height", "block_hash_hex"],
        ),
        (
            "SorafsReplicationCompletionV1",
            &[
                "provider_hex",
                "completed_by",
                "completion_epoch",
                "assignment_revision",
                "completion_authority",
                "finalized_anchor",
            ],
        ),
        (
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
        ),
        (
            "SorafsReplicationListResponseV1",
            &[
                "attestation",
                "total_count",
                "returned_count",
                "offset",
                "limit",
                "replication_orders",
            ],
        ),
    ] {
        assert_strict_object_schema(schemas, name, required, &[]);
    }
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "SorafsReplicationCompletionV1",
                property: "completion_authority",
                expected: "#/components/schemas/SorafsProviderIngestCompletionAuthorityV1",
            },
            PropertyRefContract {
                owner: "SorafsReplicationCompletionV1",
                property: "finalized_anchor",
                expected: "#/components/schemas/SorafsProviderIngestFinalizedAnchorV1",
            },
            PropertyRefContract {
                owner: "SorafsProviderIngestCompletionAuthorityV1",
                property: "signer_policy",
                expected: "#/components/schemas/SorafsProviderIngestSignerPolicyV1",
            },
            PropertyRefContract {
                owner: "SorafsReplicationOrderProjectionV1",
                property: "order",
                expected: "#/components/schemas/SorafsReplicationCanonicalOrderV1",
            },
            PropertyRefContract {
                owner: "SorafsReplicationOrderProjectionV1",
                property: "status",
                expected: "#/components/schemas/SorafsReplicationOrderStatusV1",
            },
        ],
    );
    let status_variants = contract_schema(schemas, "SorafsReplicationOrderStatusV1")
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("replication lifecycle variants");
    assert_eq!(status_variants.len(), 4);
    assert!(status_variants.iter().all(|variant| {
        variant.get("additionalProperties").and_then(Value::as_bool) == Some(false)
    }));
    let states = status_variants
        .iter()
        .filter_map(|variant| {
            variant
                .get("properties")
                .and_then(|properties| properties.get("state"))
                .and_then(|state| state.get("const"))
                .and_then(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        states,
        BTreeSet::from(["pending", "completed", "cancelled", "expired"])
    );
    assert_eq!(
        status_variants[0]
            .get("properties")
            .and_then(Value::as_object)
            .expect("pending status properties")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["state"])
    );
    assert!(status_variants[1..].iter().all(|variant| {
        variant
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key("epoch"))
    }));
    let policies = contract_schema(schemas, "SorafsProviderIngestSignerPolicyV1")
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("signer-policy variants");
    assert_eq!(policies.len(), 2);
    assert_eq!(
        variant_property(policies, 0, "revision")
            .get("const")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        variant_property(policies, 0, "predecessor_digest_hex")
            .get("type")
            .and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        variant_property(policies, 1, "revision")
            .get("minimum")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        variant_property(policies, 1, "predecessor_digest_hex")
            .get("$ref")
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationNonzeroHex32V1")
    );
    assert_eq!(
        contract_property(
            schemas,
            "SorafsReplicationOrderProjectionV1",
            "provider_completions"
        )
        .get("items")
        .and_then(|items| items.get("$ref"))
        .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationCompletionV1")
    );
    assert_eq!(
        contract_property(
            schemas,
            "SorafsReplicationListResponseV1",
            "replication_orders"
        )
        .get("items")
        .and_then(|items| items.get("$ref"))
        .and_then(Value::as_str),
        Some("#/components/schemas/SorafsReplicationOrderProjectionV1")
    );
    assert_eq!(
        contract_property(
            schemas,
            "SorafsReplicationOrderProjectionV1",
            "canonical_order_b64"
        )
        .get("maxLength")
        .and_then(Value::as_u64),
        Some(349_528)
    );
}

#[test]
fn moderation_dead_letter_openapi_is_typed_bounded_and_dual_control() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    for (name, required) in [
        (
            "SorafsModerationDeadLetterPrepareRequestV1",
            &["identity_hex", "kind", "action", "authorized_at_unix_ms"][..],
        ),
        (
            "SorafsModerationDeadLetterPrepareResponseV1",
            &[
                "schema",
                "status",
                "resolution_norito_b64",
                "signing_message_hex",
            ],
        ),
        (
            "SorafsModerationDeadLetterApplyRequestV1",
            &["resolution_norito_b64", "signature_hex"],
        ),
        (
            "SorafsModerationDeadLetterApplyResponseV1",
            &["schema", "status", "identity_hex", "kind", "action"],
        ),
    ] {
        assert_strict_object_schema(schemas, name, required, &[]);
    }
    assert_eq!(
        contract_schema(
            schemas,
            "SorafsModerationDeadLetterResolutionNoritoBase64V1"
        )
        .get("maxLength")
        .and_then(Value::as_u64),
        Some(
            u64::try_from(SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1)
                .expect("moderation resolution bound")
        )
    );
    assert_eq!(
        contract_schema(schemas, "SorafsModerationDeadLetterKindV1")
            .get("enum")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        contract_schema(schemas, "SorafsModerationDeadLetterResolutionActionV1")
            .get("enum")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    for (path, route, request_schema, response_schema, max_bytes) in [
        ("/v1/sorafs/moderation/dead-letters/prepare", iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_PREPARE_POST, "#/components/schemas/SorafsModerationDeadLetterPrepareRequestV1", "#/components/schemas/SorafsModerationDeadLetterPrepareResponseV1", SORAFS_MODERATION_DEAD_LETTER_PREPARE_REQUEST_MAX_BYTES_V1),
        ("/v1/sorafs/moderation/dead-letters/apply", iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_APPLY_POST, "#/components/schemas/SorafsModerationDeadLetterApplyRequestV1", "#/components/schemas/SorafsModerationDeadLetterApplyResponseV1", SORAFS_MODERATION_DEAD_LETTER_APPLY_REQUEST_MAX_BYTES_V1),
    ] {
        assert!(catalog_openapi_route_enabled(CatalogHttpMethod::Post, path));
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation.get("operationId").and_then(Value::as_str), Some(route.stable_route_id()));
        assert_eq!(operation_request_schema_ref(operation, path), request_schema);
        assert_eq!(operation_response_schema_ref(operation, "200", path), response_schema);
        assert_eq!(operation.get("x-iroha-max-request-bytes").and_then(Value::as_u64), Some(u64::try_from(max_bytes).expect("moderation request bound")));
        let headers = operation_header_requirements(operation).into_iter().map(|(name, required)| {
            assert!(!required, "{path} canonical auth uses alternative proof sets"); name
        }).collect::<BTreeSet<_>>();
        assert_eq!(headers, canonical_account_headers(false).into_iter().map(|(name, _)| name).collect::<BTreeSet<_>>());
        assert_eq!(operation.get("security").and_then(Value::as_array).map(Vec::len), Some(2));
        assert!(operation.get("description").and_then(Value::as_str).expect("moderation description").contains("independent"));
        let responses = operation_responses(operation, path);
        for status in ["200", "400", "401", "403", "404", "409", "429", "503"] {
            assert!(responses.contains_key(status), "{path} missing HTTP {status}");
        }
    }
}

#[test]
fn hedging_billing_openapi_is_authenticated_bounded_and_private() {
    let document = generate_spec();
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
            "{method} {path} catalog projection"
        );
        let operation = openapi_operation(&document, path, method);
        let headers = operation_header_requirements(operation)
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
            headers,
            canonical_account_headers(false)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<BTreeSet<_>>(),
            "{method} {path} canonical auth inventory"
        );
        for (status, response) in operation_responses(operation, path) {
            let headers = response
                .get("headers")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{method} {path} HTTP {status} private headers"));
            let constant = |name| {
                headers
                    .get(name)
                    .and_then(|header| header.get("schema"))
                    .and_then(|schema| schema.get("const"))
                    .and_then(Value::as_str)
            };
            assert_eq!(constant("Cache-Control"), Some("private, no-store"));
            assert_eq!(
                constant("Vary"),
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
        let operation = openapi_operation(&document, path, "get");
        let limit = operation_parameter(operation, "limit", path);
        assert_eq!(limit.get("required").and_then(Value::as_bool), Some(true));
        assert_eq!(
            parameter_schema(limit, "page limit")
                .get("maximum")
                .and_then(Value::as_u64),
            Some(100)
        );
        assert_eq!(
            operation_parameter(operation, "expected_checkpoint_fingerprint", path)
                .get("required")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
    let statement = response_content(
        openapi_operation(
            &document,
            "/v1/sorafs/billing/statements/{statement_id}",
            "get",
        ),
        "200",
        "statement",
    );
    assert_eq!(
        statement.keys().map(String::as_str).collect::<Vec<_>>(),
        ["application/x-norito"]
    );
    assert_eq!(
        statement
            .get("application/x-norito")
            .and_then(|media| media.get("schema"))
            .and_then(|schema| schema.get("x-iroha-norito-schema"))
            .and_then(Value::as_str),
        Some("BillingPublishedStatementV1")
    );
    let acknowledgement = request_content(
        openapi_operation(
            &document,
            "/v1/sorafs/billing/statements/{statement_id}/acknowledgements",
            "post",
        ),
        "acknowledgement",
    );
    assert_eq!(
        acknowledgement
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/x-norito"]
    );
    let schema = acknowledgement
        .get("application/x-norito")
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .expect("acknowledgement Norito schema");
    assert_eq!(
        schema.get("x-iroha-norito-schema").and_then(Value::as_str),
        Some(BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1)
    );
    assert_eq!(
        schema
            .get("x-iroha-norito-schema-hash")
            .and_then(Value::as_str),
        Some(BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_HEX_V1)
    );
    assert_eq!(
        schema.get("maxLength").and_then(Value::as_u64),
        Some(69_632)
    );
    let schemas = component_schemas(&document);
    let hedge = contract_schema(schemas, "HedgeIntentV1");
    let required = hedge
        .get("required")
        .and_then(Value::as_array)
        .expect("hedge required");
    assert!(
        required
            .iter()
            .any(|field| field.as_str() == Some("network_id"))
    );
    assert!(
        !required
            .iter()
            .any(|field| field.as_str() == Some("chain_id"))
    );
    assert_eq!(
        contract_property(schemas, "HedgeIntentV1", "network_id")
            .get("$ref")
            .and_then(Value::as_str),
        Some("#/components/schemas/NetworkId")
    );
    for (name, tag, expected) in [
        (
            "HedgingBillingRetentionScopeV1",
            "scope",
            &["active_epoch_only"][..],
        ),
        (
            "BillingStatementOwnerStatusV1",
            "status",
            &["published", "acknowledged"],
        ),
        ("HedgeIntentDirectionV1", "direction", &["sell_xor"]),
        (
            "HedgeIntentDispositionV1",
            "disposition",
            &["executable", "governed_overflow"],
        ),
    ] {
        let actual = contract_schema(schemas, name)
            .get("oneOf")
            .and_then(Value::as_array)
            .expect("tagged enum")
            .iter()
            .filter_map(|variant| {
                variant
                    .get("properties")
                    .and_then(|properties| properties.get(tag))
                    .and_then(|tag| tag.get("const"))
                    .and_then(Value::as_str)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected.iter().copied().collect::<BTreeSet<_>>());
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
    let success = response_content(operation, "200", "proof-stream");
    assert_eq!(
        success.keys().map(String::as_str).collect::<Vec<_>>(),
        ["application/x-ndjson"]
    );
    assert_eq!(
        success
            .get("application/x-ndjson")
            .and_then(|media| media.get("x-iroha-ndjson-item-schema"))
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
        "retired local PoR route"
    );
    assert!(
        !schemas.contains_key("SorafsStoragePorSampleRequestV1"),
        "retired PoR request schema"
    );
    let variants = contract_schema(schemas, "SorafsProofStreamHttpRequestV1")
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("proof request variants")
        .iter()
        .map(|variant| {
            variant
                .get("$ref")
                .and_then(Value::as_str)
                .expect("proof request ref")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        [
            "#/components/schemas/SorafsProofStreamPorRequestV1",
            "#/components/schemas/SorafsProofStreamPdpRequestV1",
            "#/components/schemas/SorafsProofStreamPotrRequestV1"
        ]
    );
    for (name, kind, required_field, allowed, forbidden) in [
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
        let schema = contract_schema(schemas, name);
        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false)
        );
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("proof request required");
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
                    .any(|field| field.as_str() == Some("orchestrator_job_id_hex"))
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
                "{name} finalized cursor field {field}"
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
                    "PoR must require {field}"
                );
            }
        } else {
            let dependencies = schema
                .get("dependentRequired")
                .and_then(Value::as_object)
                .expect("cursor dependencies");
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
                .and_then(|kind| kind.get("const"))
                .and_then(Value::as_str),
            Some(kind)
        );
        for field in allowed {
            assert!(
                properties.contains_key(*field),
                "{name} must publish {field}"
            );
        }
        for field in forbidden {
            assert!(
                !properties.contains_key(*field),
                "{name} incompatible field {field}"
            );
        }
        assert_eq!(
            properties
                .get("nonce_b64")
                .and_then(|nonce| nonce.get("pattern"))
                .and_then(Value::as_str),
            Some("^(?!A{22}==$)[A-Za-z0-9+/]{21}[AQgw]==$")
        );
        assert_eq!(
            properties
                .get("expected_finalized_height")
                .and_then(|height| height.get("minimum"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            properties
                .get("expected_finalized_block_hash_hex")
                .and_then(|hash| hash.get("pattern"))
                .and_then(Value::as_str),
            Some("^(?!0{64}$)[0-9a-f]{64}$")
        );
    }
    assert_eq!(
        contract_property(schemas, "SorafsProofStreamPorRequestV1", "sample_count")
            .get("maximum")
            .and_then(Value::as_u64),
        Some(500)
    );
    let proof = contract_schema(schemas, "SorafsPorProofV1");
    assert_eq!(
        proof.get("additionalProperties").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value_strings(proof.get("required").expect("PoR required"), "PoR required"),
        contract_strings("proof.por.required")
    );
    let properties = proof
        .get("properties")
        .and_then(Value::as_object)
        .expect("PoR proof properties");
    for field in [
        "chunk_digest_hex",
        "chunk_root_hex",
        "segment_digest_hex",
        "leaf_digest_hex",
    ] {
        assert_eq!(
            properties
                .get(field)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some("^[0-9a-f]{64}$"),
            "{field} digest pattern"
        );
    }
    let leaf = properties
        .get("leaf_bytes_hex")
        .and_then(Value::as_object)
        .expect("leaf bytes schema");
    assert_eq!(
        leaf.get("pattern").and_then(Value::as_str),
        Some("^(?:[0-9a-f]{2})+$")
    );
    assert_eq!(leaf.get("maxLength").and_then(Value::as_u64), Some(8_192));
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
            properties
                .get(field)
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(maximum),
            "{field} runtime bound"
        );
    }
    for (field, maximum) in [("segment_leaves_hex", 16), ("chunk_segments_hex", 64)] {
        let array = properties
            .get(field)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{field} schema"));
        assert_eq!(array.get("minItems").and_then(Value::as_u64), Some(1));
        assert_eq!(array.get("maxItems").and_then(Value::as_u64), Some(maximum));
        assert_eq!(
            array
                .get("items")
                .and_then(|items| items.get("pattern"))
                .and_then(Value::as_str),
            Some("^[0-9a-f]{64}$")
        );
    }
    let chunk_path = properties
        .get("chunk_merkle_path_hex")
        .and_then(Value::as_object)
        .expect("chunk path");
    assert_eq!(chunk_path.get("minItems").and_then(Value::as_u64), Some(0));
    assert_eq!(chunk_path.get("maxItems").and_then(Value::as_u64), Some(22));
    let item = contract_schema(schemas, "SorafsProofStreamItemV1");
    let item_properties = item
        .get("properties")
        .and_then(Value::as_object)
        .expect("proof item properties");
    assert_eq!(
        item_properties
            .get("proof")
            .and_then(|proof| proof.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SorafsPorProofV1")
    );
    for field in ["deadline_ms", "recorded_at_ms"] {
        assert_eq!(
            item_properties
                .get(field)
                .and_then(|schema| schema.get("minimum"))
                .and_then(Value::as_u64),
            Some(1)
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
    let validation = receipt
        .get("x-iroha-runtime-validation")
        .and_then(Value::as_object)
        .expect("receipt validation");
    assert_eq!(
        validation
            .get("requireByteIdenticalCanonicalReencode")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        validation
            .get("requireValidatedReceipt")
            .and_then(Value::as_bool),
        Some(true)
    );
    let kind_variants = item
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|all| all.get(1))
        .and_then(|constraint| constraint.get("oneOf"))
        .and_then(Value::as_array)
        .expect("proof-kind variants");
    for (kind, inventory) in [
        ("pdp", "proof.pdp.failures"),
        ("potr", "proof.potr.failures"),
    ] {
        let variant = kind_variants
            .iter()
            .find(|variant| {
                variant
                    .get("properties")
                    .and_then(|properties| properties.get("proof_kind"))
                    .and_then(|kind| kind.get("const"))
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .unwrap_or_else(|| panic!("{kind} variant"));
        let reasons = variant
            .get("properties")
            .and_then(|properties| properties.get("failure_reason"))
            .and_then(|reason| reason.get("enum"))
            .expect("failure reasons");
        assert_eq!(
            value_strings(reasons, "failure reasons"),
            contract_strings(inventory),
            "{kind} terminal failures"
        );
    }
}
