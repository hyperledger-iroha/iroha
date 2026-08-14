//! Exact-network governance ballot tool schemas.
use norito::json::Value;
use super::{ToolSpec, iroha_gov_post_tool_with_fields};
pub(super) fn governance_selector_v1_schema(description: &str) -> Value {
    norito::json!({
        "type": "string",
        "minLength": 1,
        "maxLength": (iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_MAX_BYTES),
        "pattern": (iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_PATTERN),
        "description": description
    })
}
fn exact_network_governance_ballot_tool(
    name: &str,
    description: &str,
    path_template: &str,
    selector: (&str, Value),
) -> ToolSpec {
    let mut tool = iroha_gov_post_tool_with_fields(
        name,
        description,
        path_template,
        &[
            (
                "network_id",
                norito::json!({
                    "type": "string",
                    "pattern": "^hash:[0-9A-F]{64}#[0-9A-F]{4}$",
                    "minLength": 74,
                    "maxLength": 74,
                    "description": "Exact canonical genesis-derived NetworkId; checksum and marker bit are verified by Torii."
                }),
            ),
            (
                "authority",
                norito::json!({
                    "type": "string",
                    "minLength": 1,
                    "description": "Canonical I105 account id equal to X-Iroha-Account."
                }),
            ),
            selector,
        ],
    );
    let schema = tool
        .input_schema
        .as_object_mut()
        .expect("governance ballot MCP schema is an object");
    schema.insert("required".to_owned(), norito::json!(["headers"]));
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("governance ballot MCP properties are an object");
    properties.insert(
        "headers".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "X-Iroha-Account",
                "X-Iroha-Signature",
                "X-Iroha-Timestamp-Ms",
                "X-Iroha-Nonce"
            ],
            "properties": {
                "X-Iroha-Account": { "type": "string", "minLength": 1 },
                "X-Iroha-Signature": { "type": "string", "minLength": 1 },
                "X-Iroha-Timestamp-Ms": { "type": "string", "pattern": "^(0|[1-9][0-9]*)$" },
                "X-Iroha-Nonce": { "type": "string", "minLength": 1 }
            },
            "description": "Canonical exact-network account signature over the exact JSON body forwarded by this tool."
        }),
    );
    tool
}
/// Build the exact-network ZK envelope ballot tool.
pub(super) fn iroha_gov_ballots_zk_v1_tool() -> ToolSpec {
    exact_network_governance_ballot_tool(
        "iroha.gov.ballots.zk_v1",
        "Draft a governance ZK v1 ballot (`/v1/gov/ballots/zk-v1`) without mutating ledger state. Requires exact-network canonical account headers over the forwarded body.",
        "/v1/gov/ballots/zk-v1",
        (
            "election_id",
            governance_selector_v1_schema("Canonical first-release governance election selector."),
        ),
    )
}
/// Build the exact-network ZK proof ballot tool.
pub(super) fn iroha_gov_ballots_zk_v1_ballot_proof_tool() -> ToolSpec {
    exact_network_governance_ballot_tool(
        "iroha.gov.ballots.zk_v1.ballot_proof",
        "Draft a governance ZK ballot proof bundle (`/v1/gov/ballots/zk-v1/ballot-proof`) without mutating ledger state. Requires exact-network canonical account headers over the forwarded body.",
        "/v1/gov/ballots/zk-v1/ballot-proof",
        (
            "election_id",
            governance_selector_v1_schema("Canonical first-release governance election selector."),
        ),
    )
}
/// Build the exact-network plain ballot tool.
pub(super) fn iroha_gov_ballots_plain_tool() -> ToolSpec {
    exact_network_governance_ballot_tool(
        "iroha.gov.ballots.plain",
        "Draft a governance plain ballot (`/v1/gov/ballots/plain`) without mutating ledger state. Requires exact-network canonical account headers over the forwarded body.",
        "/v1/gov/ballots/plain",
        (
            "referendum_id",
            governance_selector_v1_schema(
                "Canonical first-release governance referendum selector.",
            ),
        ),
    )
}
