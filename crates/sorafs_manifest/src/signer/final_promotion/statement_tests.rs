//! Canonical first-release statement admission and independently bound public fixture controls.

use super::*;
use iroha_crypto::{Algorithm, PublicKey};
use norito::json;

const GOLDEN: &[u8] = include_bytes!("tests/statement_fixture.message");

fn binding() -> SignerCustodyBindingV1 {
    SignerCustodyBindingV1 {
        chain_id: "promotion-chain".into(),
        network_id: [0x11; 32],
        runtime_handle: "software://sorafs/final-promotion-provenance/primary".into(),
        key_handle: "software://sorafs/final-promotion-provenance/key7".into(),
        service_id: "promotion-primary".into(),
        administrator_id: "promotion-security-primary".into(),
        role: SignerRoleV1::FinalPromotionProvenance,
        purpose: SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: PublicKey::from_bytes(
            Algorithm::Ed25519,
            &hex::decode("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
                .unwrap(),
        )
        .unwrap(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x41; 32],
    }
}

fn document() -> Value {
    norito::json::from_slice(&GOLDEN[SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1.len()..]).unwrap()
}

fn message(value: &Value) -> Vec<u8> {
    let mut message = SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1.to_vec();
    message.extend_from_slice(
        canonical_ascii(value, SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1)
            .unwrap()
            .as_bytes(),
    );
    message
}

fn at_mut<'a>(value: &'a mut Value, path: &str) -> &'a mut Value {
    value.pointer_mut(path).expect("existing fixture field")
}

fn rejected(value: &Value) {
    assert!(prepare_final_promotion_statement_v1(&message(value), &binding()).is_err());
}

#[test]
fn exact_python_golden_preserves_message_hash_and_full_binding_commitment() {
    let binding = binding();
    let prepared = prepare_final_promotion_statement_v1(GOLDEN, &binding).unwrap();
    assert_eq!(prepared.message().as_ptr(), GOLDEN.as_ptr());
    assert_eq!(prepared.message(), GOLDEN);
    assert_eq!(prepared.len(), 3282);
    assert!(!prepared.is_empty());
    assert_eq!(
        hex::encode(prepared.sha256()),
        "1f58d9134725471671a4efd200841305576bfd8895878bb35be6aadec0991d65"
    );
    assert_eq!(prepared.sha256(), sha256(GOLDEN));
    assert_eq!(
        prepared.binding_digest(),
        digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", &binding).unwrap()
    );
    assert_eq!(message(&document()), GOLDEN);
    assert_eq!(ROOT_FIELDS.len(), 24);
    assert_eq!(AUTH_FIELDS.len(), 8);
    assert!(!format!("{prepared:?}").contains("promotion-chain"));
}

#[test]
fn every_root_and_authentication_field_is_mandatory_and_exclusive() {
    for field in ROOT_FIELDS {
        let mut value = document();
        value.as_object_mut().unwrap().remove(*field);
        rejected(&value);
    }
    for field in AUTH_FIELDS {
        let mut value = document();
        at_mut(&mut value, "/authentication")
            .as_object_mut()
            .unwrap()
            .remove(*field);
        rejected(&value);
    }
    for path in [
        None,
        Some("authentication"),
        Some("positive_output_sha256"),
        Some("python_runtime"),
    ] {
        let mut value = document();
        let object = match path {
            None => value.as_object_mut(),
            Some(key) => at_mut(&mut value, &format!("/{key}")).as_object_mut(),
        }
        .unwrap();
        object.insert("unexpected".into(), json!("ignored"));
        rejected(&value);
    }
    let mut value = document();
    at_mut(&mut value, "/negative_receipts")
        .as_array_mut()
        .unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), json!("ignored"));
    rejected(&value);
}

