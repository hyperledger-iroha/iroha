//! Canonical path, transaction status, and contract argument tests.

use super::*;

#[test]
fn extract_runtime_upgrade_id_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "id": "upgrade-001" }
    });
    let upgrade_id =
        extract_runtime_upgrade_id_argument(args.as_object().expect("object")).expect("id");
    assert_eq!(upgrade_id, "upgrade-001");
}
#[test]
fn extract_height_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "height": 7 }
    });
    let height = extract_height_argument(args.as_object().expect("object")).expect("height");
    assert_eq!(height, "7");
}
#[test]
fn build_iso20022_payload_body_accepts_only_canonical_base64_bytes() {
    let xml = b"<Document>ok</Document>";
    let body_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, xml);
    let args = norito::json!({
        "body_base64": body_base64
    });
    let (body, content_type) =
        build_iso20022_payload_body(args.as_object().expect("object")).expect("iso body");
    assert_eq!(body, xml.to_vec());
    assert_eq!(content_type, Some("application/xml"));
    for retired in [
        norito::json!({ "message_xml": "<Document/>" }),
        norito::json!({ "xml": "<Document/>" }),
        norito::json!({ "body": "<Document/>" }),
    ] {
        build_iso20022_payload_body(retired.as_object().expect("object"))
            .expect_err("retired ISO 20022 payload shortcut must reject");
    }
}
#[test]
fn extract_definition_id_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" }
    });
    let definition_id =
        extract_definition_id_argument(args.as_object().expect("object")).expect("definition");
    assert_eq!(definition_id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
}
#[test]
fn extract_asset_id_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "asset_id": TEST_ASSET_ID }
    });
    let asset_id = extract_asset_id_argument(args.as_object().expect("object")).expect("asset id");
    assert_eq!(asset_id, TEST_ASSET_ID);
}
#[test]
fn extract_nft_id_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "nft_id": "nft-001" }
    });
    let nft_id = extract_nft_id_argument(args.as_object().expect("object")).expect("nft id");
    assert_eq!(nft_id, "nft-001");
}
#[test]
fn extract_rwa_id_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "rwa_id": "rwa-001" }
    });
    let rwa_id = extract_rwa_id_argument(args.as_object().expect("object")).expect("rwa id");
    assert_eq!(rwa_id, "rwa-001");
}
#[test]
fn extract_transaction_hash_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "hash": "deadbeef" }
    });
    let hash = extract_transaction_hash_argument(args.as_object().expect("object")).expect("hash");
    assert_eq!(hash, "deadbeef");
}
#[test]
fn extract_optional_transaction_hash_argument_accepts_only_exact_hash() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let args = norito::json!({
        "hash": (canonical_hash.clone())
    });
    let hash = extract_optional_transaction_hash_argument(args.as_object().expect("object"))
        .expect("valid hash")
        .expect("hash");
    assert_eq!(hash, canonical_hash);
}
#[test]
fn extract_transaction_status_hash_argument_accepts_only_exact_query_hash() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let args = norito::json!({
        "query": {
            "hash": (canonical_hash.clone())
        }
    });
    let hash = extract_transaction_status_hash_argument(args.as_object().expect("object"))
        .expect("valid query hash");
    assert_eq!(hash, canonical_hash);
}
#[test]
fn canonical_path_and_hash_extractors_reject_retired_aliases() {
    let cases: &[(Value, fn(&Map) -> Result<String, String>)] = &[
        (
            norito::json!({ "id": "upgrade-001" }),
            extract_runtime_upgrade_id_argument,
        ),
        (
            norito::json!({ "upgrade_id": "upgrade-001" }),
            extract_runtime_upgrade_id_argument,
        ),
        (norito::json!({ "height": 7 }), extract_height_argument),
        (
            norito::json!({ "block_height": 7 }),
            extract_height_argument,
        ),
        (
            norito::json!({ "definition_id": "definition" }),
            extract_definition_id_argument,
        ),
        (
            norito::json!({ "asset_id": "asset" }),
            extract_asset_id_argument,
        ),
        (norito::json!({ "id": "asset" }), extract_asset_id_argument),
        (norito::json!({ "nft_id": "nft" }), extract_nft_id_argument),
        (norito::json!({ "id": "nft" }), extract_nft_id_argument),
        (norito::json!({ "rwa_id": "rwa" }), extract_rwa_id_argument),
        (norito::json!({ "id": "rwa" }), extract_rwa_id_argument),
        (
            norito::json!({ "hash": "deadbeef" }),
            extract_transaction_hash_argument,
        ),
        (
            norito::json!({ "transaction_hash": "deadbeef" }),
            extract_transaction_hash_argument,
        ),
        (
            norito::json!({ "path": { "transaction_hash": "deadbeef" } }),
            extract_transaction_hash_argument,
        ),
    ];
    for (args, extract) in cases {
        extract(args.as_object().expect("object")).expect_err("retired path alias must reject");
    }
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    for args in [
        norito::json!({ "transaction_hash": (canonical_hash.clone()) }),
        norito::json!({ "query": { "hash": (canonical_hash.clone()) } }),
        norito::json!({ "query": { "transaction_hash": (canonical_hash.clone()) } }),
    ] {
        extract_optional_transaction_hash_argument(args.as_object().expect("object"))
            .expect_err("retired optional hash location must reject");
    }
    for args in [
        norito::json!({ "hash": (canonical_hash.clone()) }),
        norito::json!({ "transaction_hash": (canonical_hash.clone()) }),
        norito::json!({ "query": { "transaction_hash": (canonical_hash.clone()) } }),
    ] {
        extract_transaction_status_hash_argument(args.as_object().expect("object"))
            .expect_err("retired transaction status hash location must reject");
    }
}
#[test]
fn extract_transaction_hash_from_submit_result_accepts_encoded_submission_receipt() {
    let key_pair = checked_submission_receipt_signer_fixture();
    let tx_hash =
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xAB; 32]));
    let payload = iroha_data_model::transaction::TransactionSubmissionReceiptPayload {
        entrypoint_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::from(
            tx_hash.clone(),
        )),
        signed_transaction_hash: Some(tx_hash.clone()),
        submitted_at_ms: 1,
        submitted_at_height: 1,
        signer: key_pair.public_key().clone(),
    };
    let receipt =
        iroha_data_model::transaction::TransactionSubmissionReceipt::sign(payload, &key_pair);
    let receipt_bytes = norito::to_bytes(&receipt).expect("receipt bytes");
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, receipt_bytes);
    let submit_result = norito::json!({
        "status": 202,
        "body": encoded
    });
    let hash = extract_transaction_hash_from_submit_result(&submit_result).expect("hash");
    assert_eq!(hash, tx_hash.to_string());
}
#[test]
fn extract_transaction_hash_from_submit_result_accepts_tx_hash_hex_field() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let submit_result = norito::json!({
        "status": 202,
        "body": {
            "ok": true,
            "tx_hash_hex": (canonical_hash.clone())
        }
    });
    let hash = extract_transaction_hash_from_submit_result(&submit_result).expect("hash");
    assert_eq!(hash, canonical_hash);
}
#[test]
fn extract_transaction_hash_from_submit_result_accepts_json_receipt_payload() {
    let canonical_hash = format!("{}1", "a".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let submit_result = norito::json!({
        "status": 202,
        "body": {
            "payload": {
                "signed_transaction_hash": (canonical_hash.clone())
            },
            "signature": "ignored"
        }
    });
    let hash = extract_transaction_hash_from_submit_result(&submit_result).expect("hash");
    assert_eq!(hash, canonical_hash);
}
#[test]
fn extract_transaction_hash_from_submit_result_rejects_noncanonical_json_receipt_hashes() {
    let hash_body = "AB".repeat(iroha_crypto::Hash::LENGTH);
    let hash_literal = norito::literal::format("hash", &hash_body);
    for noncanonical in [hash_literal, hash_body] {
        let submit_result = norito::json!({
            "status": 202,
            "body": {
                "payload": {
                    "signed_transaction_hash": noncanonical
                },
                "signature": "ignored"
            }
        });
        extract_transaction_hash_from_submit_result(&submit_result)
            .expect_err("receipt hashes must already use canonical lowercase Iroha text");
    }

    let canonical_entrypoint_hash = format!("{}1", "0".repeat(63));
    let entrypoint_only = norito::json!({
        "status": 202,
        "body": {
            "payload": {
                "entrypoint_hash": (canonical_entrypoint_hash)
            }
        }
    });
    extract_transaction_hash_from_submit_result(&entrypoint_only)
        .expect_err("entrypoint hashes cannot be reinterpreted as signed transaction hashes");
}
#[test]
fn canonical_transaction_hash_rejects_unbounded_or_noncanonical_inputs() {
    let uppercase = format!("{}B", "A".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let unmarked = "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES);
    let oversized = "f".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES + 1);
    for invalid in [
        "deadbeef",
        uppercase.as_str(),
        unmarked.as_str(),
        oversized.as_str(),
    ] {
        canonical_transaction_hash(invalid).expect_err("noncanonical transaction hash");
    }
}
#[test]
fn transaction_status_query_borrows_canonical_hash() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let arguments = norito::json!({
        "query": {
            "hash": (canonical_hash.clone())
        }
    });
    let route = append_transaction_status_query(
        "/v1/pipeline/transactions/status".to_owned(),
        arguments.as_object().expect("arguments"),
        &canonical_hash,
    )
    .expect("status route");
    assert_eq!(
        route,
        format!("/v1/pipeline/transactions/status?hash={canonical_hash}&scope=global")
    );

    let with_scope_override = norito::json!({
        "query": {
            "hash": (canonical_hash.clone()),
            "scope": "local"
        }
    });
    append_transaction_status_query(
        "/v1/pipeline/transactions/status".to_owned(),
        with_scope_override.as_object().expect("arguments"),
        &canonical_hash,
    )
    .expect_err("callers cannot override the fixed global status scope");
}
#[test]
fn dispatch_source_keeps_source_sized_request_clones_closed() {
    let source = include_str!("../../mcp.rs");
    for forbidden in [
        "let mut adapted = arguments.clone();",
        "fn canonical_submit_arguments",
        "let mut status_arguments = Map::new();",
        "let headers = response.headers().clone();",
        "json::to_vec(body_value)",
    ] {
        assert!(!source.contains(forbidden), "found `{forbidden}`");
    }
    let borrowed_source = include_str!("../borrowed_dispatch.rs");
    for forbidden in [
        "form_urlencoded::Serializer",
        "json::to_string(value)",
        "urlencoding::encode",
    ] {
        assert!(
            !source.contains(forbidden) && !borrowed_source.contains(forbidden),
            "found `{forbidden}`"
        );
    }
}
#[test]
fn exact_global_pipeline_status_binds_hash_scope_and_shape() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let canonical = norito::json!({
        "status": 200,
        "body": {
            "hash": (canonical_hash.clone()),
            "status": {
                "kind": "Applied",
                "block_height": 7
            },
            "scope": "global",
            "resolved_from": "state"
        }
    });
    let decoded = decode_exact_global_pipeline_status(&canonical, &canonical_hash)
        .expect("exact global status");
    assert!(fixed_pipeline_status_is_applied(&decoded).expect("fixed outcome"));

    for (pointer, replacement) in [
        ("/body/hash", Value::from(format!("{}1", "a".repeat(63)))),
        ("/body/scope", Value::from("local")),
        ("/body/resolved_from", Value::from("legacy")),
    ] {
        let mut invalid = canonical.clone();
        *invalid.pointer_mut(pointer).expect("fixture pointer") = replacement;
        decode_exact_global_pipeline_status(&invalid, &canonical_hash)
            .expect_err("mismatched global status evidence must fail closed");
    }
    let mut unknown = canonical.clone();
    unknown
        .pointer_mut("/body")
        .and_then(Value::as_object_mut)
        .expect("body object")
        .insert("legacy".to_owned(), Value::Null);
    decode_exact_global_pipeline_status(&unknown, &canonical_hash)
        .expect_err("unknown response fields must fail closed");
}
#[test]
fn fixed_pipeline_finality_accepts_only_state_resolved_outcomes() {
    let response = |kind: &str, resolved_from: &str| PipelineTransactionStatusResponse {
        hash: format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1)),
        status: iroha_torii_shared::PipelineTransactionStatus {
            kind: kind.to_owned(),
            block_height: Some(7),
        },
        scope: "global".to_owned(),
        resolved_from: resolved_from.to_owned(),
    };
    assert!(fixed_pipeline_status_is_applied(&response("Applied", "state")).expect("applied"));
    assert!(
        !fixed_pipeline_status_is_applied(&response("Applied", "cache"))
            .expect("cached Applied remains pending")
    );
    for failure in ["Rejected", "Expired"] {
        fixed_pipeline_status_is_applied(&response(failure, "state"))
            .expect_err("fixed failure outcomes must never be configurable successes");
        for source in ["cache", "queue"] {
            assert!(
                !fixed_pipeline_status_is_applied(&response(failure, source))
                    .expect("non-state failure hints remain pending")
            );
        }
    }
    fixed_pipeline_status_is_applied(&response("applied", "state"))
        .expect_err("status spelling is exact and case-sensitive");
}
#[test]
fn applied_wait_status_poll_accepts_only_exact_200_or_404() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    assert!(
        exact_pipeline_status_poll_has_body(200, &canonical_hash).expect("HTTP 200 has a body")
    );
    assert!(
        !exact_pipeline_status_poll_has_body(404, &canonical_hash)
            .expect("HTTP 404 is the only pending response")
    );
    for status_code in [0, 201, 202, 204, 429, 500, 503] {
        let error = exact_pipeline_status_poll_has_body(status_code, &canonical_hash)
            .expect_err("every other HTTP response must fail closed");
        assert!(error.contains("expected exact HTTP 200"));
    }
}
#[test]
fn applied_wait_result_has_one_exact_v1_key_set() {
    let canonical_hash = format!("{}1", "0".repeat(CANONICAL_TRANSACTION_HASH_HEX_BYTES - 1));
    let final_result = norito::json!({
        "status": 200,
        "body": {
            "hash": (canonical_hash.clone()),
            "status": { "kind": "Applied", "block_height": 7 },
            "scope": "global",
            "resolved_from": "state"
        }
    });
    let result = build_transaction_applied_wait_result(
        &canonical_hash,
        3,
        25,
        Some(norito::json!({ "status": 202 })),
        final_result.clone(),
    )
    .expect("exact Applied wait result");
    let object = result.as_object().expect("result object");
    assert_eq!(object.len(), 7);
    for key in [
        "status",
        "hash",
        "terminal_kind",
        "attempts",
        "elapsed_ms",
        "submit",
        "final",
    ] {
        assert!(object.contains_key(key), "missing exact key `{key}`");
    }
    assert_eq!(
        object.get("terminal_kind").and_then(Value::as_str),
        Some("Applied")
    );
    assert!(!object.contains_key("tx_hash"));
    assert!(!object.contains_key("final_status"));

    let without_submit =
        build_transaction_applied_wait_result(&canonical_hash, 1, 0, None, final_result)
            .expect("exact read-only Applied wait result");
    let object = without_submit.as_object().expect("result object");
    assert_eq!(object.len(), 6);
    assert!(!object.contains_key("submit"));
}
#[test]
fn extract_code_hash_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "code_hash": "cafebabe" }
    });
    let hash = extract_code_hash_argument(args.as_object().expect("object")).expect("hash");
    assert_eq!(hash, "cafebabe");
}
#[test]
fn extract_contract_address_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": {
            "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
        }
    });
    let contract_address = extract_contract_address_argument(args.as_object().expect("object"))
        .expect("contract address");
    assert_eq!(
        contract_address,
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    );
}
#[test]
fn extract_instruction_index_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "index": 3 }
    });
    let index =
        extract_instruction_index_argument(args.as_object().expect("object")).expect("index");
    assert_eq!(index, "3");
}
#[test]
fn extract_block_identifier_argument_requires_canonical_path_field() {
    let args = norito::json!({
        "path": { "identifier": 7 }
    });
    let identifier =
        extract_block_identifier_argument(args.as_object().expect("object")).expect("id");
    assert_eq!(identifier, "7");
}
#[test]
fn remaining_canonical_path_extractors_reject_retired_flat_aliases() {
    let cases: [(Value, fn(&Map) -> Result<String, String>); 11] = [
        (
            norito::json!({ "code_hash": "cafebabe" }),
            extract_code_hash_argument,
        ),
        (
            norito::json!({ "hash": "cafebabe" }),
            extract_code_hash_argument,
        ),
        (
            norito::json!({ "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw" }),
            extract_contract_address_argument,
        ),
        (
            norito::json!({ "index": 7 }),
            extract_instruction_index_argument,
        ),
        (
            norito::json!({ "instruction_index": 7 }),
            extract_instruction_index_argument,
        ),
        (
            norito::json!({ "path": { "instruction_index": 7 } }),
            extract_instruction_index_argument,
        ),
        (
            norito::json!({ "identifier": 7 }),
            extract_block_identifier_argument,
        ),
        (
            norito::json!({ "block_identifier": 7 }),
            extract_block_identifier_argument,
        ),
        (
            norito::json!({ "block_height": 7 }),
            extract_block_identifier_argument,
        ),
        (
            norito::json!({ "block_hash": "cafebabe" }),
            extract_block_identifier_argument,
        ),
        (
            norito::json!({ "path": { "block_height": 7 } }),
            extract_block_identifier_argument,
        ),
    ];
    for (args, extract) in cases {
        extract(args.as_object().expect("object")).expect_err("retired path alias must reject");
    }
}
