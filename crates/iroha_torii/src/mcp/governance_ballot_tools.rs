//! Exact-network governance ballot tool schemas.
use super::{ToolSpec, canonical_account_auth_headers_schema, iroha_gov_post_tool_with_fields};
use norito::json::Value;
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
                    "description": "Canonical I105 AccountId carried in the JSON ballot body. The X-Iroha-Account HTTP header separately uses lowercase canonical 0x address hex or an exact canonical ASCII alias for the signer."
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
        canonical_account_auth_headers_schema(
            "Canonical exact-network signature tuple or multisig witness over the exact JSON body forwarded by this tool. X-Iroha-Account is optional with a witness. Its ASCII signer encoding is distinct from the body's canonical I105 authority encoding, which route logic matches to the verified proof subject.",
        ),
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
