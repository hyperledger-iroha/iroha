// Third bounded OpenAPI component-schema construction frame.

fn insert_openapi_schemas_part_3(schemas: &mut Map) {
    schemas.insert(
        "PublicLaneValidatorStatus".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["type"],
            "additionalProperties": false,
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["PendingActivation", "Active", "Jailed", "Exiting", "Exited", "Slashed"],
                    "description": "Lifecycle status for the validator."
                },
                "activates_at_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Activation epoch when pending; host auto-promotes once the current epoch meets this value."
                },
                "reason": {
                    "type": "string",
                    "nullable": true,
                    "description": "Jail reason when applicable."
                },
                "releases_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Release timestamp (ms) when exiting."
                },
                "slash_id": {
                    "type": "string",
                    "nullable": true,
                    "description": "Slash identifier when slashed."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneValidatorRecord".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "validator", "peer_id", "stake_account", "total_stake", "self_stake", "status"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane serviced by the validator."
                },
                "validator": {
                    "type": "string",
                    "description": "Validator authority account literal rendered as canonical I105."
                },
                "peer_id": {
                    "type": "string",
                    "description": "Peer identity bound to the validator for consensus and routed traffic."
                },
                "stake_account": {
                    "type": "string",
                    "description": "Account backing the bonded stake."
                },
                "total_stake": {
                    "type": "string",
                    "description": "Total bonded stake for the validator."
                },
                "self_stake": {
                    "type": "string",
                    "description": "Validator-supplied stake amount."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata (commission, endpoints, jurisdiction flags, etc.)."
                },
                "status": {
                    "$ref": "#/components/schemas/PublicLaneValidatorStatus"
                },
                "activation_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Epoch recorded when the validator first became active."
                },
                "activation_height": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Block height recorded when the validator first became active."
                },
                "last_reward_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Epoch that last produced a reward payout."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneValidatorListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of validator entries returned."
                },
                "items": {
                    "type": "array",
                    "description": "Validator records for the lane.",
                    "items": { "$ref": "#/components/schemas/PublicLaneValidatorRecord" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneUnbonding".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["request_id", "amount", "release_at_ms"],
            "additionalProperties": false,
            "properties": {
                "request_id": {
                    "type": "string",
                    "description": "Client-supplied identifier rendered as hex."
                },
                "amount": {
                    "type": "string",
                    "description": "Amount scheduled for release."
                },
                "release_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Release timestamp (ms)."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneStakeShare".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "validator", "staker", "bonded", "metadata", "pending_unbonds"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane serviced by the stake entry."
                },
                "validator": {
                    "type": "string",
                    "description": "Validator account literal rendered as canonical I105."
                },
                "staker": {
                    "type": "string",
                    "description": "Staker account literal rendered as canonical I105."
                },
                "bonded": {
                    "type": "string",
                    "description": "Bonded amount for the validator/staker pair."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata captured alongside the stake share."
                },
                "pending_unbonds": {
                    "type": "array",
                    "description": "Pending unbonding requests for the stake.",
                    "items": { "$ref": "#/components/schemas/PublicLaneUnbonding" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneStakeListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of stake entries returned."
                },
                "items": {
                    "type": "array",
                    "description": "Stake shares for the lane.",
                    "items": { "$ref": "#/components/schemas/PublicLaneStakeShare" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLanePendingReward".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "lane_id",
                "account",
                "asset",
                "last_claimed_epoch",
                "pending_through_epoch",
                "amount"
            ],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "account": {
                    "type": "string",
                    "description": "Account literal for the reward recipient."
                },
                "asset": {
                    "type": "string",
                    "description": "Asset identifier for the reward payouts."
                },
                "last_claimed_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Latest epoch that was already claimed (0 when none)."
                },
                "pending_through_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Latest epoch included in `amount`."
                },
                "amount": {
                    "type": "string",
                    "description": "Reward amount available to claim (decimal string)."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLanePendingRewardListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of pending reward entries."
                },
                "items": {
                    "type": "array",
                    "description": "Pending rewards for the lane/account.",
                    "items": { "$ref": "#/components/schemas/PublicLanePendingReward" }
                }
            }
        }),
    );
    schemas.insert(
        "DaDigest32".to_owned(),
        norito::json!({
            "type": "array",
            "minItems": 1,
            "maxItems": 1,
            "description": "Canonical Norito JSON layout for a transparent 32-byte digest wrapper.",
            "items": {
                "type": "array",
                "minItems": 32,
                "maxItems": 32,
                "items": { "type": "integer", "minimum": 0, "maximum": 255 }
            }
        }),
    );
    schemas.insert(
        "DaIrohaHash".to_owned(),
        norito::json!({
            "type": "string",
            "pattern": "^hash:[0-9A-F]{64}#[0-9A-F]{4}$",
            "description": "Canonical checksummed Iroha hash literal. Decoders also verify the CRC-16 checksum and marked-hash bit."
        }),
    );
    schemas.insert(
        "DaTaggedProofScheme".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["type", "value"],
            "additionalProperties": false,
            "properties": {
                "type": { "type": "string", "enum": ["MerkleSha256"] },
                "value": { "type": "null" }
            },
            "description": "First-release DA V1 supports MerkleSha256 only; unknown proof-scheme discriminants are rejected."
        }),
    );
    schemas.insert(
        "DaProofPolicy".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "dataspace_id", "alias", "proof_scheme"],
            "additionalProperties": false,
            "properties": {
                "lane_id": { "type": "integer", "format": "uint32" },
                "dataspace_id": { "type": "integer", "format": "uint64" },
                "alias": { "type": "string" },
                "proof_scheme": { "$ref": "#/components/schemas/DaTaggedProofScheme" }
            }
        }),
    );
    schemas.insert(
        "DaProofPolicyBundle".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["version", "policy_hash", "policies"],
            "additionalProperties": false,
            "properties": {
                "version": { "type": "integer", "format": "uint16", "const": 1 },
                "policy_hash": { "$ref": "#/components/schemas/DaIrohaHash" },
                "policies": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/DaProofPolicy" }
                }
            }
        }),
    );
    schemas.insert(
        "DaListSnapshot".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["block_height", "block_hash"],
            "additionalProperties": false,
            "properties": {
                "block_height": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Committed chain height observed when the first page was constructed."
                },
                "block_hash": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/DaIrohaHash" },
                        { "type": "null" }
                    ],
                    "description": "Canonical block hash at `block_height`; null only when `block_height` is zero."
                }
            },
            "oneOf": [
                {
                    "properties": {
                        "block_height": { "const": 0 },
                        "block_hash": { "type": "null" }
                    }
                },
                {
                    "properties": {
                        "block_height": {
                            "type": "integer",
                            "format": "uint64",
                            "minimum": 1
                        },
                        "block_hash": { "$ref": "#/components/schemas/DaIrohaHash" }
                    }
                }
            ],
            "description": "Exact canonical ledger tip that binds every continuation cursor to one immutable view."
        }),
    );
    schemas.insert(
        "DaCommitmentKey".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "epoch", "sequence"],
            "additionalProperties": false,
            "properties": {
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" }
            },
            "description": "Canonical DA commitment ordering key."
        }),
    );
    schemas.insert(
        "DaCommitmentListCursor".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["snapshot", "after"],
            "additionalProperties": false,
            "properties": {
                "snapshot": { "$ref": "#/components/schemas/DaListSnapshot" },
                "after": { "$ref": "#/components/schemas/DaCommitmentKey" }
            },
            "description": "Server-issued forward-only commitment cursor. Torii rejects noncanonical, unknown, or stale cursors."
        }),
    );
    schemas.insert(
        "DaCommitmentListRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "default": 100,
                    "description": "Maximum raw index rows to inspect. Defaults to 100; larger values are capped at 1,000. Filtered rows still consume the bound."
                },
                "cursor": { "$ref": "#/components/schemas/DaCommitmentListCursor" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProofRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "manifest_hash": { "$ref": "#/components/schemas/DaDigest32" },
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentQueryRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "manifest_hash": { "$ref": "#/components/schemas/DaDigest32" },
                "storage_ticket": { "$ref": "#/components/schemas/DaDigest32" },
                "alias": {
                    "type": "string",
                    "maxLength": 256,
                    "x-iroha-maxUtf8Bytes": 256,
                    "description": "Pin-alias lookup key; at most 256 UTF-8 bytes."
                },
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" }
            }
        }),
    );
    schemas.insert(
        "DaTaggedStorageClass".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["type", "value"],
            "additionalProperties": false,
            "properties": {
                "type": { "type": "string", "enum": ["Hot", "Warm", "Cold"] },
                "value": { "type": "null" }
            }
        }),
    );
    schemas.insert(
        "DaGovernanceTag".to_owned(),
        norito::json!({
            "type": "array",
            "minItems": 1,
            "maxItems": 1,
            "items": { "type": "string" },
            "description": "Canonical Norito JSON layout for the transparent governance-tag wrapper."
        }),
    );
    schemas.insert(
        "DaRetentionPolicy".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["hot_retention_secs", "cold_retention_secs", "required_replicas", "storage_class", "governance_tag"],
            "additionalProperties": false,
            "properties": {
                "hot_retention_secs": { "type": "integer", "format": "uint64" },
                "cold_retention_secs": { "type": "integer", "format": "uint64" },
                "required_replicas": { "type": "integer", "format": "uint16" },
                "storage_class": { "$ref": "#/components/schemas/DaTaggedStorageClass" },
                "governance_tag": { "$ref": "#/components/schemas/DaGovernanceTag" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentRecord".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "lane_id", "epoch", "sequence", "client_blob_id", "manifest_hash",
                "proof_scheme", "chunk_root", "proof_digest",
                "retention_class", "storage_ticket", "acknowledgement_sig"
            ],
            "additionalProperties": false,
            "properties": {
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" },
                "client_blob_id": { "$ref": "#/components/schemas/DaDigest32" },
                "manifest_hash": { "$ref": "#/components/schemas/DaDigest32" },
                "proof_scheme": { "$ref": "#/components/schemas/DaTaggedProofScheme" },
                "chunk_root": { "$ref": "#/components/schemas/DaIrohaHash" },
                "proof_digest": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/DaIrohaHash" },
                        { "type": "null" }
                    ]
                },
                "retention_class": { "$ref": "#/components/schemas/DaRetentionPolicy" },
                "storage_ticket": { "$ref": "#/components/schemas/DaDigest32" },
                "acknowledgement_sig": {
                    "type": "string",
                    "pattern": "^[0-9A-F]{128}$",
                    "description": "Canonical uppercase Ed25519 acknowledgement signature."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentLocation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["block_height", "index_in_bundle"],
            "additionalProperties": false,
            "properties": {
                "block_height": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "description": "Height of the block that sealed the commitment."
                },
                "index_in_bundle": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Index within the block's commitment bundle."
                }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentListCursor".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["snapshot", "after"],
            "additionalProperties": false,
            "properties": {
                "snapshot": { "$ref": "#/components/schemas/DaListSnapshot" },
                "after": { "$ref": "#/components/schemas/DaCommitmentLocation" }
            },
            "description": "Server-issued forward-only pin-intent cursor. Torii rejects noncanonical, unknown, or stale cursors."
        }),
    );
    schemas.insert(
        "DaPinIntentListRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "default": 100,
                    "description": "Maximum raw index rows to inspect. Defaults to 100; larger values are capped at 1,000. Filtered or inactive rows still consume the bound."
                },
                "cursor": { "$ref": "#/components/schemas/DaPinIntentListCursor" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentWithLocation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["commitment", "location"],
            "additionalProperties": false,
            "properties": {
                "commitment": { "$ref": "#/components/schemas/DaCommitmentRecord" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["policies", "commitments", "next_cursor"],
            "additionalProperties": false,
            "properties": {
                "policies": { "$ref": "#/components/schemas/DaProofPolicyBundle" },
                "commitments": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/DaCommitmentWithLocation" }
                },
                "next_cursor": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/DaCommitmentListCursor" },
                        { "type": "null" }
                    ],
                    "description": "Continuation for the next bounded scan, or null when the raw index is exhausted."
                }
            }
        }),
    );
    schemas.insert(
        "MerkleDirection".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["direction", "value"],
            "additionalProperties": false,
            "properties": {
                "direction": { "type": "string", "enum": ["Left", "Right"] },
                "value": { "type": "null" }
            },
            "description": "Canonical tagged Norito JSON direction of a sibling node."
        }),
    );
    schemas.insert(
        "MerklePathItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["sibling", "direction"],
            "additionalProperties": false,
            "properties": {
                "sibling": {
                    "$ref": "#/components/schemas/DaIrohaHash"
                },
                "direction": { "$ref": "#/components/schemas/MerkleDirection" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["commitment", "location", "bundle_hash", "bundle_len", "root", "path"],
            "additionalProperties": false,
            "properties": {
                "commitment": { "$ref": "#/components/schemas/DaCommitmentRecord" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" },
                "bundle_hash": {
                    "allOf": [{ "$ref": "#/components/schemas/DaIrohaHash" }],
                    "description": "Header commitment to the V1 tree version, leaf count, and Merkle root; this is not a hash of the full encoded bundle."
                },
                "bundle_len": {
                    "type": "integer",
                    "format": "uint32",
                    "minimum": 1,
                    "description": "Total number of commitments in the bundle."
                },
                "root": { "$ref": "#/components/schemas/DaIrohaHash" },
                "path": {
                    "type": "array",
                    "maxItems": 32,
                    "items": { "$ref": "#/components/schemas/MerklePathItem" },
                    "description": "Merkle path proving inclusion of the commitment."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProofPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["policies", "proof"],
            "additionalProperties": false,
            "properties": {
                "policies": { "$ref": "#/components/schemas/DaProofPolicyBundle" },
                "proof": { "$ref": "#/components/schemas/DaCommitmentProof" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProofResponse".to_owned(),
        norito::json!({
            "anyOf": [
                { "$ref": "#/components/schemas/DaCommitmentProofPayload" },
                { "type": "null" }
            ]
        }),
    );
    schemas.insert(
        "DaCommitmentVerifyResponse".to_owned(),
        norito::json!({
            "oneOf": [
                {
                    "type": "object",
                    "required": ["valid", "error"],
                    "additionalProperties": false,
                    "properties": {
                        "valid": { "const": true },
                        "error": { "type": "null" }
                    }
                },
                {
                    "type": "object",
                    "required": ["valid", "error"],
                    "additionalProperties": false,
                    "properties": {
                        "valid": { "const": false },
                        "error": {
                            "type": "string",
                            "minLength": 1,
                            "description": "Deterministic commitment-proof verification failure."
                        }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "DaPinIntent".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "lane_id", "epoch", "sequence", "storage_ticket", "manifest_hash",
                "alias", "owner"
            ],
            "additionalProperties": false,
            "properties": {
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" },
                "storage_ticket": { "$ref": "#/components/schemas/DaDigest32" },
                "manifest_hash": { "$ref": "#/components/schemas/DaDigest32" },
                "alias": {
                    "type": "string",
                    "nullable": true,
                    "maxLength": 256,
                    "x-iroha-maxUtf8Bytes": 256,
                    "description": "Optional pin alias; committed values contain at most 256 UTF-8 bytes."
                },
                "owner": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional canonical I105 universal account identifier."
                }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentWithLocation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["intent", "location"],
            "additionalProperties": false,
            "properties": {
                "intent": { "$ref": "#/components/schemas/DaPinIntent" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["intents", "next_cursor"],
            "additionalProperties": false,
            "properties": {
                "intents": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/DaPinIntentWithLocation" }
                },
                "next_cursor": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/DaPinIntentListCursor" },
                        { "type": "null" }
                    ],
                    "description": "Continuation for the next bounded scan, or null when the raw index is exhausted."
                }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["intent", "location", "bundle_hash", "bundle_len", "root", "path"],
            "additionalProperties": false,
            "properties": {
                "intent": { "$ref": "#/components/schemas/DaPinIntent" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" },
                "bundle_hash": {
                    "allOf": [{ "$ref": "#/components/schemas/DaIrohaHash" }],
                    "description": "Header commitment to the V1 tree version, leaf count, and Merkle root; this is not a hash of the full encoded bundle."
                },
                "bundle_len": {
                    "type": "integer",
                    "format": "uint32",
                    "minimum": 1
                },
                "root": { "$ref": "#/components/schemas/DaIrohaHash" },
                "path": {
                    "type": "array",
                    "maxItems": 32,
                    "items": { "$ref": "#/components/schemas/MerklePathItem" }
                }
            }
        }),
    );
    schemas.insert(
        "DaPinIntentProofResponse".to_owned(),
        norito::json!({
            "anyOf": [
                { "$ref": "#/components/schemas/DaPinIntentProof" },
                { "type": "null" }
            ]
        }),
    );
    schemas.insert(
        "DaPinIntentVerifyResponse".to_owned(),
        norito::json!({
            "oneOf": [
                {
                    "type": "object",
                    "required": ["valid", "error"],
                    "additionalProperties": false,
                    "properties": {
                        "valid": { "const": true },
                        "error": { "type": "null" }
                    }
                },
                {
                    "type": "object",
                    "required": ["valid", "error"],
                    "additionalProperties": false,
                    "properties": {
                        "valid": { "const": false },
                        "error": {
                            "type": "string",
                            "minLength": 1,
                            "description": "Deterministic pin-intent proof verification failure."
                        }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "OperatorWebAuthnOptionsResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["publicKey"],
            "additionalProperties": false,
            "properties": {
                "publicKey": { "$ref": "#/components/schemas/JsonValue" }
            }
        }),
    );
    schemas.insert(
        "OperatorWebAuthnRegistrationResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["status", "credential_id", "credentials_total"],
            "additionalProperties": false,
            "properties": {
                "status": { "type": "string" },
                "credential_id": { "type": "string" },
                "credentials_total": { "type": "integer", "format": "uint32" }
            }
        }),
    );
    schemas.insert(
        "OperatorWebAuthnLoginResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["status", "session_token", "expires_in_secs", "credential_id"],
            "additionalProperties": false,
            "properties": {
                "status": { "type": "string" },
                "session_token": { "type": "string" },
                "expires_in_secs": { "type": "integer", "format": "uint64" },
                "credential_id": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "AssetTransferRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "authority",
                "asset_definition_id",
                "asset_balance_scope",
                "amount",
                "destination",
                "fee_payment",
                "creation_time_ms",
                "transaction_ttl_ms"
            ],
            "additionalProperties": false,
            "properties": {
                "authority": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 512,
                    "description": "Exact canonical I105 single-key Ed25519 authority."
                },
                "asset_definition_id": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 64,
                    "description": "Exact canonical unprefixed Base58 asset-definition identifier."
                },
                "asset_balance_scope": {
                    "type": "string",
                    "maxLength": 30,
                    "pattern": "^(global|dataspace:(0|[1-9][0-9]{0,19}))$",
                    "description": "Explicit exact balance bucket; Torii additionally enforces the u64 range."
                },
                "amount": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 192,
                    "description": "Exact canonical strictly positive Iroha Quantity text."
                },
                "destination": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 512,
                    "description": "Exact canonical I105 destination account."
                },
                "memo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 256,
                    "description": "At most 256 UTF-8 bytes and free of Unicode control characters."
                },
                "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                "creation_time_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "description": "Explicit epoch milliseconds; at most five minutes old or thirty seconds in the future."
                },
                "transaction_ttl_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "maximum": 600000
                },
                "public_key_hex": {
                    "type": "string",
                    "minLength": 64,
                    "maxLength": 64,
                    "pattern": "^[0-9a-f]{64}$"
                },
                "signature_base64": {
                    "type": "string",
                    "minLength": 88,
                    "maxLength": 88,
                    "description": "Canonical padded base64 for an exact 64-byte Ed25519 signature."
                }
            },
            "oneOf": [
                {
                    "not": {
                        "anyOf": [
                            { "required": ["public_key_hex"] },
                            { "required": ["signature_base64"] }
                        ]
                    }
                },
                { "required": ["public_key_hex", "signature_base64"] }
            ]
        }),
    );
    schemas.insert(
        "ContractCallRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "entrypoint", "fee_payment"],
            "additionalProperties": false,
            "properties": {
                "authority": {"type": "string", "description": "Canonical transaction authority."},
                "public_key_hex": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                "signature_b64": {"type": "string", "minLength": 1},
                "contract_address": {"type": "string"},
                "contract_alias": {"type": "string"},
                "entrypoint": {"type": "string", "minLength": 1},
                "payload": {"$ref": "#/components/schemas/JsonValue"},
                "creation_time_ms": {"type": "integer", "format": "uint64"},
                "transaction_ttl_ms": {"type": "integer", "format": "uint64", "minimum": 1},
                "fee_payment": {"$ref": "#/components/schemas/FeePaymentIntent"}
            },
            "oneOf": [
                {"required": ["contract_address"], "not": {"required": ["contract_alias"]}},
                {"required": ["contract_alias"], "not": {"required": ["contract_address"]}}
            ]
        }),
    );
    schemas.insert(
        "ContractCallSimulateRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "entrypoint", "gas_limit"],
            "additionalProperties": false,
            "properties": {
                "authority": {"type": "string"},
                "contract_address": {"type": "string"},
                "contract_alias": {"type": "string"},
                "entrypoint": {"type": "string", "minLength": 1},
                "payload": {"$ref": "#/components/schemas/JsonValue"},
                "gas_limit": {"type": "integer", "format": "uint64", "minimum": 1}
            },
            "oneOf": [
                {"required": ["contract_address"], "not": {"required": ["contract_alias"]}},
                {"required": ["contract_alias"], "not": {"required": ["contract_address"]}}
            ]
        }),
    );
    schemas.insert(
        "AssetTransferIntent".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "chain_id",
                "authority",
                "asset_definition_id",
                "asset_balance_scope",
                "amount",
                "destination",
                "fee_payment",
                "creation_time_ms",
                "transaction_ttl_ms"
            ],
            "additionalProperties": false,
            "properties": {
                "chain_id": { "type": "string", "minLength": 1 },
                "authority": { "type": "string", "minLength": 1, "maxLength": 512 },
                "asset_definition_id": { "type": "string", "minLength": 1, "maxLength": 64 },
                "asset_balance_scope": { "type": "string", "maxLength": 30 },
                "amount": { "type": "string", "minLength": 1, "maxLength": 192 },
                "destination": { "type": "string", "minLength": 1, "maxLength": 512 },
                "memo": { "type": "string", "minLength": 1, "maxLength": 256 },
                "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                "creation_time_ms": { "type": "integer", "format": "uint64", "minimum": 1 },
                "transaction_ttl_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 1,
                    "maximum": 600000
                }
            }
        }),
    );
    schemas.insert(
        "AssetTransferReceipt".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "operation_kind",
                "status",
                "transport",
                "intent",
                "payload_signing_hash_hex"
            ],
            "additionalProperties": false,
            "properties": {
                "operation_kind": { "type": "string", "enum": ["asset_transfer"] },
                "status": { "type": "string", "enum": ["pending_signature", "submitted", "applied"] },
                "transport": { "type": "string", "enum": ["torii"] },
                "intent": { "$ref": "#/components/schemas/AssetTransferIntent" },
                "payload_signing_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                },
                "transaction_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                },
                "entrypoint_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                }
            }
        }),
    );
    schemas.insert(
        "AssetTransferResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "submitted", "intent", "receipt"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "submitted": {
                    "type": "boolean",
                    "description": "True when the final signed transaction was accepted or was already known."
                },
                "intent": { "$ref": "#/components/schemas/AssetTransferIntent" },
                "transaction_payload_b64": {
                    "type": "string",
                    "description": "Canonical padded base64 for the exact unsigned Norito TransactionPayload."
                },
                "signing_message_b64": {
                    "type": "string",
                    "minLength": 44,
                    "maxLength": 44,
                    "description": "Canonical padded base64 for the exact 32-byte HashOf<TransactionPayload>."
                },
                "transaction_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                },
                "entrypoint_hash_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{64}$"
                },
                "pipeline_status": {
                    "$ref": "#/components/schemas/PipelineTransactionStatusResponse"
                },
                "receipt": { "$ref": "#/components/schemas/AssetTransferReceipt" }
            }
        }),
    );
    schemas.insert(
        "MultisigAccountSelector".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Active canonical multisig AccountId; native policies can make this substantially longer than a single-key identifier."
                },
                "multisig_account_alias": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 512,
                    "pattern": "^[^\\s@]+@(?:[^\\s.@]+\\.)?[^\\s.@]+$",
                    "description": "Stable multisig alias in name@dataspace or name@domain.dataspace format."
                }
            },
            "oneOf": [
                { "required": ["multisig_account_id"] },
                { "required": ["multisig_account_alias"] }
            ]
        }),
    );
    schemas.insert(
        "MultisigSpecPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["signatories", "quorum", "transaction_ttl_ms"],
            "additionalProperties": false,
            "properties": {
                "signatories": {
                    "type": "object",
                    "description": "Map of signer account ids to weight.",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint8"
                    }
                },
                "quorum": {
                    "type": "integer",
                    "format": "uint16"
                },
                "transaction_ttl_ms": {
                    "type": "integer",
                    "format": "uint64"
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["instructions", "proposed_at_ms", "expires_at_ms", "approvals"],
            "additionalProperties": false,
            "properties": {
                "instructions": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "proposed_at_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "approvals": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "is_relayed": {
                    "anyOf": [
                        { "type": "boolean" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposeRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "fee_payment", "instructions"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                        "memo": { "type": "string" },
                        "instructions": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/MultisigProposeInstructionInput" }
                        }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposeInstructionInput".to_owned(),
        norito::json!({
            "type": "object",
            "description": "Structured JSON InstructionBox object. For native Norito, send the entire MultisigProposeRequest body as application/x-norito; the JSON instructions field does not accept per-instruction Norito blobs. Deliberate policy-change invalidation uses the custom payload documented by MultisigInvalidateOutstandingPayload."
        }),
    );
    schemas.insert(
        "MultisigApproveRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "fee_payment"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "ok",
                "resolved_multisig_account_id",
                "submitted",
                "proposal_id",
                "instructions_hash",
                "tx_hash_hex",
                "executed_tx_hash_hex",
                "creation_time_ms",
                "transaction_payload_b64",
                "signing_message_b64"
            ],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "proposal_id": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "instructions_hash": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "tx_hash_hex": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "executed_tx_hash_hex": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "creation_time_ms": { "oneOf": [{ "type": "integer", "format": "uint64" }, { "type": "null" }] },
                "transaction_payload_b64": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "signing_message_b64": { "oneOf": [{ "type": "string" }, { "type": "null" }] }
            }
        }),
    );
    schemas.insert(
        "MultisigContractCallProposeRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "entrypoint", "fee_payment"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "contract_address": { "type": "string" },
                        "contract_alias": { "type": "string" },
                        "entrypoint": { "type": "string" },
                        "payload": { "type": "object" },
                        "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigContractCallApproveRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "fee_payment"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigContractCallResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "resolved_multisig_account_id", "submitted"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "proposal_id": { "type": "string" },
                "instructions_hash": { "type": "string" },
                "tx_hash_hex": { "type": "string" },
                "executed_tx_hash_hex": { "type": "string" },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "transaction_payload_b64": { "type": "string" },
                "signing_message_b64": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "MultisigCancelRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "fee_payment"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigCancelResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "ok",
                "resolved_multisig_account_id",
                "submitted",
                "action",
                "target_proposal_id",
                "target_instructions_hash",
                "cancel_proposal_id",
                "cancel_instructions_hash",
                "tx_hash_hex",
                "executed_tx_hash_hex",
                "creation_time_ms",
                "transaction_payload_b64",
                "signing_message_b64"
            ],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "action": { "type": "string" },
                "target_proposal_id": { "type": "string" },
                "target_instructions_hash": { "type": "string" },
                "cancel_proposal_id": { "type": "string" },
                "cancel_instructions_hash": { "type": "string" },
                "tx_hash_hex": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "executed_tx_hash_hex": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "creation_time_ms": { "oneOf": [{ "type": "integer", "format": "uint64" }, { "type": "null" }] },
                "transaction_payload_b64": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                "signing_message_b64": { "oneOf": [{ "type": "string" }, { "type": "null" }] }
            }
        }),
    );
    schemas.insert(
        "MultisigSpecRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Active canonical multisig AccountId; native policies can make this substantially longer than a single-key identifier."
                },
                "multisig_account_alias": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 512,
                    "pattern": "^[^\\s@]+@(?:[^\\s.@]+\\.)?[^\\s.@]+$",
                    "description": "Canonical stable alias in name@dataspace or name@domain.dataspace form."
                }
            },
            "oneOf": [
                { "required": ["multisig_account_id"] },
                { "required": ["multisig_account_alias"] }
            ]
        }),
    );
    schemas.insert(
        "MultisigSpecResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "spec"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Resolved canonical multisig AccountId."
                },
                "spec": { "$ref": "#/components/schemas/MultisigSpecPayload" }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsQueryRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Active canonical multisig AccountId; native policies can make this substantially longer than a single-key identifier."
                },
                "multisig_account_alias": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 512,
                    "pattern": "^[^\\s@]+@(?:[^\\s.@]+\\.)?[^\\s.@]+$",
                    "description": "Canonical stable alias in name@dataspace or name@domain.dataspace form."
                },
                "status": {
                    "type": "array",
                    "maxItems": 4,
                    "uniqueItems": true,
                    "items": {
                        "type": "string",
                        "enum": ["COLLECTING_SIGNATURES", "FINALIZED", "CANCELED", "EXPIRED"]
                    }
                },
                "cursor": {
                    "oneOf": [
                        {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 512,
                            "pattern": "^[A-Za-z0-9_-]+$",
                            "description": "Opaque canonical base64url cursor returned by the preceding page."
                        },
                        { "type": "null" }
                    ]
                },
                "limit": {
                    "oneOf": [
                        {
                            "type": "integer",
                            "format": "uint64",
                            "minimum": 1,
                            "maximum": (crate::routing::MULTISIG_PROPOSALS_MAX_PAGE_LIMIT)
                        },
                        { "type": "null" }
                    ]
                }
            },
            "oneOf": [
                { "required": ["multisig_account_id"] },
                { "required": ["multisig_account_alias"] }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposalEntry".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["proposal_id", "instructions_hash", "operation_type", "proposal", "status"],
            "additionalProperties": false,
            "properties": {
                "proposal_id": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                "instructions_hash": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                "operation_type": { "type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[A-Z][A-Z0-9_]*$" },
                "intent": { "$ref": "#/components/schemas/JsonValue" },
                "proposal": { "$ref": "#/components/schemas/MultisigProposalPayload" },
                "status": {
                    "type": "string",
                    "enum": ["COLLECTING_SIGNATURES", "FINALIZED", "CANCELED", "EXPIRED"]
                },
                "terminal_at_ms": {
                    "oneOf": [
                        { "type": "integer", "format": "uint64" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsQueryResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "proposals"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Resolved canonical multisig AccountId."
                },
                "proposals": {
                    "type": "array",
                    "maxItems": (crate::routing::MULTISIG_PROPOSALS_MAX_PAGE_LIMIT),
                    "items": { "$ref": "#/components/schemas/MultisigProposalEntry" }
                },
                "next_cursor": {
                    "oneOf": [
                        { "type": "string", "minLength": 1, "maxLength": 512, "pattern": "^[A-Za-z0-9_-]+$" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsResolveRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Active canonical multisig AccountId; native policies can make this substantially longer than a single-key identifier."
                },
                "multisig_account_alias": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 512,
                    "pattern": "^[^\\s@]+@(?:[^\\s.@]+\\.)?[^\\s.@]+$",
                    "description": "Canonical stable alias in name@dataspace or name@domain.dataspace form."
                },
                "proposal_id": {
                    "type": "string",
                    "minLength": 64,
                    "maxLength": 64,
                    "pattern": "^[0-9a-f]{64}$"
                },
                "instructions_hash": {
                    "type": "string",
                    "minLength": 64,
                    "maxLength": 64,
                    "pattern": "^[0-9a-f]{64}$"
                }
            },
            "allOf": [
                {
                    "oneOf": [
                        { "required": ["multisig_account_id"] },
                        { "required": ["multisig_account_alias"] }
                    ]
                },
                {
                    "oneOf": [
                        { "required": ["proposal_id"] },
                        { "required": ["instructions_hash"] }
                    ]
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposalResolveResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "proposal_id", "instructions_hash", "operation_type", "proposal", "status"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Resolved canonical multisig AccountId."
                },
                "proposal_id": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                "instructions_hash": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                "operation_type": { "type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[A-Z][A-Z0-9_]*$" },
                "intent": { "$ref": "#/components/schemas/JsonValue" },
                "proposal": { "$ref": "#/components/schemas/MultisigProposalPayload" },
                "status": {
                    "type": "string",
                    "enum": ["COLLECTING_SIGNATURES", "FINALIZED", "CANCELED", "EXPIRED"]
                },
                "terminal_at_ms": {
                    "oneOf": [
                        { "type": "integer", "format": "uint64" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
}

#[inline(never)]
fn insert_account_recovery_schemas(schemas: &mut Map) {
    schemas.insert(
        "MultisigInvalidateOutstandingPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["InvalidateOutstanding"],
            "additionalProperties": false,
            "description": "Exact MultisigInstructionBox custom payload that atomically terminalizes every other outstanding proposal. It must execute as the target multisig account.",
            "properties": {
                "InvalidateOutstanding": {
                    "type": "object",
                    "required": ["account"],
                    "additionalProperties": false,
                    "properties": {
                        "account": { "type": "string", "minLength": 1 }
                    }
                }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryPolicySetRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_alias", "signer_account_id", "fee_payment", "guardians", "quorum", "timelock_ms"],
            "additionalProperties": false,
            "properties": {
                "account_alias": { "type": "string", "minLength": 3, "maxLength": 512 },
                "signer_account_id": { "type": "string", "minLength": 1 },
                "public_key_hex": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                "signature_b64": { "type": "string", "minLength": 88, "maxLength": 88 },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                "guardians": {
                    "type": "array",
                    "minItems": 3,
                    "maxItems": 3,
                    "items": { "$ref": "#/components/schemas/AccountRecoveryGuardian" }
                },
                "quorum": { "type": "integer", "format": "uint16", "const": 2 },
                "timelock_ms": { "type": "integer", "format": "uint64", "const": 259200000 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryGuardian".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account", "weight"],
            "additionalProperties": false,
            "properties": {
                "account": { "type": "string", "minLength": 1 },
                "weight": { "type": "integer", "format": "uint16", "const": 1 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryMultisigMember".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["public_key", "weight"],
            "additionalProperties": false,
            "properties": {
                "public_key": {
                    "type": "string",
                    "pattern": "^ed0120[0-9a-f]{64}$",
                    "description": "Canonical Iroha Ed25519 public-key literal."
                },
                "weight": { "type": "integer", "format": "uint16", "const": 1 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryMultisigPolicy".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["version", "threshold", "members"],
            "additionalProperties": false,
            "properties": {
                "version": { "type": "integer", "format": "uint8", "const": 1 },
                "threshold": {
                    "type": "integer",
                    "format": "uint16",
                    "minimum": 1,
                    "description": "Must be 1 for one member, or between 2 and N for two or more members."
                },
                "members": {
                    "type": "array",
                    "minItems": 1,
                    "uniqueItems": true,
                    "items": { "$ref": "#/components/schemas/AccountRecoveryMultisigMember" }
                }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryMultisigController".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["kind", "payload"],
            "additionalProperties": false,
            "properties": {
                "kind": { "const": "Multisig" },
                "payload": { "$ref": "#/components/schemas/AccountRecoveryMultisigPolicy" }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryPolicy".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["guardians", "quorum", "timelock_ms"],
            "additionalProperties": false,
            "properties": {
                "guardians": {
                    "type": "array",
                    "minItems": 3,
                    "maxItems": 3,
                    "uniqueItems": true,
                    "items": { "$ref": "#/components/schemas/AccountRecoveryGuardian" }
                },
                "quorum": { "type": "integer", "format": "uint16", "const": 2 },
                "timelock_ms": { "type": "integer", "format": "uint64", "const": 259200000 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryProposeRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_alias", "signer_account_id", "fee_payment", "new_controller"],
            "additionalProperties": false,
            "properties": {
                "account_alias": { "type": "string", "minLength": 3, "maxLength": 512 },
                "signer_account_id": { "type": "string", "minLength": 1 },
                "public_key_hex": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                "signature_b64": { "type": "string", "minLength": 88, "maxLength": 88 },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                "new_controller": {
                    "$ref": "#/components/schemas/AccountRecoveryMultisigController",
                    "description": "Native AccountController::Multisig JSON value with weight-one Ed25519 members."
                }
            }
        }),
    );
    for name in [
        "AccountRecoveryApproveRequest",
        "AccountRecoveryFinalizeRequest",
    ] {
        schemas.insert(
            name.to_owned(),
            norito::json!({
                "type": "object",
                "required": ["account_alias", "signer_account_id", "fee_payment"],
                "additionalProperties": false,
                "properties": {
                    "account_alias": { "type": "string", "minLength": 3, "maxLength": 512 },
                    "signer_account_id": { "type": "string", "minLength": 1 },
                    "public_key_hex": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                    "signature_b64": { "type": "string", "minLength": 88, "maxLength": 88 },
                    "creation_time_ms": { "type": "integer", "format": "uint64" },
                    "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" }
                }
            }),
        );
    }
    schemas.insert(
        "AccountRecoveryStatusRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_alias"],
            "additionalProperties": false,
            "properties": {
                "account_alias": { "type": "string", "minLength": 3, "maxLength": 512 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryMutationResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "action", "account_alias", "resolved_active_account_id", "submitted", "creation_time_ms", "fee_payment"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "action": { "type": "string", "enum": ["SET_POLICY", "PROPOSE", "APPROVE", "FINALIZE"] },
                "account_alias": { "type": "string" },
                "resolved_active_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "tx_hash_hex": {
                    "oneOf": [
                        { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                        { "type": "null" }
                    ]
                },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "fee_payment": { "$ref": "#/components/schemas/FeePaymentIntent" },
                "transaction_payload_b64": {
                    "oneOf": [
                        {
                            "type": "string",
                            "minLength": 4,
                            "description": "Canonical padded-base64 Norito TransactionPayload bytes."
                        },
                        { "type": "null" }
                    ]
                },
                "signing_message_b64": {
                    "oneOf": [
                        { "type": "string", "minLength": 44, "maxLength": 44 },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryInvalidatedProposalEvidence".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["proposal_id", "status", "terminal_at_ms"],
            "additionalProperties": false,
            "properties": {
                "proposal_id": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" },
                "status": { "type": "string", "enum": ["CANCELED", "EXPIRED"] },
                "terminal_at_ms": { "type": "integer", "format": "uint64", "minimum": 1 }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "active_account_id_at_proposal", "proposed_controller", "approvals", "invalidated_multisig_proposal_hashes", "proposed_by", "execute_after_ms", "status"],
            "additionalProperties": false,
            "properties": {
                "alias": { "$ref": "#/components/schemas/JsonValue" },
                "active_account_id_at_proposal": { "type": "string", "minLength": 1 },
                "proposed_controller": { "$ref": "#/components/schemas/AccountRecoveryMultisigController" },
                "approvals": {
                    "type": "array",
                    "uniqueItems": true,
                    "items": { "type": "string", "minLength": 1 }
                },
                "invalidated_multisig_proposal_hashes": {
                    "type": "array",
                    "uniqueItems": true,
                    "items": { "type": "string", "minLength": 64, "maxLength": 64, "pattern": "^[0-9a-f]{64}$" }
                },
                "proposed_by": { "type": "string", "minLength": 1 },
                "execute_after_ms": { "type": "integer", "format": "uint64" },
                "status": { "$ref": "#/components/schemas/JsonValue" }
            }
        }),
    );
    schemas.insert(
        "AccountRecoveryStatusResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_alias", "resolved_active_account_id", "invalidated_proposals", "invalidation_evidence_complete"],
            "additionalProperties": false,
            "properties": {
                "account_alias": { "type": "string" },
                "resolved_active_account_id": { "type": "string" },
                "policy": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/AccountRecoveryPolicy" },
                        { "type": "null" }
                    ]
                },
                "request": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/AccountRecoveryRequest" },
                        { "type": "null" }
                    ]
                },
                "invalidated_proposals": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/AccountRecoveryInvalidatedProposalEvidence" }
                },
                "invalidation_evidence_complete": { "type": "boolean" }
            }
        }),
    );
}

fn queue_error_snapshot_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["state", "queued", "capacity", "saturated"],
        "properties": {
            "state": {
                "type": "string",
                "enum": ["healthy", "saturated"],
                "description": "Queue state label, for example `healthy` or `saturated`."
            },
            "queued": {
                "type": "integer",
                "minimum": 0,
                "description": "Current queued transaction count."
            },
            "capacity": {
                "type": "integer",
                "minimum": 0,
                "description": "Configured queue capacity."
            },
            "saturated": {
                "type": "boolean",
                "description": "Whether the queue was saturated when the error was emitted."
            }
        }
    })
}

fn bounded_error_detail_text_schema(description: &str) -> Value {
    norito::json!({
        "type": "string",
        "minLength": 1,
        "maxLength": (utils::MAX_ERROR_DETAIL_CHARACTERS),
        "pattern": "^(?!\\s)(?:[^\\u0000-\\u001F\\u007F-\\u009F])*[^\\s\\u0000-\\u001F\\u007F-\\u009F]$",
        "description": (description)
    })
}

fn reject_code_schema(description: &str) -> Value {
    norito::json!({
        "type": "string",
        "minLength": 1,
        "maxLength": (utils::MAX_REJECT_CODE_BYTES),
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$",
        "description": (description)
    })
}

fn axt_error_details_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "code": (reject_code_schema("Stable AXT rejection code.")),
            "reason": (bounded_error_detail_text_schema("Human-readable AXT rejection label.")),
            "snapshot_version": {
                "type": "integer",
                "minimum": 0,
                "description": "AXT policy snapshot version used to reject the request."
            },
            "dataspace": {
                "type": "integer",
                "minimum": 0,
                "description": "Dataspace id involved in the rejection."
            },
            "lane": {
                "type": "integer",
                "minimum": 0,
                "description": "Lane id involved in the rejection."
            },
            "next_min_handle_era": {
                "type": "integer",
                "minimum": 0,
                "description": "Minimum handle era a client should use for retry."
            },
            "next_min_sub_nonce": {
                "type": "integer",
                "minimum": 0,
                "description": "Minimum sub-nonce a client should use for retry."
            }
        }
    })
}

