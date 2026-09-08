//! Static authority for Torii's OpenAPI description.
//!
//! The package-local document is an exact mirror of the canonical release
//! artifact. Torii parses it once with Norito JSON, removes operations disabled
//! by the compiled route catalog, and drops schemas for hard-retired surfaces.
//! This keeps every feature profile aligned with the mounted router without
//! compiling a second schema builder.

use iroha_torii_shared::route_catalog::{
    AuthenticationPolicy, CATALOGED_ROUTES, CatalogProjection, EnabledFeatures,
    HttpMethod as CatalogHttpMethod, RouteCatalog, RouteDescriptor,
};
use norito::json::{Map, Value};
use std::{collections::BTreeMap, sync::LazyLock};
/// OpenAPI operation extension consumed by the MCP policy bridge.
pub(crate) const TOOL_EFFECT_EXTENSION: &str = "x-iroha-tool-effect";
/// OpenAPI operation extension carrying the catalog's versioned authentication contract.
pub(crate) const ROUTE_AUTH_EXTENSION: &str = "x-iroha-route-auth";
/// Package-local source authority for Torii's OpenAPI contract.
const CANONICAL_OPENAPI_JSON: &str = include_str!("../assets/openapi/torii.json");
static COMPILED_OPENAPI_SPEC: LazyLock<Value> = LazyLock::new(|| {
    let mut document: Value = norito::json::from_str(CANONICAL_OPENAPI_JSON)
        .expect("package-local Torii OpenAPI authority must be valid Norito JSON");
    ensure_catalog_security_schemes(&mut document);
    {
        let paths = document
            .as_object_mut()
            .and_then(|document| document.get_mut("paths"))
            .and_then(Value::as_object_mut)
            .expect("package-local Torii OpenAPI authority must contain a paths object");
        retain_catalog_openapi_operations(paths, crate::router::builder::compiled_route_features());
    }
    install_kagemusha_v1_contract(&mut document);
    remove_hard_retired_schemas(&mut document);
    document
});
static COMPILED_OPENAPI_JSON: LazyLock<String> = LazyLock::new(|| {
    norito::json::to_string_pretty(compiled_spec())
        .expect("compiled Torii OpenAPI authority must serialize as Norito JSON")
});
fn retain_catalog_openapi_operations(paths: &mut Map, enabled_features: EnabledFeatures<'_>) {
    const OPERATION_METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];
    let projected =
        RouteCatalog::new(CATALOGED_ROUTES).project(CatalogProjection::OpenApi, enabled_features);
    let enabled: BTreeMap<(String, &'static str), &RouteDescriptor> = projected
        .iter()
        .filter_map(|route| {
            let method = match route.method() {
                CatalogHttpMethod::Get => "get",
                CatalogHttpMethod::Post => "post",
                CatalogHttpMethod::Put => "put",
                CatalogHttpMethod::Patch => "patch",
                CatalogHttpMethod::Delete => "delete",
                CatalogHttpMethod::Any => return None,
            };
            Some(((route.path().replace("{*", "{"), method), *route))
        })
        .collect();
    for (path, path_item) in paths.iter_mut() {
        let Some(methods) = path_item.as_object_mut() else {
            continue;
        };
        for method in OPERATION_METHODS {
            let Some(descriptor) = enabled.get(&(path.clone(), method)) else {
                methods.remove(method);
                continue;
            };
            let Some(operation) = methods.get_mut(method).and_then(Value::as_object_mut) else {
                continue;
            };
            operation.insert(
                ROUTE_AUTH_EXTENSION.to_owned(),
                route_auth_metadata(**descriptor),
            );
            apply_catalog_operation_contract(operation, **descriptor);
            if !descriptor.requires_private_no_store() {
                continue;
            }
            let Some(responses) = operation
                .get_mut("responses")
                .and_then(Value::as_object_mut)
            else {
                continue;
            };
            for response in responses.values_mut() {
                let Some(response) = response.as_object_mut() else {
                    continue;
                };
                let headers = response
                    .entry("headers".to_owned())
                    .or_insert_with(|| Value::Object(Map::new()))
                    .as_object_mut()
                    .expect("OpenAPI response headers must be an object");
                headers.insert(
                    "Cache-Control".to_owned(),
                    norito::json!({
                        "description": "Authenticated responses which must never be retained.",
                        "required": true,
                        "schema": {
                            "const": "private, no-store",
                            "type": "string"
                        }
                    }),
                );
            }
        }
    }
    paths.retain(|_, path_item| {
        path_item.as_object().is_some_and(|methods| {
            OPERATION_METHODS
                .iter()
                .any(|method| methods.contains_key(*method))
        })
    });
}
fn route_auth_metadata(descriptor: RouteDescriptor) -> Value {
    norito::json!({
        "schemaVersion": (descriptor.auth_metadata_schema_version()),
        "stableRouteId": (descriptor.stable_route_id()),
        "authentication": (descriptor.authentication().as_str()),
        "admission": (descriptor.admission().as_str())
    })
}
fn apply_catalog_operation_contract(operation: &mut Map, descriptor: RouteDescriptor) {
    if let Some(security) = standard_security_requirements(descriptor.authentication()) {
        operation.insert("security".to_owned(), security);
    }
    if descriptor.method() == CatalogHttpMethod::Post
        && descriptor.stable_route_id().starts_with("iso20022.")
    {
        operation.insert(
            "requestBody".to_owned(),
            norito::json!({
                "content": {
                    "application/xml": {
                        "schema": {
                            "$ref": "#/components/schemas/XmlText"
                        }
                    }
                },
                "required": true
            }),
        );
    }
}
fn standard_security_requirements(authentication: AuthenticationPolicy) -> Option<Value> {
    let canonical_single_signature = norito::json!({
        "IrohaCanonicalAccount": [],
        "IrohaCanonicalNonce": [],
        "IrohaCanonicalSignature": [],
        "IrohaCanonicalTimestampMs": []
    });
    let canonical_witness = norito::json!({ "IrohaCanonicalWitness": [] });
    let operator_signature = norito::json!({
        "IrohaOperatorPublicKey": [],
        "IrohaOperatorTimestampMs": [],
        "IrohaOperatorNonce": [],
        "IrohaOperatorSignature": []
    });
    match authentication {
        AuthenticationPolicy::ToriiDefault => Some(norito::json!([
            {},
            { "IrohaApiToken": [] }
        ])),
        AuthenticationPolicy::OnboardingToken => {
            Some(norito::json!([{ "IrohaOnboardingToken": [] }]))
        }
        AuthenticationPolicy::CanonicalAccountSignature => Some(Value::Array(vec![
            canonical_single_signature,
            canonical_witness,
        ])),
        AuthenticationPolicy::OptionalCanonicalAccountSignature
        | AuthenticationPolicy::ManifestConditionalContent => Some(Value::Array(vec![
            Value::Object(Map::new()),
            canonical_single_signature,
            canonical_witness,
        ])),
        AuthenticationPolicy::OperatorSignature => Some(Value::Array(vec![operator_signature])),
        AuthenticationPolicy::Unauthenticated => Some(Value::Array(Vec::new())),
        AuthenticationPolicy::CanonicalSignedBody
        | AuthenticationPolicy::IdentityBoundSignature
        | AuthenticationPolicy::OperatorCredentialExchange
        | AuthenticationPolicy::ProtocolHandshake
        | AuthenticationPolicy::NestedRouteAuthentication => None,
    }
}
fn ensure_catalog_security_schemes(document: &mut Value) {
    let security_schemes = document
        .as_object_mut()
        .and_then(|document| document.get_mut("components"))
        .and_then(Value::as_object_mut)
        .and_then(|components| components.get_mut("securitySchemes"))
        .and_then(Value::as_object_mut)
        .expect("package-local Torii OpenAPI authority must contain component security schemes");
    for (name, header_name, description) in [
        (
            "IrohaApiToken",
            "X-API-Token",
            "Deployment-configured Torii API token. Whether it is required is selected by node configuration.",
        ),
        (
            "IrohaOnboardingToken",
            "X-Iroha-Onboarding-Token",
            "Dedicated single-use onboarding token.",
        ),
        (
            "IrohaOperatorPublicKey",
            "X-Iroha-Operator-Public-Key",
            "Allow-listed exact-network operator public key bound into the request signature.",
        ),
        (
            "IrohaOperatorTimestampMs",
            "X-Iroha-Operator-Timestamp-Ms",
            "Fresh Unix timestamp in milliseconds bound into the operator request signature.",
        ),
        (
            "IrohaOperatorNonce",
            "X-Iroha-Operator-Nonce",
            "Fresh nonce bound into the operator request signature.",
        ),
        (
            "IrohaOperatorSignature",
            "X-Iroha-Operator-Signature",
            "Canonical operator signature over the exact request.",
        ),
    ] {
        security_schemes.insert(
            name.to_owned(),
            norito::json!({
                "type": "apiKey",
                "in": "header",
                "name": (header_name),
                "description": (description)
            }),
        );
    }
}

