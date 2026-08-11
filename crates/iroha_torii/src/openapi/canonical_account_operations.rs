//! Shared OpenAPI operation contracts for exact-network account-authenticated requests.

use norito::json::{Map, Value};

/// Build one JSON GET operation with canonical account headers, security, and private caching.
pub(super) fn json_get(
    tag: &str,
    summary: &str,
    description: &str,
    schema_ref: &str,
    mut parameters: Vec<Value>,
) -> Map {
    parameters.extend(super::canonical_request_auth_header_parameters());
    let mut methods = super::json_get_operation(tag, summary, description, schema_ref, parameters);
    let operation = methods
        .get_mut("get")
        .and_then(Value::as_object_mut)
        .expect("JSON GET operation");
    super::insert_canonical_request_auth_contract(operation);
    insert_private_no_store_response_contract(operation);
    methods
}

/// Build one JSON POST operation with canonical account headers, security, and private caching.
pub(super) fn json_post(
    tag: &str,
    summary: &str,
    description: &str,
    request_schema_ref: &str,
    response_schema_ref: &str,
    mut parameters: Vec<Value>,
) -> Map {
    parameters.extend(super::canonical_request_auth_header_parameters());
    let mut methods = super::json_post_operation(
        tag,
        summary,
        description,
        request_schema_ref,
        response_schema_ref,
        parameters,
    );
    let operation = methods
        .get_mut("post")
        .and_then(Value::as_object_mut)
        .expect("JSON POST operation");
    super::insert_canonical_request_auth_contract(operation);
    insert_private_no_store_response_contract(operation);
    methods
}

pub(super) fn insert_private_no_store_response_contract(operation: &mut Map) {
    let Some(Value::Object(responses)) = operation.get_mut("responses") else {
        return;
    };
    for response in responses.values_mut() {
        if let Value::Object(response) = response {
            let private_headers = private_no_store_response_headers();
            if let Some(Value::Object(headers)) = response.get_mut("headers") {
                headers.extend(private_headers);
            } else {
                response.insert("headers".into(), Value::Object(private_headers));
            }
        }
    }
}

pub(super) fn private_no_store_response_headers() -> Map {
    let mut headers = Map::new();
    headers.insert(
        "Cache-Control".into(),
        norito::json!({
            "description": "Authenticated responses which must never be retained.",
            "required": true,
            "schema": {
                "type": "string",
                "const": "private, no-store"
            }
        }),
    );
    headers.insert(
        "Vary".into(),
        norito::json!({
            "description": "Canonical authentication headers selecting the private response.",
            "required": true,
            "schema": {
                "type": "string",
                "const": "X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, X-Iroha-Nonce, X-Iroha-Witness"
            }
        }),
    );
    headers
}