fn fee_error_details_schema() -> Value {
    let codes = iroha_data_model::nexus::FeeRejectionCode::ALL
        .iter()
        .map(|code| code.as_str())
        .collect::<Vec<_>>();
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["code", "retryable"],
        "properties": {
            "code": {
                "type": "string",
                "enum": (codes),
                "description": "Stable fee-payment rejection code."
            },
            "retryable": {
                "type": "boolean",
                "description": "Whether the same signed request can succeed after a state or capacity change."
            },
            "program_id": (bounded_error_detail_text_schema("Exact canonical sponsor-program identifier selected by the signed transaction.")),
            "program_revision": {
                "type": "integer",
                "format": "uint64",
                "minimum": 1,
                "description": "Exact immutable sponsor-program revision selected by the signed transaction."
            },
            "asset_definition_id": (bounded_error_detail_text_schema("Canonical fee asset involved in the rejection.")),
            "required": {
                "type": "string",
                "minLength": 1,
                "maxLength": 156,
                "pattern": "^(?:0|[1-9][0-9]*)(?:\\.[0-9]*[1-9])?$",
                "description": "Canonical deterministic amount required at the observation state."
            },
            "available": {
                "type": "string",
                "minLength": 1,
                "maxLength": 156,
                "pattern": "^(?:0|[1-9][0-9]*)(?:\\.[0-9]*[1-9])?$",
                "description": "Canonical deterministic amount available at the observation state."
            },
            "rule_id": (bounded_error_detail_text_schema("Stable matching sponsor-program rule identifier.")),
            "observation_height": {
                "type": "integer",
                "format": "uint64",
                "minimum": 0,
                "description": "Consensus height at which the decision was evaluated."
            },
            "remediation": (bounded_error_detail_text_schema("Concrete repair or retry guidance."))
        }
    })
}

