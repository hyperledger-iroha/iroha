//! Shared public request validation binds independent custody without claiming payload validation.
use super::*;

#[test]
fn public_request_validation_rejects_invalid_or_substituted_coordinates() {
    let fixture = receipt_fixture();
    let custody = verify_signer_custody_use_v1(
        &fixture.receipt.custody_record,
        &fixture.binding,
        &fixture.trust,
        &fixture.current,
    )
    .unwrap();
    let original = fixture.receipt.request;
    original.validate_binding(&fixture.binding).unwrap();
    original.validate_custody(&custody).unwrap();
    for field in 0..7 {
        let mut changed = original;
        match field {
            0 => changed.operation_id = [0; 32],
            1 => changed.statement_digest = [0; 32],
            2 => changed.statement_size = 0,
            3 => changed.statement_size = SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1,
            4 => changed.binding_digest[0] ^= 1,
            5 => changed.original_custody.record_digest[0] ^= 1,
            _ => changed.original_custody.control_state_digest[0] ^= 1,
        }
        assert_ne!(
            changed, original,
            "mutation {field} must change the request"
        );
        assert_eq!(
            changed.validate_custody(&custody).unwrap_err(),
            if field < 4 {
                SignerFinalPromotionReceiptErrorV1::InvalidReceipt
            } else {
                SignerFinalPromotionReceiptErrorV1::StatementMismatch
            },
            "mutation {field}"
        );
    }
    let mut changed_control = fixture.current.clone();
    changed_control.current_anchor.state_digest[0] ^= 1;
    let changed_custody = verify_signer_custody_use_v1(
        &fixture.receipt.custody_record,
        &fixture.binding,
        &fixture.trust,
        &changed_control,
    )
    .expect("same enrolled key with a different independently supplied control cut");
    assert_eq!(
        original.validate_custody(&changed_custody).unwrap_err(),
        SignerFinalPromotionReceiptErrorV1::StatementMismatch
    );
}

#[test]
fn public_request_bounds_do_not_replace_prepared_statement_validation() {
    let fixture = receipt_fixture();
    let custody = verify_signer_custody_use_v1(
        &fixture.receipt.custody_record,
        &fixture.binding,
        &fixture.trust,
        &fixture.current,
    )
    .unwrap();
    let prepared =
        prepare_final_promotion_statement_v1(&fixture.message, &fixture.binding).unwrap();
    for size in [1, SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64] {
        let mut public = fixture.receipt.request;
        public.statement_size = size;
        assert_ne!(size, fixture.expected.statement_size);
        public.validate_custody(&custody).unwrap();
        public.validate_binding(&fixture.binding).unwrap();
        let mut expected = fixture.expected;
        expected.statement_size = size;
        assert_eq!(
            SignerFinalPromotionRequestV1::new(&custody, &expected, &prepared).unwrap_err(),
            SignerFinalPromotionReceiptErrorV1::StatementMismatch,
            "only the prepared constructor compares the exact statement bytes"
        );
    }
}

#[test]
fn public_binding_validation_rejects_malformed_custody_identities_before_digest_comparison() {
    let fixture = receipt_fixture();
    let original = fixture.receipt.request;
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| b.chain_id.clear(),
        |b| b.network_id = [0; 32],
        |b| b.runtime_handle = "unknown://promotion/primary".into(),
        |b| b.key_handle = "pkcs11:promotion/../primary".into(),
        |b| b.service_id = "promotion\0service".into(),
        |b| b.administrator_id = "invalid administrator".into(),
        |b| b.administrator_id = b.service_id.clone(),
        |b| {
            b.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "invalid deployment".into(),
            }
        },
        |b| b.key_revision = 0,
        |b| b.policy_revision = 0,
        |b| b.policy_digest = [0; 32],
    ];
    original.validate_binding(&fixture.binding).unwrap();
    for mutate in mutations {
        let mut binding = fixture.binding.clone();
        mutate(&mut binding);
        assert_ne!(binding, fixture.binding);
        assert_eq!(
            original.validate_binding(&binding),
            Err(SignerFinalPromotionReceiptErrorV1::Custody(
                SignerCustodyErrorV1::InvalidRecord
            )),
            "canonical binding validation must precede its changed digest"
        );
    }
}