/// Replace the pre-release KAGEMUSHA API projection with the sole aggregate-balance V1 contract.
///
/// The checked-in document is shared with release tooling, so this closed rewrite happens before
/// feature projection is exposed by a running node. It intentionally removes every lineage,
/// note-inventory, anchor-drawdown, hop-count, and compatibility component instead of publishing
/// aliases for them.
fn install_kagemusha_v1_contract(document: &mut Value) {
    {
        let paths = document
            .as_object_mut()
            .and_then(|document| document.get_mut("paths"))
            .and_then(Value::as_object_mut)
            .expect("package-local Torii OpenAPI authority must contain a paths object");

        let readiness = kagemusha_operation_mut(paths, "/v1/kagemusha/readiness", "get");
        readiness.insert("operationId".to_owned(), Value::from("kagemushaReadiness"));
        readiness.insert("tags".to_owned(), norito::json!(["KAGEMUSHA"]));
        readiness.insert(
            "description".to_owned(),
            Value::from(
                "Report the sole KAGEMUSHA V1 aggregate-balance capability. The protocol has no hop, ancestry, origin, input, fan-in, or proof-depth admission limit.",
            ),
        );
        set_kagemusha_response_schema(
            readiness,
            "200",
            "KagemushaReadinessV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_READINESS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_READINESS_MAX_BYTES_V1,
        );

        let top_up = kagemusha_operation_mut(paths, "/v1/kagemusha/top-up", "post");
        top_up.insert("operationId".to_owned(), Value::from("kagemushaTopUp"));
        top_up.insert("tags".to_owned(), norito::json!(["KAGEMUSHA"]));
        set_kagemusha_idempotency_key_parameter(top_up);
        top_up.insert(
            "description".to_owned(),
            Value::from(
                concat!(
                    "Submit one canonical versioned payer-signed `SignedTransaction` containing exactly one native `iroha.kagemusha.v1.top_up` instruction. ",
                    "The transaction must target this network, bind `QueuePlanSynced` admission, and name the embedded payer as its authority. ",
                    "Torii verifies and queues the original transaction unchanged; it never rebuilds or signs it. ",
                    "The HTTP body uses the configured `torii.max_content_len` transaction-ingress limit: KAGEMUSHA-enabled nodes require at least 32 KiB, and the first-release Torii protocol permits at most 64,000,000 bytes. ",
                    "The embedded top-up request is limited to 16 KiB.",
                ),
            ),
        );
        set_kagemusha_norito_request(
            top_up,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_SCHEMA_NAME_V1,
            None,
        );
        set_kagemusha_response_schema(
            top_up,
            "202",
            "KagemushaOperationStatusV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        );
        install_kagemusha_terminal_replay_response(top_up);
        set_kagemusha_operation_location_header(top_up);

        let redeem = kagemusha_operation_mut(paths, "/v1/kagemusha/redeem", "post");
        redeem.insert("operationId".to_owned(), Value::from("kagemushaRedeem"));
        redeem.insert("tags".to_owned(), norito::json!(["KAGEMUSHA"]));
        set_kagemusha_idempotency_key_parameter(redeem);
        redeem.insert(
            "description".to_owned(),
            Value::from(
                "Verify one full or partial aggregate-balance redemption voucher, consume its terminal nullifier, debit the pooled reserve, and credit the beneficiary atomically.",
            ),
        );
        set_kagemusha_norito_request(
            redeem,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
            Some(iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1),
        );
        set_kagemusha_response_schema(
            redeem,
            "202",
            "KagemushaOperationStatusV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        );
        install_kagemusha_terminal_replay_response(redeem);
        set_kagemusha_operation_location_header(redeem);

        let status =
            kagemusha_operation_mut(paths, "/v1/kagemusha/operations/{operation_id}", "get");
        status.insert(
            "operationId".to_owned(),
            Value::from("kagemushaOperationStatus"),
        );
        status.insert("tags".to_owned(), norito::json!(["KAGEMUSHA"]));
        set_kagemusha_operation_id_path_parameter(status);
        status.insert(
            "description".to_owned(),
            Value::from(
                "Return one idempotent KAGEMUSHA V1 reserve operation. Applied results carry consensus finality and an exact ordinary-write receipt witness; clients authenticate them against an independently pinned context.",
            ),
        );
        set_kagemusha_response_schema(
            status,
            "200",
            "KagemushaOperationStatusV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        );
    }

    let tags = document
        .as_object_mut()
        .and_then(|document| document.get_mut("tags"))
        .and_then(Value::as_array_mut)
        .expect("package-local Torii OpenAPI authority must contain top-level tags");
    let mut kagemusha_tag_count = 0_usize;
    for tag in tags.iter_mut().filter_map(Value::as_object_mut) {
        let is_kagemusha = tag
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("KAGEMUSHA"));
        if is_kagemusha {
            kagemusha_tag_count += 1;
            tag.insert("name".to_owned(), Value::from("KAGEMUSHA"));
        }
    }
    assert_eq!(
        kagemusha_tag_count, 1,
        "package-local Torii OpenAPI authority must declare exactly one KAGEMUSHA tag"
    );

    let schemas = document
        .as_object_mut()
        .and_then(|document| document.get_mut("components"))
        .and_then(Value::as_object_mut)
        .and_then(|components| components.get_mut("schemas"))
        .and_then(Value::as_object_mut)
        .expect("package-local Torii OpenAPI authority must contain component schemas");
    schemas.insert(
        "KagemushaBytes32V1".to_owned(),
        norito::json!({
            "description": "Exactly 32 unsigned bytes.",
            "type": "array",
            "minItems": 32,
            "maxItems": 32,
            "items": { "type": "integer", "minimum": 0, "maximum": 255 }
        }),
    );
    schemas.insert(
        "KagemushaReadinessV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["kagemusha_handoff_capability", "wire_version", "device_lifecycle_version", "ready"],
            "properties": {
                "kagemusha_handoff_capability": { "type": "string", "const": "kagemusha_handoff_v1" },
                "wire_version": { "type": "integer", "const": 1 },
                "device_lifecycle_version": { "type": "integer", "const": 1 },
                "ready": { "type": "boolean" }
            }
        }),
    );
    schemas.insert(
        "KagemushaOperationKindV1".to_owned(),
        norito::json!({ "type": "string", "enum": ["top_up", "redemption"] }),
    );
    schemas.insert(
        "KagemushaOperationStateV1".to_owned(),
        norito::json!({ "type": "string", "enum": ["pending", "applied", "rejected"] }),
    );
    schemas.insert(
        "KagemushaOperationRejectionCodeV1".to_owned(),
        norito::json!({
            "type": "string",
            "enum": [
                "invalid_request", "unauthorized", "insufficient_online_balance",
                "invalid_proof", "hardware_policy_rejected", "identity_conflict",
                "reserve_underflow", "arithmetic_overflow", "internal_failure"
            ]
        }),
    );
    schemas.insert(
        "KagemushaOperationRejectionV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["code", "detail_digest"],
            "properties": {
                "code": { "$ref": "#/components/schemas/KagemushaOperationRejectionCodeV1" },
                "detail_digest": { "$ref": "#/components/schemas/KagemushaBytes32V1" }
            }
        }),
    );
    schemas.insert(
        "KagemushaOperationResultV1".to_owned(),
        norito::json!({
            "description": "A constant-shape top-up or redemption result carrying its exact request, pooled-reserve receipt, consensus finality, and—only for top-up—the byte-identical mint credit.",
            "type": "object",
            "additionalProperties": false,
            "required": ["kind", "result"],
            "properties": {
                "kind": { "$ref": "#/components/schemas/KagemushaOperationKindV1" },
                "result": {
                    "description": "The canonical typed KagemushaTopUpResultV1 or KagemushaRedemptionResultV1 value.",
                    "type": "string",
                    "format": "byte"
                }
            }
        }),
    );
    schemas.insert(
        "KagemushaOperationStatusV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["version", "operation_id", "kind", "state", "result", "rejection"],
            "properties": {
                "version": { "type": "integer", "const": 1 },
                "operation_id": { "$ref": "#/components/schemas/KagemushaBytes32V1" },
                "kind": { "$ref": "#/components/schemas/KagemushaOperationKindV1" },
                "state": { "$ref": "#/components/schemas/KagemushaOperationStateV1" },
                "result": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/KagemushaOperationResultV1" },
                        { "type": "null" }
                    ]
                },
                "rejection": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/KagemushaOperationRejectionV1" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
}