fn error_details_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "layer": (bounded_error_detail_text_schema("Public surface layer that produced the error.")),
            "reject_code": (reject_code_schema("ISO-20022-style or Torii-local rejection code when available.")),
            "queue": {
                "$ref": "#/components/schemas/QueueErrorSnapshot"
            },
            "retry_after_seconds": {
                "type": "integer",
                "minimum": 0,
                "description": "Suggested retry delay in seconds for transient errors."
            },
            "endpoint": (bounded_error_detail_text_schema("Endpoint associated with throttling or version failures; query strings and fragments are forbidden.")),
            "field": (bounded_error_detail_text_schema("Field associated with validation or decode failures.")),
            "expected": (bounded_error_detail_text_schema("Expected field value, status, profile, or discriminant.")),
            "actual": (bounded_error_detail_text_schema("Actual field value, status, profile, or discriminant.")),
            "profile": (bounded_error_detail_text_schema("Network profile involved in the error.")),
            "chain_discriminant": {
                "type": "integer",
                "minimum": 0,
                "maximum": 65535,
                "description": "I105 chain discriminant involved in the error."
            },
            "tx_hash": (bounded_error_detail_text_schema("Signed transaction hash involved in finality/status failures.")),
            "last_status": (bounded_error_detail_text_schema("Last observed transaction status when a finality wait failed.")),
            "hint": (bounded_error_detail_text_schema("Actionable debugging hint for callers.")),
            "axt": {
                "$ref": "#/components/schemas/AxtErrorDetails"
            },
            "fee": {
                "$ref": "#/components/schemas/FeeErrorDetails"
            }
        }
    })
}

