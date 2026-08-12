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
        "connect.session.status",
    )?;
    decode_canonical(arguments, "sid", 32)?;
    decode_canonical(arguments, "token_management", 32)?;
    let sid = required_string(arguments, "sid")?;
    let token = required_string(arguments, "token_management")?;
    let path = format!("/v1/connect/status?sid={sid}");
    let headers = norito::json!({ "Authorization": (format!("Bearer {token}")) });
    dispatch_route_with_extra_header_policy(
        app,
        inbound_headers,
        Method::GET,
        path.as_str(),
        Some(&headers),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ExtraHeaderPolicy::ConnectManagement,
    )
    .await
}

fn connect_session_status_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.session.status".to_owned(),
        effect: manual_tool_effect_from_name("connect.session.status"),
        description: "Get one Iroha Connect session status using its management token; node aggregate telemetry is not projected through MCP.".to_owned(),
        method: Method::GET,
        path_template: "/v1/connect/status".to_owned(),
        input_schema: norito::json!({
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
                    "description": "Management bearer token returned by `connect.session.create`."
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_connect_session_status_tool() -> ToolSpec {
    let mut tool = connect_session_status_tool();
    tool.name = "iroha.connect.session.status".to_owned();
    tool.description = "Alias for connect.session.status.".to_owned();
    tool
}

#[cfg(test)]
mod connect_status_tests {
    use super::{ToolEffect, connect_session_status_tool};

    #[test]
    fn session_status_requires_exact_sid_and_management_token() {
        let tool = connect_session_status_tool();
        assert_eq!(tool.effect, ToolEffect::Read);
        assert_eq!(tool.path_template, "/v1/connect/status");
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