const KAGEMUSHA_NONZERO_OPERATION_ID_PATTERN_V1: &str = "^(?!0{64}$)[0-9a-f]{64}$";
const KAGEMUSHA_OPERATION_LOCATION_PATTERN_V1: &str =
    "^/v1/kagemusha/operations/(?!0{64}$)[0-9a-f]{64}$";

fn set_kagemusha_idempotency_key_parameter(operation: &mut Map) {
    operation.insert(
        "parameters".to_owned(),
        norito::json!([{
            "description": "Exact lowercase hexadecimal form of the request's nonzero 32-byte operation ID. Reusing an ID with the same canonical operation payload is idempotent; binding it to any different payload conflicts.",
            "in": "header",
            "name": "Idempotency-Key",
            "required": true,
            "schema": {
                "type": "string",
                "minLength": 64,
                "maxLength": 64,
                "pattern": KAGEMUSHA_NONZERO_OPERATION_ID_PATTERN_V1
            }
        }]),
    );
}

fn set_kagemusha_operation_id_path_parameter(operation: &mut Map) {
    operation.insert(
        "parameters".to_owned(),
        norito::json!([{
            "description": "Exact lowercase hexadecimal form of one nonzero 32-byte KAGEMUSHA V1 operation ID.",
            "in": "path",
            "name": "operation_id",
            "required": true,
            "schema": {
                "type": "string",
                "minLength": 64,
                "maxLength": 64,
                "pattern": KAGEMUSHA_NONZERO_OPERATION_ID_PATTERN_V1
            }
        }]),
    );
}