#[test]
fn provider_implementation_is_operator_selected_and_absent_from_signed_statement() {
    for scheme in ["software", "signer", "hsm", "kms", "pkcs11"] {
        let mut selected = binding();
        selected.runtime_handle = format!("{scheme}://sorafs/final-promotion-provenance/primary");
        selected.key_handle = format!("{scheme}://sorafs/final-promotion-provenance/key7");
        let prepared = prepare_final_promotion_statement_v1(GOLDEN, &selected)
            .expect("same authorized statement for any configured signer implementation");
        assert_eq!(prepared.message(), GOLDEN);
        assert_eq!(
            prepared.binding_digest(),
            digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", &selected).unwrap()
        );
    }
    for (path, field, claim) in [
        (None, "signing_backend", "hardware"),
        (None, "signer_qualification", "hardware-key-qualified"),
        (Some("/authentication"), "backend", "hardware"),
        (None, "signing_backend", "software"),
    ] {
        let mut value = document();
        let object = match path {
            None => value.as_object_mut(),
            Some(path) => at_mut(&mut value, path).as_object_mut(),
        }
        .unwrap();
        object.insert(field.into(), json!(claim));
        rejected(&value);
    }
}

#[test]
fn duplicate_keys_at_root_and_each_nested_object_are_rejected() {
    let source = std::str::from_utf8(GOLDEN).unwrap();
    for (needle, replacement) in [
        (
            "{\"aggregate_checker_sha256\":",
            "{\"schema\":\"duplicate\",\"aggregate_checker_sha256\":",
        ),
        (
            "\"authentication\":{",
            "\"authentication\":{\"kind\":\"external-ed25519\",",
        ),
        (
            "\"positive_output_sha256\":{",
            "\"positive_output_sha256\":{\"first_aggregate_sha256\":\"duplicate\",",
        ),
        (
            "\"python_runtime\":{",
            "\"python_runtime\":{\"version\":\"duplicate\",",
        ),
        (
            "\"negative_receipts\":[{",
            "\"negative_receipts\":[{\"mutation_id\":\"duplicate\",",
        ),
    ] {
        assert!(source.contains(needle));
        let candidate = source.replacen(needle, replacement, 1);
        assert!(prepare_final_promotion_statement_v1(candidate.as_bytes(), &binding()).is_err());
    }
}

#[test]
fn whitespace_key_order_escape_and_numeric_spellings_are_not_alternate_encodings() {
    let source = std::str::from_utf8(GOLDEN).unwrap();
    for candidate in [
        format!("{source}\n"),
        source.replacen(
            "{\"aggregate_checker_sha256\"",
            "{ \"aggregate_checker_sha256\"",
            1,
        ),
        source.replace(
            "\"baseline_input_count\":22",
            "\"baseline_input_count\":22.0",
        ),
        source.replace(
            "\"generated_at_unix\":1900000000",
            "\"generated_at_unix\":19e8",
        ),
        source.replace("\"CPython\"", "\"\\u0043Python\""),
        source.replace("https://github.com", "https:\\/\\/github.com"),
    ] {
        assert!(prepare_final_promotion_statement_v1(candidate.as_bytes(), &binding()).is_err());
    }
    let mut value = document();
    let root = value.as_object_mut().unwrap();
    let first = root.remove("aggregate_checker_sha256").unwrap();
    let mut reordered = norito::json::to_json(&value).unwrap();
    reordered.pop();
    reordered.push_str(&format!(
        ",\"aggregate_checker_sha256\":{}}}",
        norito::json::to_json(&first).unwrap()
    ));
    let mut candidate = SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1.to_vec();
    candidate.extend_from_slice(reordered.as_bytes());
    assert_eq!(
        prepare_final_promotion_statement_v1(&candidate, &binding()).unwrap_err(),
        FinalPromotionStatementErrorV1::InvalidEncoding
    );
}