fn shared_error_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["code", "message"],
        "properties": {
            "code": {
                "type": "string",
                "minLength": 1,
                "maxLength": 64,
                "pattern": "^[a-z0-9][a-z0-9_]{0,63}$",
                "description": "Application-specific error identifier."
            },
            "message": {
                "type": "string",
                "minLength": 1,
                "maxLength": (utils::MAX_ERROR_MESSAGE_CHARACTERS),
                "pattern": "^(?!\\s)(?:[^\\u0000-\\u001F\\u007F-\\u009F])*[^\\s\\u0000-\\u001F\\u007F-\\u009F]$",
                "description": "Non-empty human-readable error message without surrounding whitespace or control characters. This text is not a stable client identifier."
            },
            "details": {
                "anyOf": [
                    { "$ref": "#/components/schemas/ErrorDetails" },
                    { "type": "null" }
                ],
                "description": "Optional machine-readable error context such as queue pressure, retry hints, rejection codes, endpoint names, or AXT metadata."
            }
        }
    })
}

fn privacy_schema_ref(name: &str) -> Value {
    norito::json!({ "$ref": (format!("#/components/schemas/{name}")) })
}

fn privacy_closed_tagged_unit_schema(tag: &str, content: &str, labels: &[&str]) -> Value {
    let labels: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();
    let mut properties = Map::new();
    properties.insert(
        tag.to_owned(),
        norito::json!({ "type": "string", "enum": (labels) }),
    );
    properties.insert(content.to_owned(), norito::json!({ "type": "null" }));
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": [(tag), (content)],
        "properties": (properties)
    })
}