fn set_kagemusha_operation_location_header(operation: &mut Map) {
    let responses = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .expect("KAGEMUSHA V1 operation must expose responses");
    for status in ["200", "202"] {
        let schema = responses
            .get_mut(status)
            .and_then(Value::as_object_mut)
            .and_then(|response| response.get_mut("headers"))
            .and_then(Value::as_object_mut)
            .and_then(|headers| headers.get_mut("Location"))
            .and_then(Value::as_object_mut)
            .and_then(|location| location.get_mut("schema"))
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| {
                panic!("KAGEMUSHA V1 submission response {status} must expose a Location schema")
            });
        schema.insert(
            "pattern".to_owned(),
            Value::from(KAGEMUSHA_OPERATION_LOCATION_PATTERN_V1),
        );
    }
}

fn install_kagemusha_terminal_replay_response(operation: &mut Map) {
    let responses = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .expect("KAGEMUSHA V1 operation must expose responses");
    let mut response = responses
        .get("202")
        .cloned()
        .expect("KAGEMUSHA V1 submission must expose an accepted response");
    let response_object = response
        .as_object_mut()
        .expect("KAGEMUSHA V1 accepted response must be an object");
    response_object.insert(
        "description".to_owned(),
        Value::from("An exact replay resolved to the operation's terminal status."),
    );
    response_object
        .get_mut("headers")
        .and_then(Value::as_object_mut)
        .expect("KAGEMUSHA V1 accepted response must expose headers")
        .remove("Retry-After");
    responses.insert("200".to_owned(), response);
}