#[test]
fn canonical_unicode_uses_python_lowercase_utf16_escapes() {
    let value = json!({"z": "é𝄞", "a": "\"\\\n\t\r\u{8}\u{c}\u{0}\u{7f}"});
    assert_eq!(
        canonical_ascii(&value, 1024).unwrap(),
        r#"{"a":"\"\\\n\t\r\b\f\u0000\u007f","z":"\u00e9\ud834\udd1e"}"#
    );
    let mut statement = document();
    *at_mut(&mut statement, "/python_runtime/implementation") = json!("Pyé𝄞");
    let canonical = message(&statement);
    assert!(prepare_final_promotion_statement_v1(&canonical, &binding()).is_ok());
    let uppercase = String::from_utf8(canonical.clone())
        .unwrap()
        .replace("\\u00e9", "\\u00E9");
    assert!(prepare_final_promotion_statement_v1(uppercase.as_bytes(), &binding()).is_err());
    let unescaped = String::from_utf8(canonical)
        .unwrap()
        .replace("\\u00e9", "é");
    assert!(prepare_final_promotion_statement_v1(unescaped.as_bytes(), &binding()).is_err());
}

#[test]
fn first_release_roles_profiles_and_signed_statuses_have_no_fallback() {
    for (field, wrong) in [
        (
            "schema",
            "sorafs.production_readiness.production_promotion_provenance.v2",
        ),
        ("status", "ready"),
        ("attestation_scope", "release-manifest"),
        ("signing_provider", "local"),
        ("oidc_identity_status", "pending"),
        ("cosign_provenance_status", "pending"),
    ] {
        let mut value = document();
        *at_mut(&mut value, &format!("/{field}")) = json!(wrong);
        rejected(&value);
    }
    for (field, wrong) in [("kind", "local-ed25519"), ("algorithm", "ml-dsa-65")] {
        let mut value = document();
        *at_mut(&mut value, &format!("/authentication/{field}")) = json!(wrong);
        rejected(&value);
    }
    let mut wrong = binding();
    wrong.role = SignerRoleV1::ReleaseManifest;
    wrong.purpose = SignerPurposeBindingV1::ReleaseManifest {
        deployment_id: "production-primary".into(),
    };
    assert_eq!(
        prepare_final_promotion_statement_v1(GOLDEN, &wrong).unwrap_err(),
        FinalPromotionStatementErrorV1::InvalidBinding
    );
    wrong.role = SignerRoleV1::Promotion;
    wrong.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    assert!(prepare_final_promotion_statement_v1(GOLDEN, &wrong).is_err());
    wrong.role = SignerRoleV1::FinalPromotionAccountTransaction;
    wrong.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "production-primary".into(),
    };
    assert_eq!(
        prepare_final_promotion_statement_v1(GOLDEN, &wrong).unwrap_err(),
        FinalPromotionStatementErrorV1::InvalidBinding
    );
}

#[test]
fn independent_chain_network_deployment_and_every_auth_identity_are_exact() {
    for (field, value) in [
        ("chain_id", json!("another-chain")),
        ("network_id_hex", json!("22".repeat(32))),
        ("deployment_id", json!("another-deployment")),
    ] {
        let mut candidate = document();
        *at_mut(&mut candidate, &format!("/{field}")) = value;
        rejected(&candidate);
    }
    for (field, value) in [
        ("service_id", json!("other-primary")),
        ("administrator_id", json!("other-security")),
        ("key_revision", json!(8_u64)),
        ("policy_revision", json!(10_u64)),
        ("policy_digest_sha256", json!("42".repeat(32))),
        ("public_key_fingerprint_sha256", json!("43".repeat(32))),
    ] {
        let mut candidate = document();
        *at_mut(&mut candidate, &format!("/authentication/{field}")) = value;
        rejected(&candidate);
    }
    let mut independent = binding();
    let first = prepare_final_promotion_statement_v1(GOLDEN, &independent).unwrap();
    independent.runtime_handle = "kms://sorafs/promotion/primary".into();
    let second = prepare_final_promotion_statement_v1(GOLDEN, &independent).unwrap();
    assert_eq!(first.sha256(), second.sha256());
    assert_ne!(first.binding_digest(), second.binding_digest());
    independent.key_handle = "software://sorafs/final-promotion-provenance/key8".into();
    assert_ne!(
        second.binding_digest(),
        prepare_final_promotion_statement_v1(GOLDEN, &independent)
            .unwrap()
            .binding_digest()
    );
    independent.network_id = [0; 32];
    assert_eq!(
        prepare_final_promotion_statement_v1(GOLDEN, &independent).unwrap_err(),
        FinalPromotionStatementErrorV1::InvalidBinding
    );
}

