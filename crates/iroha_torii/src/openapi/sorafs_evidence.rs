//! SoraFS evidence-viewer schemas for Torii's OpenAPI document.
//!
//! Keeping this closed schema family together makes its insertion order
//! explicit while leaving the parent OpenAPI assembly order unchanged.

use norito::json::Map;

/// Insert the closed SoraFS evidence-viewer schema family in canonical order.
pub(super) fn insert_sorafs_evidence_schemas(schemas: &mut Map) {
    schemas.insert(
        "SorafsEvidenceHex32V1".to_owned(),
        norito::json!({
            "type": "string",
            "minLength": 64,
            "maxLength": 64,
            "pattern": "^[0-9a-f]{64}$",
            "description": "Canonical lowercase 32-byte hexadecimal value without a prefix."
        }),
    );
    schemas.insert(
        "SorafsEvidenceNonzeroHex32V1".to_owned(),
        norito::json!({
            "type": "string",
            "minLength": 64,
            "maxLength": 64,
            "pattern": "^(?!0{64}$)[0-9a-f]{64}$",
            "description": "Canonical lowercase non-zero 32-byte hexadecimal value without a prefix."
        }),
    );
    schemas.insert(
        "SorafsEvidenceSignatureHexV1".to_owned(),
        norito::json!({
            "type": "string",
            "minLength": 128,
            "maxLength": 128,
            "pattern": "^[0-9a-f]{128}$",
            "description": "Canonical lowercase 64-byte Ed25519 signature."
        }),
    );
    schemas.insert(
        "SorafsEvidenceReceiptCursorV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["sequence", "receipt_digest_hex"],
            "properties": {
                "sequence": { "type": "integer", "format": "uint64", "minimum": 1 },
                "receipt_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceSignedCheckpointAnchorV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "version",
                "checkpoint_generation",
                "predecessor_checkpoint_revision_hex",
                "predecessor_checkpoint_digest_hex",
                "checkpoint_digest_hex",
                "receipt_count",
                "chain_head",
                "compaction_archive_head_digest_hex",
                "checkpoint_store_handle",
                "checkpoint_store_revision",
                "checkpoint_store_policy_digest_hex",
                "signer_handle",
                "signer_public_key_hex",
                "signature_hex"
            ],
            "properties": {
                "version": { "type": "integer", "format": "uint16", "enum": [1] },
                "checkpoint_generation": { "type": "integer", "format": "uint64", "minimum": 1 },
                "predecessor_checkpoint_revision_hex": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                        { "type": "null" }
                    ],
                    "description": "Null only at checkpoint generation one."
                },
                "predecessor_checkpoint_digest_hex": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                        { "type": "null" }
                    ],
                    "description": "Null only at checkpoint generation one."
                },
                "checkpoint_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "receipt_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "chain_head": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceReceiptCursorV1" },
                        { "type": "null" }
                    ],
                    "description": "Null exactly when receipt_count is zero; otherwise sequence equals receipt_count."
                },
                "compaction_archive_head_digest_hex": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                        { "type": "null" }
                    ],
                    "description": "Exact signed compaction archive head digest, or null before the first compaction."
                },
                "checkpoint_store_handle": { "type": "string", "minLength": 1, "maxLength": 256 },
                "checkpoint_store_revision": { "type": "integer", "format": "uint64", "minimum": 1 },
                "checkpoint_store_policy_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "signer_handle": { "type": "string", "minLength": 1, "maxLength": 256 },
                "signer_public_key_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "signature_hex": { "$ref": "#/components/schemas/SorafsEvidenceSignatureHexV1" }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceSignedCompactionArchiveHeadV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "version",
                "generation",
                "predecessor_head_digest_hex",
                "predecessor_operation_id_hex",
                "operation_id_hex",
                "source_checkpoint_generation",
                "source_checkpoint_revision_hex",
                "source_checkpoint_anchor",
                "compacted_through_unix_ms",
                "maximum_records",
                "challenge_count",
                "session_count",
                "compacted_payload_digest_hex",
                "archive_handle",
                "archive_revision",
                "archive_policy_digest_hex",
                "archive_id_hex",
                "archive_public_key_hex",
                "signer_handle",
                "signer_public_key_hex",
                "signature_hex",
                "head_digest_hex",
                "archive_signature_hex"
            ],
            "properties": {
                "version": { "type": "integer", "format": "uint16", "enum": [1] },
                "generation": { "type": "integer", "format": "uint64", "minimum": 1 },
                "predecessor_head_digest_hex": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                        { "type": "null" }
                    ]
                },
                "predecessor_operation_id_hex": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                        { "type": "null" }
                    ]
                },
                "operation_id_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "source_checkpoint_generation": { "type": "integer", "format": "uint64", "minimum": 1 },
                "source_checkpoint_revision_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "source_checkpoint_anchor": {
                    "$ref": "#/components/schemas/SorafsEvidenceSignedCheckpointAnchorV1"
                },
                "compacted_through_unix_ms": { "type": "integer", "format": "uint64", "minimum": 1 },
                "maximum_records": { "type": "integer", "format": "uint32", "minimum": 1, "maximum": 1024 },
                "challenge_count": { "type": "integer", "format": "uint32", "minimum": 0 },
                "session_count": { "type": "integer", "format": "uint32", "minimum": 0 },
                "compacted_payload_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "archive_handle": { "type": "string", "minLength": 1, "maxLength": 256 },
                "archive_revision": { "type": "integer", "format": "uint64", "minimum": 1 },
                "archive_policy_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "archive_id_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "archive_public_key_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "signer_handle": { "type": "string", "minLength": 1, "maxLength": 256 },
                "signer_public_key_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "signature_hex": { "$ref": "#/components/schemas/SorafsEvidenceSignatureHexV1" },
                "head_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "archive_signature_hex": { "$ref": "#/components/schemas/SorafsEvidenceSignatureHexV1" }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceSignedReceiptV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "schema",
                "receipt_norito_b64",
                "sequence",
                "kind",
                "receipt_digest_hex",
                "previous_receipt_digest_hex",
                "issued_at_unix_ms",
                "signer_handle",
                "signer_public_key_hex",
                "signature_hex"
            ],
            "properties": {
                "schema": { "type": "string", "const": "sorafs.evidence.signed_receipt.v1" },
                "receipt_norito_b64": { "type": "string", "minLength": 1 },
                "sequence": { "type": "integer", "format": "uint64", "minimum": 1 },
                "kind": {
                    "type": "string",
                    "enum": [
                        "challenge_issued",
                        "session_issued",
                        "manifest_accessed",
                        "range_accessed",
                        "interaction_recorded",
                        "legal_hold_placed",
                        "legal_hold_released",
                        "retention_evaluated",
                        "erasure_completed",
                        "erasure_denied_legal_hold"
                    ]
                },
                "receipt_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "previous_receipt_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceHex32V1" },
                "issued_at_unix_ms": { "type": "integer", "format": "uint64", "minimum": 1 },
                "signer_handle": { "type": "string", "minLength": 1, "maxLength": 256 },
                "signer_public_key_hex": { "$ref": "#/components/schemas/SorafsEvidenceNonzeroHex32V1" },
                "signature_hex": { "$ref": "#/components/schemas/SorafsEvidenceSignatureHexV1" }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceAuditProjectionV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "schema",
                "version",
                "projection_norito_b64",
                "checkpoint_anchor",
                "compaction_archive_head",
                "predecessor",
                "page_limit",
                "has_more",
                "next_cursor",
                "projection_digest_hex",
                "receipts"
            ],
            "properties": {
                "schema": {
                    "type": "string",
                    "const": "sorafs.evidence.audit_transparency_projection.v1"
                },
                "version": { "type": "integer", "format": "uint16", "enum": [1] },
                "projection_norito_b64": { "type": "string", "minLength": 1 },
                "checkpoint_anchor": {
                    "$ref": "#/components/schemas/SorafsEvidenceSignedCheckpointAnchorV1"
                },
                "compaction_archive_head": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceSignedCompactionArchiveHeadV1" },
                        { "type": "null" }
                    ]
                },
                "predecessor": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceReceiptCursorV1" },
                        { "type": "null" }
                    ]
                },
                "page_limit": {
                    "type": "integer",
                    "format": "uint16",
                    "minimum": 1,
                    "maximum": 256
                },
                "has_more": { "type": "boolean" },
                "next_cursor": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/SorafsEvidenceReceiptCursorV1" },
                        { "type": "null" }
                    ]
                },
                "projection_digest_hex": { "$ref": "#/components/schemas/SorafsEvidenceHex32V1" },
                "receipts": {
                    "type": "array",
                    "maxItems": 256,
                    "items": { "$ref": "#/components/schemas/SorafsEvidenceSignedReceiptV1" }
                }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceAuditStatusV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "schema",
                "status_norito_b64",
                "challenge_count",
                "session_count",
                "receipt_count",
                "active_legal_hold_count",
                "retention_count",
                "erasure_count",
                "checkpoint_anchor"
            ],
            "properties": {
                "schema": { "type": "string", "const": "sorafs.evidence.audit_status.v1" },
                "status_norito_b64": { "type": "string", "minLength": 1 },
                "challenge_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "session_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "receipt_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "active_legal_hold_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "retention_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "erasure_count": { "type": "integer", "format": "uint64", "minimum": 0 },
                "checkpoint_anchor": {
                    "$ref": "#/components/schemas/SorafsEvidenceSignedCheckpointAnchorV1"
                }
            }
        }),
    );
    schemas.insert(
        "SorafsEvidenceApiErrorV1".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["schema", "error"],
            "properties": {
                "schema": { "type": "string", "const": "sorafs.evidence.error.v1" },
                "error": {
                    "type": "string",
                    "enum": [
                        "canonical_authentication_required",
                        "explicit_evidence_auditor_or_legal_role_required",
                        "invalid_query",
                        "after_sequence",
                        "after_receipt_digest_hex",
                        "expected_checkpoint_digest_hex",
                        "limit",
                        "unexpected_query",
                        "invalid_evidence_request",
                        "evidence_checkpoint_changed",
                        "evidence_resource_exhausted",
                        "evidence_viewer_unavailable",
                        "canonical_encoding_unavailable"
                    ]
                }
            }
        }),
    );
}