fn kagemusha_operation_mut<'a>(paths: &'a mut Map, path: &str, method: &str) -> &'a mut Map {
    paths
        .get_mut(path)
        .and_then(Value::as_object_mut)
        .and_then(|path_item| path_item.get_mut(method))
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("cataloged KAGEMUSHA V1 operation {method} {path} is missing"))
}

fn set_kagemusha_norito_request(
    operation: &mut Map,
    schema_name: &str,
    maximum_bytes: Option<usize>,
) {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::from("string"));
    schema.insert("format".to_owned(), Value::from("binary"));
    schema.insert("x-iroha-norito-schema".to_owned(), Value::from(schema_name));
    if let Some(maximum_bytes) = maximum_bytes {
        schema.insert(
            "x-iroha-max-bytes".to_owned(),
            Value::from(maximum_bytes as u64),
        );
    }
    operation.insert(
        "requestBody".to_owned(),
        norito::json!({
            "required": true,
            "content": {
                "application/x-norito": {
                    "schema": (Value::Object(schema))
                }
            }
        }),
    );
}

fn set_kagemusha_response_schema(
    operation: &mut Map,
    status: &str,
    component: &str,
    maximum_norito_bytes: usize,
    maximum_json_bytes: usize,
) {
    let response = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut(status))
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("KAGEMUSHA V1 response {status} is missing"));
    let content = response
        .entry("content".to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("KAGEMUSHA V1 response content must be an object");
    for (media_type, maximum_bytes) in [
        ("application/json", maximum_json_bytes),
        ("application/x-norito", maximum_norito_bytes),
    ] {
        content.insert(
            media_type.to_owned(),
            norito::json!({
                "schema": {
                    "$ref": (format!("#/components/schemas/{component}")),
                    "x-iroha-max-bytes": (maximum_bytes as u64)
                }
            }),
        );
    }
}
fn remove_hard_retired_schemas(document: &mut Value) {
    let schemas = document
        .as_object_mut()
        .and_then(|document| document.get_mut("components"))
        .and_then(Value::as_object_mut)
        .and_then(|components| components.get_mut("schemas"))
        .and_then(Value::as_object_mut)
        .expect("package-local Torii OpenAPI authority must contain component schemas");
    for schema in [
        "GovernanceEnactRequestV1",
        "GovernanceFinalizeRequestV1",
        "UploadedModelEncryptionRecipientResponse",
        "PrivateUploadedModelExecuteRequest",
        "PrivateUploadedModelExecuteResponse",
        "PrivateUploadedModelQuantizedCpuModelDto",
        "PrivateUploadedModelReceiptListResponse",
        "SoraPrivateModelArtifactRefV1",
        "SoraPrivateUploadedModelExecutionReceiptV1",
        "SoraUploadedModelKeyEncapsulationV1",
        "SoraUploadedModelKeyWrapAeadV1",
        "SoraUploadedModelEncryptionRecipientV1",
        "SoraUploadedModelWrappedKeyV1",
        "SoraUploadedModelRuntimeFormatV1",
        "ModelHostAdvertisePayload",
        "SignedModelHostAdvertiseRequest",
        "ModelHostHeartbeatPayload",
        "SignedModelHostHeartbeatRequest",
        "ModelHostWithdrawPayload",
        "SignedModelHostWithdrawRequest",
        "ModelHostStatusResponse",
        "SoraModelHostCapabilityRecordV1",
        "SoraHfPlacementStatusV1",
        "SoraHfPlacementHostRoleV1",
        "SoraHfPlacementHostStatusV1",
        "SoraHfPlacementHostAssignmentV1",
        "SoraHfPlacementRecordV1",
        "SoraModelHostViolationKindV1",
        "SoraModelHostViolationEvidenceRecordV1",
        "KagemushaReadiness",
        "KagemushaReadinessBlocker",
        "KagemushaActiveTransferVerifier",
        "KagemushaActiveTopUpShieldVerifier",
        "KagemushaAuthenticatedArtifactSet",
    ] {
        schemas.remove(schema);
    }
}
/// Borrow the feature-pruned OpenAPI document cached for this binary.
#[must_use]
pub(crate) fn compiled_spec() -> &'static Value {
    LazyLock::force(&COMPILED_OPENAPI_SPEC)
}
/// Borrow the catalog-pruned JSON response cached for this binary.
#[must_use]
pub(crate) fn compiled_spec_json() -> &'static str {
    LazyLock::force(&COMPILED_OPENAPI_JSON).as_str()
}
/// Return an owned copy of the feature-pruned OpenAPI document.
#[must_use]
pub fn generate_spec() -> Value {
    compiled_spec().clone()
}
#[cfg(test)]
mod tests;