fn privacy_tagged_variant_schema(
    tag: &str,
    label: &str,
    content: &str,
    content_schema: Value,
) -> Value {
    let mut properties = Map::new();
    properties.insert(
        tag.to_owned(),
        norito::json!({ "type": "string", "const": (label) }),
    );
    properties.insert(content.to_owned(), content_schema);
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "required": [(tag), (content)],
        "properties": (properties)
    })
}

fn privacy_protocol_id_const_schema(label: &str) -> Value {
    privacy_tagged_variant_schema(
        "protocol",
        label,
        "value",
        norito::json!({ "type": "null" }),
    )
}

fn bootle_lantern_fixed_binary_schema(
    description: &str,
    byte_length: u64,
    wire_layout: &str,
) -> Value {
    norito::json!({
        "type": "string",
        "format": "binary",
        "contentMediaType": "application/x-norito",
        "description": description,
        "minLength": byte_length,
        "maxLength": byte_length,
        "x-iroha-exact-byte-length": byte_length,
        "x-iroha-wire-layout": wire_layout
    })
}

fn privacy_issuance_schemas(schemas: &mut Map) {
    schemas.insert(
        "BootleLanternIssuanceAuthorizeRequestV1".to_owned(),
        bootle_lantern_fixed_binary_schema(
            "The unique first-release authorization request representation: an empty zero-octet body. Content-Type is still required to be exactly application/x-norito.",
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_REQUEST_BYTES_V1,
            "empty",
        ),
    );
    schemas.insert(
        "BootleLanternIssuanceAuthorizationWireV1".to_owned(),
        bootle_lantern_fixed_binary_schema(
            "One exact canonical 320-byte ILA1 Bootle/Lantern issuance authorization.",
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_BYTES_V1,
            "ILA1[320]",
        ),
    );
    schemas.insert(
        "BootleLanternIssuanceIssueRequestV1".to_owned(),
        bootle_lantern_fixed_binary_schema(
            "The unique first-release issue request: canonical ILA1[320] immediately concatenated with canonical ILQ1[71576], without an outer envelope, padding, or trailing bytes.",
            BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1,
            "ILA1[320] || ILQ1[71576]",
        ),
    );
    schemas.insert(
        "BootleLanternIssuanceResponseWireV1".to_owned(),
        bootle_lantern_fixed_binary_schema(
            "One exact canonical 3,176-byte ILR1 Bootle/Lantern blind-issuance response.",
            BOOTLE_LANTERN_ISSUANCE_RESPONSE_BYTES_V1,
            "ILR1[3176]",
        ),
    );
}

