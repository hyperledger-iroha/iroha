//! Exact Iroha Connect OpenAPI paths and session schemas.

use norito::json::{Map, Value};

use super::{
    json_delete_operation, json_get_operation, json_post_operation,
    operator_signature_header_parameters, required_string_query_param, string_header_param,
    string_path_param, text_get_operation,
};

pub(super) fn connect_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/connect/session".to_owned(),
        Value::Object(json_post_operation(
            "Connect",
            "Open a Connect session.",
            "Create a Connect session from the exact genesis-derived NetworkId, application X25519 public key, and fresh nonce. The supplied SID must be their canonical derivation; labels, raw SID shortcuts, and unsigned identity substitutions are rejected.",
            "#/components/schemas/ConnectSessionCreateRequest",
            "#/components/schemas/ConnectSessionCreateResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/connect/session/{sid}".to_owned(),
        Value::Object(json_delete_operation(
            "Connect",
            "Close a Connect session.",
            "Terminate a Connect session by session id. Requires `Authorization: Bearer <token_management>` from session creation.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("sid", "Connect session id."),
                string_header_param(
                    "Authorization",
                    "Bearer management token returned by `/v1/connect/session`.",
                    true,
                ),
            ],
        )),
    );
    let mut websocket = text_get_operation(
        "Connect",
        "Connect to the Connect WebSocket.",
        "Upgrade to the Connect WebSocket stream for exactly one session role. Authenticate with either the role bearer token or its `iroha-connect.token.v1.*` WebSocket subprotocol form; query-string tokens are rejected.",
        None,
    );
    websocket.insert(
        "parameters".to_owned(),
        Value::Array(vec![
            required_string_query_param("sid", "Canonical Connect session id."),
            required_string_query_param("role", "Exact role: `app` or `wallet`."),
            string_header_param(
                "Authorization",
                "Optional `Bearer <role-token>` authentication; omit only when using the Connect token subprotocol.",
                false,
            ),
            string_header_param(
                "Sec-WebSocket-Protocol",
                "Optional `iroha-connect.token.v1.<base64url>` role-token authentication; omit only when using Authorization.",
                false,
            ),
        ]),
    );
    paths.insert("/v1/connect/ws".to_owned(), Value::Object(websocket));
    paths.insert(
        "/v1/connect/status".to_owned(),
        Value::Object(json_get_operation(
            "Connect",
            "Fetch one Connect session status.",
            "Return the bounded status for exactly one Connect session. The required `sid` is authorized by the management bearer token returned when the session was created; unknown and duplicate query parameters are rejected.",
            "#/components/schemas/JsonValue",
            vec![
                required_string_query_param(
                    "sid",
                    "Connect session id authorized by the management token.",
                ),
                string_header_param(
                    "Authorization",
                    "Bearer management token returned by `/v1/connect/session`.",
                    true,
                ),
            ],
        )),
    );
    paths.insert(
        "/v1/connect/status/aggregate".to_owned(),
        Value::Object(json_get_operation(
            "Connect",
            "Fetch aggregate Connect node status.",
            "Return redacted node-local Connect relay counters and policy state. This operator-only route requires a fresh signature bound to the exact NetworkId, GET target, and empty body.",
            "#/components/schemas/JsonValue",
            operator_signature_header_parameters(),
        )),
    );
    paths
}

pub(super) fn insert_connect_schemas(schemas: &mut Map) {
    schemas.insert(
        "ConnectSessionCreateRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["sid", "network_id", "app_pk", "nonce"],
            "additionalProperties": false,
            "properties": {
                "sid": {
                    "type": "string",
                    "pattern": "^[A-Za-z0-9_-]{43}$",
                    "description": "Canonical unpadded base64url BLAKE2b-256 derivation over exact NetworkId bytes, app_pk, and nonce."
                },
                "network_id": {
                    "type": "string",
                    "pattern": "^hash:[0-9A-F]{64}#[0-9A-F]{4}$",
                    "description": "Canonical checksummed genesis-derived NetworkId."
                },
                "app_pk": {
                    "type": "string",
                    "pattern": "^[A-Za-z0-9_-]{43}$",
                    "description": "Canonical unpadded base64url X25519 application public key (32 bytes)."
                },
                "nonce": {
                    "type": "string",
                    "pattern": "^[A-Za-z0-9_-]{22}$",
                    "description": "Canonical unpadded base64url fresh nonzero session nonce (16 bytes)."
                },
                "node": {
                    "type": "string",
                    "description": "Optional node hint copied into the generated deep links."
                }
            }
        }),
    );
    schemas.insert(
        "ConnectSessionCreateResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "sid", "network_id", "app_pk", "nonce", "wallet_uri", "app_uri",
                "token_app", "token_wallet", "token_management", "token_relay"
            ],
            "additionalProperties": false,
            "properties": {
                "sid": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" },
                "network_id": { "type": "string", "pattern": "^hash:[0-9A-F]{64}#[0-9A-F]{4}$" },
                "app_pk": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" },
                "nonce": { "type": "string", "pattern": "^[A-Za-z0-9_-]{22}$" },
                "wallet_uri": { "type": "string", "minLength": 1 },
                "app_uri": { "type": "string", "minLength": 1 },
                "token_app": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" },
                "token_wallet": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" },
                "token_management": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" },
                "token_relay": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" }
            }
        }),
    );
}

