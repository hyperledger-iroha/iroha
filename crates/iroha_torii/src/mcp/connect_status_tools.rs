// Management-token status for one Connect session. Node aggregate telemetry is
// deliberately absent from MCP because it has no public projection.
async fn dispatch_connect_session_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    reject_unknown_arguments(
        arguments,
        &["sid", "token_management", "accept"],
        "iroha.connect.session.status",
    )?;
    let sid = canonical_connect_sid_argument(arguments)?;
    decode_canonical(arguments, "token_management", 32)?;
    let token = required_string(arguments, "token_management")?;
    let path = format!("/v1/connect/status?sid={sid}");
    dispatch_route_with_borrowed_headers(
        app,
        inbound_headers,
        Method::GET,
        path.as_str(),
        None,
        Vec::new(),
        None,
        arguments.get("accept").and_then(Value::as_str),
        ExtraHeaderPolicy::ConnectManagement,
        Some(token),
    )
    .await
}
fn iroha_connect_session_status_tool() -> ToolSpec {
    ToolSpec::route(
        "iroha.connect.session.status".to_owned(),
        "Get one Iroha Connect session status using its management token; node aggregate telemetry is not projected through MCP.".to_owned(),
        manual_tool_effect_from_name("iroha.connect.session.status"),
        Method::GET,
        "/v1/connect/status".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["sid", "token_management"],
            "properties": {
                "sid": {
                    "type": "string",
                    "pattern": "^[A-Za-z0-9_-]{43}$"
                },
                "token_management": {
                    "type": "string",
                    "pattern": "^[A-Za-z0-9_-]{43}$",
                    "description": "Management bearer token returned by `iroha.connect.session.create`."
                },
                "accept": { "type": "string" }
            }
        }),
    )
}
#[cfg(test)]
mod connect_status_tests {
    use super::{ToolEffect, iroha_connect_session_status_tool};
    #[test]
    fn session_status_requires_exact_sid_and_management_token() {
        let tool = iroha_connect_session_status_tool();
        let (effect, _, path_template) = tool.route_backing().expect("route-backed tool");
        assert_eq!(effect, ToolEffect::Read);
        assert_eq!(path_template, "/v1/connect/status");
        let required = tool.input_schema["required"]
            .as_array()
            .expect("session required fields");
        assert_eq!(required.len(), 2);
        assert!(required.iter().any(|value| value.as_str() == Some("sid")));
        assert!(
            required
                .iter()
                .any(|value| value.as_str() == Some("token_management"))
        );
    }
}