fn privacy_capability_schemas(schemas: &mut Map) {
    const PROTOCOL_LABELS: [&str; 12] = [
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "verange-transparent-range-v1",
        "iroha-zk-ams-v1",
        "vega-existing-credential-zk-v0",
        "iroha-zk-x509-stark-p256-v0",
        "iroha-jindo-polynomial-commitment-v0",
        "iroha-bootle-lantern-anoncred-v1",
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
        "pq-masp-stark-v0",
    ];
    const PROOF_SYSTEM_LABELS: [&str; 9] = [
        "stark-fri-sha256-goldilocks",
        "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
        "anonymous-pgc-p256",
        "iroha-verange-p256",
        "vega-neutron-nova-spartan-hyrax-t256",
        "jindo-polynomial-commitment",
        "halo2-ipa-pasta",
        "fcmp-plus-plus-curve-tree-bulletproofs",
        "lantern-lnp22-module-linear-norm",
    ];
    const ENGINE_LABELS: [&str; 9] = [
        "native-goldilocks-stark-fri",
        "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
        "native-anonymous-pgc-p256",
        "native-verange-p256",
        "native-vega",
        "native-jindo",
        "native-halo2-orchard",
        "native-fcmp-plus-plus",
        "native-lantern-lnp22",
    ];

    schemas.insert(
        "PrivacyProtocolIdV1".to_owned(),
        privacy_closed_tagged_unit_schema("protocol", "value", &PROTOCOL_LABELS),
    );
    schemas.insert(
        "PrivacyProofSystemIdV1".to_owned(),
        privacy_closed_tagged_unit_schema("proof_system", "value", &PROOF_SYSTEM_LABELS),
    );
    schemas.insert(
        "PrivacyEngineIdV1".to_owned(),
        privacy_closed_tagged_unit_schema("engine", "value", &ENGINE_LABELS),
    );
    schemas.insert(
        "PrivacyFixed32BytesV1".to_owned(),
        norito::json!({
            "type": "array",
            "minItems": 32,
            "maxItems": 32,
            "items": { "type": "integer", "minimum": 0, "maximum": 255 }
        }),
    );
    schemas.insert(
        "PrivacyConsensusLimitsV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "max_actions_per_transaction",
                "max_actions_per_block",
                "max_proof_bytes_per_action",
                "max_action_bytes",
                "max_privacy_bytes_per_transaction",
                "max_privacy_bytes_per_block",
                "max_statement_and_encrypted_output_bytes_per_transaction",
                "max_nullifiers_per_action",
                "max_commitments_per_action",
                "retained_root_count"
            ],
            "properties": {
                "max_actions_per_transaction": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_actions_per_block": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_proof_bytes_per_action": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_action_bytes": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_privacy_bytes_per_transaction": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_privacy_bytes_per_block": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_statement_and_encrypted_output_bytes_per_transaction": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_nullifiers_per_action": { "type": "integer", "format": "uint32", "minimum": 1 },
                "max_commitments_per_action": { "type": "integer", "format": "uint32", "minimum": 1 },
                "retained_root_count": { "type": "integer", "format": "uint32", "minimum": 1 }
            }
        }),
    );
    schemas.insert(
        "PrivacyConsensusPolicyTighteningV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["scheduled_at_height", "effective_at_height", "next_limits"],
            "properties": {
                "scheduled_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                "effective_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                "next_limits": { "$ref": "#/components/schemas/PrivacyConsensusLimitsV1" }
            }
        }),
    );
    schemas.insert(
        "PrivacyConsensusPolicyV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["current_limits", "pending_tightening"],
            "properties": {
                "current_limits": { "$ref": "#/components/schemas/PrivacyConsensusLimitsV1" },
                "pending_tightening": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/PrivacyConsensusPolicyTighteningV1" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );

    let activation_limit_variants = vec![
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[0],
            "limits",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[1],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_anonymity_set_size", "max_recipient_count"],
                "properties": {
                    "max_anonymity_set_size": { "type": "integer", "format": "uint32", "enum": [16, 32, 64] },
                    "max_recipient_count": { "type": "integer", "format": "uint32", "minimum": 1, "maximum": 8 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[2],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_aggregation_count"],
                "properties": {
                    "max_aggregation_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[3],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_batch_size", "max_ring_size"],
                "properties": {
                    "max_batch_size": { "type": "integer", "format": "uint32", "minimum": 1 },
                    "max_ring_size": { "type": "integer", "format": "uint32", "enum": [16, 32, 64] }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[4],
            "limits",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[5],
            "limits",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[6],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_polynomial_count"],
                "properties": {
                    "max_polynomial_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[7],
            "limits",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[8],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_action_count"],
                "properties": {
                    "max_action_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[9],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_input_count", "max_output_count"],
                "properties": {
                    "max_input_count": { "type": "integer", "format": "uint32", "minimum": 1 },
                    "max_output_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[10],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_input_count", "max_output_count"],
                "properties": {
                    "max_input_count": { "type": "integer", "format": "uint32", "minimum": 1 },
                    "max_output_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "protocol",
            PROTOCOL_LABELS[11],
            "limits",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["max_input_count", "max_output_count"],
                "properties": {
                    "max_input_count": { "type": "integer", "format": "uint32", "minimum": 1 },
                    "max_output_count": { "type": "integer", "format": "uint32", "minimum": 1 }
                }
            }),
        ),
    ];
    schemas.insert(
        "PrivacyProtocolActivationLimitsV1".to_owned(),
        norito::json!({ "oneOf": (activation_limit_variants) }),
    );
    schemas.insert(
        "PrivacyProtocolLimitsTighteningV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["scheduled_at_height", "effective_at_height", "next_limits"],
            "properties": {
                "scheduled_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                "effective_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                "next_limits": { "$ref": "#/components/schemas/PrivacyProtocolActivationLimitsV1" }
            }
        }),
    );

    let lifecycle_variants = vec![
        privacy_tagged_variant_schema(
            "state",
            "proposed",
            "record",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["proposed_at_height", "activate_at_height"],
                "properties": {
                    "proposed_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "activate_at_height": { "type": "integer", "format": "uint64", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "state",
            "active",
            "record",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["proposed_at_height", "activated_at_height", "state_since_height"],
                "properties": {
                    "proposed_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "activated_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "state_since_height": { "type": "integer", "format": "uint64", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "state",
            "suspended",
            "record",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["proposed_at_height", "activated_at_height", "state_since_height"],
                "properties": {
                    "proposed_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "activated_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "state_since_height": { "type": "integer", "format": "uint64", "minimum": 1 }
                }
            }),
        ),
        privacy_tagged_variant_schema(
            "state",
            "retired",
            "record",
            norito::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["proposed_at_height", "activated_at_height", "state_since_height"],
                "properties": {
                    "proposed_at_height": { "type": "integer", "format": "uint64", "minimum": 1 },
                    "activated_at_height": {
                        "oneOf": [
                            { "type": "integer", "format": "uint64", "minimum": 1 },
                            { "type": "null" }
                        ]
                    },
                    "state_since_height": { "type": "integer", "format": "uint64", "minimum": 1 }
                }
            }),
        ),
    ];
    schemas.insert(
        "PrivacyProtocolLifecycleV1".to_owned(),
        norito::json!({ "oneOf": (lifecycle_variants) }),
    );
    schemas.insert(
        "PrivacyAssuranceV1".to_owned(),
        privacy_closed_tagged_unit_schema("assurance", "value", &["experimental"]),
    );
    schemas.insert(
        "PrivacyCompiledProfileSnapshotV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "protocol_id",
                "proof_system_id",
                "engine_id",
                "parameter_id",
                "parameter_digest",
                "verifier_digest",
                "statement_schema_digest",
                "engine_manifest_digest",
                "protocol_limits"
            ],
            "properties": {
                "protocol_id": { "$ref": "#/components/schemas/PrivacyProtocolIdV1" },
                "proof_system_id": { "$ref": "#/components/schemas/PrivacyProofSystemIdV1" },
                "engine_id": { "$ref": "#/components/schemas/PrivacyEngineIdV1" },
                "parameter_id": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "parameter_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "verifier_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "statement_schema_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "engine_manifest_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "protocol_limits": { "$ref": "#/components/schemas/PrivacyProtocolActivationLimitsV1" }
            }
        }),
    );
    schemas.insert(
        "PrivacyCompiledStatementSchemaErrorV1".to_owned(),
        privacy_closed_tagged_unit_schema(
            "schema_error",
            "detail",
            &["conflicting-stable-type-id", "missing-type-reference"],
        ),
    );
    let unavailable_variants = vec![
        privacy_tagged_variant_schema(
            "reason",
            "engine-unavailable",
            "detail",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "reason",
            "profile-initialization-failed",
            "detail",
            norito::json!({ "type": "null" }),
        ),
        privacy_tagged_variant_schema(
            "reason",
            "statement-schema-invalid",
            "detail",
            privacy_schema_ref("PrivacyCompiledStatementSchemaErrorV1"),
        ),
    ];
    schemas.insert(
        "PrivacyCompiledProfileUnavailableReasonV1".to_owned(),
        norito::json!({ "oneOf": (unavailable_variants) }),
    );
    let profile_result_variants = vec![
        privacy_tagged_variant_schema(
            "status",
            "available",
            "value",
            privacy_schema_ref("PrivacyCompiledProfileSnapshotV1"),
        ),
        privacy_tagged_variant_schema(
            "status",
            "unavailable",
            "value",
            privacy_schema_ref("PrivacyCompiledProfileUnavailableReasonV1"),
        ),
    ];
    schemas.insert(
        "PrivacyCompiledProfileResultV1".to_owned(),
        norito::json!({ "oneOf": (profile_result_variants) }),
    );
    schemas.insert(
        "PrivacyProtocolActivationRecordV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "protocol_id",
                "proof_system_id",
                "engine_id",
                "parameter_id",
                "parameter_digest",
                "verifier_digest",
                "statement_schema_digest",
                "engine_manifest_digest",
                "lifecycle",
                "protocol_limits",
                "pending_protocol_limits_tightening",
                "assurance"
            ],
            "properties": {
                "protocol_id": { "$ref": "#/components/schemas/PrivacyProtocolIdV1" },
                "proof_system_id": { "$ref": "#/components/schemas/PrivacyProofSystemIdV1" },
                "engine_id": { "$ref": "#/components/schemas/PrivacyEngineIdV1" },
                "parameter_id": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "parameter_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "verifier_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "statement_schema_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "engine_manifest_digest": { "$ref": "#/components/schemas/PrivacyFixed32BytesV1" },
                "lifecycle": { "$ref": "#/components/schemas/PrivacyProtocolLifecycleV1" },
                "protocol_limits": { "$ref": "#/components/schemas/PrivacyProtocolActivationLimitsV1" },
                "pending_protocol_limits_tightening": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/PrivacyProtocolLimitsTighteningV1" },
                        { "type": "null" }
                    ]
                },
                "assurance": { "$ref": "#/components/schemas/PrivacyAssuranceV1" }
            }
        }),
    );
    schemas.insert(
        "PrivacyCapabilityRowV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["protocol_id", "compiled_profile", "activation"],
            "properties": {
                "protocol_id": { "$ref": "#/components/schemas/PrivacyProtocolIdV1" },
                "compiled_profile": { "$ref": "#/components/schemas/PrivacyCompiledProfileResultV1" },
                "activation": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/PrivacyProtocolActivationRecordV1" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );

    let ordered_protocol_rows = PROTOCOL_LABELS
        .into_iter()
        .map(|label| {
            norito::json!({
                "allOf": [
                    { "$ref": "#/components/schemas/PrivacyCapabilityRowV1" },
                    {
                        "type": "object",
                        "properties": {
                            "protocol_id": (privacy_protocol_id_const_schema(label))
                        }
                    }
                ]
            })
        })
        .collect::<Vec<_>>();
    schemas.insert(
        "PrivacyCapabilitySnapshotV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["version", "committed_height", "consensus_policy", "protocols"],
            "properties": {
                "version": { "type": "integer", "format": "uint32", "const": 1 },
                "committed_height": { "type": "integer", "format": "uint64", "minimum": 0 },
                "consensus_policy": { "$ref": "#/components/schemas/PrivacyConsensusPolicyV1" },
                "protocols": {
                    "type": "array",
                    "minItems": 12,
                    "maxItems": 12,
                    "prefixItems": (ordered_protocol_rows),
                    "items": false
                }
            }
        }),
    );
}