#[cfg(test)]
mod tests {
    use norito::json::{Map, Value};

    use super::{connect_paths, insert_connect_schemas};

    #[test]
    fn session_create_operation_references_exact_schemas() {
        let paths = connect_paths();
        let post = paths["/v1/connect/session"]
            .get("post")
            .and_then(Value::as_object)
            .expect("Connect create POST");
        let request_ref = post
            .get("requestBody")
            .and_then(|value| value.get("content"))
            .and_then(|value| value.get("application/json"))
            .and_then(|value| value.get("schema"))
            .and_then(|value| value.get("$ref"))
            .and_then(Value::as_str);
        assert_eq!(
            request_ref,
            Some("#/components/schemas/ConnectSessionCreateRequest")
        );
        let response_ref = post
            .get("responses")
            .and_then(|value| value.get("200"))
            .and_then(|value| value.get("content"))
            .and_then(|value| value.get("application/json"))
            .and_then(|value| value.get("schema"))
            .and_then(|value| value.get("$ref"))
            .and_then(Value::as_str);
        assert_eq!(
            response_ref,
            Some("#/components/schemas/ConnectSessionCreateResponse")
        );
    }

    #[test]
    fn session_create_schema_requires_only_canonical_identity_fields() {
        let mut schemas = Map::new();
        insert_connect_schemas(&mut schemas);
        let request = schemas["ConnectSessionCreateRequest"]
            .as_object()
            .expect("request schema");
        let required = request["required"].as_array().expect("required fields");
        for field in ["sid", "network_id", "app_pk", "nonce"] {
            assert!(required.iter().any(|value| value.as_str() == Some(field)));
        }
        let properties = request["properties"]
            .as_object()
            .expect("request properties");
        for retired in ["chain", "chain_id", "chainId", "session_id", "body"] {
            assert!(!properties.contains_key(retired));
        }
        assert_eq!(request["additionalProperties"].as_bool(), Some(false));

        let response = schemas["ConnectSessionCreateResponse"]
            .as_object()
            .expect("response schema");
        assert_eq!(response["additionalProperties"].as_bool(), Some(false));
        for field in [
            "sid",
            "network_id",
            "app_pk",
            "nonce",
            "token_app",
            "token_wallet",
            "token_management",
            "token_relay",
        ] {
            assert!(
                response["required"]
                    .as_array()
                    .expect("response required fields")
                    .iter()
                    .any(|value| value.as_str() == Some(field))
            );
        }
    }

    #[test]
    fn status_routes_have_disjoint_protocol_and_operator_authentication() {
        let paths = connect_paths();
        let session = paths["/v1/connect/status"]["get"]
            .as_object()
            .expect("session status GET");
        let session_parameters = session["parameters"]
            .as_array()
            .expect("session status parameters");
        for name in ["sid", "Authorization"] {
            let parameter = session_parameters
                .iter()
                .find(|parameter| parameter["name"].as_str() == Some(name))
                .expect("required session-status parameter");
            assert_eq!(parameter["required"].as_bool(), Some(true));
        }
        assert!(session_parameters.iter().all(|parameter| {
            !parameter["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("X-Iroha-Operator-"))
        }));

        let aggregate = paths["/v1/connect/status/aggregate"]["get"]
            .as_object()
            .expect("aggregate status GET");
        let aggregate_parameters = aggregate["parameters"]
            .as_array()
            .expect("aggregate operator parameters");
        for name in [
            "X-Iroha-Operator-Public-Key",
            "X-Iroha-Operator-Timestamp-Ms",
            "X-Iroha-Operator-Nonce",
            "X-Iroha-Operator-Signature",
        ] {
            let parameter = aggregate_parameters
                .iter()
                .find(|parameter| parameter["name"].as_str() == Some(name))
                .expect("required aggregate operator parameter");
            assert_eq!(parameter["required"].as_bool(), Some(true));
        }
        assert!(aggregate_parameters.iter().all(|parameter| {
            !matches!(parameter["name"].as_str(), Some("sid" | "Authorization"))
        }));
    }

    #[test]
    fn websocket_documents_exact_session_role_and_header_token_transport() {
        let paths = connect_paths();
        let websocket = paths["/v1/connect/ws"]["get"]
            .as_object()
            .expect("Connect websocket GET");
        let parameters = websocket["parameters"]
            .as_array()
            .expect("Connect websocket parameters");
        for name in ["sid", "role"] {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter["name"].as_str() == Some(name))
                .expect("required websocket query parameter");
            assert_eq!(parameter["in"].as_str(), Some("query"));
            assert_eq!(parameter["required"].as_bool(), Some(true));
        }
        for name in ["Authorization", "Sec-WebSocket-Protocol"] {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter["name"].as_str() == Some(name))
                .expect("optional websocket token header");
            assert_eq!(parameter["in"].as_str(), Some("header"));
            assert_eq!(parameter["required"].as_bool(), Some(false));
        }
    }
}
