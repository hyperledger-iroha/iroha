//! OpenAPI schemas for the exact-network account faucet protocol.

use norito::json::Map;

/// Insert the account faucet request, puzzle, and response schemas.
pub(super) fn insert(schemas: &mut Map) {
    schemas.insert(
        "AccountFaucetRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "pow_anchor_height", "pow_nonce_hex"],
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical domainless I105 account receiving the faucet claim."
                },
                "pow_anchor_height": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1
                },
                "pow_nonce_hex": {
                    "type": "string",
                    "pattern": "^(?:[0-9a-f]{2}){1,32}$",
                    "description": "One to 32 nonce bytes encoded as canonical lowercase hexadecimal."
                }
            }
        }),
    );
    schemas.insert(
        "AccountFaucetPuzzle".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "algorithm",
                "network_id",
                "chain_discriminant",
                "difficulty_bits",
                "anchor_height",
                "anchor_block_hash_hex",
                "challenge_salt_hex",
                "scrypt_log_n",
                "scrypt_r",
                "scrypt_p",
                "max_anchor_age_blocks"
            ],
            "additionalProperties": false,
            "properties": {
                "algorithm": {
                    "type": "string",
                    "enum": ["scrypt-leading-zero-bits-v2"]
                },
                "network_id": {
                    "$ref": "#/components/schemas/NetworkId",
                    "description": "Exact genesis-derived network identity whose raw 32 bytes are hashed immediately after the faucet domain separator."
                },
                "chain_discriminant": {
                    "type": "integer",
                    "format": "uint16",
                    "minimum": 0,
                    "maximum": 65535,
                    "description": "Display discriminator used to validate the canonical I105 account text; it is not a replay domain."
                },
                "difficulty_bits": {
                    "type": "integer",
                    "format": "uint8",
                    "minimum": 1,
                    "maximum": 255
                },
                "anchor_height": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1
                },
                "anchor_block_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                },
                "challenge_salt_hex": {
                    "oneOf": [
                        { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                        { "type": "null" }
                    ]
                },
                "scrypt_log_n": {
                    "type": "integer",
                    "format": "uint8",
                    "minimum": 1,
                    "maximum": 30
                },
                "scrypt_r": {
                    "type": "integer",
                    "format": "uint32",
                    "minimum": 1
                },
                "scrypt_p": {
                    "type": "integer",
                    "format": "uint32",
                    "minimum": 1
                },
                "max_anchor_age_blocks": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1
                }
            }
        }),
    );
    schemas.insert(
        "AccountFaucetResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "account_id",
                "asset_definition_id",
                "asset_id",
                "amount",
                "tx_hash_hex",
                "status"
            ],
            "additionalProperties": false,
            "properties": {
                "account_id": { "type": "string" },
                "asset_definition_id": { "type": "string" },
                "asset_id": { "type": "string" },
                "amount": { "$ref": "#/components/schemas/Quantity" },
                "tx_hash_hex": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                "status": { "type": "string", "enum": ["QUEUED"] }
            }
        }),
    );
}
