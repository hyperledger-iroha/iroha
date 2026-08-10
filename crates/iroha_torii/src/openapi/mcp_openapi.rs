//! OpenAPI projection for the native MCP transport.

use super::{Map, Value, single_json_response};

pub(super) fn mcp_paths() -> Map {
    let mut get = Map::new();
    get.insert("tags".into(), norito::json!(["MCP"]));
    get.insert("summary".into(), norito::json!("Fetch MCP capabilities."));
    get.insert(
        "description".into(),
        norito::json!("Returns server capabilities and tool count for the native MCP bridge."),
    );
    get.insert("operationId".into(), norito::json!("mcpCapabilities"));
    get.insert(
        "responses".into(),
        Value::Object(single_json_response("#/components/schemas/JsonValue")),
    );

    let mut post = Map::new();
    post.insert("tags".into(), norito::json!(["MCP"]));
    post.insert(
        "summary".into(),
        norito::json!("Execute MCP JSON-RPC request."),
    );
    post.insert(
        "description".into(),
        norito::json!(
            "Accepts bounded JSON-RPC payloads. Tool calls dispatch only to cataloged routes, and each target enforces its own exact authentication and admission before any effect. Canonical account or operator proofs belong in the selected tool's arguments.headers and are never inferred from the outer MCP envelope."
        ),
    );
    post.insert("operationId".into(), norito::json!("mcpJsonRpc"));
    post.insert(
        "requestBody".into(),
        norito::json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/JsonValue" }
                }
            }
        }),
    );
    post.insert(
        "responses".into(),
        Value::Object(single_json_response("#/components/schemas/JsonValue")),
    );

    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(get));
    methods.insert("post".to_owned(), Value::Object(post));
    let mut paths = Map::new();
    paths.insert("/v1/mcp".to_owned(), Value::Object(methods));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_documents_bounded_target_authenticated_json_rpc() {
        let paths = mcp_paths();
        let post = paths
            .get("/v1/mcp")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("MCP POST operation");
        assert_eq!(
            post.get("requestBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("required"))
                .and_then(Value::as_bool),
            Some(true)
        );
        let description = post
            .get("description")
            .and_then(Value::as_str)
            .expect("MCP POST description");
        assert!(description.contains("cataloged routes"));
        assert!(description.contains("arguments.headers"));
    }
}