#[test]
fn public_binding_validation_preserves_role_fourteen_and_full_binding_commitment() {
    let fixture = receipt_fixture();
    let original = fixture.receipt.request;
    for field in 0..4 {
        let mut binding = fixture.binding.clone();
        match field {
            0 => binding.role = SignerRoleV1::ReleaseManifest,
            1 => {
                binding.purpose = SignerPurposeBindingV1::StreamToken {
                    provider_id: [1; 32],
                }
            }
            2 => binding.algorithm = SignerKeyAlgorithmV1::MlDsa,
            3 => {
                binding.role = SignerRoleV1::FinalPromotionAccountTransaction;
                binding.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                    deployment_id: "promotion-primary".into(),
                };
                crate::signer::custody::validate_binding(&binding).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            original.validate_binding(&binding),
            Err(SignerFinalPromotionReceiptErrorV1::WrongPurpose)
        );
    }
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| b.chain_id = "other-chain".into(),
        |b| b.network_id[0] ^= 1,
        |b| b.runtime_handle = "hsm://promotion/alternate".into(),
        |b| b.key_handle = "pkcs11:promotion/alternate".into(),
        |b| b.service_id = "promotion-secondary".into(),
        |b| b.administrator_id = "promotion-security-secondary".into(),
        |b| {
            b.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "promotion-secondary".into(),
            }
        },
        |b| {
            b.public_key = iroha_crypto::KeyPair::try_from_seed(
                vec![0x71; 32],
                iroha_crypto::Algorithm::Ed25519,
            )
            .unwrap()
            .public_key()
            .clone()
        },
        |b| b.key_revision += 1,
        |b| b.policy_revision += 1,
        |b| b.policy_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut binding = fixture.binding.clone();
        mutate(&mut binding);
        assert_ne!(binding, fixture.binding);
        crate::signer::custody::validate_binding(&binding).unwrap();
        assert_eq!(
            original.validate_binding(&binding),
            Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch)
        );
    }
}

#[test]
fn public_binding_validation_requires_bounded_request_but_never_claims_current_custody() {
    let fixture = receipt_fixture();
    let original = fixture.receipt.request;
    for field in 0..7 {
        let mut request = original;
        match field {
            0 => request.operation_id = [0; 32],
            1 => request.statement_digest = [0; 32],
            2 => request.statement_size = 0,
            3 => request.statement_size = SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1,
            4 => request.original_custody.record_digest = [0; 32],
            5 => request.original_custody.control_state_digest = [0; 32],
            6 => request.binding_digest = [0; 32],
            _ => unreachable!(),
        }
        assert_eq!(
            request.validate_binding(&fixture.binding),
            Err(if field == 6 {
                SignerFinalPromotionReceiptErrorV1::StatementMismatch
            } else {
                SignerFinalPromotionReceiptErrorV1::InvalidReceipt
            })
        );
    }
    let custody = verify_signer_custody_use_v1(
        &fixture.receipt.custody_record,
        &fixture.binding,
        &fixture.trust,
        &fixture.current,
    )
    .unwrap();
    let mut another_custody = original;
    another_custody.original_custody.record_digest[0] ^= 1;
    another_custody.validate_binding(&fixture.binding).unwrap();
    assert_eq!(
        another_custody.validate_custody(&custody),
        Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch)
    );
}

#[test]
fn public_request_has_one_closed_json_and_canonical_frame() {
    let request = receipt_fixture().receipt.request;
    let canonical = norito::encode_canonical(&request).unwrap();
    assert_eq!(
        norito::decode_canonical::<SignerFinalPromotionRequestV1>(&canonical).unwrap(),
        request
    );
    let json = norito::json::to_json(&request).unwrap();
    assert_eq!(
        norito::json::from_str::<SignerFinalPromotionRequestV1>(&json).unwrap(),
        request
    );
    let extra = json.replacen('{', "{\"compatibility\":true,", 1);
    assert!(norito::json::from_str::<SignerFinalPromotionRequestV1>(&extra).is_err());
    assert_eq!(
        norito::encode_canonical(
            &norito::json::from_str::<SignerFinalPromotionRequestV1>(&json).unwrap()
        )
        .unwrap(),
        canonical
    );
}