#[test]
fn integers_reject_bool_float_zero_overflow_and_wrong_baseline_count() {
    for replacement in [
        json!(true),
        json!(false),
        json!(22.0),
        json!("22"),
        json!(-1_i64),
        json!(0_u64),
        json!(23_u64),
    ] {
        let mut value = document();
        *at_mut(&mut value, "/baseline_input_count") = replacement;
        rejected(&value);
    }
    for replacement in [
        json!(true),
        json!(1.0),
        json!("1"),
        json!(0_u64),
        json!(u64::MAX),
    ] {
        let mut value = document();
        *at_mut(&mut value, "/generated_at_unix") = replacement;
        rejected(&value);
    }
    for field in ["key_revision", "policy_revision"] {
        for replacement in [json!(true), json!(7.0), json!(0_u64), json!("7")] {
            let mut value = document();
            *at_mut(&mut value, &format!("/authentication/{field}")) = replacement;
            rejected(&value);
        }
    }
    for timestamp in [1_u64, i64::MAX as u64] {
        let mut value = document();
        *at_mut(&mut value, "/generated_at_unix") = json!(timestamp);
        assert!(prepare_final_promotion_statement_v1(&message(&value), &binding()).is_ok());
    }
}

#[test]
fn all_digest_fields_reject_zero_uppercase_short_and_nonhex() {
    let fields = [
        "baseline_input_set_sha256",
        "negative_archive_manifest_sha256",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "cosign_bundle_sha256",
        "network_id_hex",
    ];
    for field in fields {
        for invalid in [
            "00".repeat(32),
            "AB".repeat(32),
            "g1".repeat(32),
            "12".repeat(31),
        ] {
            let mut value = document();
            *at_mut(&mut value, &format!("/{field}")) = json!(invalid);
            rejected(&value);
        }
    }
    for field in POSITIVE_FIELDS {
        let mut value = document();
        *at_mut(&mut value, &format!("/positive_output_sha256/{field}")) = json!("00".repeat(32));
        rejected(&value);
        let mut value = document();
        at_mut(&mut value, "/positive_output_sha256")
            .as_object_mut()
            .unwrap()
            .remove(*field);
        rejected(&value);
    }
    let mut value = document();
    *at_mut(&mut value, "/python_runtime/executable_sha256") = json!("00".repeat(32));
    rejected(&value);
    let mut value = document();
    *at_mut(&mut value, "/negative_receipts/0/sha256") = json!("00".repeat(32));
    rejected(&value);
}

#[test]
fn six_negative_rows_keep_exact_order_filename_shape_and_nonzero_digest() {
    for index in 0..NEGATIVE_CASES.len() {
        let mut value = document();
        at_mut(&mut value, "/negative_receipts")
            .as_array_mut()
            .unwrap()
            .remove(index);
        rejected(&value);
        let mut value = document();
        at_mut(&mut value, "/negative_receipts")
            .as_array_mut()
            .unwrap()
            .swap(index, (index + 1) % NEGATIVE_CASES.len());
        rejected(&value);
        let mut value = document();
        *at_mut(
            &mut value,
            &format!("/negative_receipts/{index}/receipt_file"),
        ) = json!("../receipt.json");
        rejected(&value);
        let mut value = document();
        *at_mut(
            &mut value,
            &format!("/negative_receipts/{index}/mutation_id"),
        ) = json!("another-case");
        rejected(&value);
    }
    let mut value = document();
    at_mut(&mut value, "/negative_receipts")
        .as_array_mut()
        .unwrap()
        .push(json!({}));
    rejected(&value);
    let mut value = document();
    *at_mut(&mut value, "/negative_receipts") = json!({});
    rejected(&value);
}

