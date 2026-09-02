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

/// Replace the pre-release offline API projection with the sole aggregate-balance V1 contract.
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
        readiness.insert(
            "description".to_owned(),
            Value::from(
                "Report the sole Kagemusha V1 aggregate-balance capability. The protocol has no hop, ancestry, origin, input, fan-in, or proof-depth admission limit.",
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
        top_up.insert(
            "description".to_owned(),
            Value::from(
                "Atomically debit online funds, increase the asset's pooled Kagemusha V1 reserve, and create one device-bound aggregate mint credit.",
            ),
        );
        set_kagemusha_norito_request(
            top_up,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
        );
        set_kagemusha_response_schema(
            top_up,
            "202",
            "KagemushaOperationStatusV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        );

        let redeem = kagemusha_operation_mut(paths, "/v1/kagemusha/redeem", "post");
        redeem.insert(
            "description".to_owned(),
            Value::from(
                "Verify one full or partial aggregate-balance redemption voucher, consume its terminal nullifier, debit the pooled reserve, and credit the beneficiary atomically.",
            ),
        );
        set_kagemusha_norito_request(
            redeem,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
        );
        set_kagemusha_response_schema(
            redeem,
            "202",
            "KagemushaOperationStatusV1",
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        );

        let status = kagemusha_operation_mut(paths, "/v1/kagemusha/operations/{operation_id}", "get");
        status.insert(
            "description".to_owned(),
            Value::from(
                "Return one idempotent Kagemusha V1 reserve operation. Applied results carry consensus finality and an exact ordinary-write receipt witness; clients authenticate them against an independently pinned context.",
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

fn kagemusha_operation_mut<'a>(paths: &'a mut Map, path: &str, method: &str) -> &'a mut Map {
    paths
        .get_mut(path)
        .and_then(Value::as_object_mut)
        .and_then(|path_item| path_item.get_mut(method))
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("cataloged Kagemusha V1 operation {method} {path} is missing"))
}

fn set_kagemusha_norito_request(operation: &mut Map, schema_name: &str, maximum_bytes: usize) {
    operation.insert(
        "requestBody".to_owned(),
        norito::json!({
            "required": true,
            "content": {
                "application/x-norito": {
                    "schema": {
                        "type": "string",
                        "format": "binary",
                        "x-iroha-norito-schema": (schema_name),
                        "x-iroha-max-bytes": (maximum_bytes as u64)
                    }
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
        .unwrap_or_else(|| panic!("Kagemusha V1 response {status} is missing"));
    let content = response
        .entry("content".to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("Kagemusha V1 response content must be an object");
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
mod tests {
    use super::*;
    use crate::utils;
    use iroha_torii_shared::{
        route_catalog::{ApiSurface, musubi as musubi_routes},
        sorafs_hedging_billing_api::{
            BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_HEX_V1,
            BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1,
        },
        sorafs_moderation_api::{
            SORAFS_MODERATION_DEAD_LETTER_APPLY_REQUEST_MAX_BYTES_V1,
            SORAFS_MODERATION_DEAD_LETTER_PREPARE_REQUEST_MAX_BYTES_V1,
            SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1,
        },
        uri,
    };
    use sorafs_node::evidence_viewer::EVIDENCE_VIEWER_MAX_OPAQUE_TOKEN_BYTES_V1;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    const GOVERNANCE_HASH_LITERAL_PATTERN: &str =
        "^(?:[bB][lL][aA][kK][eE]2[bB]32:)?(?:0[xX])?[0-9a-fA-F]{64}$";
    const GOVERNANCE_LOWER_HEX32_PATTERN: &str = "^[0-9a-f]{64}$";
    const GOVERNANCE_EXACT_TOKEN_PATTERN: &str = r"^[^\s\u0000-\u001F\u007F-\u009F]+$";
    const GOVERNANCE_SELECTOR_V1_PATTERN: &str =
        iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_PATTERN;
    const GOVERNANCE_U64_DECIMAL_PATTERN: &str = concat!(
        "^(?:0|[1-9][0-9]{0,18}|",
        "1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|",
        "1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|",
        "1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|",
        "1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|",
        "1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|",
        "184467440737095[0-4][0-9]{4}|18446744073709550[0-9]{3}|",
        "18446744073709551[0-5][0-9]{2}|1844674407370955160[0-9]|",
        "1844674407370955161[0-4]|18446744073709551615)$"
    );
    const KAGEMUSHA_COMMAND_COMMON_BAD_REQUEST_REJECT_CODES: &[&str] = &[
        "idempotency_key_invalid",
        "idempotency_key_missing",
        "operation_id_invalid",
        "kagemusha_amount_exceeds_limit",
        "kagemusha_asset_not_found",
        "kagemusha_asset_scale_invalid",
        "kagemusha_asset_scale_mismatch",
        "kagemusha_authorization_invalid",
        "kagemusha_hardware_authorization_invalid",
        "kagemusha_wrong_network",
    ];
    const KAGEMUSHA_TOP_UP_BAD_REQUEST_REJECT_CODES: &[&str] = &[
        "kagemusha_top_up_invalid",
        "kagemusha_confidential_state_unavailable",
        "kagemusha_topup_shield_verifier_unavailable",
        "kagemusha_topup_shield_verifier_mismatch",
        "kagemusha_confidential_state_invalid",
        "kagemusha_topup_tree_full",
        "kagemusha_topup_state_conflict",
        "kagemusha_topup_snapshot_stale",
    ];
    const KAGEMUSHA_REDEEM_BAD_REQUEST_REJECT_CODES: &[&str] = &["kagemusha_redeem_invalid"];
    const TRANSACTION_ACCEPTANCE_BAD_REQUEST_REJECT_CODES: &[&str] = &[
        "transaction_rejected",
        "PRTRY:NTS_UNHEALTHY",
        "PRTRY:TX_UNSUPPORTED_AUTHORITY",
        "PRTRY:TX_SIGNATURE_ALGO_DENIED",
        "PRTRY:TX_SIGNATURE_INVALID",
        "PRTRY:TX_SIGNATURE_MALFORMED",
        "PRTRY:TX_SIGNATURE_MISSING",
        "PRTRY:TX_SIGNATURE_UNKNOWN_SIGNER",
        "PRTRY:TX_SIGNATURE_INSUFFICIENT",
        "ED07",
        "PRTRY:KAGEMUSHA_V1_OPERATION_CARRIER_REJECTED",
        "PRTRY:ROUTE_UNRESOLVED",
    ];
    const TRANSACTION_SUBMISSION_FORBIDDEN_REJECT_CODES: &[&str] = &[
        "PRTRY:QUEUE_GOVERNANCE_REJECTED",
        "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
        "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
        "PRTRY:NEXUS_FEE_ADMISSION_REJECTED",
    ];
    const TRANSACTION_SUBMISSION_CONFLICT_REJECT_CODES: &[&str] = &[
        "PRTRY:ALREADY_COMMITTED",
        "PRTRY:ALREADY_ENQUEUED",
        "PRTRY:KAGEMUSHA_V1_OPERATION_ID_CONFLICT",
    ];
    const TRANSACTION_SUBMISSION_RATE_LIMIT_REJECT_CODES: &[&str] = &[
        "PRTRY:QUEUE_FULL",
        "PRTRY:QUEUE_LATENCY",
        "PRTRY:QUEUE_RATE",
    ];
    const TRANSACTION_SUBMISSION_UNAVAILABLE_REJECT_CODES: &[&str] = &[
        "transaction_admission_worker_failed",
        "route_unavailable",
        "PRTRY:QUEUE_PLAN_JOURNAL_UNAVAILABLE",
        "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN",
        "PRTRY:KAGEMUSHA_V1_OPERATION_INDEX_INCONSISTENT",
    ];
    const KAGEMUSHA_COMMAND_FORBIDDEN_REJECT_CODES: &[&str] = &[
        "kagemusha_auth_header_unsupported",
        "PRTRY:QUEUE_GOVERNANCE_REJECTED",
        "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
        "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
        "PRTRY:NEXUS_FEE_ADMISSION_REJECTED",
    ];
    const KAGEMUSHA_COMMAND_CONFLICT_REJECT_CODES: &[&str] = &[
        "idempotency_key_conflict",
        "operation_id_conflict",
        "kagemusha_operation_retry_exhausted",
        "PRTRY:ALREADY_COMMITTED",
        "PRTRY:ALREADY_ENQUEUED",
        "PRTRY:KAGEMUSHA_V1_OPERATION_ID_CONFLICT",
    ];
    const KAGEMUSHA_COMMAND_RATE_LIMIT_REJECT_CODES: &[&str] = &[
        "PRTRY:QUEUE_FULL",
        "PRTRY:QUEUE_LATENCY",
        "PRTRY:QUEUE_RATE",
    ];
    const KAGEMUSHA_COMMAND_UNAVAILABLE_REJECT_CODES: &[&str] = &[
        "kagemusha_service_unavailable",
        "kagemusha_not_ready",
        "kagemusha_command_authority_not_ready",
        "kagemusha_command_fee_asset_not_ready",
        "kagemusha_command_authority_unfunded",
        "kagemusha_command_body_admission_saturated",
        "kagemusha_command_memory_admission_saturated",
        "kagemusha_command_admission_configuration_invalid",
        "kagemusha_operation_capacity_exhausted",
        "kagemusha_operation_admission_inconsistent",
        "kagemusha_operation_pending_unavailable",
        "kagemusha_operation_history_unavailable",
        "kagemusha_operation_evidence_inconsistent",
        "kagemusha_recursive_release_invalid",
        "kagemusha_recursive_release_outside_issuance_window",
    ];
    const KAGEMUSHA_OPERATION_STATUS_UNAVAILABLE_REJECT_CODES: &[&str] = &[
        "kagemusha_service_unavailable",
        "kagemusha_operation_pending_unavailable",
        "kagemusha_operation_history_unavailable",
        "kagemusha_operation_evidence_inconsistent",
        "kagemusha_topup_finality_proof_unavailable",
    ];
    fn kagemusha_command_bad_request_reject_codes(operation_id: &str) -> Vec<&'static str> {
        let mut codes = KAGEMUSHA_COMMAND_COMMON_BAD_REQUEST_REJECT_CODES.to_vec();
        match operation_id {
            "offlineTopUp" => codes.extend_from_slice(KAGEMUSHA_TOP_UP_BAD_REQUEST_REJECT_CODES),
            "offlineRedeem" => codes.extend_from_slice(KAGEMUSHA_REDEEM_BAD_REQUEST_REJECT_CODES),
            _ => panic!("unexpected offline command operation id"),
        }
        codes.extend_from_slice(TRANSACTION_ACCEPTANCE_BAD_REQUEST_REJECT_CODES);
        codes
    }
    fn transaction_submission_bad_request_reject_codes() -> Vec<&'static str> {
        let mut codes = vec!["invalid_transaction_payload"];
        codes.extend_from_slice(TRANSACTION_ACCEPTANCE_BAD_REQUEST_REJECT_CODES);
        codes
    }
    fn canonical_document() -> Value {
        norito::json::from_str(CANONICAL_OPENAPI_JSON)
            .expect("package-local OpenAPI authority must parse")
    }
    fn openapi_schemas() -> Map {
        component_schemas(&canonical_document()).clone()
    }
    fn sccp_schemas() -> Map {
        openapi_schemas()
            .into_iter()
            .filter(|(name, _)| name.starts_with("Sccp"))
            .collect()
    }
    fn schema_ref(name: &str) -> Value {
        norito::json!({ "$ref": (format!("#/components/schemas/{name}")) })
    }
    fn tags_section() -> Value {
        canonical_document()
            .get("tags")
            .cloned()
            .expect("package-local OpenAPI authority must contain tags")
    }
    fn subscription_paths() -> Map {
        canonical_document()
            .get("paths")
            .and_then(Value::as_object)
            .expect("package-local OpenAPI authority paths")
            .iter()
            .filter(|(path, _)| path.starts_with("/v1/subscriptions"))
            .map(|(path, item)| (path.clone(), item.clone()))
            .collect()
    }
    fn subscription_schemas(schemas: &mut Map) {
        schemas.extend(
            openapi_schemas()
                .into_iter()
                .filter(|(name, _)| name.starts_with("Subscription")),
        );
    }
    const COMPONENT_REF_PREFIX: &str = "#/components/";
    const COMPONENT_SCHEMA_REF_PREFIX: &str = "#/components/schemas/";
    #[derive(Clone, Copy)]
    enum ComponentRefContext {
        Document,
        Components,
        SchemaMap,
        Schema,
        HeaderMap,
        Header,
    }
    impl ComponentRefContext {
        fn child(self, key: &str) -> Self {
            match self {
                Self::Schema | Self::SchemaMap => Self::Schema,
                Self::HeaderMap => Self::Header,
                Self::Components => match key {
                    "schemas" => Self::SchemaMap,
                    "headers" => Self::HeaderMap,
                    _ => Self::Document,
                },
                Self::Document if key == "components" => Self::Components,
                Self::Document if key == "headers" => Self::HeaderMap,
                Self::Document | Self::Header if key == "schema" || key.ends_with("-schema") => {
                    Self::Schema
                }
                Self::Document => Self::Document,
                Self::Header => Self::Header,
            }
        }
        fn expected_component(self) -> Option<&'static str> {
            match self {
                Self::Schema => Some("schemas"),
                Self::Header => Some("headers"),
                _ => None,
            }
        }
    }
    fn documented_reject_codes<'a>(responses: &'a Map, status: &str) -> Vec<&'a str> {
        responses
            .get(status)
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("x-iroha-reject-code"))
            .and_then(Value::as_object)
            .and_then(|header| header.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("HTTP {status} x-iroha-reject-code enum"))
            .iter()
            .map(|code| code.as_str().expect("reject-code enum value"))
            .collect()
    }
    fn response_documents_reject_code(responses: &Map, status: &str) -> bool {
        responses
            .get(status)
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .is_some_and(|headers| headers.contains_key("x-iroha-reject-code"))
    }
    fn component_schemas(document: &Value) -> &Map {
        component_collections(document)
            .get("schemas")
            .and_then(Value::as_object)
            .expect("component schemas")
    }
    fn component_collections(document: &Value) -> &Map {
        document
            .get("components")
            .and_then(Value::as_object)
            .expect("OpenAPI components")
    }
    fn openapi_operation<'a>(document: &'a Value, path: &str, method: &str) -> &'a Map {
        document
            .get("paths")
            .and_then(Value::as_object)
            .and_then(|paths| paths.get(path))
            .and_then(Value::as_object)
            .and_then(|path_item| path_item.get(method))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{method} {path} operation"))
    }
    fn assert_canonical_auth_required_response(
        operation: &Map,
        path: &str,
        expected_reject_code: &str,
    ) {
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("POST {path} responses"));
        assert_eq!(
            documented_reject_codes(responses, "401"),
            vec![expected_reject_code],
            "POST {path} exact 401 reject code"
        );
        let challenge = responses
            .get("401")
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("WWW-Authenticate"))
            .and_then(Value::as_object)
            .and_then(|header| header.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("const"))
            .and_then(Value::as_str);
        assert_eq!(challenge, Some("Signature"), "POST {path} challenge");
    }
    fn assert_alias_auth_required_response(operation: &Map, path: &str) {
        assert_canonical_auth_required_response(operation, path, "alias_auth_required");
    }
    fn operation_request_schema_ref<'a>(operation: &'a Map, path: &str) -> &'a str {
        operation
            .get("requestBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("request schema for {path}"))
    }
    fn operation_response_schema_ref<'a>(operation: &'a Map, status: &str, path: &str) -> &'a str {
        operation
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get(status))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("HTTP {status} response schema for {path}"))
    }
    fn assert_strict_object_schema(
        schemas: &Map,
        name: &str,
        required_fields: &[&str],
        optional_fields: &[&str],
    ) {
        let schema = schemas
            .get(name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name} schema"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false)),
            "{name} must reject unknown fields"
        );
        let actual_required = schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{name} required fields"))
            .iter()
            .map(|field| field.as_str().expect("required field name"))
            .collect::<BTreeSet<_>>();
        let expected_required = required_fields.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual_required, expected_required, "{name} required fields");
        let actual_properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name} properties"))
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected_properties = required_fields
            .iter()
            .chain(optional_fields)
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_properties, expected_properties, "{name} properties");
    }
    fn catalog_openapi_route_enabled(method: CatalogHttpMethod, path: &str) -> bool {
        RouteCatalog::new(CATALOGED_ROUTES)
            .project(
                CatalogProjection::OpenApi,
                crate::router::builder::compiled_route_features(),
            )
            .into_iter()
            .any(|route| route.method() == method && route.path() == path)
    }
    fn expected_operation_effect(method: &str, path: &str) -> &'static str {
        if expected_operator_operation(method, path) {
            return "operator";
        }
        if method == "post" && path.starts_with("/v1/musubi/instructions/") {
            return "build_instruction";
        }
        if expected_read_operation(method, path) {
            return "read";
        }
        "write"
    }
    fn expected_operator_operation(method: &str, path: &str) -> bool {
        let catalog_method = match method {
            "get" => Some(CatalogHttpMethod::Get),
            "post" => Some(CatalogHttpMethod::Post),
            "put" => Some(CatalogHttpMethod::Put),
            "patch" => Some(CatalogHttpMethod::Patch),
            "delete" => Some(CatalogHttpMethod::Delete),
            _ => None,
        };
        if catalog_method.is_some_and(|method| {
            RouteCatalog::new(CATALOGED_ROUTES)
                .routes()
                .iter()
                .any(|route| {
                    route.method() == method
                        && route.path() == path
                        && route.surface() == ApiSurface::Operator
                })
        }) {
            return true;
        }
        if method == "get" {
            return false;
        }
        path.starts_with("/v1/operator/")
            || matches!(
                path,
                uri::CONFIGURATION
                    | "/v1/internal/torii/proxy"
                    | "/v1/nexus/lifecycle"
                    | "/v1/nexus/lane-lifecycle"
                    | "/v1/gov/protected-namespaces"
            )
    }
    fn expected_read_operation(method: &str, path: &str) -> bool {
        matches!(method, "get" | "head" | "options")
            || (method == "post" && path.starts_with("/v1/musubi/queries/"))
            || (method == "post"
                && matches!(
                    path,
                    uri::QUERY
                        | "/v1/accounts/query"
                        | "/v1/accounts/faucet/prepare"
                        | "/v1/accounts/onboard/plan"
                        | "/v1/accounts/onboard/prepare"
                        | "/v1/accounts/onboarding/current-state"
                        | "/v1/aliases/by-account"
                        | "/v1/aliases/setup/plan"
                        | "/v1/aliases/lease/renew/plan"
                        | "/v1/aliases/auto-renew/plan"
                        | "/v1/aliases/resolve"
                        | "/v1/aliases/resolve-index"
                        | "/v1/retail/recipients/lookup"
                        | "/v1/retail/recipients/route"
                        | "/v1/fee-sponsor-programs/by-id"
                        | "/v1/fees/quote"
                        | "/v1/assets/aliases/resolve"
                        | "/v1/assets/definitions/query"
                        | "/v1/assets/holders/query"
                        | "/v1/assets/query"
                        | "/v1/contracts/aliases/resolve"
                        | "/v1/contracts/deployment-state"
                        | "/v1/contracts/view"
                        | "/v1/contracts/view/batch"
                        | "/v1/controls/asset-transfer/query"
                        | "/v1/da/commitments"
                        | "/v1/da/commitments/prove"
                        | "/v1/da/commitments/verify"
                        | "/v1/da/pin-intents"
                        | "/v1/da/pin-intents/prove"
                        | "/v1/da/pin-intents/verify"
                        | "/v1/domains/query"
                        | "/v1/accounts/recovery/status"
                        | "/v1/multisig/proposals/query"
                        | "/v1/multisig/proposals/resolve"
                        | "/v1/multisig/spec"
                        | "/v1/nfts/query"
                        | "/v1/proofs/query"
                        | "/v1/rwas/query"
                        | "/v1/soracloud/ciphertext/query"
                        | "/v1/pipeline/transactions/status"
                        | "/v1/pipeline/transactions/details"
                        | "/v1/zk/merkle-path"
                        | "/v1/zk/roots"
                        | "/v1/zk/verify-batch"
                        | "/v1/zk/vote/tally"
                ))
    }
    fn operation_header_requirements(operation: &Map) -> Vec<(String, bool)> {
        operation
            .get("parameters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
            .map(|parameter| {
                (
                    parameter
                        .get("name")
                        .and_then(Value::as_str)
                        .expect("header parameter name")
                        .to_owned(),
                    parameter
                        .get("required")
                        .and_then(Value::as_bool)
                        .expect("header required flag"),
                )
            })
            .collect()
    }
    fn assert_no_retired_vpn_fee_fields(value: &Value, location: &str) {
        match value {
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    assert_no_retired_vpn_fee_fields(value, &format!("{location}[{index}]"));
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    assert!(
                        !matches!(
                            key.as_str(),
                            "lease_fee_nanos" | "earned_fee_nanos" | "refunded_fee_nanos"
                        ),
                        "retired VPN fee field {key} at {location}"
                    );
                    assert_no_retired_vpn_fee_fields(value, &format!("{location}.{key}"));
                }
            }
            _ => {}
        }
    }
    fn assert_component_refs_resolve(
        value: &Value,
        components: &Map,
        location: &str,
        context: ComponentRefContext,
        reference_count: &mut usize,
    ) {
        match value {
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    assert_component_refs_resolve(
                        value,
                        components,
                        &format!("{location}[{index}]"),
                        context,
                        reference_count,
                    );
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    let child_location = format!("{location}.{key}");
                    if key == "$ref" {
                        *reference_count += 1;
                        let reference = value.as_str().unwrap_or_else(|| {
                            panic!("OpenAPI $ref at {child_location} must be a string")
                        });
                        let component_path = reference
                            .strip_prefix(COMPONENT_REF_PREFIX)
                            .unwrap_or_else(|| {
                                panic!(
                                    "OpenAPI $ref at {child_location} must target a local component root: {reference}"
                                )
                            });
                        let mut segments = component_path.split('/');
                        let kind = segments.next().unwrap_or_default();
                        let name = segments.next().unwrap_or_default();
                        assert!(
                            !kind.is_empty() && !name.is_empty(),
                            "OpenAPI $ref at {child_location} has no component kind or name"
                        );
                        assert!(
                            segments.next().is_none(),
                            "OpenAPI $ref at {child_location} must not target a nested component path: {reference}"
                        );
                        let expected_kind = context.expected_component().unwrap_or_else(|| {
                            panic!(
                                "OpenAPI $ref at {child_location} is not permitted at this location: {reference}"
                            )
                        });
                        assert_eq!(
                            kind, expected_kind,
                            "OpenAPI $ref at {child_location} targets {kind}, but this location requires {expected_kind}"
                        );
                        let collection = components
                            .get(kind)
                            .and_then(Value::as_object)
                            .unwrap_or_else(|| {
                                panic!(
                                    "OpenAPI $ref at {child_location} targets missing component collection {kind}"
                                )
                            });
                        assert!(
                            collection.contains_key(name),
                            "OpenAPI $ref at {child_location} targets missing component {kind}/{name}"
                        );
                    }
                    assert_component_refs_resolve(
                        value,
                        components,
                        &child_location,
                        context.child(key),
                        reference_count,
                    );
                }
            }
            _ => {}
        }
    }
    fn component_properties<'a>(schemas: &'a Map, name: &str) -> &'a Map {
        schemas
            .get(name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name} properties"))
    }
    fn component_required<'a>(schemas: &'a Map, name: &str) -> Vec<&'a str> {
        schemas
            .get(name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("required"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{name} required fields"))
            .iter()
            .map(|field| field.as_str().expect("required field name"))
            .collect()
    }
    fn property_ref<'a>(schemas: &'a Map, owner: &str, property: &str) -> &'a str {
        component_properties(schemas, owner)
            .get(property)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{owner}.{property} schema reference"))
    }
    fn property_integer_bounds(schemas: &Map, owner: &str, property: &str) -> (u64, u64) {
        let schema = component_properties(schemas, owner)
            .get(property)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{owner}.{property} property schema"));
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("integer"),
            "{owner}.{property} must be an integer"
        );
        (
            schema
                .get("minimum")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{owner}.{property} minimum")),
            schema
                .get("maximum")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{owner}.{property} maximum")),
        )
    }
    fn property_array_bounds(schemas: &Map, owner: &str, property: &str) -> (u64, u64) {
        let schema = component_properties(schemas, owner)
            .get(property)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{owner}.{property} property schema"));
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("array"),
            "{owner}.{property} must be an array"
        );
        (
            schema
                .get("minItems")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{owner}.{property} minItems")),
            schema
                .get("maxItems")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{owner}.{property} maxItems")),
        )
    }
    fn nullable_property_ref<'a>(schemas: &'a Map, owner: &str, property: &str) -> &'a str {
        let schema = component_properties(schemas, owner)
            .get(property)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{owner}.{property} property schema"));
        let one_of = schema.get("oneOf").and_then(Value::as_array);
        let any_of = schema.get("anyOf").and_then(Value::as_array);
        assert!(
            one_of.is_some() ^ any_of.is_some(),
            "{owner}.{property} must use exactly one nullable union keyword"
        );
        let variants = one_of.or(any_of).expect("checked nullable union");
        assert_eq!(
            variants.len(),
            2,
            "{owner}.{property} nullable union must have exactly two variants"
        );
        assert_eq!(
            variants
                .get(1)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("type"))
                .and_then(Value::as_str),
            Some("null"),
            "{owner}.{property} second variant must be null"
        );
        variants
            .first()
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{owner}.{property} typed nullable reference"))
    }
    fn collect_component_refs(value: &Value, refs: &mut BTreeSet<String>) {
        match value {
            Value::Array(values) => {
                for value in values {
                    collect_component_refs(value, refs);
                }
            }
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    let component = reference
                        .strip_prefix(COMPONENT_SCHEMA_REF_PREFIX)
                        .unwrap_or_else(|| {
                            panic!("Kagemusha schema has a non-component reference: {reference}")
                        });
                    refs.insert(component.to_owned());
                }
                for value in object.values() {
                    collect_component_refs(value, refs);
                }
            }
            _ => {}
        }
    }
    fn reachable_component_graph(schemas: &Map, roots: &[&str]) -> BTreeSet<String> {
        let mut pending = roots
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<VecDeque<_>>();
        let mut reachable = BTreeSet::new();
        while let Some(name) = pending.pop_front() {
            if !reachable.insert(name.clone()) {
                continue;
            }
            let schema = schemas
                .get(&name)
                .unwrap_or_else(|| panic!("component reference does not resolve: {name}"));
            let mut refs = BTreeSet::new();
            collect_component_refs(schema, &mut refs);
            for referenced in refs {
                assert!(
                    schemas.contains_key(&referenced),
                    "component {name} references missing component {referenced}"
                );
                if !reachable.contains(&referenced) {
                    pending.push_back(referenced);
                }
            }
        }
        reachable
    }
    #[test]
    fn openapi_authorities_have_only_resolvable_component_refs() {
        for (label, document) in [
            ("package-local", canonical_document()),
            ("compiled", generate_spec()),
        ] {
            let components = component_collections(&document);
            let mut reference_count = 0;
            assert_component_refs_resolve(
                &document,
                components,
                "$",
                ComponentRefContext::Document,
                &mut reference_count,
            );
            assert!(
                reference_count > 0,
                "{label} OpenAPI document unexpectedly contains no component references"
            );
        }
    }
    #[test]
    fn package_openapi_authority_is_canonical_norito_json() {
        let parsed = canonical_document();
        let rendered = norito::json::to_string_pretty(&parsed)
            .expect("serialize package-local Torii OpenAPI authority");
        assert_eq!(
            rendered.as_bytes(),
            CANONICAL_OPENAPI_JSON.as_bytes(),
            "package-local OpenAPI authority must use canonical pretty Norito JSON bytes"
        );
    }
    #[test]
    fn standalone_ballot_drafts_publish_one_exact_success_and_standard_bad_request() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        assert_strict_object_schema(
            schemas,
            "GovernanceBallotDraftResponseV1",
            &["drafted", "tx_instructions"],
            &[],
        );
        let response = component_properties(schemas, "GovernanceBallotDraftResponseV1");
        assert_eq!(response["drafted"]["const"].as_bool(), Some(true));
        assert_eq!(response["tx_instructions"]["minItems"].as_u64(), Some(1));
        assert_eq!(response["tx_instructions"]["maxItems"].as_u64(), Some(1));
        assert_eq!(
            response["tx_instructions"]["items"]["$ref"].as_str(),
            Some("#/components/schemas/GovernanceBallotInstructionDraftV1")
        );
        assert_strict_object_schema(
            schemas,
            "GovernanceBallotInstructionDraftV1",
            &["wire_id", "payload_hex"],
            &[],
        );

        for path in [
            "/v1/gov/ballots/plain",
            "/v1/gov/ballots/zk-v1",
            "/v1/gov/ballots/zk-v1/ballot-proof",
        ] {
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation_response_schema_ref(operation, "200", path),
                "#/components/schemas/GovernanceBallotDraftResponseV1"
            );
            assert_eq!(
                operation_response_schema_ref(operation, "400", path),
                "#/components/schemas/ErrorEnvelope"
            );
        }
    }
    #[test]
    fn account_onboarding_current_state_openapi_is_one_closed_v1_observation() {
        const PATH: &str = "/v1/accounts/onboarding/current-state";
        const REQUEST: &str = "AccountOnboardingCurrentStateRequest";
        const RESPONSE: &str = "AccountOnboardingCurrentStateResponse";

        let document = canonical_document();
        let schemas = component_schemas(&document);
        assert_strict_object_schema(schemas, REQUEST, &["version", "account_id", "alias"], &[]);

        assert_strict_object_schema(
            schemas,
            RESPONSE,
            &[
                "version",
                "network_id",
                "account_id",
                "alias",
                "account_exists",
                "alias_target_account_id",
                "observed_block_height",
                "observed_block_hash",
            ],
            &[],
        );
        for owner in [REQUEST, RESPONSE] {
            let version = component_properties(schemas, owner)
                .get("version")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{owner}.version schema"));
            assert_eq!(version.get("type").and_then(Value::as_str), Some("integer"));
            assert_eq!(version.get("const").and_then(Value::as_u64), Some(1));
        }
        assert_eq!(
            property_ref(schemas, RESPONSE, "network_id"),
            "#/components/schemas/NetworkId"
        );
        assert_eq!(
            property_ref(schemas, RESPONSE, "observed_block_hash"),
            "#/components/schemas/Hash"
        );
        let height = component_properties(schemas, RESPONSE)
            .get("observed_block_height")
            .and_then(Value::as_object)
            .expect("atomic onboarding observed height schema");
        assert_eq!(height.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(height.get("format").and_then(Value::as_str), Some("uint64"));
        assert_eq!(height.get("minimum").and_then(Value::as_u64), Some(1));
        let alias_target = component_properties(schemas, RESPONSE)
            .get("alias_target_account_id")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("oneOf"))
            .and_then(Value::as_array)
            .expect("atomic onboarding alias target nullable union");
        assert_eq!(alias_target.len(), 2);
        assert_eq!(
            alias_target[0].get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            alias_target[1].get("type").and_then(Value::as_str),
            Some("null")
        );

        let operation = openapi_operation(&document, PATH, "post");
        assert_eq!(
            operation_request_schema_ref(operation, PATH),
            "#/components/schemas/AccountOnboardingCurrentStateRequest"
        );
        assert_eq!(
            operation_response_schema_ref(operation, "200", PATH),
            "#/components/schemas/AccountOnboardingCurrentStateResponse"
        );
        assert_eq!(
            operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("read")
        );
        let parameters = operation
            .get("parameters")
            .and_then(Value::as_array)
            .expect("atomic onboarding auth headers");
        assert_eq!(parameters.len(), 5);
        let parameter_names = parameters
            .iter()
            .map(|parameter| {
                let parameter = parameter.as_object().expect("auth header parameter");
                assert_eq!(parameter.get("in").and_then(Value::as_str), Some("header"));
                assert_eq!(parameter.get("required"), Some(&Value::Bool(false)));
                parameter
                    .get("name")
                    .and_then(Value::as_str)
                    .expect("auth header name")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            parameter_names,
            BTreeSet::from([
                "X-Iroha-Account",
                "X-Iroha-Nonce",
                "X-Iroha-Signature",
                "X-Iroha-Timestamp-Ms",
                "X-Iroha-Witness",
            ])
        );
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("atomic onboarding responses");
        assert_eq!(
            responses
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "200", "400", "401", "403", "408", "409", "413", "415", "429", "500", "502", "503",
            ])
        );
        assert_eq!(
            documented_reject_codes(responses, "401"),
            vec!["alias_auth_required", "alias_auth_invalid"]
        );
        assert_eq!(
            documented_reject_codes(responses, "409"),
            vec!["alias.catalog.mapping_conflict", "route_conflict"]
        );
        let challenges = responses
            .get("401")
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("WWW-Authenticate"))
            .and_then(Value::as_object)
            .and_then(|header| header.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .expect("atomic onboarding authentication challenges")
            .iter()
            .map(|challenge| challenge.as_str().expect("authentication challenge"))
            .collect::<Vec<_>>();
        assert_eq!(
            challenges,
            vec!["IrohaApiToken realm=\"torii\"", "Signature"]
        );
    }
    #[test]
    fn connect_status_openapi_separates_session_and_operator_aggregate() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        assert_strict_object_schema(
            schemas,
            "ConnectSessionStatus",
            &[
                "sid",
                "app_attached",
                "wallet_attached",
                "approved",
                "buffered_frames",
                "buffered_bytes",
                "last_seq_app_to_wallet",
                "last_seq_wallet_to_app",
                "origin",
            ],
            &[],
        );
        assert_strict_object_schema(
            schemas,
            "ConnectPolicyStatus",
            &[
                "ws_max_sessions",
                "ws_per_ip_max_sessions",
                "ws_rate_per_ip_per_min",
                "session_ttl_ms",
                "frame_max_bytes",
                "session_buffer_max_bytes",
                "relay_enabled",
                "relay_strategy",
                "relay_effective_strategy",
                "relay_p2p_attached",
                "p2p_ttl_hops",
                "heartbeat_interval_ms",
                "heartbeat_miss_tolerance",
                "heartbeat_min_interval_ms",
            ],
            &[],
        );
        assert_strict_object_schema(
            schemas,
            "ConnectStatus",
            &[
                "enabled",
                "sessions_total",
                "sessions_active",
                "per_ip_sessions",
                "buffered_sessions",
                "total_buffer_bytes",
                "dedupe_size",
                "policy",
                "frames_in_total",
                "frames_out_total",
                "ciphertext_total",
                "dedupe_drops_total",
                "buffer_drops_total",
                "plaintext_control_drops_total",
                "monotonic_drops_total",
                "sequence_violation_closes_total",
                "role_direction_mismatch_total",
                "ping_miss_total",
                "p2p_rebroadcasts_total",
                "p2p_rebroadcast_skipped_total",
                "p2p_auth_failures_total",
                "p2p_ttl_drops_total",
                "p2p_unknown_session_drops_total",
                "p2p_session_claims_in_total",
                "p2p_session_claims_installed_total",
                "p2p_session_claim_conflicts_total",
                "p2p_role_consumed_total",
                "p2p_session_terminated_total",
            ],
            &[],
        );
        assert_eq!(
            property_ref(schemas, "ConnectStatus", "policy"),
            "#/components/schemas/ConnectPolicyStatus"
        );

        let session = openapi_operation(&document, "/v1/connect/status", "get");
        assert_eq!(
            operation_response_schema_ref(session, "200", "Connect session status"),
            "#/components/schemas/ConnectSessionStatus"
        );
        let session_parameters = session
            .get("parameters")
            .and_then(Value::as_array)
            .expect("Connect session status parameters");
        assert_eq!(session_parameters.len(), 2);
        for parameter in session_parameters {
            let parameter = parameter
                .as_object()
                .expect("Connect session status parameter");
            assert_eq!(parameter.get("required"), Some(&Value::Bool(true)));
        }
        assert_eq!(
            session_parameters
                .iter()
                .map(|parameter| parameter["name"].as_str().expect("parameter name"))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["Authorization", "sid"])
        );

        let aggregate = openapi_operation(&document, "/v1/connect/status/aggregate", "get");
        assert_eq!(
            operation_response_schema_ref(aggregate, "200", "Connect aggregate status"),
            "#/components/schemas/ConnectStatus"
        );
        assert_eq!(
            aggregate.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("operator")
        );
        let operator_headers = aggregate
            .get("parameters")
            .and_then(Value::as_array)
            .expect("Connect aggregate operator headers");
        assert_eq!(operator_headers.len(), 4);
        for parameter in operator_headers {
            let parameter = parameter
                .as_object()
                .expect("Connect aggregate operator header");
            assert_eq!(parameter.get("in").and_then(Value::as_str), Some("header"));
            assert_eq!(parameter.get("required"), Some(&Value::Bool(true)));
        }
        assert_eq!(
            operator_headers
                .iter()
                .map(|parameter| parameter["name"].as_str().expect("operator header name"))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "X-Iroha-Operator-Nonce",
                "X-Iroha-Operator-Public-Key",
                "X-Iroha-Operator-Signature",
                "X-Iroha-Operator-Timestamp-Ms",
            ])
        );
    }
    #[test]
    fn retired_apartment_execution_history_is_absent() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        for retired in [
            "AgentRuntimeExecutionSummary",
            "AgentRuntimeWorkflowStepSummary",
        ] {
            assert!(
                !schemas.contains_key(retired),
                "retired schema {retired} must not remain in the first-release authority"
            );
        }
        let apartment_status = schemas
            .get("AgentApartmentStatusEntry")
            .and_then(Value::as_object)
            .expect("AgentApartmentStatusEntry schema");
        let properties = apartment_status
            .get("properties")
            .and_then(Value::as_object)
            .expect("AgentApartmentStatusEntry properties");
        assert!(!properties.contains_key("runtime_recent_runs"));
        assert!(
            apartment_status
                .get("required")
                .and_then(Value::as_array)
                .expect("AgentApartmentStatusEntry required fields")
                .iter()
                .all(|field| field.as_str() != Some("runtime_recent_runs"))
        );
    }
    #[test]
    fn uploaded_private_model_runtime_openapi_surface_is_absent() {
        let document = generate_spec();
        let paths = document["paths"].as_object().expect("OpenAPI paths object");
        for retired_path in [
            "/v1/soracloud/model/upload/encryption-recipient",
            "/v1/soracloud/model/upload/private/execute",
            "/v1/soracloud/model/upload/private/receipts",
        ] {
            assert!(
                !paths.contains_key(retired_path),
                "retired uploaded private-model path `{retired_path}` must not be advertised"
            );
        }

        for retained_path in [
            "/v1/soracloud/model/upload/register",
            "/v1/soracloud/model/upload/status",
        ] {
            assert!(
                paths.contains_key(retained_path),
                "registry-only uploaded-model path `{retained_path}` must remain advertised"
            );
        }

        let schemas = component_schemas(&document);
        for retired_schema in [
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
        ] {
            assert!(
                !schemas.contains_key(retired_schema),
                "retired uploaded private-model schema `{retired_schema}` must not be registered"
            );
        }
        for retained_schema in ["SoraUploadedModelBundleV1", "UploadedModelStatusResponse"] {
            assert!(
                schemas.contains_key(retained_schema),
                "registry-only uploaded-model schema `{retained_schema}` must remain registered"
            );
        }

        assert_strict_object_schema(
            schemas,
            "SoraHfSourceRecordV1",
            &[
                "schema_version",
                "source_id",
                "repo_id",
                "resolved_revision",
                "created_at_ms",
                "updated_at_ms",
            ],
            &[],
        );
        let hf_source_properties = schemas["SoraHfSourceRecordV1"]["properties"]
            .as_object()
            .expect("SoraHfSourceRecordV1 properties");
        for retired_field in [
            "model_name",
            "adapter_id",
            "normalized_runtime_hash",
            "resource_profile",
            "source_artifact_hash",
            "source_profile",
            "status",
            "last_error",
        ] {
            assert!(
                !hf_source_properties.contains_key(retired_field),
                "registry metadata must not revive retired field `{retired_field}`"
            );
        }
        for retired_schema in [
            "SoraHfBackendFamilyV1",
            "SoraHfModelFormatV1",
            "SoraHfModelSizeBucketV1",
            "SoraHfResourceProfileV1",
            "SoraHfSourceProfileV1",
            "SoraHfSourceStatusV1",
        ] {
            assert!(
                !schemas.contains_key(retired_schema),
                "derived runtime/tariff classification `{retired_schema}` must stay retired"
            );
        }
        let runtime_snapshot = schemas
            .get("SoracloudRuntimeSnapshot")
            .and_then(Value::as_object)
            .expect("SoracloudRuntimeSnapshot schema");
        assert!(
            !runtime_snapshot
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.contains_key("hf_sources")),
            "generated Hugging Face imports are storage records, not runtime plans"
        );
        assert!(
            runtime_snapshot
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| required
                    .iter()
                    .all(|field| field.as_str() != Some("hf_sources"))),
            "SoracloudRuntimeSnapshot must not require a retired HF runtime field"
        );

        let soracloud_tag = document["tags"]
            .as_array()
            .expect("OpenAPI tags")
            .iter()
            .find(|tag| tag["name"].as_str() == Some("Soracloud"))
            .expect("Soracloud OpenAPI tag");
        assert_eq!(
            soracloud_tag["description"].as_str(),
            Some("Soracloud service, model registry, and Inrou runtime endpoints.")
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one cohesive exact Soracloud release-route and schema authority audit"
    )]
    fn soracloud_release_openapi_matches_the_exact_closed_catalog_surface() {
        use iroha_torii_shared::route_catalog::{
            AdmissionPolicy, AuthenticationPolicy, RouteEffect,
        };

        fn method_name(method: CatalogHttpMethod) -> &'static str {
            match method {
                CatalogHttpMethod::Get => "get",
                CatalogHttpMethod::Post => "post",
                CatalogHttpMethod::Put => "put",
                CatalogHttpMethod::Patch => "patch",
                CatalogHttpMethod::Delete => "delete",
                CatalogHttpMethod::Any => {
                    panic!("ANY gateways cannot enter the Soracloud OpenAPI surface")
                }
            }
        }
        fn assert_closed_exact_schema(value: &Value, location: &str) {
            match value {
                Value::Array(values) => {
                    for (index, value) in values.iter().enumerate() {
                        assert_closed_exact_schema(value, &format!("{location}/{index}"));
                    }
                }
                Value::Object(schema) => {
                    assert!(
                        !schema.contains_key("default"),
                        "body schema {location} must not infer an omitted default"
                    );
                    if schema.get("additionalProperties") == Some(&Value::Bool(true)) {
                        assert_eq!(
                            location,
                            format!("{COMPONENT_SCHEMA_REF_PREFIX}JsonValue"),
                            "only the explicitly dynamic JSON value may remain open"
                        );
                    }
                    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                        assert_eq!(
                            schema.get("type").and_then(Value::as_str),
                            Some("object"),
                            "typed property inventory at {location} must be an object"
                        );
                        assert_eq!(
                            schema.get("additionalProperties"),
                            Some(&Value::Bool(false)),
                            "typed body object {location} must reject unknown fields"
                        );
                        let required = schema
                            .get("required")
                            .and_then(Value::as_array)
                            .unwrap_or_else(|| {
                                panic!("typed body object {location} required fields")
                            })
                            .iter()
                            .map(|field| field.as_str().expect("required body field"))
                            .collect::<BTreeSet<_>>();
                        let declared = properties
                            .keys()
                            .map(String::as_str)
                            .collect::<BTreeSet<_>>();
                        assert_eq!(
                            required, declared,
                            "typed body object {location} must require every V1 field, including nullable fields"
                        );
                    } else if schema.get("type").and_then(Value::as_str) == Some("object") {
                        let additional = schema.get("additionalProperties");
                        assert!(
                            location == format!("{COMPONENT_SCHEMA_REF_PREFIX}JsonValue")
                                || matches!(
                                    additional,
                                    Some(Value::Object(_)) | Some(Value::Bool(false))
                                ),
                            "map object {location} must type its values or be closed"
                        );
                    }
                    for (key, value) in schema {
                        assert_closed_exact_schema(value, &format!("{location}/{key}"));
                    }
                }
                _ => {}
            }
        }

        let document = generate_spec();
        let schemas = component_schemas(&document);
        let routes = CATALOGED_ROUTES
            .iter()
            .filter(|route| {
                route.surface() == ApiSurface::Public && route.path().starts_with("/v1/soracloud/")
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 55, "canonical Soracloud release inventory");

        let expected = routes
            .iter()
            .map(|route| {
                assert!(route.projections().openapi(), "{}", route.path());
                assert!(route.projections().sdk(), "{}", route.path());
                (
                    route.path().replace("{*", "{"),
                    method_name(route.method()).to_owned(),
                )
            })
            .collect::<BTreeSet<_>>();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("canonical OpenAPI paths");
        let actual = paths
            .iter()
            .filter(|(path, _)| path.starts_with("/v1/soracloud/"))
            .flat_map(|(path, item)| {
                let item = item.as_object().expect("Soracloud path item");
                ["get", "post", "put", "patch", "delete"]
                    .into_iter()
                    .filter_map(move |method| {
                        item.contains_key(method)
                            .then(|| (path.clone(), method.to_owned()))
                    })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual.len(), 55, "canonical Soracloud OpenAPI inventory");
        assert_eq!(
            actual, expected,
            "Soracloud OpenAPI/catalog method-path equality"
        );

        let exact_contracts = [
            ("/v1/soracloud/status", "get", None, "SoracloudStatusV1"),
            (
                "/v1/soracloud/services/{service_name}/public-discovery",
                "get",
                None,
                "ServicePublicDiscoveryResponse",
            ),
            (
                "/v1/soracloud/services/{service_name}/revisions/{service_version}/public-discovery",
                "get",
                None,
                "ServicePublicDiscoveryResponse",
            ),
            (
                "/v1/soracloud/deploy",
                "post",
                Some("SignedBundleRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/upgrade",
                "post",
                Some("SignedBundleRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/apps/deploy",
                "post",
                Some("SignedAppInfraRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/apps/upgrade",
                "post",
                Some("SignedAppInfraRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/apps/status",
                "get",
                None,
                "AppInfraStatusResponse",
            ),
            (
                "/v1/soracloud/apps/{app_name}/status",
                "get",
                None,
                "AppInfraStatusResponse",
            ),
            (
                "/v1/soracloud/rollback",
                "post",
                Some("SignedRollbackRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/rollout",
                "post",
                Some("SignedRolloutAdvanceRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/state/mutate",
                "post",
                Some("SignedStateMutationRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/service/config/set",
                "post",
                Some("SignedServiceConfigSetRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/service/config/delete",
                "post",
                Some("SignedServiceConfigDeleteRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/service/config/status",
                "get",
                None,
                "ServiceConfigStatusResponse",
            ),
            (
                "/v1/soracloud/service/secret/set",
                "post",
                Some("SignedServiceSecretSetRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/service/secret/delete",
                "post",
                Some("SignedServiceSecretDeleteRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/service/secret/status",
                "get",
                None,
                "ServiceSecretStatusResponse",
            ),
            (
                "/v1/soracloud/fhe/job/run",
                "post",
                Some("SignedFheJobRunRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/decrypt/request",
                "post",
                Some("SignedDecryptionRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/health/access/request",
                "post",
                Some("SignedDecryptionRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/health/compliance/report",
                "get",
                None,
                "HealthComplianceReportResponse",
            ),
            (
                "/v1/soracloud/ciphertext/query",
                "post",
                Some("SignedCiphertextQueryRequest"),
                "CiphertextQueryResponse",
            ),
            (
                "/v1/soracloud/training/job/start",
                "post",
                Some("SignedTrainingJobStartRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/training/job/checkpoint",
                "post",
                Some("SignedTrainingJobCheckpointRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/training/job/retry",
                "post",
                Some("SignedTrainingJobRetryRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/training/job/status",
                "get",
                None,
                "TrainingJobStatusResponse",
            ),
            (
                "/v1/soracloud/model/weight/register",
                "post",
                Some("SignedModelWeightRegisterRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/model/weight/promote",
                "post",
                Some("SignedModelWeightPromoteRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/model/weight/rollback",
                "post",
                Some("SignedModelWeightRollbackRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/model/weight/status",
                "get",
                None,
                "ModelWeightStatusResponse",
            ),
            (
                "/v1/soracloud/model/artifact/register",
                "post",
                Some("SignedModelArtifactRegisterRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/model/artifact/status",
                "get",
                None,
                "ModelArtifactStatusResponse",
            ),
            (
                "/v1/soracloud/model/upload/register",
                "post",
                Some("SignedUploadedModelRegisterRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/model/upload/status",
                "get",
                None,
                "UploadedModelStatusResponse",
            ),
            (
                "/v1/soracloud/hf/lease/join",
                "post",
                Some("SignedHfSharedLeaseJoinRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/hf/lease/status",
                "get",
                None,
                "HfSharedLeaseStatusResponse",
            ),
            (
                "/v1/soracloud/hf/lease/leave",
                "post",
                Some("SignedHfLeaseLeaveRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/hf/lease/renew",
                "post",
                Some("SignedHfLeaseRenewRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/deploy",
                "post",
                Some("SignedAgentDeployRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/lease/renew",
                "post",
                Some("SignedAgentLeaseRenewRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/restart",
                "post",
                Some("SignedAgentRestartRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/status",
                "get",
                None,
                "AgentStatusResponse",
            ),
            (
                "/v1/soracloud/agent/wallet/spend",
                "post",
                Some("SignedAgentWalletSpendRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/wallet/approve",
                "post",
                Some("SignedAgentWalletApproveRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/policy/revoke",
                "post",
                Some("SignedAgentPolicyRevokeRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/message/send",
                "post",
                Some("SignedAgentMessageSendRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/message/ack",
                "post",
                Some("SignedAgentMessageAckRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/mailbox/status",
                "get",
                None,
                "AgentMailboxStatusResponse",
            ),
            (
                "/v1/soracloud/agent/autonomy/allow",
                "post",
                Some("SignedAgentArtifactAllowRequest"),
                "SoracloudMutationDraftResponse",
            ),
            (
                "/v1/soracloud/agent/autonomy/status",
                "get",
                None,
                "AgentAutonomyStatusResponse",
            ),
        ];
        assert_eq!(exact_contracts.len(), 55);
        assert_eq!(
            exact_contracts
                .iter()
                .map(|(path, method, _, _)| ((*path).to_owned(), (*method).to_owned()))
                .collect::<BTreeSet<_>>(),
            actual,
            "every canonical Soracloud operation must have one explicit schema contract"
        );
        for (path, method, request, response) in exact_contracts {
            let operation = openapi_operation(&document, path, method);
            if let Some(request) = request {
                assert_eq!(
                    operation_request_schema_ref(operation, path),
                    format!("{COMPONENT_SCHEMA_REF_PREFIX}{request}"),
                    "{method} {path} request root"
                );
            } else {
                assert!(
                    operation.get("requestBody").is_none(),
                    "{method} {path} must not infer a request body"
                );
            }
            assert_eq!(
                operation_response_schema_ref(operation, "200", path),
                format!("{COMPONENT_SCHEMA_REF_PREFIX}{response}"),
                "{method} {path} response root"
            );
        }

        let mut roots = BTreeSet::new();
        for route in routes {
            let path = route.path().replace("{*", "{");
            let method = method_name(route.method());
            let operation = openapi_operation(&document, &path, method);
            let expected_effect = if route.effect() == RouteEffect::ReadOnly {
                "read"
            } else {
                "write"
            };
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some(expected_effect),
                "{method} {path} effect"
            );
            let expected_headers = match (route.authentication(), route.admission()) {
                (
                    AuthenticationPolicy::CanonicalAccountSignature,
                    AdmissionPolicy::AuthenticatedAccount,
                ) => canonical_account_header_requirements(false),
                (AuthenticationPolicy::ToriiDefault, AdmissionPolicy::Public) => Vec::new(),
                pair => panic!("unexpected Soracloud authentication/admission {pair:?} at {path}"),
            };
            let headers = operation_header_requirements(operation);
            assert_eq!(headers, expected_headers, "{method} {path} auth headers");
            assert!(
                headers
                    .iter()
                    .all(|(name, _)| !name
                        .eq_ignore_ascii_case("x-iroha-internal-soracloud-account")),
                "{method} {path} exposes the internal local-read account header"
            );

            let response = operation_response_schema_ref(operation, "200", &path);
            let response_root = response
                .strip_prefix(COMPONENT_SCHEMA_REF_PREFIX)
                .unwrap_or_else(|| panic!("{method} {path} response must use a component schema"));
            assert_ne!(
                response_root, "JsonValue",
                "{method} {path} untyped response"
            );
            roots.insert(response_root.to_owned());
            if route.method() == CatalogHttpMethod::Post {
                let request = operation_request_schema_ref(operation, &path);
                let request_root = request
                    .strip_prefix(COMPONENT_SCHEMA_REF_PREFIX)
                    .unwrap_or_else(|| panic!("POST {path} request must use a component schema"));
                assert_ne!(request_root, "JsonValue", "POST {path} untyped request");
                roots.insert(request_root.to_owned());
            } else {
                assert!(
                    operation.get("requestBody").is_none(),
                    "GET {path} must not infer a request body"
                );
            }
        }

        let mut pending = roots.into_iter().collect::<VecDeque<_>>();
        let mut reachable = BTreeSet::new();
        let mut dynamic_json_parents = BTreeSet::new();
        while let Some(name) = pending.pop_front() {
            if !reachable.insert(name.clone()) {
                continue;
            }
            let schema = schemas.get(&name).unwrap_or_else(|| {
                panic!("Soracloud component reference does not resolve: {name}")
            });
            assert_closed_exact_schema(schema, &format!("{COMPONENT_SCHEMA_REF_PREFIX}{name}"));
            let mut references = BTreeSet::new();
            collect_component_refs(schema, &mut references);
            if references.contains("JsonValue") {
                dynamic_json_parents.insert(name.clone());
            }
            pending.extend(references);
        }
        assert_eq!(
            dynamic_json_parents,
            BTreeSet::from([
                "ServiceConfigSetRequest".to_owned(),
                "ServiceConfigStatusEntry".to_owned(),
                "SignedBundleRequest".to_owned(),
                "SoraServiceConfigEntryV1".to_owned(),
            ]),
            "only explicitly dynamic configuration JSON fields may use JsonValue"
        );
        assert!(reachable.contains("JsonValue"));

        assert_strict_object_schema(
            schemas,
            "SoracloudLocalReadBinding",
            &[
                "binding_name",
                "state_key",
                "payload_commitment",
                "artifact_hash",
            ],
            &[],
        );
        let serialized = norito::json::to_string(&document).expect("serialize Soracloud authority");
        for retired in [
            "cap-bound-local-signing",
            "SoracloudHfSharedLeaseJoinDraftV1",
            "PrivateUploadedModelArtifactRef\"",
            "PrivateUploadedModelQuantizedCpuModel\"",
            "PrivateUploadedModelReceipt\"",
            "SoracloudTxInstr\"",
            "x-iroha-internal-soracloud-account",
        ] {
            assert!(
                !serialized.contains(retired),
                "retired or internal Soracloud compatibility surface remains: {retired}"
            );
        }
    }
    #[test]
    fn pipeline_preflight_schema_exposes_only_per_scheme_signature_batch_caps() {
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let pipeline = component_properties(schemas, "PipelinePreflightResponse")
            .get("pipeline")
            .and_then(Value::as_object)
            .expect("PipelinePreflightResponse.pipeline schema");
        let properties = pipeline
            .get("properties")
            .and_then(Value::as_object)
            .expect("PipelinePreflightResponse.pipeline properties");
        let required = pipeline
            .get("required")
            .and_then(Value::as_array)
            .expect("PipelinePreflightResponse.pipeline required fields");

        assert!(properties.get("signature_batch_max").is_none());
        assert!(
            !required
                .iter()
                .any(|field| field.as_str() == Some("signature_batch_max"))
        );
        for field in [
            "signature_batch_max_ed25519",
            "signature_batch_max_secp256k1",
            "signature_batch_max_pqc",
            "signature_batch_max_bls",
        ] {
            assert!(
                properties.get(field).is_some(),
                "missing schema for {field}"
            );
            assert!(
                required
                    .iter()
                    .any(|required_field| required_field.as_str() == Some(field)),
                "{field} must be required"
            );
        }
    }
    #[test]
    fn checked_openapi_assets_match_package_authority() {
        let latest = include_str!("../../../artifacts/openapi/torii.json");
        let current = include_str!("../../../artifacts/openapi/versions/current/torii.json");
        let package = CANONICAL_OPENAPI_JSON;
        assert_eq!(
            latest.as_bytes(),
            current.as_bytes(),
            "latest/current artifact drift"
        );
        assert_eq!(
            latest.as_bytes(),
            package.as_bytes(),
            "release/package authority drift"
        );
    }
    #[test]
    fn public_lane_staking_schema_closes_status_variants_and_unbond_cutoff() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        let status = schemas["PublicLaneValidatorStatus"]
            .as_object()
            .expect("public-lane validator status schema");
        assert_eq!(
            status
                .get("discriminator")
                .and_then(Value::as_object)
                .and_then(|value| value.get("propertyName"))
                .and_then(Value::as_str),
            Some("type")
        );
        let variants = status
            .get("oneOf")
            .and_then(Value::as_array)
            .expect("closed public-lane validator status variants");
        let expected = BTreeMap::from([
            ("Active", BTreeSet::from(["type"])),
            ("Exited", BTreeSet::from(["type"])),
            ("Exiting", BTreeSet::from(["releases_at_ms", "type"])),
            (
                "PendingActivation",
                BTreeSet::from(["activates_at_height", "type"]),
            ),
            ("Slashed", BTreeSet::from(["slash_id", "type"])),
        ]);
        let mut observed = BTreeMap::new();
        for variant in variants {
            let variant = variant.as_object().expect("status variant object");
            assert_eq!(variant.get("type").and_then(Value::as_str), Some("object"));
            assert_eq!(
                variant.get("additionalProperties").and_then(Value::as_bool),
                Some(false)
            );
            let properties = variant
                .get("properties")
                .and_then(Value::as_object)
                .expect("status variant properties");
            let tag = properties["type"]
                .get("const")
                .and_then(Value::as_str)
                .expect("status variant discriminator constant");
            let property_names = properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let required = variant
                .get("required")
                .and_then(Value::as_array)
                .expect("status variant required fields")
                .iter()
                .map(|field| field.as_str().expect("required field name"))
                .collect::<BTreeSet<_>>();
            assert_eq!(required, property_names, "{tag} payload must be exact");
            assert!(observed.insert(tag, property_names).is_none());
        }
        assert_eq!(observed, expected);

        let unbonding = schemas["PublicLaneUnbonding"]
            .as_object()
            .expect("public-lane unbonding schema");
        let properties = unbonding["properties"]
            .as_object()
            .expect("public-lane unbonding properties");
        assert!(properties.contains_key("slashable_through_height"));
        assert!(!properties.contains_key("scheduled_at_height"));
        assert!(
            unbonding["required"]
                .as_array()
                .expect("public-lane unbonding required fields")
                .iter()
                .any(|field| field.as_str() == Some("slashable_through_height"))
        );
    }
    #[cfg(all(
        feature = "node-api",
        feature = "ws_integration_tests",
        feature = "telemetry",
        feature = "profiling",
        feature = "schema",
        feature = "zk-verify-batch"
    ))]
    #[test]
    fn compiled_projection_matches_served_bytes() {
        let generated = norito::json::to_string_pretty(&generate_spec())
            .expect("serialize compiled release Torii OpenAPI");
        let served = compiled_spec_json();
        assert_eq!(
            generated.as_bytes(),
            served.as_bytes(),
            "compiled/served document drift"
        );
    }
    #[test]
    fn transaction_payload_schema_requires_closed_domain_admission_and_positive_ttl() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        assert_strict_object_schema(
            schemas,
            "TransactionPayload",
            &openapi_contract_strings("openapi.transaction_payload.required").collect::<Vec<_>>(),
            &["nonce"],
        );
        let properties = schemas["TransactionPayload"]["properties"]
            .as_object()
            .expect("TransactionPayload properties");
        for retired in ["chain", "chain_id", "chainId"] {
            assert!(
                !properties.contains_key(retired),
                "retired transaction identity key `{retired}` must be absent"
            );
        }
        assert_eq!(
            properties["domain"].get("$ref").and_then(Value::as_str),
            Some("#/components/schemas/TransactionDomain")
        );
        assert_eq!(
            properties["admission_intent"]
                .get("$ref")
                .and_then(Value::as_str),
            Some("#/components/schemas/TransactionAdmissionIntent")
        );
        assert_eq!(
            properties["time_to_live_ms"]
                .get("minimum")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            schemas["NetworkId"]["allOf"][0]
                .get("$ref")
                .and_then(Value::as_str),
            Some("#/components/schemas/Hash")
        );
        let variants = schemas["TransactionDomain"]["oneOf"]
            .as_array()
            .expect("TransactionDomain variants");
        assert_eq!(variants.len(), 2);
        for variant in variants {
            assert_eq!(
                variant.get("additionalProperties").and_then(Value::as_bool),
                Some(false)
            );
        }
        assert_eq!(
            variants[0]["properties"]["kind"]
                .get("const")
                .and_then(Value::as_str),
            Some("network")
        );
        assert_eq!(
            variants[0]["properties"]["value"]
                .get("$ref")
                .and_then(Value::as_str),
            Some("#/components/schemas/NetworkId")
        );
        assert_eq!(
            variants[1]["properties"]["kind"]
                .get("const")
                .and_then(Value::as_str),
            Some("genesis")
        );
        assert!(variants[1]["properties"].get("value").is_none());

        let admission_schema = schemas["TransactionAdmissionIntent"]
            .as_object()
            .expect("TransactionAdmissionIntent schema");
        assert_eq!(
            admission_schema
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["oneOf"]),
            "TransactionAdmissionIntent must expose only its closed union"
        );
        let admission_variants = admission_schema["oneOf"]
            .as_array()
            .expect("TransactionAdmissionIntent variants");
        let admission_labels =
            openapi_contract_strings("openapi.transaction_admission_intent.labels")
                .collect::<Vec<_>>();
        assert_eq!(admission_variants.len(), admission_labels.len());
        for (variant, expected_label) in admission_variants.iter().zip(admission_labels) {
            let variant = variant
                .as_object()
                .expect("TransactionAdmissionIntent object variant");
            assert_eq!(
                variant.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                BTreeSet::from(["additionalProperties", "properties", "required", "type"]),
                "TransactionAdmissionIntent variant shape"
            );
            assert_eq!(variant.get("type").and_then(Value::as_str), Some("object"));
            assert_eq!(
                variant.get("additionalProperties").and_then(Value::as_bool),
                Some(false)
            );
            let required = variant["required"]
                .as_array()
                .expect("TransactionAdmissionIntent required fields")
                .iter()
                .map(|field| field.as_str().expect("required field name"))
                .collect::<BTreeSet<_>>();
            assert_eq!(required, BTreeSet::from(["intent", "value"]));
            let intent_properties = variant["properties"]
                .as_object()
                .expect("TransactionAdmissionIntent properties");
            assert_eq!(
                intent_properties
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["intent", "value"])
            );
            assert_eq!(
                intent_properties["intent"]
                    .get("const")
                    .and_then(Value::as_str),
                Some(expected_label)
            );
            assert_eq!(
                intent_properties["intent"]
                    .as_object()
                    .expect("TransactionAdmissionIntent intent property")
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["const"])
            );
            assert_eq!(
                intent_properties["value"]
                    .get("type")
                    .and_then(Value::as_str),
                Some("null")
            );
            assert_eq!(
                intent_properties["value"]
                    .as_object()
                    .expect("TransactionAdmissionIntent value property")
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["type"])
            );
        }
    }
    #[test]
    fn authenticated_transaction_nullable_fields_are_required_and_nullable() {
        let document = canonical_document();
        let schemas = component_schemas(&document);

        let payload = &schemas["TransactionPayload"];
        assert!(
            payload["required"]
                .as_array()
                .expect("TransactionPayload required fields")
                .iter()
                .any(|field| field.as_str() == Some("attachments"))
        );
        assert_eq!(
            payload["properties"]["attachments"]["type"],
            norito::json!(["string", "null"])
        );

        for variant in schemas["FeePaymentIntent"]["oneOf"]
            .as_array()
            .expect("fee-payment variants")
        {
            let value = &variant["properties"]["value"];
            assert_eq!(
                value.get("additionalProperties").and_then(Value::as_bool),
                Some(false)
            );
            assert!(
                value["required"]
                    .as_array()
                    .expect("fee-payment required fields")
                    .iter()
                    .any(|field| field.as_str() == Some("gas_limit"))
            );
            assert_eq!(
                value["properties"]["gas_limit"]["type"],
                norito::json!(["integer", "null"])
            );
        }

        let receipt_payload = &schemas["TransactionSubmissionReceipt"]["properties"]["payload"];
        assert_eq!(
            receipt_payload
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            receipt_payload["required"]
                .as_array()
                .expect("receipt-payload required fields")
                .iter()
                .any(|field| field.as_str() == Some("signed_transaction_hash"))
        );
        assert_eq!(
            receipt_payload["properties"]["signed_transaction_hash"]["type"],
            norito::json!(["string", "null"])
        );
    }
    #[test]
    fn incoming_static_openapi_contracts_remain_bound_to_runtime_routes() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        for [name, network_property, retired_property, target] in openapi_contract_fixed_rows::<4>(
            "openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.1",
        ) {
            let properties = schemas[name]["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("{name} properties"));
            assert!(!properties.contains_key(retired_property), "{name}");
            assert_eq!(
                property_ref(schemas, name, network_property),
                format!("{COMPONENT_SCHEMA_REF_PREFIX}{target}"),
                "{name}.{network_property} reference drift"
            );
        }
        assert_eq!(
            property_ref(schemas, "KagemushaUnshieldPublicInputs", "network_tag"),
            "#/components/schemas/KagemushaFixed32Bytes"
        );
        assert!(
            !schemas["KagemushaUnshieldPublicInputs"]["properties"]
                .as_object()
                .expect("KagemushaUnshieldPublicInputs properties")
                .contains_key("chain_tag")
        );
        for name in openapi_contract_strings(
            "openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.strings.1",
        ) {
            assert!(schemas.contains_key(name), "missing static schema {name}");
        }
        assert!(!schemas.contains_key("PrivacyCapabilityRowV1"));
        assert!(!schemas.contains_key("PrivacyCapabilitySnapshotV1"));
        let protocols = schemas["PrivacyExact12CapabilityManifestV1"]["properties"]["protocols"]
            .as_object()
            .expect("Exact12 protocols schema");
        assert_eq!(protocols["minItems"].as_u64(), Some(12));
        assert_eq!(protocols["maxItems"].as_u64(), Some(12));
        assert_eq!(
            protocols["prefixItems"]
                .as_array()
                .expect("Exact12 positional schemas")
                .len(),
            12
        );
        assert_eq!(protocols["items"].as_bool(), Some(false));
        let row_properties = schemas["PrivacyExact12CapabilityRowV1"]["properties"]
            .as_object()
            .expect("Exact12 row properties");
        assert_eq!(
            row_properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "activation",
                "compiled_profile",
                "execution_mode",
                "operation_schema",
                "privacy_feature_mask",
                "protocol_id",
                "readiness",
            ])
        );
        let readiness_variants = schemas["PrivacyCapabilityReadinessV1"]["oneOf"]
            .as_array()
            .expect("Exact12 readiness variants");
        assert_eq!(
            readiness_variants
                .iter()
                .map(|variant| {
                    variant["properties"]["readiness"]["const"]
                        .as_str()
                        .expect("Exact12 readiness tag")
                })
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["production-qualified", "unavailable"])
        );
        let activation_properties = schemas["PrivacyProtocolActivationRecordV1"]["properties"]
            .as_object()
            .expect("privacy activation properties");
        assert!(!activation_properties.contains_key("production_qualification"));
        assert!(!activation_properties.contains_key("assurance"));
        for retired in [
            "PrivacyAssuranceV1",
            "PrivacyCapabilityActivationStateV1",
            "PrivacyCapabilityLimitationV1",
        ] {
            assert!(
                !schemas.contains_key(retired),
                "retired pre-release privacy schema remains: {retired}"
            );
        }
        let unavailable_variants = schemas["PrivacyCapabilityUnavailableReasonV1"]["oneOf"]
            .as_array()
            .expect("Exact12 unavailable-reason variants");
        assert_eq!(
            unavailable_variants
                .iter()
                .map(|variant| {
                    variant["properties"]["reason"]["const"]
                        .as_str()
                        .expect("Exact12 unavailable-reason tag")
                })
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "compiled-profile",
                "invalid-production-qualification",
                "missing-production-qualification",
                "not-registered",
                "proposed",
                "retired",
                "suspended",
            ])
        );
        let qualification = schemas["PrivacyExact12QualificationRecordV1"]["properties"]
            .as_object()
            .expect("production qualification properties");
        assert_eq!(
            qualification
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["deployment_qualification", "release_manifest",])
        );
        let manifest_properties = schemas["PrivacyExact12CapabilityManifestV1"]["properties"]
            .as_object()
            .expect("Exact12 manifest properties");
        assert!(manifest_properties.contains_key("qualification"));
        assert!(!schemas.contains_key("PrivacyProtocolProductionQualificationV1"));
        let security_claim = schemas["PrivacySecurityClaimV1"]["properties"]
            .as_object()
            .expect("security claim properties");
        assert_eq!(
            security_claim
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "achieved_security_bits",
                "audit_bundle_digest",
                "catalog_commitment",
                "parameter_digest",
                "protocol_id",
                "reduction_digest",
                "security_model",
                "target_security_bits",
                "verifier_digest",
            ])
        );
        assert_eq!(
            schemas["PrivacySecurityModelV1"]["properties"]["security_model"]["enum"]
                .as_array()
                .expect("closed privacy security models")
                .iter()
                .map(|value| value.as_str().expect("security-model label"))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["classical-rom", "pq-qrom"])
        );
        let catalog_commitment = schemas["PrivacyExact12CatalogCommitmentV1"]["const"]
            .as_array()
            .expect("pinned Exact12 catalog commitment")
            .iter()
            .map(|byte| {
                u8::try_from(byte.as_u64().expect("catalog commitment byte"))
                    .expect("catalog commitment byte fits u8")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            catalog_commitment,
            iroha_data_model::privacy::PrivacyExact12CatalogCommitmentV1::canonical()
                .digest()
                .to_le_bytes()
        );
        let details = openapi_operation(&document, "/v1/pipeline/transactions/details", "post");
        assert_eq!(
            operation_request_schema_ref(details, "transaction details"),
            "#/components/schemas/VersionedSignedQueryJson"
        );
        assert_eq!(
            operation_response_schema_ref(details, "200", "transaction details"),
            "#/components/schemas/PipelineTransactionDetailsResponse"
        );
        assert_eq!(
            details.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("read")
        );
        let connect = openapi_operation(&document, "/v1/connect/session", "post");
        assert_eq!(
            operation_request_schema_ref(connect, "Connect session"),
            "#/components/schemas/ConnectSessionCreateRequest"
        );
        assert_eq!(
            operation_response_schema_ref(connect, "200", "Connect session"),
            "#/components/schemas/ConnectSessionCreateResponse"
        );
        for [path, request_schema] in openapi_contract_fixed_rows::<2>(
            "openapi.incoming_static_openapi_contracts_remain_bound_to_runtime_routes.rows.2",
        ) {
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation_request_schema_ref(operation, path),
                format!("{COMPONENT_SCHEMA_REF_PREFIX}{request_schema}")
            );
            assert!(operation.contains_key("security"), "POST {path}");
            assert!(
                operation_header_requirements(operation)
                    .iter()
                    .any(|(name, _)| name == "X-Iroha-Account"),
                "POST {path} must publish account authentication"
            );
        }
        let pin = openapi_operation(&document, "/v1/sorafs/pin", "get");
        let pin_parameters = pin["parameters"]
            .as_array()
            .expect("pin list parameters")
            .iter()
            .filter_map(|parameter| parameter.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            pin_parameters,
            [
                "expected_finalized_height",
                "expected_finalized_block_hash_hex",
                "limit",
                "max_bytes",
                "after_digest_hex",
                "status",
            ]
        );
        assert_eq!(
            operation_response_schema_ref(pin, "200", "pin list"),
            "#/components/schemas/PinManifestPageV1"
        );
        let axt_properties = schemas["AxtErrorDetails"]["properties"]
            .as_object()
            .expect("AXT error details properties");
        assert!(axt_properties.contains_key("active_handle_era"));
        assert!(axt_properties.contains_key("next_handle_counter"));
        assert!(!axt_properties.contains_key("next_min_handle_era"));
        assert!(!axt_properties.contains_key("next_min_sub_nonce"));
        let error_details_properties = schemas["ErrorDetails"]["properties"]
            .as_object()
            .expect("error details properties");
        assert!(error_details_properties.contains_key("entrypoint_hash"));
        assert!(error_details_properties.contains_key("tx_hash"));
    }
    #[test]
    fn static_account_operations_publish_exact_auth_and_private_responses() {
        let document = canonical_document();
        for (path, methods) in openapi_contract_rows("openapi.static_account_operations_publish_exact_auth_and_private_responses.method_rows")
            .iter()
            .map(|row| {
                let (path, methods) = row.split_first().expect("account operation contract row");
                (path.as_str(), methods.iter().map(String::as_str))
            }) {
            for method in methods {
                let operation = openapi_operation(&document, path, method);
                assert!(operation.contains_key("security"), "{method} {path}");
                assert!(
                    operation.contains_key("x-iroha-canonical-auth-v1"),
                    "{method} {path}"
                );
                let header_names = operation_header_requirements(operation)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>();
                for name in openapi_contract_strings("openapi.static_account_operations_publish_exact_auth_and_private_responses.strings.1") {
                    assert_eq!(
                        header_names
                            .iter()
                            .filter(|actual| actual.as_str() == name)
                            .count(),
                        1,
                        "{method} {path} must publish one {name} header"
                    );
                }
                assert!(
                    operation["responses"]
                        .as_object()
                        .expect("operation responses")
                        .values()
                        .all(|response| {
                            response["headers"]["Cache-Control"]["schema"]["const"].as_str()
                                == Some("private, no-store")
                        }),
                    "{method} {path} must publish private no-store responses"
                );
            }
        }
        for [path, method] in openapi_contract_fixed_rows::<2>(
            "openapi.static_account_operations_publish_exact_auth_and_private_responses.rows.1",
        ) {
            let operation = openapi_operation(&document, path, method);
            assert!(
                !operation_header_requirements(operation)
                    .iter()
                    .any(|(name, _)| name == "X-Iroha-Account"),
                "{method} {path} must retain its non-account admission contract"
            );
        }
    }
    #[test]
    fn compiled_private_cache_contract_follows_the_route_catalog() {
        let document = generate_spec();
        for route in RouteCatalog::new(CATALOGED_ROUTES)
            .project(
                CatalogProjection::OpenApi,
                crate::router::builder::compiled_route_features(),
            )
            .into_iter()
            .filter(|route| {
                route.requires_private_no_store() && route.method() != CatalogHttpMethod::Any
            })
        {
            let path = route.path().replace("{*", "{");
            let method = catalog_method_name(route.method());
            let operation = openapi_operation(&document, &path, method);
            let responses = operation
                .get("responses")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{method} {path} responses"));
            assert!(!responses.is_empty(), "{method} {path} responses");
            for (status, response) in responses {
                assert_eq!(
                    response["headers"]["Cache-Control"]["schema"]["const"].as_str(),
                    Some("private, no-store"),
                    "{method} {path} response {status} must follow authentication {:?}",
                    route.authentication(),
                );
            }
        }
    }
    #[test]
    fn operator_credential_management_contract_is_closed_and_two_factor() {
        const INVENTORY_PATH: &str = "/v1/operator/auth/credentials";
        const DELETE_PATH: &str = "/v1/operator/auth/credentials/{credential_id}";
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let inventory = openapi_operation(&document, INVENTORY_PATH, "get");
        let deletion = openapi_operation(&document, DELETE_PATH, "delete");

        for (operation, stable_route_id) in [
            (inventory, "operator.authentication.credentials"),
            (deletion, "operator.authentication.credential_delete"),
        ] {
            assert_eq!(
                operation_header_requirements(operation),
                [
                    "X-Iroha-Operator-Public-Key",
                    "X-Iroha-Operator-Timestamp-Ms",
                    "X-Iroha-Operator-Nonce",
                    "X-Iroha-Operator-Signature",
                    "X-Iroha-Operator-Session",
                ]
                .into_iter()
                .map(|name| (name.to_owned(), true))
                .collect::<Vec<_>>()
            );
            let session_parameter = operation["parameters"]
                .as_array()
                .expect("credential-management parameters")
                .iter()
                .find(|parameter| {
                    parameter.get("name").and_then(Value::as_str)
                        == Some("X-Iroha-Operator-Session")
                })
                .expect("operator session header parameter");
            assert_eq!(session_parameter["schema"]["minLength"].as_u64(), Some(43));
            assert_eq!(session_parameter["schema"]["maxLength"].as_u64(), Some(43));
            assert_eq!(
                session_parameter["schema"]["pattern"].as_str(),
                Some("^[A-Za-z0-9_-]{43}$")
            );
            let security = operation
                .get("security")
                .and_then(Value::as_array)
                .expect("operator credential-management security requirements");
            assert_eq!(security.len(), 1);
            let signature_headers = security[0]
                .as_object()
                .expect("conjunctive operator-signature requirement")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                signature_headers,
                [
                    "IrohaOperatorNonce",
                    "IrohaOperatorPublicKey",
                    "IrohaOperatorSignature",
                    "IrohaOperatorTimestampMs",
                ]
                .into_iter()
                .collect()
            );
            let route_auth = operation
                .get(ROUTE_AUTH_EXTENSION)
                .and_then(Value::as_object)
                .expect("catalog route-auth metadata");
            assert_eq!(
                route_auth.get("stableRouteId").and_then(Value::as_str),
                Some(stable_route_id)
            );
            assert_eq!(
                route_auth.get("authentication").and_then(Value::as_str),
                Some("operator_signature")
            );
            assert!(
                operation["responses"]
                    .as_object()
                    .expect("credential-management responses")
                    .values()
                    .all(|response| {
                        response["headers"]["Cache-Control"]["schema"]["const"].as_str()
                            == Some("private, no-store")
                    })
            );
        }

        assert_eq!(
            inventory["responses"]
                .as_object()
                .expect("credential inventory responses")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            ["200", "401", "403", "429", "500"].into_iter().collect()
        );
        assert_eq!(
            operation_response_schema_ref(inventory, "200", INVENTORY_PATH),
            "#/components/schemas/OperatorWebAuthnCredentialListResponse"
        );
        assert!(
            inventory["responses"]["500"]["description"]
                .as_str()
                .is_some_and(|description| {
                    description.contains("operator_webauthn_state_unavailable")
                        && !description.contains("capacity")
                })
        );

        assert_eq!(
            deletion["responses"]
                .as_object()
                .expect("credential deletion responses")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            ["200", "400", "401", "403", "404", "409", "429", "500"]
                .into_iter()
                .collect()
        );
        assert_eq!(
            operation_response_schema_ref(deletion, "200", DELETE_PATH),
            "#/components/schemas/OperatorWebAuthnCredentialDeleteResponse"
        );
        for (status, code) in [
            ("404", "operator_webauthn_credential_not_found"),
            ("409", "operator_webauthn_last_credential"),
        ] {
            assert!(
                deletion["responses"][status]["description"]
                    .as_str()
                    .is_some_and(|description| description.contains(code)),
                "DELETE {DELETE_PATH} HTTP {status} must document {code}"
            );
        }
        let delete_internal_error = deletion["responses"]["500"]["description"]
            .as_str()
            .expect("credential deletion internal-error description");
        assert!(delete_internal_error.contains("operator_webauthn_state_unavailable"));
        assert!(delete_internal_error.contains("operator_webauthn_persist_failed"));

        assert_strict_object_schema(
            schemas,
            "OperatorWebAuthnCredentialListResponse",
            &["credentials", "credentials_total"],
            &[],
        );
        assert_strict_object_schema(
            schemas,
            "OperatorWebAuthnCredentialMetadata",
            &["credential_id", "algorithm", "sign_count", "created_at_ms"],
            &[],
        );
        assert_strict_object_schema(
            schemas,
            "OperatorWebAuthnCredentialDeleteResponse",
            &["status", "credential_id", "credentials_total"],
            &[],
        );
        let metadata_properties = schemas["OperatorWebAuthnCredentialMetadata"]["properties"]
            .as_object()
            .expect("credential metadata properties");
        assert!(!metadata_properties.contains_key("public_key"));
        assert!(!metadata_properties.contains_key("verification_key"));
        assert_eq!(
            schemas["OperatorWebAuthnAlgorithm"]["enum"]
                .as_array()
                .expect("credential algorithms"),
            &[Value::from("es256"), Value::from("ed25519")]
        );
        assert_eq!(
            schemas["OperatorWebAuthnCredentialId"]["minLength"].as_u64(),
            Some(1)
        );
        assert_eq!(
            schemas["OperatorWebAuthnCredentialId"]["maxLength"].as_u64(),
            Some(1366)
        );
    }
    #[test]
    fn musubi_provider_bundle_attestation_and_exact_release_contract_is_static() {
        const PROVIDER_ATTESTATION_WIRE_ID: &str =
            "iroha.musubi.v1.provider_bundle_attestation.register";
        const PROVIDER_QUERY_PATH: &str = "/v1/musubi/queries/provider-bundle-attestation";
        const PROVIDER_REGISTER_PATH: &str =
            "/v1/musubi/instructions/provider-bundle-attestation-register";
        let document = canonical_document();
        let schemas = component_schemas(&document);
        for (name, required, properties) in
            openapi_contract_rows("openapi.musubi_provider_bundle_attestation.schema_rows")
                .iter()
                .map(|row| {
                    let required_len = row[1].parse::<usize>().expect("required-field count");
                    let required = row[2..2 + required_len]
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    let properties = row[2 + required_len..]
                        .chunks_exact(2)
                        .map(|pair| (pair[0].as_str(), pair[1].as_str()));
                    (row[0].as_str(), required, properties)
                })
        {
            let schema = schemas
                .get(name)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing {name} schema"));
            assert_eq!(
                schema.get("additionalProperties").and_then(Value::as_bool),
                Some(false),
                "{name} must remain closed"
            );
            assert_eq!(component_required(schemas, name), required);
            for (property, target) in properties {
                assert_eq!(
                    property_ref(schemas, name, property),
                    format!("{COMPONENT_SCHEMA_REF_PREFIX}{target}"),
                    "{name}.{property} reference drift"
                );
            }
        }
        for (path, request_type, response_type, effect) in [
            (
                PROVIDER_REGISTER_PATH,
                "RegisterMusubiProviderBundleAttestationV1",
                "MusubiInstructionEnvelopeV1",
                "build_instruction",
            ),
            (
                PROVIDER_QUERY_PATH,
                "MusubiProviderBundleAttestationKeyV1",
                "MusubiProviderBundleAttestationRecordV1",
                "read",
            ),
            (
                "/v1/musubi/queries/exact-release",
                "MusubiExactReleaseQueryV1",
                "MusubiExactReleaseSnapshotV1",
                "read",
            ),
        ] {
            let path_item = document
                .get("paths")
                .and_then(Value::as_object)
                .and_then(|paths| paths.get(path))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing static Musubi path {path}"));
            assert_eq!(
                path_item.keys().map(String::as_str).collect::<Vec<_>>(),
                vec!["post"],
                "{path} must expose only POST"
            );
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation
                    .get("x-iroha-norito-request-type")
                    .and_then(Value::as_str),
                Some(request_type)
            );
            assert_eq!(
                operation
                    .get("x-iroha-norito-response-type")
                    .and_then(Value::as_str),
                Some(response_type)
            );
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some(effect)
            );
            assert_eq!(
                operation
                    .get("requestBody")
                    .and_then(|body| body.get("content"))
                    .and_then(|content| content.get("application/json"))
                    .and_then(|media| media.get("schema"))
                    .and_then(|schema| schema.get("$ref"))
                    .and_then(Value::as_str),
                Some(format!("{COMPONENT_SCHEMA_REF_PREFIX}{request_type}").as_str())
            );
            assert_eq!(
                operation
                    .get("responses")
                    .and_then(|responses| responses.get("200"))
                    .and_then(|response| response.get("content"))
                    .and_then(|content| content.get("application/json"))
                    .and_then(|media| media.get("schema"))
                    .and_then(|schema| schema.get("$ref"))
                    .and_then(Value::as_str),
                Some(format!("{COMPONENT_SCHEMA_REF_PREFIX}{response_type}").as_str())
            );
        }
        let wire_ids = component_properties(schemas, "MusubiInstructionEnvelopeV1")
            .get("wire_id")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .expect("Musubi instruction wire-id enum")
            .iter()
            .map(|wire_id| wire_id.as_str().expect("Musubi wire id"))
            .collect::<Vec<_>>();
        let provider_index = wire_ids
            .iter()
            .position(|wire_id| *wire_id == PROVIDER_ATTESTATION_WIRE_ID)
            .expect("provider bundle-attestation wire id");
        assert_eq!(
            wire_ids
                .iter()
                .filter(|wire_id| **wire_id == PROVIDER_ATTESTATION_WIRE_ID)
                .count(),
            1
        );
        assert_eq!(
            wire_ids.get(provider_index.wrapping_sub(1)),
            Some(&"iroha.musubi.v1.archive.register")
        );
        assert_eq!(
            wire_ids.get(provider_index + 1),
            Some(&"iroha.musubi.v1.archive_location.add")
        );
        let preview_variants = schemas
            .get("MusubiInstructionPreviewV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("oneOf"))
            .and_then(Value::as_array)
            .expect("Musubi instruction preview variants");
        let provider_variants = preview_variants
            .iter()
            .filter(|variant| {
                variant
                    .get("properties")
                    .and_then(|properties| properties.get("wire_id"))
                    .and_then(|wire_id| wire_id.get("const"))
                    .and_then(Value::as_str)
                    == Some(PROVIDER_ATTESTATION_WIRE_ID)
            })
            .collect::<Vec<_>>();
        assert_eq!(provider_variants.len(), 1);
        assert_eq!(
            provider_variants[0]
                .get("properties")
                .and_then(|properties| properties.get("payload"))
                .and_then(|payload| payload.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/RegisterMusubiProviderBundleAttestationV1")
        );
    }
    #[test]
    fn static_authority_is_the_complete_catalog_projection_with_exact_effects() {
        fn method_name(method: CatalogHttpMethod) -> &'static str {
            match method {
                CatalogHttpMethod::Get => "get",
                CatalogHttpMethod::Post => "post",
                CatalogHttpMethod::Put => "put",
                CatalogHttpMethod::Patch => "patch",
                CatalogHttpMethod::Delete => "delete",
                CatalogHttpMethod::Any => {
                    panic!("ANY protocol gateways cannot enter the OpenAPI projection")
                }
            }
        }
        let document = canonical_document();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("static OpenAPI authority paths");
        let expected: BTreeSet<_> = RouteCatalog::new(CATALOGED_ROUTES)
            .routes()
            .iter()
            .filter(|route| route.projections().openapi())
            .map(|route| {
                (
                    route.path().replace("{*", "{"),
                    method_name(route.method()).to_owned(),
                )
            })
            .collect();
        let actual: BTreeSet<_> = paths
            .iter()
            .flat_map(|(path, item)| {
                let methods = item.as_object().expect("static OpenAPI path item");
                ["get", "post", "put", "patch", "delete"]
                    .into_iter()
                    .filter_map(move |method| {
                        methods
                            .contains_key(method)
                            .then(|| (path.clone(), method.to_owned()))
                    })
            })
            .collect();
        assert_eq!(
            actual, expected,
            "static OpenAPI authority must be the feature-independent catalog superset"
        );
        for (path, method) in actual {
            let operation = paths
                .get(&path)
                .and_then(Value::as_object)
                .and_then(|path_item| path_item.get(&method))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing static operation {method} {path}"));
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some(expected_operation_effect(&method, &path)),
                "static tool effect drift for {method} {path}"
            );
            let descriptor = RouteCatalog::new(CATALOGED_ROUTES)
                .routes()
                .iter()
                .find(|descriptor| {
                    descriptor.projections().openapi()
                        && descriptor.path().replace("{*", "{") == path
                        && method_name(descriptor.method()) == method
                })
                .unwrap_or_else(|| panic!("missing catalog descriptor for {method} {path}"));
            assert_eq!(
                operation.get(ROUTE_AUTH_EXTENSION),
                Some(&route_auth_metadata(*descriptor)),
                "static route-auth metadata drift for {method} {path}"
            );
            if let Some(expected_security) =
                standard_security_requirements(descriptor.authentication())
            {
                assert_eq!(
                    operation.get("security"),
                    Some(&expected_security),
                    "static standard security drift for {method} {path}"
                );
            }
        }
    }
    #[test]
    fn sccp_schema_serialization_excludes_retired_and_secret_fields() {
        assert_eq!(
            iroha_data_model::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64,
            9_007_199_254_740_991
        );
        let schemas = sccp_schemas();
        let material_properties = schemas
            .get("SccpSoraOutboundMaterialV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("SCCP outbound material properties");
        for forbidden in ["private_key", "secret", "signer", "seed", "mnemonic"] {
            assert!(
                !material_properties.contains_key(forbidden),
                "outbound material must not advertise `{forbidden}`"
            );
        }
        let serialized =
            norito::json::to_string(&Value::Object(schemas)).expect("serialize SCCP schemas");
        for forbidden in openapi_contract_strings(
            "openapi.sccp_schema_serialization_excludes_retired_and_secret_fields.strings.1",
        ) {
            assert!(
                !serialized.contains(forbidden),
                "retired or secret SCCP field `{forbidden}` reappeared"
            );
        }
    }
    #[test]
    fn sccp_ton_openapi_tracks_state_init_and_curve_neutral_wire_contract() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        let ton_deployment = schemas
            .get("SccpTonDestinationDeploymentV1")
            .and_then(Value::as_object)
            .expect("TON deployment schema");
        let properties = ton_deployment
            .get("properties")
            .and_then(Value::as_object)
            .expect("TON deployment properties");
        let required = ton_deployment
            .get("required")
            .and_then(Value::as_array)
            .expect("TON deployment required fields");
        for field in ["jetton_master_initial_data_hash", "route_initial_data_hash"] {
            assert_eq!(
                properties
                    .get(field)
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("$ref"))
                    .and_then(Value::as_str),
                Some("#/components/schemas/SccpNonzeroUpperHex32"),
                "TON StateInit commitment `{field}` must remain a nonzero hash",
            );
            assert!(
                required.iter().any(|entry| entry.as_str() == Some(field)),
                "TON StateInit commitment `{field}` must remain required",
            );
        }

        let expected_max = u64::try_from(iroha_sccp::SCCP_DESTINATION_PROOF_MAX_BASE64_BYTES_V1)
            .expect("SCCP outer-envelope base64 bound fits u64");
        for schema_name in [
            "SccpBridgeProofPrepareRequest",
            "SccpBridgeProofSignedRequest",
        ] {
            let proof = schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("destination_proof_b64"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{schema_name} destination proof schema"));
            assert_eq!(
                proof.get("maxLength").and_then(Value::as_u64),
                Some(expected_max),
                "submit bound must cover the closed outer destination-proof envelope",
            );
            let description = proof
                .get("description")
                .and_then(Value::as_str)
                .expect("destination proof description");
            assert!(description.contains("BridgeSccpDestinationProofV1"));
            assert!(description.contains("TON BLS12-381"));
        }

        let proof_request_response = document
            .get("paths")
            .and_then(Value::as_object)
            .and_then(|paths| paths.get("/v1/sccp/proof-requests/{message_id}"))
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("responses"))
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .expect("SCCP proof-request response content");
        assert_eq!(
            proof_request_response
                .get("application/json")
                .and_then(Value::as_object)
                .and_then(|content| content.get("schema")),
            Some(&schema_ref("SccpProofRequestV1")),
        );
        let binary_description = proof_request_response
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|content| content.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("description"))
            .and_then(Value::as_str)
            .expect("SCCP proof-request binary description");
        for concrete_type in [
            "iroha_sccp::SccpGroth16Bn254ProofRequestV1",
            "iroha_sccp::SccpTonGroth16Bls12381ProofRequestV1",
            "No enum wrapper",
        ] {
            assert!(binary_description.contains(concrete_type));
        }
    }
    #[test]
    fn production_constants_embedded_in_openapi_remain_frozen() {
        fn at<'a>(mut value: &'a Value, path: &[&str]) -> &'a Value {
            for component in path {
                value = value
                    .get(*component)
                    .unwrap_or_else(|| panic!("missing OpenAPI authority path {path:?}"));
            }
            value
        }
        fn u64_at(value: &Value, path: &[&str]) -> u64 {
            at(value, path)
                .as_u64()
                .unwrap_or_else(|| panic!("OpenAPI authority path {path:?} is not a u64"))
        }
        let document = canonical_document();
        let schema_length = |name: &str| {
            u64_at(
                &document,
                &["components", "schemas", name, "x-iroha-exact-byte-length"],
            )
        };
        assert_eq!(
            (
                schema_length("BootleLanternIssuanceAuthorizeRequestV1"),
                schema_length("BootleLanternIssuanceAuthorizationWireV1"),
                schema_length("BootleLanternIssuanceIssueRequestV1"),
                schema_length("BootleLanternIssuanceResponseWireV1"),
            ),
            (0, 320, 71_896, 3_176)
        );
        assert_eq!(
            u64_at(
                &document,
                &[
                    "paths",
                    "/v1/ledger/block/{height}",
                    "get",
                    "responses",
                    "200",
                    "content",
                    "application/x-norito",
                    "schema",
                    "x-iroha-max-bytes",
                ],
            ),
            u64::try_from(
                iroha_data_model::block::proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            )
            .expect("authenticated block proof byte limit fits u64")
        );
        assert_eq!(
            (
                u64_at(
                    &document,
                    &[
                        "components",
                        "schemas",
                        "ErrorEnvelope",
                        "properties",
                        "message",
                        "maxLength",
                    ],
                ),
                u64_at(
                    &document,
                    &[
                        "components",
                        "schemas",
                        "ErrorDetails",
                        "properties",
                        "hint",
                        "maxLength",
                    ],
                ),
                u64_at(
                    &document,
                    &[
                        "components",
                        "schemas",
                        "ErrorDetails",
                        "properties",
                        "reject_code",
                        "maxLength",
                    ],
                ),
            ),
            (
                u64::try_from(utils::MAX_ERROR_MESSAGE_CHARACTERS).expect("message bound"),
                u64::try_from(utils::MAX_ERROR_DETAIL_CHARACTERS).expect("detail bound"),
                u64::try_from(utils::MAX_REJECT_CODE_BYTES).expect("reject-code bound"),
            )
        );
        let multisig_limit = at(
            &document,
            &[
                "components",
                "schemas",
                "MultisigProposalsQueryRequest",
                "properties",
                "limit",
                "oneOf",
            ],
        )
        .as_array()
        .and_then(|variants| variants.first())
        .and_then(|variant| variant.get("maximum"))
        .and_then(Value::as_u64)
        .expect("multisig proposal query maximum");
        assert_eq!(
            multisig_limit,
            crate::routing::MULTISIG_PROPOSALS_MAX_PAGE_LIMIT
        );
    }
    // Textual inclusion preserves the original OpenAPI test-module paths.
    include!("openapi/tests/sorafs_contracts.rs");
    #[test]
    fn openapi_route_auth_metadata_matches_enabled_catalog_projection() {
        let document = generate_spec();
        let projected = RouteCatalog::new(CATALOGED_ROUTES).project(
            CatalogProjection::OpenApi,
            crate::router::builder::compiled_route_features(),
        );
        for descriptor in projected {
            let method = match descriptor.method() {
                CatalogHttpMethod::Get => "get",
                CatalogHttpMethod::Post => "post",
                CatalogHttpMethod::Put => "put",
                CatalogHttpMethod::Patch => "patch",
                CatalogHttpMethod::Delete => "delete",
                CatalogHttpMethod::Any => {
                    panic!("ANY protocol gateways cannot enter the OpenAPI projection")
                }
            };
            let path = descriptor.path().replace("{*", "{");
            let operation = openapi_operation(&document, &path, method);
            assert_eq!(
                operation.get(ROUTE_AUTH_EXTENSION),
                Some(&route_auth_metadata(*descriptor)),
                "{method} {path} route-auth metadata"
            );
        }
    }
    #[test]
    fn openapi_standard_security_matches_enabled_catalog_authentication() {
        let document = generate_spec();
        let projected = RouteCatalog::new(CATALOGED_ROUTES).project(
            CatalogProjection::OpenApi,
            crate::router::builder::compiled_route_features(),
        );
        for descriptor in projected {
            let method = match descriptor.method() {
                CatalogHttpMethod::Get => "get",
                CatalogHttpMethod::Post => "post",
                CatalogHttpMethod::Put => "put",
                CatalogHttpMethod::Patch => "patch",
                CatalogHttpMethod::Delete => "delete",
                CatalogHttpMethod::Any => {
                    panic!("ANY protocol gateways cannot enter the OpenAPI projection")
                }
            };
            let path = descriptor.path().replace("{*", "{");
            let operation = openapi_operation(&document, &path, method);
            if let Some(expected) = standard_security_requirements(descriptor.authentication()) {
                assert_eq!(
                    operation.get("security"),
                    Some(&expected),
                    "{method} {path} standard security"
                );
            }
        }

        let schemes = document
            .get("components")
            .and_then(|components| components.get("securitySchemes"))
            .and_then(Value::as_object)
            .expect("security schemes");
        for (scheme, header) in [
            ("IrohaOperatorPublicKey", "X-Iroha-Operator-Public-Key"),
            ("IrohaOperatorTimestampMs", "X-Iroha-Operator-Timestamp-Ms"),
            ("IrohaOperatorNonce", "X-Iroha-Operator-Nonce"),
            ("IrohaOperatorSignature", "X-Iroha-Operator-Signature"),
        ] {
            assert_eq!(
                schemes
                    .get(scheme)
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str),
                Some(header),
                "operator security scheme {scheme}"
            );
        }
    }
    #[test]
    fn protocol_specific_bootle_bearer_security_is_preserved() {
        let document = generate_spec();
        for path in [
            "/v1/privacy/bootle-lantern/issuance/authorize",
            "/v1/privacy/bootle-lantern/issuance/issue",
        ] {
            assert_eq!(
                openapi_operation(&document, path, "post").get("security"),
                Some(&norito::json!([
                    { "BootleLanternIssuanceBearer": [] }
                ])),
                "{path} must retain its protocol-specific bearer scheme"
            );
        }
    }
    #[test]
    fn openapi_operations_equal_the_enabled_catalog_projection() {
        use iroha_torii_shared::route_catalog::{
            CATALOGED_ROUTES, CatalogProjection, HttpMethod, RouteCatalog,
        };
        fn openapi_path(path: &str) -> String {
            // Axum marks a wildcard parameter with `*`; OpenAPI path templates
            // use the same parameter name without the router-specific marker.
            path.replace("{*", "{")
        }
        fn method_name(method: HttpMethod) -> &'static str {
            match method {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Patch => "patch",
                HttpMethod::Delete => "delete",
                HttpMethod::Any => {
                    panic!("ANY protocol gateways cannot enter the OpenAPI projection")
                }
            }
        }
        let expected: BTreeSet<_> = RouteCatalog::new(CATALOGED_ROUTES)
            .project(
                CatalogProjection::OpenApi,
                crate::router::builder::compiled_route_features(),
            )
            .into_iter()
            .map(|route| {
                (
                    openapi_path(route.path()),
                    method_name(route.method()).to_owned(),
                )
            })
            .collect();
        #[cfg(all(
            feature = "app_api",
            feature = "telemetry",
            feature = "profiling",
            feature = "schema",
            feature = "connect",
            feature = "zk-verify-batch",
            feature = "push"
        ))]
        assert_eq!(
            expected.len(),
            553,
            "the supported full Torii documentation profile must remain exactly 553 cataloged operations"
        );
        let spec = generate_spec();
        let paths = spec
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        let operation_methods = ["get", "post", "delete", "put", "patch"];
        let actual: BTreeSet<_> = paths
            .iter()
            .flat_map(|(path, item)| {
                let item = item.as_object().expect("OpenAPI path item");
                operation_methods.into_iter().filter_map(move |method| {
                    item.contains_key(method)
                        .then(|| (path.clone(), method.to_owned()))
                })
            })
            .collect();
        let missing: Vec<_> = expected.difference(&actual).cloned().collect();
        let undocumented_catalog_extras: Vec<_> = actual.difference(&expected).cloned().collect();
        assert!(
            missing.is_empty() && undocumented_catalog_extras.is_empty(),
            "OpenAPI/catalog projection mismatch; missing from OpenAPI: {missing:#?}; absent from enabled catalog projection: {undocumented_catalog_extras:#?}"
        );
    }
    #[test]
    fn every_operation_uses_one_declared_top_level_tag() {
        let document = generate_spec();
        let declared: BTreeSet<_> = document
            .get("tags")
            .and_then(Value::as_array)
            .expect("top-level tags")
            .iter()
            .map(|tag| {
                tag.get("name")
                    .and_then(Value::as_str)
                    .expect("top-level tag name")
            })
            .collect();
        assert_eq!(
            declared.len(),
            document
                .get("tags")
                .and_then(Value::as_array)
                .expect("top-level tags")
                .len(),
            "top-level tag names must be unique"
        );
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        for (path, path_item) in paths {
            let methods = path_item.as_object().expect("path item");
            for method in ["get", "post", "put", "patch", "delete"] {
                let Some(operation) = methods.get(method).and_then(Value::as_object) else {
                    continue;
                };
                let tags = operation
                    .get("tags")
                    .and_then(Value::as_array)
                    .unwrap_or_else(|| panic!("{method} {path} tags"));
                assert_eq!(tags.len(), 1, "{method} {path} must use exactly one tag");
                let tag = tags[0]
                    .as_str()
                    .unwrap_or_else(|| panic!("{method} {path} tag name"));
                assert!(
                    declared.contains(tag),
                    "{method} {path} uses undeclared tag {tag}"
                );
            }
        }
    }
    #[test]
    fn exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent() {
        let document = generate_spec();
        let schemas = component_schemas(&document);
        for [name, pattern] in openapi_contract_fixed_rows::<2>(
            "openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.rows.1",
        ) {
            let schema = schemas
                .get(name)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{name} schema"));
            assert_eq!(schema.get("type").and_then(Value::as_str), Some("string"));
            assert_eq!(schema.get("pattern").and_then(Value::as_str), Some(pattern));
            assert_eq!(schema.get("maxLength").and_then(Value::as_u64), Some(155));
        }
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        for path in openapi_contract_strings(
            "openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.strings.1",
        ) {
            assert!(
                !paths.contains_key(path),
                "retired path leaked into OpenAPI: {path}"
            );
        }
        for schema in openapi_contract_strings(
            "openapi.exact_quantity_components_remain_canonical_and_legacy_deal_api_is_absent.strings.2",
        ) {
            assert!(
                !schemas.contains_key(schema),
                "retired process-local deal schema leaked into OpenAPI: {schema}"
            );
        }
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn retired_sorafs_economics_surface_is_absent() {
        let document = generate_spec();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        for path in
            openapi_contract_strings("openapi.retired_sorafs_economics_surface_is_absent.strings.1")
        {
            assert!(
                !paths.contains_key(path),
                "retired process-local economics path leaked into OpenAPI: {path}"
            );
        }
        let schemas = component_schemas(&document);
        for schema in
            openapi_contract_strings("openapi.retired_sorafs_economics_surface_is_absent.strings.2")
        {
            assert!(
                !schemas.contains_key(schema),
                "retired process-local economics schema leaked into OpenAPI: {schema}"
            );
        }
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn converted_catalog_families_have_exact_openapi_operations() {
        use iroha_torii_shared::route_catalog::{self, HttpMethod};
        let spec = generate_spec();
        let paths = spec
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        let mut descriptors = route_catalog::aliases::ROUTES
            .iter()
            .chain(route_catalog::operator_authentication::ROUTES)
            .chain(route_catalog::iso20022::ROUTES)
            .chain(route_catalog::data_availability::ROUTES)
            .chain(route_catalog::musubi::ROUTES)
            .chain(route_catalog::mcp_transport::ROUTES)
            .copied()
            .collect::<Vec<_>>();
        descriptors.extend(
            route_catalog::sorafs::ROUTES
                .iter()
                .filter(|descriptor| descriptor.projections().openapi())
                .copied(),
        );
        #[cfg(feature = "connect")]
        descriptors.extend_from_slice(route_catalog::connect::ROUTES);
        for descriptor in descriptors {
            assert!(descriptor.projections().openapi());
            let method = match descriptor.method() {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Delete => "delete",
                other => panic!("unexpected converted route method: {other:?}"),
            };
            assert!(
                paths
                    .get(descriptor.path())
                    .and_then(Value::as_object)
                    .is_some_and(|operation| operation.contains_key(method)),
                "missing {method} OpenAPI operation for {} ({})",
                descriptor.stable_route_id(),
                descriptor.path()
            );
        }
        for unsupported_path in openapi_contract_strings(
            "openapi.converted_catalog_families_have_exact_openapi_operations.strings.1",
        ) {
            assert!(
                !paths.contains_key(unsupported_path),
                "unsupported path leaked into OpenAPI: {unsupported_path}"
            );
        }
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn soracloud_status_documents_only_the_canonical_routing_count() {
        let document = generate_spec();
        let description = openapi_operation(&document, "/v1/soracloud/status", "get")
            .get("description")
            .and_then(Value::as_str)
            .expect("Soracloud status description");

        assert!(description.contains("`configured_lane_count`"));
        assert!(!description.contains("`lane_count`"));
        assert!(!description.contains("legacy"));
    }
    #[test]
    fn canonical_stream_operations_publish_fail_closed_contract() {
        let document = generate_spec();
        let paths = document["paths"].as_object().expect("paths");
        for path in ["/v1/events/sse", "/v1/contracts/events/sse"] {
            let get = paths[path]["get"].as_object().expect("SSE GET operation");
            assert_eq!(
                get.get("x-iroha-replay-supported").and_then(Value::as_bool),
                Some(false),
                "{path} must not advertise replay"
            );
            assert_eq!(
                get.get("x-iroha-lag-behavior").and_then(Value::as_str),
                Some("terminal_stream_error")
            );
            let responses = get["responses"].as_object().expect("SSE responses");
            assert!(responses.contains_key("200"));
            assert!(responses.contains_key("400"));
        }
        for path in [uri::SUBSCRIPTION, uri::BLOCKS_STREAM] {
            let get = paths[path]["get"]
                .as_object()
                .expect("WebSocket GET operation");
            assert_eq!(
                get.get("x-iroha-websocket-subprotocol")
                    .and_then(Value::as_str),
                Some(iroha_torii_shared::NORITO_V1_WEBSOCKET_SUBPROTOCOL)
            );
            assert_eq!(
                get.get("x-iroha-max-subscription-message-bytes")
                    .and_then(Value::as_u64),
                Some(256 * 1024)
            );
            let responses = get["responses"].as_object().expect("WebSocket responses");
            assert!(responses.contains_key("101"));
            assert!(!responses.contains_key("200"));
            assert!(responses.contains_key("400"));
            assert!(responses.contains_key("401"));
        }
    }
    #[test]
    fn retired_alias_voprf_surface_does_not_reappear() {
        fn assert_absent(surface: &str, source: &str, forbidden: &[&str]) {
            for needle in forbidden {
                assert!(
                    !source.contains(needle),
                    "retired alias VOPRF surface `{needle}` reappeared in {surface}"
                );
            }
        }
        assert_absent(
            "Torii runtime",
            include_str!("lib.rs"),
            &["/v1/aliases/voprf/evaluate", "handler_alias_voprf_evaluate"],
        );
        assert_absent(
            "Torii request DTOs",
            include_str!("routing.rs"),
            &[
                "AliasVoprfBackendDto",
                "AliasVoprfEvaluateRequestDto",
                "AliasVoprfEvaluateResponseDto",
            ],
        );
    }
    #[test]
    fn content_route_documents_conditional_cache_and_auth_contract() {
        const PATH: &str = "/v1/content/{bundle}/{path}";
        let document = generate_spec();
        let operation = openapi_operation(&document, PATH, "get");
        let description = operation
            .get("description")
            .and_then(Value::as_str)
            .expect("content operation description");
        for phrase in openapi_contract_strings(
            "openapi.content_route_documents_conditional_cache_and_auth_contract.strings.1",
        ) {
            assert!(
                description.contains(phrase),
                "content operation must document `{phrase}`"
            );
        }
        let parameters = operation
            .get("parameters")
            .and_then(Value::as_array)
            .expect("content parameters");
        let auth_headers = parameters
            .iter()
            .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
            .map(|parameter| {
                (
                    parameter
                        .get("name")
                        .and_then(Value::as_str)
                        .expect("content auth header"),
                    parameter.get("required").and_then(Value::as_bool),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            auth_headers,
            BTreeSet::from([
                ("X-Iroha-Account", Some(false)),
                ("X-Iroha-Nonce", Some(false)),
                ("X-Iroha-Signature", Some(false)),
                ("X-Iroha-Timestamp-Ms", Some(false)),
                ("X-Iroha-Witness", Some(false)),
            ])
        );
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("content responses");
        let success_headers = responses
            .get("200")
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .expect("content success cache headers");
        let cache_description = success_headers
            .get("Cache-Control")
            .and_then(Value::as_object)
            .and_then(|header| header.get("description"))
            .and_then(Value::as_str)
            .expect("content cache-control description");
        assert!(cache_description.contains("Public bundles"));
        assert!(cache_description.contains("private, no-store"));
        assert_eq!(
            success_headers
                .get("Vary")
                .and_then(Value::as_object)
                .and_then(|header| header.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some(crate::content::CANONICAL_CONTENT_AUTH_VARY)
        );
        for [name, expected] in openapi_contract_fixed_rows::<2>(
            "openapi.content_route_documents_conditional_cache_and_auth_contract.rows.1",
        ) {
            assert_eq!(
                success_headers
                    .get(name)
                    .and_then(Value::as_object)
                    .and_then(|header| header.get("schema"))
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("const"))
                    .and_then(Value::as_str),
                Some(expected),
                "content response must document the {name} boundary"
            );
        }
        let unauthorized = responses
            .get("401")
            .and_then(Value::as_object)
            .and_then(|response| response.get("description"))
            .and_then(Value::as_str)
            .expect("content unauthorized description");
        assert!(unauthorized.contains("canonical request authentication"));
        let not_found = responses
            .get("404")
            .and_then(Value::as_object)
            .and_then(|response| response.get("description"))
            .and_then(Value::as_str)
            .expect("content not-found description");
        assert!(not_found.contains("unknown or expired"));
        assert!(not_found.contains("authenticate and authorize before revealing"));
    }
    #[test]
    fn ledger_executed_block_wire_cached_loading_is_safe_from_256_kib_callers() {
        const SMALL_CALLER_STACK_BYTES: usize = 256 * 1024;
        let caller = std::thread::Builder::new()
            .name("openapi-small-stack-regression".to_owned())
            .stack_size(SMALL_CALLER_STACK_BYTES)
            .spawn(|| {
                let compiled = generate_spec();
                let rendered = compiled_spec_json();
                let reparsed: Value =
                    norito::json::from_str(rendered).expect("cached OpenAPI JSON must parse");
                assert_eq!(reparsed, compiled);
                for (variant, document) in [("owned", &compiled), ("borrowed", compiled_spec())] {
                    let operation = openapi_operation(document, "/v1/ledger/block/{height}", "get");
                    assert_eq!(
                        operation.get("operationId").and_then(Value::as_str),
                        Some("ledgerExecutedBlockWire"),
                        "missing canonical executed-block operation in {variant} OpenAPI",
                    );
                }
                #[cfg(feature = "app_api")]
                {
                    for (variant, document) in [("owned", &compiled), ("borrowed", compiled_spec())]
                    {
                        let paths = document
                            .get("paths")
                            .and_then(Value::as_object)
                            .unwrap_or_else(|| panic!("{variant} OpenAPI paths"));
                        assert!(
                            paths.contains_key("/v1/kagemusha/readiness"),
                            "universal offline capability route missing from {variant} OpenAPI",
                        );
                    }
                }
            })
            .expect("spawn adversarial small-stack OpenAPI caller");
        if let Err(payload) = caller.join() {
            std::panic::resume_unwind(payload);
        }
    }
    #[test]
    fn generated_spec_includes_documented_paths() {
        let doc = generate_spec();
        if std::env::var("PRINT_TORII_SPEC").is_ok() {
            if let Ok(json) = norito::json::to_string_pretty(&doc) {
                println!("{json}");
            }
        }
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        assert!(!paths.contains_key("/v1/aliases/voprf/evaluate"));
        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas section");
        for retired_schema in ["AliasVoprfEvaluateRequest", "AliasVoprfEvaluateResponse"] {
            assert!(
                !schemas.contains_key(retired_schema),
                "retired alias VOPRF schema {retired_schema} reappeared"
            );
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.1",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(!paths.contains_key("/v1/fee-sponsor-policies/by-id"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.2",
        ) {
            assert!(paths.contains_key(path));
        }
        for path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.1")
        {
            assert_eq!(
                paths.contains_key(path),
                catalog_openapi_route_enabled(CatalogHttpMethod::Get, path),
                "{path} presence must follow the enabled catalog OpenAPI projection"
            );
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.3",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(paths.contains_key(
            "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material"
        ));
        assert!(paths.contains_key("/v1/bridge/proofs/submit"));
        assert!(paths.contains_key("/v1/bridge/messages"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.4",
        ) {
            assert!(!paths.contains_key(path));
        }
        for retired in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.2")
        {
            assert!(
                !paths.contains_key(retired),
                "retired path {retired} leaked"
            );
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.5",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(paths.contains_key(uri::TRANSACTION));
        assert!(paths.contains_key(uri::TRANSACTION_ENTRYPOINT));
        assert!(paths.contains_key(uri::TRANSACTIONS_BATCH));
        assert!(paths.contains_key(uri::QUERY));
        assert!(paths.contains_key(uri::SUBSCRIPTION));
        #[cfg(feature = "schema")]
        assert!(paths.contains_key(uri::SCHEMA));
        #[cfg(not(feature = "schema"))]
        assert!(!paths.contains_key(uri::SCHEMA));
        #[cfg(feature = "profiling")]
        assert!(paths.contains_key(uri::PROFILE));
        #[cfg(not(feature = "profiling"))]
        assert!(!paths.contains_key(uri::PROFILE));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.6",
        ) {
            assert!(!paths.contains_key(path));
        }
        let da_ingest_responses = paths
            .get("/v1/da/ingest")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("responses"))
            .and_then(Value::as_object)
            .expect("DA ingest response map");
        assert!(da_ingest_responses.contains_key("202"));
        assert!(!da_ingest_responses.contains_key("200"));
        #[cfg(feature = "connect")]
        assert!(paths.contains_key("/v1/connect/session"));
        #[cfg(not(feature = "connect"))]
        assert!(!paths.contains_key("/v1/connect/session"));
        assert!(paths.contains_key("/v1/vpn/profile"));
        assert!(paths.contains_key("/v1/vpn/quotes"));
        let vpn_quotes_post_description = paths
            .get("/v1/vpn/quotes")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("description"))
            .and_then(Value::as_str)
            .expect("vpn quote create description");
        assert!(vpn_quotes_post_description.contains("metering_public_key_hex"));
        assert!(vpn_quotes_post_description.contains("OpenVpnLeaseEscrow"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.7",
        ) {
            assert!(paths.contains_key(path));
        }
        let vpn_receipts_post_description = paths
            .get("/v1/vpn/receipts")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("description"))
            .and_then(Value::as_str)
            .expect("vpn receipt submit description");
        assert!(vpn_receipts_post_description.contains("settle_lease_instruction"));
        assert!(vpn_receipts_post_description.contains("SettleVpnLease"));
        assert!(paths.contains_key("/v1/mcp"));
        assert!(paths.contains_key("/v1/zk/attachments"));
        let verifying_key_get_description = paths
            .get("/v1/zk/vk/{backend}/{name}")
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .and_then(|get| get.get("description"))
            .and_then(Value::as_str)
            .expect("verifying-key detail description");
        assert!(verifying_key_get_description.contains("record_norito_base64"));
        assert!(verifying_key_get_description.contains("namespace"));
        assert!(verifying_key_get_description.contains("owner_manifest_id"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.8",
        ) {
            assert!(paths.contains_key(path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.9",
        ) {
            assert!(!paths.contains_key(path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.10",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(paths.contains_key(iroha_torii_shared::uri::GOV_PROPOSE_SCCP_ROUTE_GOVERNANCE));
        assert!(paths.contains_key(iroha_torii_shared::uri::GOV_CAPABILITIES));
        assert!(paths.contains_key(iroha_torii_shared::uri::GOV_CITIZEN_DRAFT));
        assert!(paths.contains_key("/v1/gov/citizens"));
        assert!(paths.contains_key("/v1/gov/stream"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.11",
        ) {
            assert!(!paths.contains_key(path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.12",
        ) {
            assert!(paths.contains_key(path));
        }
        let reputation_latest = paths
            .get("/v1/sorafs/reputation/latest")
            .and_then(Value::as_object)
            .expect("reputation latest OpenAPI operation");
        assert!(reputation_latest.contains_key("get"));
        assert!(!reputation_latest.contains_key("post"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.13",
        ) {
            assert!(paths.contains_key(path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.14",
        ) {
            assert!(!paths.contains_key(path));
        }
        for repair_command_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.3")
        {
            assert!(paths.contains_key(repair_command_path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.15",
        ) {
            assert!(paths.contains_key(path));
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_absent.16",
        ) {
            assert!(!paths.contains_key(path));
        }
        for unsupported_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.4")
        {
            assert!(
                !paths.contains_key(unsupported_path),
                "unsupported path leaked into OpenAPI: {unsupported_path}"
            );
        }
        for live_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.5")
        {
            assert!(
                paths.contains_key(live_path),
                "live PoR route missing from OpenAPI: {live_path}"
            );
        }
        assert!(paths.contains_key("/v1/sorafs/appeals/pricing/config"));
        assert!(paths.contains_key("/v1/sorafs/appeals/pricing/status"));
        let appeal_pricing_status_description = paths
            .get("/v1/sorafs/appeals/pricing/status")
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .and_then(|get| get.get("description"))
            .and_then(Value::as_str)
            .expect("appeal pricing status description");
        assert!(appeal_pricing_status_description.contains("native deposit lifecycle"));
        assert!(
            appeal_pricing_status_description
                .contains("durable finalized-ledger transaction forwarder")
        );
        assert!(appeal_pricing_status_description.contains("runtime-only signer providers"));
        assert!(
            appeal_pricing_status_description.contains(
                "hosted dashboard, and four-peer rollout evidence remain promotion gates"
            )
        );
        let stale_pending_runtime_phrase =
            ["still pending runtime escrow", " and ledger integration"].concat();
        assert!(!appeal_pricing_status_description.contains(&stale_pending_runtime_phrase));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.17",
        ) {
            assert!(paths.contains_key(path));
        }
        for publication_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.6")
        {
            let operation = paths
                .get(publication_path)
                .and_then(Value::as_object)
                .expect("appeal-finance publication readback path item");
            assert!(operation.contains_key("get"), "{publication_path}");
            let post = operation
                .get("post")
                .and_then(Value::as_object)
                .expect("authenticated appeal-finance publication operation");
            assert!(
                post.get("security")
                    .and_then(Value::as_array)
                    .is_some_and(|requirements| !requirements.is_empty()),
                "appeal-finance publication must require canonical authentication: {publication_path}"
            );
            let responses = post
                .get("responses")
                .and_then(Value::as_object)
                .expect("appeal-finance publication responses");
            assert!(responses.contains_key("202"), "{publication_path}");
            assert!(!responses.contains_key("200"), "{publication_path}");
        }
        assert!(paths.contains_key("/v1/sorafs/transparency/cycles"));
        assert!(paths.contains_key("/v1/sorafs/transparency/cycles/{cycle_id_hex}"));
        assert!(
            paths.contains_key(
                "/v1/sorafs/transparency/cycles/{cycle_id_hex}/entries/{entry_id_hex}"
            )
        );
        assert!(paths.contains_key("/v1/sorafs/transparency/explorer"));
        assert!(paths.contains_key("/v1/sorafs/transparency/explorer/ui"));
        assert!(!paths.contains_key("/v1/sorafs/transparency/source-entries/{source_kind}"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.18",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(
            paths.contains_key("/v1/sorafs/moderation/ballots/{case_id}/{round_id}/no-show-plan")
        );
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.19",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(
            paths.contains_key(
                "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff"
            )
        );
        assert!(
            paths.contains_key(
                "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel"
            )
        );
        assert!(paths.contains_key("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/object"));
        for evidence_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.7")
        {
            assert!(
                paths.contains_key(evidence_path),
                "missing production evidence-viewer route {evidence_path}"
            );
        }
        assert!(
            !paths.contains_key(
                "/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-sessions"
            )
        );
        assert!(
            !paths
                .contains_key("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-access")
        );
        for retired_path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.8")
        {
            assert!(
                !paths.contains_key(retired_path),
                "retired evidence-viewer audit route leaked into OpenAPI: {retired_path}"
            );
        }
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.20",
        ) {
            assert!(paths.contains_key(path));
        }
        assert!(!paths.contains_key("/v1/sns/names"));
        assert!(!paths.contains_key("/v1/sns/names/{namespace}/{literal}/renew"));
        for path in openapi_contract_strings(
            "openapi.generated_spec_includes_documented_paths.path_present.21",
        ) {
            let expected = path != "/v1/soranet/privacy/event" || cfg!(feature = "telemetry");
            assert_eq!(
                paths.contains_key(path),
                expected,
                "feature-pruned path contract drift for {path}"
            );
        }
        for path in
            openapi_contract_strings("openapi.generated_spec_includes_documented_paths.strings.9")
        {
            assert!(
                paths.contains_key(path),
                "missing final offline route {path}"
            );
        }
        assert!(!paths.contains_key("/v1/attestation/issue"));
        let topup_post = paths
            .get("/v1/kagemusha/top-up")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("offline top-up post operation");
        let topup_description = topup_post
            .get("description")
            .and_then(Value::as_str)
            .expect("offline top-up description");
        assert!(topup_description.contains("pooled Kagemusha V1 reserve"));
        let redeem_post = paths
            .get("/v1/kagemusha/redeem")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("offline redeem post operation");
        let redeem_description = redeem_post
            .get("description")
            .and_then(Value::as_str)
            .expect("offline redeem description");
        assert!(redeem_description.contains("redemption voucher"));
        let topup_request_content = topup_post
            .get("requestBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("content"))
            .and_then(Value::as_object)
            .expect("Kagemusha V1 top-up request content");
        assert_eq!(
            topup_request_content
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["application/x-norito"]
        );
        let topup_norito_schema = topup_request_content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .expect("typed top-up Norito schema");
        assert_eq!(
            topup_norito_schema
                .get("x-iroha-norito-schema")
                .and_then(Value::as_str),
            Some(iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1)
        );
        assert_eq!(
            topup_norito_schema
                .get("x-iroha-max-bytes")
                .and_then(Value::as_u64),
            Some(iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1 as u64)
        );
        let redeem_request_content = redeem_post
            .get("requestBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("content"))
            .and_then(Value::as_object)
            .expect("Kagemusha V1 redeem request content");
        assert_eq!(
            redeem_request_content
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["application/x-norito"]
        );
        let redeem_norito_schema = redeem_request_content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .expect("typed redeem Norito schema");
        assert_eq!(
            redeem_norito_schema
                .get("x-iroha-norito-schema")
                .and_then(Value::as_str),
            Some(iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1)
        );
        assert_eq!(
            redeem_norito_schema
                .get("x-iroha-max-bytes")
                .and_then(Value::as_u64),
            Some(
                iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1
                    as u64
            )
        );
        let accepted = topup_post
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("202"))
            .and_then(Value::as_object)
            .expect("offline top-up accepted response");
        let accepted_headers = accepted
            .get("headers")
            .and_then(Value::as_object)
            .expect("offline top-up accepted headers");
        assert!(accepted_headers.contains_key("Location"));
        assert!(accepted_headers.contains_key("Retry-After"));
    }
    #[test]
    fn generated_spec_exposes_only_kagemusha_v1() {
        let document = generate_spec();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        let schemas = component_schemas(&document);

        assert_eq!(
            schemas
                .get("KagemushaReadinessV1")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("required"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(4)
        );
        assert!(schemas.contains_key("KagemushaOperationStatusV1"));
        assert!(
            schemas
                .keys()
                .all(|name| !name.starts_with("KagemushaRecipient"))
        );

        let readiness = paths["/v1/kagemusha/readiness"]["get"]
            .as_object()
            .expect("readiness operation");
        assert_eq!(
            operation_response_schema_ref(readiness, "200", "/v1/kagemusha/readiness"),
            "#/components/schemas/KagemushaReadinessV1"
        );
        assert!(
            readiness["description"]
                .as_str()
                .is_some_and(|description| description.contains("no hop"))
        );

        for (path, request_schema, request_maximum) in [
            (
                "/v1/kagemusha/top-up",
                iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1,
                iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
            ),
            (
                "/v1/kagemusha/redeem",
                iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
                iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
            ),
        ] {
            let operation = paths[path]["post"].as_object().expect("offline operation");
            let wire = &operation["requestBody"]["content"]["application/x-norito"]["schema"];
            assert_eq!(wire["x-iroha-norito-schema"].as_str(), Some(request_schema));
            assert_eq!(
                wire["x-iroha-max-bytes"].as_u64(),
                Some(request_maximum as u64)
            );
            assert_eq!(
                operation_response_schema_ref(operation, "202", path),
                "#/components/schemas/KagemushaOperationStatusV1"
            );
        }
    }
    #[test]
    fn musubi_v1_openapi_matches_the_complete_catalog_and_declares_models() {
        let document = generate_spec();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        let schemas = component_schemas(&document);
        let actual = paths
            .keys()
            .map(String::as_str)
            .filter(|path| path.starts_with("/v1/musubi/"))
            .collect::<BTreeSet<_>>();
        let expected = musubi_routes::ROUTES
            .iter()
            .map(|route| route.path())
            .collect::<BTreeSet<_>>();
        assert_eq!(musubi_routes::ROUTES.len(), 31);
        assert_eq!(actual, expected);
        let mut schema_roots = BTreeSet::new();
        for route in musubi_routes::ROUTES {
            let path = route.path();
            let path_item = paths
                .get(path)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("Musubi OpenAPI path {path}"));
            assert_eq!(
                path_item.keys().map(String::as_str).collect::<Vec<_>>(),
                vec!["post"],
                "{path} must remain POST-only"
            );
            let operation = path_item
                .get("post")
                .and_then(Value::as_object)
                .expect("Musubi POST operation");
            let request_type = operation
                .get("x-iroha-norito-request-type")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{path} exact request type"));
            let response_type = operation
                .get("x-iroha-norito-response-type")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{path} exact response type"));
            let request_schema_reference = operation
                .get("requestBody")
                .and_then(Value::as_object)
                .and_then(|request_body| request_body.get("content"))
                .and_then(Value::as_object)
                .and_then(|content| content.get("application/json"))
                .and_then(Value::as_object)
                .and_then(|media| media.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str);
            let response_schema_reference = operation
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("200"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("content"))
                .and_then(Value::as_object)
                .and_then(|content| content.get("application/json"))
                .and_then(Value::as_object)
                .and_then(|media| media.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str);
            for (model_type, schema_reference) in [
                (request_type, request_schema_reference),
                (response_type, response_schema_reference),
            ] {
                assert!(model_type.ends_with("V1"), "{path} exact V1 model");
                let expected_reference = format!("{COMPONENT_SCHEMA_REF_PREFIX}{model_type}");
                assert_eq!(
                    schema_reference,
                    Some(expected_reference.as_str()),
                    "{path} must reference its declared exact model"
                );
                let schema = schemas
                    .get(model_type)
                    .and_then(Value::as_object)
                    .unwrap_or_else(|| panic!("{path} component schema {model_type}"));
                assert_eq!(schema.get("type").and_then(Value::as_str), Some("object"));
                assert_eq!(
                    schema.get("additionalProperties").and_then(Value::as_bool),
                    Some(false),
                    "{path} component schema {model_type} must reject unknown fields"
                );
                schema_roots.insert(model_type.to_owned());
            }
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some(if path.starts_with("/v1/musubi/queries/") {
                    "read"
                } else {
                    "build_instruction"
                }),
                "{path} tool effect"
            );
        }
        let mut pending = schema_roots.into_iter().collect::<VecDeque<_>>();
        let mut visited = BTreeSet::new();
        while let Some(schema_name) = pending.pop_front() {
            if !visited.insert(schema_name.clone()) {
                continue;
            }
            assert_ne!(schema_name, "JsonValue", "Musubi schemas must stay typed");
            let schema = schemas
                .get(&schema_name)
                .unwrap_or_else(|| panic!("missing Musubi component schema {schema_name}"));
            let mut values = vec![schema];
            while let Some(value) = values.pop() {
                match value {
                    Value::Object(object) => {
                        if object.get("type").and_then(Value::as_str) == Some("object")
                            || object.contains_key("properties")
                        {
                            assert_eq!(
                                object.get("additionalProperties").and_then(Value::as_bool),
                                Some(false),
                                "Musubi schema {schema_name} contains an open object"
                            );
                        }
                        if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                            let referenced_name = reference
                                .strip_prefix(COMPONENT_SCHEMA_REF_PREFIX)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "Musubi schema {schema_name} uses a non-local reference {reference}"
                                    )
                                });
                            assert!(
                                schemas.contains_key(referenced_name),
                                "Musubi schema {schema_name} references missing component {referenced_name}"
                            );
                            pending.push_back(referenced_name.to_owned());
                        }
                        values.extend(object.values());
                    }
                    Value::Array(items) => values.extend(items),
                    _ => {}
                }
            }
        }
    }
    #[test]
    fn musubi_instruction_previews_discriminate_equal_payload_shapes_by_wire_id() {
        use iroha_data_model::isi::musubi::{
            AcceptMusubiPackageMaintainerV1, RevokeMusubiPackageMaintainerInvitationV1,
        };
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let variants = schemas
            .get("MusubiInstructionPreviewV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("oneOf"))
            .and_then(Value::as_array)
            .expect("Musubi instruction preview variants");
        assert_eq!(variants.len(), 19);
        let mut bindings = BTreeSet::new();
        let mut wire_ids = BTreeSet::new();
        for variant in variants {
            let variant = variant.as_object().expect("closed preview variant");
            assert_eq!(
                variant.get("additionalProperties").and_then(Value::as_bool),
                Some(false)
            );
            let properties = variant
                .get("properties")
                .and_then(Value::as_object)
                .expect("preview variant properties");
            let wire_id = properties
                .get("wire_id")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str)
                .expect("preview variant wire id");
            let payload = properties
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str)
                .expect("preview variant payload reference");
            assert!(
                bindings.insert((wire_id.to_owned(), payload.to_owned())),
                "preview variants must not repeat a wire-id/payload binding"
            );
            assert!(
                wire_ids.insert(wire_id.to_owned()),
                "preview variants must use distinct wire ids"
            );
        }
        assert!(bindings.contains(&(
            AcceptMusubiPackageMaintainerV1::WIRE_ID.to_owned(),
            format!("{COMPONENT_SCHEMA_REF_PREFIX}AcceptMusubiPackageMaintainerV1"),
        )));
        assert!(bindings.contains(&(
            RevokeMusubiPackageMaintainerInvitationV1::WIRE_ID.to_owned(),
            format!("{COMPONENT_SCHEMA_REF_PREFIX}RevokeMusubiPackageMaintainerInvitationV1"),
        )));
        assert_ne!(
            AcceptMusubiPackageMaintainerV1::WIRE_ID,
            RevokeMusubiPackageMaintainerInvitationV1::WIRE_ID
        );
        let envelope_wire_ids = schemas
            .get("MusubiInstructionEnvelopeV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("wire_id"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .expect("Musubi instruction envelope wire ids")
            .iter()
            .map(|wire_id| wire_id.as_str().expect("wire id").to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(envelope_wire_ids, wire_ids);
    }
    #[test]
    fn musubi_crypto_text_schemas_do_not_impose_single_key_size_limits() {
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let account = schemas
            .get("MusubiAccountIdV1")
            .and_then(Value::as_object)
            .expect("Musubi account schema");
        assert!(
            !account.contains_key("maxLength"),
            "native multisignature AccountIds are bounded by their enclosing body"
        );
        let approval = schemas
            .get("MusubiControllerApprovalV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("Musubi controller approval properties");
        for field in ["public_key", "signature"] {
            assert!(
                approval
                    .get(field)
                    .and_then(Value::as_object)
                    .is_some_and(|schema| !schema.contains_key("maxLength")),
                "Musubi approval {field} must admit native post-quantum text encodings"
            );
        }
        assert_eq!(
            approval
                .get("signature")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some("^(?:[0-9A-Fa-f]{2})+$")
        );
        let provider_id = schemas
            .get("MusubiProviderIdV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("items"))
            .and_then(Value::as_object)
            .expect("Musubi provider-id hex item");
        assert_eq!(
            provider_id.get("pattern").and_then(Value::as_str),
            Some("^[0-9A-Fa-f]{64}$")
        );
    }
    #[test]
    fn musubi_cursor_and_ordered_prefix_bounds_match_the_wire_types() {
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let cursor_last_key = schemas
            .get("MusubiFinalizedCursorV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("last_key"))
            .and_then(Value::as_object)
            .expect("Musubi finalized-cursor last-key schema");
        assert_eq!(
            cursor_last_key.get("maxLength").and_then(Value::as_u64),
            Some(
                u64::try_from(iroha_data_model::musubi::MUSUBI_MAX_CURSOR_KEY_BYTES_V1)
                    .expect("cursor-key bound fits u64")
            )
        );
        let ordered_prefix = schemas
            .get("MusubiOrderedPrefixV1")
            .and_then(Value::as_object)
            .expect("Musubi ordered-prefix schema");
        assert_eq!(
            ordered_prefix.get("maxLength").and_then(Value::as_u64),
            Some(
                u64::try_from(iroha_data_model::musubi::MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1)
                    .expect("ordered-prefix bound fits u64")
            )
        );
    }
    #[test]
    fn musubi_chunker_text_bounds_match_the_wire_type() {
        let document = generate_spec();
        let schemas = component_schemas(&document);
        let chunker = schemas
            .get("MusubiChunkerProfileHandleV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("Musubi chunker-handle properties");
        for field in ["namespace", "name", "semver"] {
            assert_eq!(
                chunker
                    .get(field)
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("maxLength"))
                    .and_then(Value::as_u64),
                Some(128),
                "the per-field bound must not exclude a valid 128-byte total handle"
            );
        }
    }
    #[test]
    fn multisig_propose_schema_exposes_optional_validation_fee_bindings_as_strings() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        let request = schemas
            .get("MultisigProposeRequest")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("allOf"))
            .and_then(Value::as_array)
            .and_then(|branches| branches.get(1))
            .and_then(Value::as_object)
            .expect("MultisigProposeRequest inline schema");
        let properties = request
            .get("properties")
            .and_then(Value::as_object)
            .expect("MultisigProposeRequest properties");
        let required = request
            .get("required")
            .and_then(Value::as_array)
            .expect("MultisigProposeRequest required fields");

        for field in [
            "validation_fee_policy_version",
            "validation_fee_policy_hash",
            "validation_fee_hijiri_fee_quote_hash",
            "validation_fee_instruction_index",
            "validation_fee_transfer_entry_index",
        ] {
            let property = properties
                .get(field)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("MultisigProposeRequest.{field}"));
            assert_eq!(property.get("type").and_then(Value::as_str), Some("string"));
            assert!(
                property
                    .get("description")
                    .and_then(Value::as_str)
                    .is_some_and(|description| !description.is_empty()),
                "MultisigProposeRequest.{field} description"
            );
            assert!(
                !required
                    .iter()
                    .any(|required_field| required_field.as_str() == Some(field)),
                "MultisigProposeRequest.{field} must remain optional"
            );
        }
        for field in [
            "validation_fee_policy_hash",
            "validation_fee_hijiri_fee_quote_hash",
        ] {
            let property = properties[field]
                .as_object()
                .expect("validation-fee hash schema");
            assert_eq!(property.get("minLength").and_then(Value::as_u64), Some(64));
            assert_eq!(property.get("maxLength").and_then(Value::as_u64), Some(64));
            assert_eq!(
                property.get("pattern").and_then(Value::as_str),
                Some("^[0-9a-f]{64}$")
            );
        }
        for field in [
            "validation_fee_policy_version",
            "validation_fee_instruction_index",
            "validation_fee_transfer_entry_index",
        ] {
            let property = properties[field]
                .as_object()
                .expect("validation-fee decimal u64 schema");
            assert_eq!(property.get("minLength").and_then(Value::as_u64), Some(1));
            assert_eq!(property.get("maxLength").and_then(Value::as_u64), Some(20));
            assert_eq!(
                property.get("pattern").and_then(Value::as_str),
                Some("^(?:0|[1-9][0-9]*)$")
            );
        }
    }
    #[test]
    fn multisig_cancel_response_requires_typed_fee_payment_property() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        let response = schemas
            .get("MultisigCancelResponse")
            .and_then(Value::as_object)
            .expect("MultisigCancelResponse schema");
        let properties = response
            .get("properties")
            .and_then(Value::as_object)
            .expect("MultisigCancelResponse properties");
        let required = response
            .get("required")
            .and_then(Value::as_array)
            .expect("MultisigCancelResponse required fields");

        assert_eq!(
            properties
                .get("fee_payment")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/FeePaymentIntent")
        );
        assert!(
            required
                .iter()
                .any(|field| field.as_str() == Some("fee_payment")),
            "MultisigCancelResponse.fee_payment must remain required"
        );
    }
    #[test]
    fn multisig_propose_instruction_schema_matches_native_norito_json() {
        let document = canonical_document();
        let schemas = component_schemas(&document);
        let instruction = schemas
            .get("MultisigProposeInstructionInput")
            .and_then(Value::as_object)
            .expect("MultisigProposeInstructionInput schema");

        assert_eq!(
            instruction.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            instruction.get("contentEncoding").and_then(Value::as_str),
            Some("base64")
        );
        assert_eq!(
            instruction.get("minLength").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            instruction.get("pattern").and_then(Value::as_str),
            Some("^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$")
        );
    }
    #[test]
    fn generated_operations_declare_tool_effects() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        for (path, path_item) in paths {
            let path_map = path_item.as_object().expect("path item object");
            for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
                let Some(operation) = path_map.get(method).and_then(Value::as_object) else {
                    continue;
                };
                let effect = operation
                    .get(TOOL_EFFECT_EXTENSION)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| {
                        panic!("{method} {path} must declare {TOOL_EFFECT_EXTENSION}")
                    });
                assert!(
                    matches!(effect, "read" | "write" | "operator" | "build_instruction"),
                    "{method} {path} declared invalid effect {effect}"
                );
            }
        }
        let query = paths
            .get(uri::QUERY)
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("query post operation");
        assert_eq!(
            query.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("read")
        );
        for path in
            openapi_contract_strings("openapi.generated_operations_declare_tool_effects.strings.1")
        {
            let operation = paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|path| path.get("post"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing multisig proposal read operation: {path}"));
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some("read"),
                "{path} must retain unsigned/read semantics"
            );
        }
        assert!(!paths.contains_key("/v1/multisig/proposals/list"));
        assert!(!paths.contains_key("/v1/multisig/proposals/get"));
        assert!(!paths.contains_key("/v1/multisig/proposals/search"));
        assert!(!paths.contains_key("/v1/sumeragi/pacemaker"));
        let protected_namespaces = paths
            .get("/v1/gov/protected-namespaces")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("protected namespaces post operation");
        assert_eq!(
            protected_namespaces
                .get(TOOL_EFFECT_EXTENSION)
                .and_then(Value::as_str),
            Some("operator")
        );
        for route in RouteCatalog::new(CATALOGED_ROUTES)
            .project(
                iroha_torii_shared::route_catalog::CatalogProjection::OpenApi,
                crate::router::builder::compiled_route_features(),
            )
            .into_iter()
            .filter(|route| {
                route.method() == CatalogHttpMethod::Get && route.surface() == ApiSurface::Operator
            })
        {
            let operation = paths
                .get(route.path())
                .and_then(Value::as_object)
                .and_then(|path| path.get("get"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing operator GET operation: {}", route.path()));
            assert_eq!(
                operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
                Some("operator"),
                "operator GET must retain operator-only effect: {}",
                route.path()
            );
        }
        let musubi_publish = paths
            .get("/v1/musubi/instructions/release-publish")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("Musubi publish instruction operation");
        assert_eq!(
            musubi_publish
                .get(TOOL_EFFECT_EXTENSION)
                .and_then(Value::as_str),
            Some("build_instruction")
        );
        let musubi_resolver = paths
            .get("/v1/musubi/queries/resolver-index")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("Musubi resolver-index query operation");
        assert_eq!(
            musubi_resolver
                .get(TOOL_EFFECT_EXTENSION)
                .and_then(Value::as_str),
            Some("read")
        );
        for legacy_path in
            openapi_contract_strings("openapi.generated_operations_declare_tool_effects.strings.2")
        {
            assert!(
                !paths.contains_key(legacy_path),
                "legacy path survived: {legacy_path}"
            );
        }
    }
    #[test]
    fn sumeragi_evidence_audit_contract_is_closed_and_bounded() {
        use iroha_torii_shared::sumeragi_evidence_api::{
            SUMERAGI_EVIDENCE_COUNT_RESPONSE_MAX_BYTES,
            SUMERAGI_EVIDENCE_COUNT_RESPONSE_SCHEMA_NAME_V1,
            SUMERAGI_EVIDENCE_LIST_JSON_RESPONSE_MAX_BYTES,
            SUMERAGI_EVIDENCE_LIST_NORITO_RESPONSE_MAX_BYTES,
            SUMERAGI_EVIDENCE_LIST_WIRE_RESPONSE_SCHEMA_NAME_V1,
        };

        const LIST_PATH: &str = "/v1/sumeragi/evidence";
        const COUNT_PATH: &str = "/v1/sumeragi/evidence/count";
        let assert_vary_accept = |response: &Value, label: &str| {
            let vary = &response["headers"]["Vary"];
            assert_eq!(vary["required"].as_bool(), Some(true), "{label} Vary");
            assert_eq!(
                vary["schema"]["const"].as_str(),
                Some("Accept"),
                "{label} Vary value"
            );
        };
        let assert_not_acceptable = |operation: &Map, label: &str| {
            let response = &operation["responses"]["406"];
            let content = response["content"]
                .as_object()
                .unwrap_or_else(|| panic!("{label} 406 content"));
            assert_eq!(
                content.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                ["application/json"].into_iter().collect(),
                "{label} 406 media types"
            );
            assert_eq!(
                content["application/json"]["schema"]["$ref"].as_str(),
                Some("#/components/schemas/ErrorEnvelope"),
                "{label} 406 schema"
            );
            assert_vary_accept(response, label);
            assert_eq!(
                response["headers"]["Cache-Control"]["required"].as_bool(),
                Some(true),
                "{label} 406 cache policy"
            );
            assert_eq!(
                response["headers"]["Cache-Control"]["schema"]["const"].as_str(),
                Some("private, no-store"),
                "{label} 406 cache policy value"
            );
        };
        let canonical = canonical_document();
        let compiled = generate_spec();
        for (label, document) in [("canonical", &canonical), ("compiled", &compiled)] {
            let list = openapi_operation(document, LIST_PATH, "get");
            let list_description = list
                .get("description")
                .and_then(Value::as_str)
                .expect("evidence-list description");
            assert!(list_description.contains("committed"));
            assert!(list_description.contains("node-local pending"));
            assert_eq!(
                operation_response_schema_ref(list, "200", LIST_PATH),
                "#/components/schemas/SumeragiEvidenceListResponse",
                "{label} evidence-list response"
            );
            let list_success = &list["responses"]["200"];
            let list_content = list_success["content"]
                .as_object()
                .expect("evidence-list success content");
            assert_eq!(
                list_content
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                ["application/json", "application/x-norito"]
                    .into_iter()
                    .collect(),
                "{label} evidence-list media types"
            );
            assert_eq!(
                list_content["application/json"]["schema"]["x-iroha-max-bytes"].as_u64(),
                Some(SUMERAGI_EVIDENCE_LIST_JSON_RESPONSE_MAX_BYTES as u64),
                "{label} evidence-list JSON cap"
            );
            let list_norito = &list_content["application/x-norito"]["schema"];
            assert_eq!(list_norito["type"].as_str(), Some("string"));
            assert_eq!(list_norito["format"].as_str(), Some("binary"));
            assert_eq!(
                list_norito["x-iroha-norito-schema"].as_str(),
                Some(SUMERAGI_EVIDENCE_LIST_WIRE_RESPONSE_SCHEMA_NAME_V1)
            );
            assert_eq!(
                list_norito["x-iroha-max-bytes"].as_u64(),
                Some(SUMERAGI_EVIDENCE_LIST_NORITO_RESPONSE_MAX_BYTES as u64)
            );
            assert!(
                list_norito["description"]
                    .as_str()
                    .is_some_and(|description| description
                        .contains("SumeragiEvidenceListWireResponse")
                        && description.contains("Vec<EvidenceRecord>"))
            );
            assert_vary_accept(list_success, &format!("{label} evidence-list 200"));
            assert_not_acceptable(list, &format!("{label} evidence-list"));
            let parameters = list
                .get("parameters")
                .and_then(Value::as_array)
                .expect("evidence-list query parameters");
            assert_eq!(parameters.len(), 3, "{label} evidence-list parameter count");
            let parameter = |name: &str| {
                parameters
                    .iter()
                    .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
                    .and_then(Value::as_object)
                    .unwrap_or_else(|| panic!("{label} evidence-list `{name}` parameter"))
            };
            let limit = parameter("limit");
            assert_eq!(limit.get("in").and_then(Value::as_str), Some("query"));
            let limit = limit
                .get("schema")
                .and_then(Value::as_object)
                .expect("evidence-list limit schema");
            assert_eq!(limit.get("minimum").and_then(Value::as_u64), Some(1));
            assert_eq!(limit.get("maximum").and_then(Value::as_u64), Some(1_000));
            assert_eq!(limit.get("default").and_then(Value::as_u64), Some(50));
            let offset = parameter("offset")
                .get("schema")
                .and_then(Value::as_object)
                .expect("evidence-list offset schema");
            assert_eq!(offset.get("minimum").and_then(Value::as_u64), Some(0));
            assert_eq!(offset.get("maximum").and_then(Value::as_u64), Some(10_000));
            assert_eq!(offset.get("default").and_then(Value::as_u64), Some(0));
            let kind = parameter("kind")
                .get("schema")
                .and_then(Value::as_object)
                .expect("evidence-list kind schema");
            assert_eq!(
                kind.get("enum").and_then(Value::as_array),
                Some(&vec![Value::from("SumeragiV2Equivocation")])
            );
            let count = openapi_operation(document, COUNT_PATH, "get");
            let count_description = count
                .get("description")
                .and_then(Value::as_str)
                .expect("evidence-count description");
            assert!(count_description.contains("committed"));
            assert!(count_description.contains("node-local pending"));
            assert_eq!(
                operation_response_schema_ref(count, "200", COUNT_PATH),
                "#/components/schemas/SumeragiEvidenceCountResponse",
                "{label} evidence-count response"
            );
            let count_success = &count["responses"]["200"];
            let count_content = count_success["content"]
                .as_object()
                .expect("evidence-count success content");
            assert_eq!(
                count_content
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                ["application/json", "application/x-norito"]
                    .into_iter()
                    .collect(),
                "{label} evidence-count media types"
            );
            assert_eq!(
                count_content["application/json"]["schema"]["x-iroha-max-bytes"].as_u64(),
                Some(SUMERAGI_EVIDENCE_COUNT_RESPONSE_MAX_BYTES as u64),
                "{label} evidence-count JSON cap"
            );
            let count_norito = &count_content["application/x-norito"]["schema"];
            assert_eq!(count_norito["type"].as_str(), Some("string"));
            assert_eq!(count_norito["format"].as_str(), Some("binary"));
            assert_eq!(
                count_norito["x-iroha-norito-schema"].as_str(),
                Some(SUMERAGI_EVIDENCE_COUNT_RESPONSE_SCHEMA_NAME_V1)
            );
            assert_eq!(
                count_norito["x-iroha-max-bytes"].as_u64(),
                Some(SUMERAGI_EVIDENCE_COUNT_RESPONSE_MAX_BYTES as u64)
            );
            assert_vary_accept(count_success, &format!("{label} evidence-count 200"));
            assert_not_acceptable(count, &format!("{label} evidence-count"));
        }

        let schemas = component_schemas(&canonical);
        assert_strict_object_schema(
            schemas,
            "SumeragiEvidenceAuditRecord",
            &[
                "kind",
                "class",
                "height",
                "view",
                "epoch",
                "signer",
                "context_id",
                "artifact_hash_1",
                "artifact_hash_2",
                "recorded_height",
                "recorded_view",
                "recorded_ms",
                "consensus_admitted_height",
                "penalty_status",
            ],
            &[],
        );
        assert_strict_object_schema(
            schemas,
            "SumeragiEvidenceListResponse",
            &["total", "items"],
            &[],
        );
        assert_strict_object_schema(schemas, "SumeragiEvidenceCountResponse", &["count"], &[]);
        let record = schemas
            .get("SumeragiEvidenceAuditRecord")
            .and_then(Value::as_object)
            .expect("Sumeragi evidence audit schema");
        let properties = record
            .get("properties")
            .and_then(Value::as_object)
            .expect("Sumeragi evidence audit properties");
        assert_eq!(
            properties
                .get("penalty_status")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/SumeragiEvidencePenaltyStatus")
        );
        assert_eq!(
            properties
                .get("kind")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some("SumeragiV2Equivocation")
        );
        let classes = properties
            .get("class")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum"))
            .and_then(Value::as_array)
            .expect("evidence class enum")
            .iter()
            .map(|class| class.as_str().expect("evidence class string"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            classes,
            ["proposal", "phase_vote", "timeout_vote"]
                .into_iter()
                .collect()
        );
        for hash in ["context_id", "artifact_hash_1", "artifact_hash_2"] {
            assert_eq!(
                properties
                    .get(hash)
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("pattern"))
                    .and_then(Value::as_str),
                Some("^[0-9a-f]{64}$"),
                "{hash} must remain canonical lowercase hex"
            );
        }
        for retired in [
            "penalty_applied",
            "penalty_cancelled",
            "penalty_cancelled_at_height",
            "penalty_applied_at_height",
            "consensus_admitted_at_height",
        ] {
            assert!(
                !properties.contains_key(retired),
                "retired evidence field `{retired}` remains documented"
            );
        }
        let list_items = schemas
            .get("SumeragiEvidenceListResponse")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("items"))
            .and_then(Value::as_object)
            .expect("evidence-list items schema");
        assert_eq!(
            list_items.get("maxItems").and_then(Value::as_u64),
            Some(1_000)
        );
        assert_eq!(
            list_items
                .get("items")
                .and_then(Value::as_object)
                .and_then(|items| items.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/SumeragiEvidenceAuditRecord")
        );

        let variants = schemas
            .get("SumeragiEvidencePenaltyStatus")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("oneOf"))
            .and_then(Value::as_array)
            .expect("closed evidence penalty variants");
        assert_eq!(variants.len(), 3);
        for status in ["pending", "applied", "cancelled"] {
            let variant = variants
                .iter()
                .find(|variant| {
                    variant
                        .get("properties")
                        .and_then(Value::as_object)
                        .and_then(|properties| properties.get("status"))
                        .and_then(Value::as_object)
                        .and_then(|status| status.get("const"))
                        .and_then(Value::as_str)
                        == Some(status)
                })
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("missing `{status}` evidence penalty variant"));
            assert_eq!(
                variant.get("additionalProperties"),
                Some(&Value::Bool(false))
            );
            let required = variant
                .get("required")
                .and_then(Value::as_array)
                .expect("penalty variant required fields")
                .iter()
                .map(|field| field.as_str().expect("required field"))
                .collect::<BTreeSet<_>>();
            assert_eq!(required, ["status", "details"].into_iter().collect());
            let details = variant
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("details"))
                .and_then(Value::as_object)
                .expect("penalty variant details");
            if status == "pending" {
                assert_eq!(details.get("type").and_then(Value::as_str), Some("null"));
            } else {
                assert_eq!(
                    details.get("additionalProperties"),
                    Some(&Value::Bool(false))
                );
                assert_eq!(
                    details.get("required").and_then(Value::as_array),
                    Some(&vec![Value::from("height")])
                );
            }
        }
    }
    #[test]
    fn retired_sumeragi_vrf_surfaces_are_absent() {
        for (surface, source) in [
            ("Torii runtime handlers", include_str!("routing.rs")),
            ("Torii router mounts", include_str!("lib.rs")),
        ] {
            assert!(
                !source.contains("handle_v1_sumeragi_vrf_"),
                "retired Sumeragi VRF handler reappeared in {surface}"
            );
            assert!(
                !source.contains("/v1/sumeragi/vrf/"),
                "retired Sumeragi VRF route reappeared in {surface}"
            );
        }
        let canonical = canonical_document();
        let paths = canonical
            .get("paths")
            .and_then(Value::as_object)
            .expect("canonical paths section");
        let evidence = paths
            .get("/v1/sumeragi/evidence")
            .and_then(Value::as_object)
            .expect("retained evidence-list path");
        assert!(evidence.contains_key("get"));
        assert!(
            !evidence.contains_key("post"),
            "retired evidence submission operation remains documented"
        );
        for retired_path in [
            "/v1/sumeragi/vrf/commit",
            "/v1/sumeragi/vrf/epoch/{epoch}",
            "/v1/sumeragi/vrf/penalties/{epoch}",
            "/v1/sumeragi/vrf/reveal",
        ] {
            assert!(
                !paths.contains_key(retired_path),
                "retired Sumeragi VRF path remains in the canonical full-profile document: {retired_path}"
            );
        }
        assert!(
            paths
                .keys()
                .all(|path| !path.starts_with("/v1/sumeragi/vrf/")),
            "canonical full-profile document must not expose any retired Sumeragi VRF path"
        );
        let compiled_paths = generate_spec()
            .get("paths")
            .and_then(Value::as_object)
            .expect("compiled paths section")
            .clone();
        assert!(
            compiled_paths
                .keys()
                .all(|path| !path.starts_with("/v1/sumeragi/vrf/")),
            "compiled OpenAPI profile must not expose retired Sumeragi VRF paths"
        );
        let schemas = canonical
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("canonical schemas section");
        for retired_schema in [
            "SumeragiVrfCommitRequest",
            "SumeragiVrfRevealRequest",
            "SumeragiVrfPenaltiesReport",
        ] {
            assert!(
                !schemas.contains_key(retired_schema),
                "retired Sumeragi VRF schema remains documented: {retired_schema}"
            );
        }
    }
    #[test]
    fn validation_fee_plaintext_contracts_stay_retired_and_parliament_capabilities_are_exact() {
        const RETIRED_PATH: &str = "/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft";
        for (surface, source) in [
            (
                "Torii validation-fee implementation",
                include_str!("validation_fee_api.rs"),
            ),
            ("Torii router mounts", include_str!("lib.rs")),
            (
                "canonical route catalog",
                include_str!("../../iroha_torii_shared/src/route_catalog.rs"),
            ),
        ] {
            assert!(
                !source.contains(RETIRED_PATH),
                "retired validation-fee PLAIN ballot draft reappeared in {surface}"
            );
            assert!(
                !source.contains("ValidationFeePlain"),
                "retired validation-fee plaintext type reappeared in {surface}"
            );
        }
        for (label, document) in [
            ("canonical", canonical_document()),
            ("compiled", generate_spec()),
        ] {
            let paths = document
                .get("paths")
                .and_then(Value::as_object)
                .expect("OpenAPI paths section");
            assert!(
                !paths.contains_key(RETIRED_PATH),
                "retired validation-fee PLAIN ballot draft remains in {label} OpenAPI"
            );
            assert!(
                paths.keys().all(|path| {
                    !path.starts_with("/v1/validation-fee/") || !path.contains("plain")
                }),
                "retired validation-fee plaintext route remains in {label} OpenAPI"
            );
            let schemas = document
                .get("components")
                .and_then(Value::as_object)
                .and_then(|components| components.get("schemas"))
                .and_then(Value::as_object)
                .expect("OpenAPI schemas section");
            for retired_schema in [
                "ValidationFeePlainBallotDraftRequestV1",
                "ValidationFeePlainBallotDraftResponseV1",
                "ValidationFeePlainElectorateMemberV1",
                "ValidationFeePlainElectorateRulesV1",
                "ValidationFeePlainElectorateSnapshotV1",
            ] {
                assert!(
                    !schemas.contains_key(retired_schema),
                    "retired schema {retired_schema} remains in {label} OpenAPI"
                );
            }
            assert!(
                schemas
                    .keys()
                    .all(|name| !name.starts_with("ValidationFeePlain")),
                "retired validation-fee plaintext schema remains in {label} OpenAPI"
            );

            assert_strict_object_schema(
                schemas,
                "GovernanceCapabilitiesV1",
                &[
                    "schema",
                    "version",
                    "network_id",
                    "current_height",
                    "network_prefix",
                    "abi_version",
                    "data_model_version",
                    "approval_mode",
                    "private_ballot_protocol",
                    "mandatory_private_ballots",
                    "proposal_backed_referendum_ballots_supported",
                    "standalone_plain_ballots_supported",
                    "standalone_zk_ballots_supported",
                    "citizenship_asset_id",
                    "citizenship_bond_amount",
                    "citizenship_escrow_account",
                    "voting_asset_id",
                    "min_bond_amount",
                    "bond_escrow_account",
                    "min_enactment_delay",
                    "invitation_phase_blocks",
                    "registration_phase_blocks",
                    "survivor_freeze_phase_blocks",
                    "commitment_phase_blocks",
                    "release_delay_blocks",
                    "opening_phase_blocks",
                    "max_ballot_retries",
                    "max_corpus_entries",
                    "target_body_sizes",
                    "supported_proposal_kinds",
                    "supported_routes",
                ],
                &[],
            );
            let capabilities = component_properties(schemas, "GovernanceCapabilitiesV1");
            for retired_field in [
                "approval_threshold_denominator",
                "approval_threshold_numerator",
                "auto_finalize_plain",
                "auto_finalize_plain_scope",
                "conviction_step_blocks",
                "max_conviction",
                "min_turnout",
                "plain_voting_enabled",
                "validation_fee_plain_electorate_rules",
                "validation_fee_plain_requires_explicit_finalization",
                "window_span",
            ] {
                assert!(
                    !capabilities.contains_key(retired_field),
                    "retired GovernanceCapabilitiesV1 field {retired_field} remains in {label} OpenAPI"
                );
            }
            assert_eq!(
                capabilities["approval_mode"]["const"].as_str(),
                Some("PARLIAMENT_ATTEMPT_TIMED_OVN_V1")
            );
            assert_eq!(
                capabilities["private_ballot_protocol"]["const"].as_str(),
                Some("TIMED_OVN_TLE_THRESHOLD_BLS_V1")
            );
            assert_eq!(
                capabilities["mandatory_private_ballots"]["const"].as_bool(),
                Some(true)
            );
            assert_eq!(
                capabilities["proposal_backed_referendum_ballots_supported"]["const"].as_bool(),
                Some(false)
            );
            assert_eq!(
                capabilities["standalone_zk_ballots_supported"]["const"].as_bool(),
                Some(true)
            );
        }
    }
    #[test]
    fn pipeline_fastpq_recovery_documents_operator_auth_and_bounds() {
        use iroha_torii_shared::route_catalog::{ApiSurface, AuthenticationPolicy};
        let route = iroha_torii_shared::route_catalog::pipeline::RECOVERY_FASTPQ_PROOFS;
        assert_eq!(route.surface(), ApiSurface::Operator);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        let document = generate_spec();
        let operation = openapi_operation(
            &document,
            "/v1/pipeline/recovery/{height}/fastpq-proofs",
            "get",
        );
        assert_eq!(
            operation_header_requirements(operation)
                .into_iter()
                .map(|(name, required)| {
                    assert!(required, "operator signature headers must be required");
                    name
                })
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "X-Iroha-Operator-Public-Key".to_owned(),
                "X-Iroha-Operator-Timestamp-Ms".to_owned(),
                "X-Iroha-Operator-Nonce".to_owned(),
                "X-Iroha-Operator-Signature".to_owned(),
            ])
        );
        let parameters = operation
            .get("parameters")
            .and_then(Value::as_array)
            .expect("FASTPQ recovery parameters");
        let parameter = |name: &str| {
            parameters
                .iter()
                .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
                .unwrap_or_else(|| panic!("missing FASTPQ recovery `{name}` parameter"))
        };
        let limit_schema = parameter("limit")
            .get("schema")
            .and_then(Value::as_object)
            .expect("FASTPQ recovery limit schema");
        assert_eq!(limit_schema.get("minimum").and_then(Value::as_u64), Some(1));
        assert_eq!(
            limit_schema.get("maximum").and_then(Value::as_u64),
            Some(crate::PIPELINE_FASTPQ_RECOVERY_MAX_LIMIT as u64)
        );
        assert!(
            operation
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| {
                    description.contains("operator-only")
                        && description.contains("replay-resistant")
                        && description.contains("Heavy reconstruction")
                        && description.contains("byte caps")
                })
        );
    }
    #[test]
    fn signed_transaction_submission_documents_exact_preadmission_contract() {
        let document = generate_spec();
        let responses = document
            .get("paths")
            .and_then(Value::as_object)
            .and_then(|paths| paths.get(uri::TRANSACTION))
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("responses"))
            .and_then(Value::as_object)
            .expect("signed transaction submission responses");
        assert_eq!(
            documented_reject_codes(responses, "400"),
            transaction_submission_bad_request_reject_codes()
        );
        assert_eq!(
            documented_reject_codes(responses, "403"),
            TRANSACTION_SUBMISSION_FORBIDDEN_REJECT_CODES
        );
        assert_eq!(
            documented_reject_codes(responses, "409"),
            TRANSACTION_SUBMISSION_CONFLICT_REJECT_CODES
        );
        assert_eq!(
            documented_reject_codes(responses, "429"),
            TRANSACTION_SUBMISSION_RATE_LIMIT_REJECT_CODES
        );
        assert_eq!(
            documented_reject_codes(responses, "503"),
            TRANSACTION_SUBMISSION_UNAVAILABLE_REJECT_CODES
        );
        for status in ["413", "415", "500", "502", "504"] {
            assert!(
                !response_documents_reject_code(responses, status),
                "transaction submission HTTP {status} must not claim a canonical reject code"
            );
        }
        let conflict_description = responses
            .get("409")
            .and_then(Value::as_object)
            .and_then(|response| response.get("description"))
            .and_then(Value::as_str)
            .expect("transaction submission 409 description");
        assert!(
            conflict_description.contains("already committed")
                && conflict_description.contains("already present"),
            "a duplicate response must be documented as existing admission state"
        );
    }
    #[test]
    fn transaction_submission_503s_document_exact_outcome_unknown_identity() {
        let document = canonical_document();
        for path in [uri::TRANSACTION, uri::TRANSACTION_ENTRYPOINT] {
            let operation = openapi_operation(&document, path, "post");
            assert_eq!(
                operation_response_schema_ref(operation, "503", path),
                "#/components/schemas/ErrorEnvelope"
            );
            let unavailable = operation
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("503"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("POST {path} HTTP 503 response"));
            let description = unavailable
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("POST {path} HTTP 503 description"));
            for required_text in [
                "PRTRY:QUEUE_PLAN_JOURNAL_UNAVAILABLE",
                "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN",
                "ErrorEnvelope.details.entrypoint_hash",
                "ErrorEnvelope.details.tx_hash",
                "does not fabricate queue-pressure",
            ] {
                assert!(
                    description.contains(required_text),
                    "POST {path} HTTP 503 must document {required_text}"
                );
            }
            let headers = unavailable
                .get("headers")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("POST {path} HTTP 503 headers"));
            for (header_name, detail_name) in [
                ("x-iroha-entrypoint-hash", "entrypoint_hash"),
                ("x-iroha-signed-transaction-hash", "tx_hash"),
            ] {
                let header = headers
                    .get(header_name)
                    .and_then(Value::as_object)
                    .unwrap_or_else(|| panic!("POST {path} HTTP 503 {header_name}"));
                let header_description = header
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| panic!("POST {path} HTTP 503 {header_name} description"));
                assert!(
                    header_description.contains(
                        "Present exactly once only for PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN"
                    ) && header_description.contains(detail_name),
                    "POST {path} HTTP 503 {header_name} must document its conditional exact body binding"
                );
                if path == uri::TRANSACTION_ENTRYPOINT
                    && header_name == "x-iroha-signed-transaction-hash"
                {
                    assert!(
                        header_description.contains("External or SealedReveal")
                            && header_description.contains("inner SignedTransaction"),
                        "POST {path} HTTP 503 signed identity must be conditional on an inner signed transaction"
                    );
                }
                assert_eq!(
                    header
                        .get("schema")
                        .and_then(Value::as_object)
                        .and_then(|schema| schema.get("pattern"))
                        .and_then(Value::as_str),
                    Some("^[0-9a-f]{64}$"),
                    "POST {path} HTTP 503 {header_name} exact hash syntax"
                );
            }
        }
    }
    #[test]
    fn signed_transaction_reject_code_inventory_matches_runtime_metadata() {
        use iroha_core::{queue::Error as QueueError, tx::SignatureRejectionCode};
        let mut acceptance_codes = vec!["transaction_rejected", "PRTRY:NTS_UNHEALTHY"];
        acceptance_codes.extend(
            [
                SignatureRejectionCode::UnsupportedAuthority,
                SignatureRejectionCode::AlgorithmNotPermitted,
                SignatureRejectionCode::InvalidSignature,
                SignatureRejectionCode::MalformedSignature,
                SignatureRejectionCode::MissingSignatures,
                SignatureRejectionCode::UnknownSigner,
                SignatureRejectionCode::InsufficientWeight,
            ]
            .map(SignatureRejectionCode::as_str),
        );
        acceptance_codes.extend([
            "ED07",
            "PRTRY:KAGEMUSHA_V1_OPERATION_CARRIER_REJECTED",
            "PRTRY:ROUTE_UNRESOLVED",
        ]);
        assert_eq!(
            acceptance_codes,
            TRANSACTION_ACCEPTANCE_BAD_REQUEST_REJECT_CODES
        );
        assert_eq!(
            &KAGEMUSHA_COMMAND_FORBIDDEN_REJECT_CODES[1..],
            TRANSACTION_SUBMISSION_FORBIDDEN_REJECT_CODES
        );
        assert_eq!(
            &KAGEMUSHA_COMMAND_CONFLICT_REJECT_CODES[3..],
            TRANSACTION_SUBMISSION_CONFLICT_REJECT_CODES
        );
        assert_eq!(
            KAGEMUSHA_COMMAND_RATE_LIMIT_REJECT_CODES,
            TRANSACTION_SUBMISSION_RATE_LIMIT_REJECT_CODES
        );
        let forbidden = [
            QueueError::GovernanceNotPermitted {
                alias: "lane".to_owned(),
                reason: "policy".to_owned(),
            },
            QueueError::LaneComplianceDenied {
                alias: "lane".to_owned(),
                reason: "compliance".to_owned(),
            },
            QueueError::LanePrivacyProofRejected {
                alias: "lane".to_owned(),
                reason: "privacy".to_owned(),
            },
            QueueError::NexusFeeAdmissionRejected {
                code: iroha_data_model::nexus::FeeRejectionCode::BeneficiaryNotEligible,
                reason: "fee".to_owned(),
            },
        ];
        assert_eq!(
            forbidden
                .iter()
                .map(|error| crate::queue_rejection_metadata(error).0)
                .collect::<Vec<_>>(),
            TRANSACTION_SUBMISSION_FORBIDDEN_REJECT_CODES
        );
        for (errors, expected) in [
            (
                vec![QueueError::InBlockchain, QueueError::IsInQueue],
                &TRANSACTION_SUBMISSION_CONFLICT_REJECT_CODES[..2],
            ),
            (
                vec![
                    QueueError::Full,
                    QueueError::LatencySaturated,
                    QueueError::MaximumTransactionsPerUser,
                ],
                TRANSACTION_SUBMISSION_RATE_LIMIT_REJECT_CODES,
            ),
        ] {
            assert_eq!(
                errors
                    .iter()
                    .map(|error| crate::queue_rejection_metadata(error).0)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
    fn openapi_schemas_include_system_keys() {
        let schemas = openapi_schemas();
        for key in openapi_contract_strings("openapi.openapi_schemas_include_system_keys.strings.1")
        {
            assert!(schemas.contains_key(key), "schema missing {key}");
        }
    }
    include!("openapi/tests/diagnostics_schemas.rs");
    include!("openapi/tests/fee_quote_contract.rs");
    include!("openapi/tests/finality_app_contracts.rs");
    include!("openapi/tests/hijiri_quote_contract.rs");
    include!("openapi/tests/iso20022_auth.rs");
    include!("openapi/tests/json_value_contract.rs");
    include!("openapi/tests/prepared_account_contracts.rs");
    include!("openapi/tests/private_settlement_contract.rs");
    include!("openapi/tests/soracloud_lease_contracts.rs");
    include!("openapi/tests/vpn_da.rs");
}
