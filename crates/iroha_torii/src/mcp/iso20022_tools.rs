// ISO 20022 MCP tools require a complete signature for their exact inner target.
fn iso20022_operator_auth_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["public_key", "timestamp_ms", "nonce", "signature"],
        "properties": {
            "public_key": {
                "type": "string",
                "minLength": 1,
                "maxLength": (OPERATOR_PUBLIC_KEY_MAX_LITERAL_BYTES),
                "pattern": "^[!-~]+$",
                "description": "Iroha multihash public key of the configured operator."
            },
            "timestamp_ms": {
                "type": "integer",
                "minimum": 0,
                "maximum": (u64::MAX),
                "description": "Unix timestamp in milliseconds signed for the exact inner request."
            },
            "nonce": {
                "type": "string",
                "minLength": 1,
                "maxLength": 256,
                "pattern": "^[!-~]+$",
                "description": "Fresh nonce signed for the exact inner request."
            },
            "signature": {
                "type": "string",
                "minLength": 4,
                "maxLength": (CANONICAL_SIGNATURE_MAX_ENCODED_BYTES),
                "pattern": (CANONICAL_PADDED_BASE64_PATTERN),
                "description": "Canonical padded-base64 signature over the exact inner method, path, sorted query, body hash, timestamp, and nonce."
            }
        }
    })
}
fn iroha_iso20022_pacs008_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.pacs008.submit",
        "pacs.008",
        "/v1/iso20022/pacs008",
        "Submit an ISO 20022 pacs.008 payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_pacs009_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.pacs009.submit",
        "pacs.009",
        "/v1/iso20022/pacs009",
        "Submit an ISO 20022 pacs.009 payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_lifecycle_submit_tool(
    name: &str,
    message_type: &str,
    path_template: &str,
    description: &str,
) -> ToolSpec {
    let body_description = format!("Base64/base64url encoded {message_type} XML payload bytes.");
    ToolSpec {
        name: name.to_owned(),
        effect: manual_tool_effect_from_name(name),
        description: description.to_owned(),
        method: Method::POST,
        path_template: path_template.to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["body_base64", "operator_auth"],
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": body_description
                },
                "content_type": {
                    "type": "string",
                    "description": "Optional content type override; defaults to application/xml."
                },
                "profile": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Optional ISO bridge rail profile carried in the signed `profile` query parameter."
                },
                "operator_auth": (iso20022_operator_auth_schema()),
                "accept": { "type": "string" }
            }
        }),
    }
}
fn iroha_iso20022_pacs002_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.pacs002.submit",
        "pacs.002",
        "/v1/iso20022/pacs002",
        "Submit an ISO 20022 pacs.002 lifecycle payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_pacs004_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.pacs004.submit",
        "pacs.004",
        "/v1/iso20022/pacs004",
        "Submit an ISO 20022 pacs.004 lifecycle payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_camt056_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.camt056.submit",
        "camt.056",
        "/v1/iso20022/camt056",
        "Submit an ISO 20022 camt.056 lifecycle payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_sese023_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.sese023.submit",
        "sese.023",
        "/v1/iso20022/sese023",
        "Submit an ISO 20022 sese.023 settlement instruction payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_sese024_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.sese024.submit",
        "sese.024",
        "/v1/iso20022/sese024",
        "Submit an ISO 20022 sese.024 settlement status payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_sese025_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.sese025.submit",
        "sese.025",
        "/v1/iso20022/sese025",
        "Submit an ISO 20022 sese.025 settlement confirmation payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_colr012_submit_tool() -> ToolSpec {
    iroha_iso20022_lifecycle_submit_tool(
        "iroha.iso20022.colr012.submit",
        "colr.012",
        "/v1/iso20022/colr012",
        "Submit an ISO 20022 colr.012 collateral substitution confirmation payload from canonical `body_base64` XML bytes.",
    )
}
fn iroha_iso20022_status_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.iso20022.status.get".to_owned(),
        effect: manual_tool_effect_from_name("iroha.iso20022.status.get"),
        description: "Fetch ISO 20022 bridge status by canonical `path.msg_id`.".to_owned(),
        method: Method::GET,
        path_template: "/v1/iso20022/messages/{msg_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path", "operator_auth"],
            "properties": {
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["msg_id"],
                    "properties": {
                        "msg_id": { "type": "string" }
                    }
                },
                "operator_auth": (iso20022_operator_auth_schema()),
                "accept": { "type": "string" }
            }
        }),
    }
}