#[test]
fn empty_errors_detached_signatures_and_receipt_evidence_are_not_preimage_extensions() {
    for replacement in [json!(["failed"]), json!({}), json!(null), json!(false)] {
        let mut value = document();
        *at_mut(&mut value, "/errors") = replacement;
        rejected(&value);
    }
    for field in [
        "signature_hex",
        "operation_receipt",
        "custody_evidence",
        "completed_operation_state",
    ] {
        let mut value = document();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("11"));
        rejected(&value);
        let mut value = document();
        at_mut(&mut value, "/authentication")
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("11"));
        rejected(&value);
    }
}

#[test]
fn url_checks_are_bounded_syntax_and_do_not_claim_public_address_or_cosign_trust() {
    for url in [
        "https://example.com/path",
        "https://example.com/quoted%20path",
        "https://[2606:4700:4700::1111]/path",
        "https://127.0.0.1/path",
    ] {
        assert!(bounded_https_syntax(url));
    }
    for url in [
        "http://example.com",
        "https://",
        "https://USER@example.com",
        "https://example.com:443",
        "https://EXAMPLE.com",
        "https://example.com?token=secret",
        "https://example.com/#ref",
        "https://example.com/a b",
        "https://example.com/\\x",
        "https://[invalid]/x",
        "https://-bad.example/x",
        "https://example.com/%",
        "https://example.com/%x1",
        "https://example.com/\"bad\"",
    ] {
        assert!(!bounded_https_syntax(url), "{url}");
    }
    let mut value = document();
    *at_mut(&mut value, "/provenance_oidc_issuer") = json!("https://127.0.0.1/path");
    assert!(prepare_final_promotion_statement_v1(&message(&value), &binding()).is_ok());
    // Public-address classification and exact OIDC/cosign trust are independently mandatory.
}

#[test]
fn raw_prefix_depth_string_and_allocation_limits_precede_admission() {
    for message in [
        &[][..],
        &GOLDEN[1..],
        &GOLDEN[..SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1.len()],
    ] {
        assert!(prepare_final_promotion_statement_v1(message, &binding()).is_err());
    }
    let oversized = vec![b' '; SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 + 1];
    assert_eq!(
        prepare_final_promotion_statement_v1(&oversized, &binding()).unwrap_err(),
        FinalPromotionStatementErrorV1::ResourceLimit
    );
    let mut candidate = document();
    *at_mut(&mut candidate, "/python_runtime/version") = json!("a".repeat(MAX_TEXT_BYTES + 1));
    assert_eq!(
        prepare_final_promotion_statement_v1(&message(&candidate), &binding()).unwrap_err(),
        FinalPromotionStatementErrorV1::ResourceLimit
    );
    let mut candidate = document();
    *at_mut(&mut candidate, "/errors") = json!([[[[["too-deep"]]]]]);
    assert_eq!(
        prepare_final_promotion_statement_v1(&message(&candidate), &binding()).unwrap_err(),
        FinalPromotionStatementErrorV1::ResourceLimit
    );
    let zero_allocation = norito::DecodeLimits::new(32, MAX_TEXT_BYTES, MAX_ELEMENTS, 0, 4);
    let result = norito::with_decode_limits_scope(zero_allocation, || {
        prepare_final_promotion_statement_v1(GOLDEN, &binding())
    });
    assert_eq!(
        result.unwrap_err(),
        FinalPromotionStatementErrorV1::ResourceLimit
    );
}

#[test]
fn fixed_errors_never_echo_candidate_strings() {
    for error in [
        FinalPromotionStatementErrorV1::InvalidBinding,
        FinalPromotionStatementErrorV1::BindingMismatch,
        FinalPromotionStatementErrorV1::InvalidEncoding,
        FinalPromotionStatementErrorV1::InvalidSchema,
        FinalPromotionStatementErrorV1::ResourceLimit,
    ] {
        assert!(!error.to_string().contains("promotion-primary"));
    }
    for value in [
        json!(""),
        json!(" trimmed "),
        json!("line\nbreak"),
        json!(true),
    ] {
        assert!(text(&value).is_err());
    }
}
