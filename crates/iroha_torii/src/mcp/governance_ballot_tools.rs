//! Exact-network governance ballot tool schemas.
use super::{ToolSpec, canonical_account_auth_headers_schema, iroha_gov_post_tool_with_fields};
use norito::json::Value;
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
const GOVERNANCE_HASH_LITERAL_PATTERN: &str =
    "^(?:[bB][lL][aA][kK][eE]2[bB]32:)?(?:0[xX])?[0-9a-fA-F]{64}$";
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
    required_request_fields: &[(&str, Value)],
    optional_request_fields: &[(&str, Value)],
) -> ToolSpec {
    let mut fields = vec![
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
    ];
    fields.extend(required_request_fields.iter().cloned());
    let mut tool = iroha_gov_post_tool_with_fields(name, description, path_template, &fields);
    let schema = tool
        .input_schema
        .as_object_mut()
        .expect("governance ballot MCP schema is an object");
    schema.insert("required".to_owned(), norito::json!(["headers"]));
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("governance ballot MCP properties are an object");
    {
        let body_properties = properties
            .get_mut("body")
            .and_then(Value::as_object_mut)
            .and_then(|body| body.get_mut("properties"))
            .and_then(Value::as_object_mut)
            .expect("governance ballot MCP body properties are an object");
        for (field, field_schema) in optional_request_fields {
            body_properties.insert((*field).to_owned(), field_schema.clone());
        }
    }
    for (field, field_schema) in optional_request_fields {
        properties.insert((*field).to_owned(), field_schema.clone());
    }
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
        &[
            (
                "election_id",
                governance_selector_v1_schema(
                    "Canonical first-release governance election selector.",
                ),
            ),
            (
                "backend",
                norito::json!({
                    "type": "string",
                    "minLength": 1,
                    "pattern": "^[^\\s\\u0000-\\u001F\\u007F-\\u009F]+$",
                    "description": "Exact non-empty proof backend token."
                }),
            ),
            (
                "envelope_b64",
                norito::json!({
                    "type": "string",
                    "minLength": 4,
                    "contentEncoding": "base64",
                    "description": "Non-empty base64-encoded ZK proof envelope."
                }),
            ),
        ],
        &[
            (
                "root_hint",
                norito::json!({
                    "type": ["string", "null"],
                    "pattern": (GOVERNANCE_HASH_LITERAL_PATTERN)
                }),
            ),
            (
                "owner",
                norito::json!({
                    "type": ["string", "null"],
                    "minLength": 1,
                    "description": "Optional canonical I105 lock owner; when present it must equal authority."
                }),
            ),
            (
                "amount",
                norito::json!({
                    "type": ["string", "null"],
                    "maxLength": 155,
                    "pattern": "^(0|[1-9][0-9]*)(\\.[0-9]{0,27}[1-9])?$"
                }),
            ),
            (
                "duration_blocks",
                norito::json!({
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "maximum": (u64::MAX)
                }),
            ),
            (
                "direction",
                norito::json!({
                    "type": ["string", "null"],
                    "enum": ["Aye", "Nay", "Abstain", null]
                }),
            ),
            (
                "nullifier",
                norito::json!({
                    "type": ["string", "null"],
                    "pattern": (GOVERNANCE_HASH_LITERAL_PATTERN)
                }),
            ),
        ],
    )
}
/// Build the exact-network ZK proof ballot tool.
pub(super) fn iroha_gov_ballots_zk_v1_ballot_proof_tool() -> ToolSpec {
    exact_network_governance_ballot_tool(
        "iroha.gov.ballots.zk_v1.ballot_proof",
        "Draft a governance ZK ballot proof bundle (`/v1/gov/ballots/zk-v1/ballot-proof`) without mutating ledger state. Requires exact-network canonical account headers over the forwarded body.",
        "/v1/gov/ballots/zk-v1/ballot-proof",
        &[
            (
                "election_id",
                governance_selector_v1_schema(
                    "Canonical first-release governance election selector.",
                ),
            ),
            (
                "ballot",
                norito::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["backend", "envelope_bytes"],
                    "properties": {
                        "backend": {
                            "type": "string",
                            "minLength": 1,
                            "pattern": "^[^\\s\\u0000-\\u001F\\u007F-\\u009F]+$"
                        },
                        "envelope_bytes": {
                            "type": "string",
                            "minLength": 4,
                            "contentEncoding": "base64"
                        },
                        "root_hint": {
                            "type": ["string", "null"],
                            "pattern": (GOVERNANCE_HASH_LITERAL_PATTERN)
                        },
                        "owner": {
                            "type": ["string", "null"],
                            "minLength": 1
                        },
                        "nullifier": {
                            "type": ["string", "null"],
                            "pattern": (GOVERNANCE_HASH_LITERAL_PATTERN)
                        },
                        "amount": {
                            "type": ["string", "null"],
                            "maxLength": 155,
                            "pattern": "^(0|[1-9][0-9]*)(\\.[0-9]{0,27}[1-9])?$"
                        },
                        "duration_blocks": {
                            "type": ["integer", "null"],
                            "minimum": 0,
                            "maximum": (u64::MAX)
                        },
                        "direction": {
                            "type": ["string", "null"],
                            "enum": ["Aye", "Nay", "Abstain", null]
                        }
                    },
                    "description": "Canonical BallotProof object; backend and non-empty base64 envelope_bytes are required."
                }),
            ),
        ],
        &[],
    )
}
/// Build the exact-network plain ballot tool.
pub(super) fn iroha_gov_ballots_plain_tool() -> ToolSpec {
    exact_network_governance_ballot_tool(
        "iroha.gov.ballots.plain",
        "Draft a governance plain ballot (`/v1/gov/ballots/plain`) without mutating ledger state. Requires exact-network canonical account headers over the forwarded body.",
        "/v1/gov/ballots/plain",
        &[
            (
                "referendum_id",
                governance_selector_v1_schema(
                    "Canonical first-release governance referendum selector.",
                ),
            ),
            (
                "owner",
                norito::json!({
                    "type": "string",
                    "minLength": 1,
                    "description": "Canonical I105 AccountId that owns the governance lock."
                }),
            ),
            (
                "amount",
                norito::json!({
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 155,
                    "pattern": "^(0|[1-9][0-9]*)(\\.[0-9]{0,27}[1-9])?$",
                    "description": "Canonical non-negative exact decimal Quantity."
                }),
            ),
            (
                "duration_blocks",
                norito::json!({
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 20,
                    "pattern": (GOVERNANCE_U64_DECIMAL_PATTERN),
                    "description": "Canonical unsigned decimal u64 block duration."
                }),
            ),
            (
                "direction",
                norito::json!({
                    "type": "string",
                    "enum": ["Aye", "Nay", "Abstain"]
                }),
            ),
        ],
        &[],
    )
}
