//! Static authority for Torii's OpenAPI description.
//!
//! The package-local document is an exact mirror of the canonical release
//! artifact. Torii parses it once with Norito JSON, installs the catalog's
//! security schemes, and removes operations disabled by the compiled route
//! catalog. This keeps every feature profile aligned with the mounted router
//! without compiling a second schema builder or rewriting the checked-in
//! contract at runtime.

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
        AuthenticationPolicy::PrivateRootOwnerToken => {
            Some(norito::json!([{ "IrohaApiToken": [] }]))
        }
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
            "Deployment-configured Torii API token. Immutable private roots require their owner token on every route; global roots apply the listener configuration.",
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
