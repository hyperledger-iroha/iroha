//! Final-promotion receipt regressions using independent synthetic signer, attester and observer keys.

use super::*;
use crate::signer::{custody::*, protocol::*, receipt::SignerReceiptErrorV1};
use iroha_crypto::Signature;

pub(crate) mod fixtures {
    use crate as manifest;
    include!("tests/fixture_support.rs");
}
use fixtures::{ReceiptFixture, receipt_fixture, verify_receipt};

mod request_validation;

#[test]
fn exact_canonical_receipt_roundtrips_and_recovers_after_timely_completion() {
    let fixture = receipt_fixture();
    let bytes = norito::encode_canonical(&fixture.receipt).unwrap();
    let decoded: SignerFinalPromotionReceiptV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, fixture.receipt);
    let verified = verify_receipt(&fixture).unwrap();
    assert_eq!(verified.completion(), &fixture.completion);
    assert_eq!(
        verified.statement_digest(),
        fixture.expected.statement_digest
    );
    assert_eq!(
        verified.custody().record_digest(),
        fixture.current.active_head.record_digest
    );
    assert!(!format!("{verified:?}").contains("key-7"));
    assert!(!format!("{:?}", fixture.receipt).contains("production-primary"));
}

#[test]
fn independent_reviewed_statement_operation_and_binding_cannot_be_replaced() {
    for mutate in [
        |f: &mut ReceiptFixture| f.message[0] ^= 1,
        |f: &mut ReceiptFixture| f.expected.statement_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.expected.statement_size += 1,
        |f: &mut ReceiptFixture| f.expected.operation_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.binding.chain_id = "another-chain".into(),
        |f: &mut ReceiptFixture| f.binding.network_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.binding.key_handle = "hsm:another-key".into(),
        |f: &mut ReceiptFixture| f.binding.policy_digest[0] ^= 1,
        |f: &mut ReceiptFixture| {
            f.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "another-deployment".into(),
            }
        },
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn independent_authority_active_head_revocation_and_time_are_mandatory() {
    for mutate in [
        |f: &mut ReceiptFixture| f.trust.public_key = f.signer.public_key().clone(),
        |f: &mut ReceiptFixture| f.trust.authority.policy_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.current.active_head.record_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.current.active_head.sequence += 1,
        |f: &mut ReceiptFixture| f.current.signer_revoked = true,
        |f: &mut ReceiptFixture| f.current.attester_revoked = true,
        |f: &mut ReceiptFixture| f.current.now_unix_ms = 200_000,
        |f: &mut ReceiptFixture| f.current.anchor_observed_at_unix_ms = 1,
        |f: &mut ReceiptFixture| f.current.current_anchor.state_digest[0] ^= 1,
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn every_original_completion_coordinate_and_finality_is_bound() {
    for mutate in [
        |c: &mut SignerCompletedOperationV1| c.operation_id[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.intent_digest[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.original_custody.record_digest[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.original_custody.control_state_digest[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.reservation.reservation_id[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.reservation.fence += 1,
        |c: &mut SignerCompletedOperationV1| c.commitment.audit.sequence += 1,
        |c: &mut SignerCompletedOperationV1| c.commitment.response_digest[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.signatures_digest[0] ^= 1,
        |c: &mut SignerCompletedOperationV1| c.completed_at_unix_ms = 90_000,
        |c: &mut SignerCompletedOperationV1| c.completed_at_unix_ms = 150_000,
        |c: &mut SignerCompletedOperationV1| c.anchor.height = 99,
        |c: &mut SignerCompletedOperationV1| c.anchor.height = 111,
        |c: &mut SignerCompletedOperationV1| {
            c.anchor.height = 110;
            c.anchor.block_hash = [0x99; 32];
        },
        |c: &mut SignerCompletedOperationV1| {
            c.anchor.height = 100;
            c.anchor.block_hash = [0x99; 32];
        },
        |c: &mut SignerCompletedOperationV1| c.anchor.operation_state_digest = [0; 32],
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f.completion);
        assert_eq!(
            verify_receipt(&f).unwrap_err(),
            SignerFinalPromotionReceiptErrorV1::Operation(SignerReceiptErrorV1::CompletionMismatch)
        );
    }
}

#[test]
fn signatures_order_bytes_messages_provenance_and_response_are_bound() {
    for mutate in [
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures[0].signature[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures[1].signature[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures[2].signature[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures[3].signature[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures[3].message_digest[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.signatures.swap(1, 2),
        |r: &mut SignerFinalPromotionReceiptV1| {
            r.signatures.pop();
        },
        |r: &mut SignerFinalPromotionReceiptV1| r.commitment.audit.digest[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.provenance.intent_digest[0] ^= 1,
        |r: &mut SignerFinalPromotionReceiptV1| r.provenance.signing_anchor.height = 111,
        |r: &mut SignerFinalPromotionReceiptV1| {
            r.provenance.signing_anchor.height = 90;
            r.provenance.signing_anchor.block_hash = [0x99; 32];
        },
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f.receipt);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn other_purpose_authorizations_never_upgrade_to_final_promotion_receipts() {
    for mutate in [
        |s: &mut SignerCustodyStatementV1| {
            s.binding.role = SignerRoleV1::Promotion;
            s.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
        },
        |s: &mut SignerCustodyStatementV1| {
            s.binding.role = SignerRoleV1::ReleaseManifest;
            s.binding.purpose = SignerPurposeBindingV1::ReleaseManifest {
                deployment_id: "production-primary".into(),
            };
        },
    ] {
        let mut f = receipt_fixture();
        let mut record: SignerCustodyRecordV1 =
            norito::decode_canonical(&f.receipt.custody_record).unwrap();
        mutate(&mut record.statement);
        f.binding = record.statement.binding.clone();
        // Deliberately sign malformed claims without the safe authority producer helper.
        let mut payload = SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1.to_vec();
        payload.extend(norito::encode_canonical(&record.statement).unwrap());
        record.attestation = Signature::new(f.attester.private_key(), &payload)
            .payload()
            .try_into()
            .unwrap();
        f.receipt.custody_record = norito::encode_canonical(&record).unwrap();
        if matches!(
            record.statement.binding.role,
            SignerRoleV1::Promotion | SignerRoleV1::ReleaseManifest
        ) {
            let enrollment = SignerCustodyEnrollmentContextV1 {
                now_unix_ms: f.current.now_unix_ms,
                anchor_observed_at_unix_ms: f.current.anchor_observed_at_unix_ms,
                current_anchor: record.statement.anchor,
                next_sequence: record.statement.sequence,
                predecessor_digest: record.statement.predecessor_digest,
                signer_revoked: false,
                attester_revoked: false,
            };
            let enrolled = verify_signer_custody_enrollment_v1(
                &f.receipt.custody_record,
                &f.binding,
                &f.trust,
                &enrollment,
            )
            .unwrap();
            f.current.active_head.record_digest = enrolled.record_digest();
            let other_custody = verify_signer_custody_use_v1(
                &f.receipt.custody_record,
                &f.binding,
                &f.trust,
                &f.current,
            )
            .expect("independently active valid other-purpose custody");
            let original = receipt_fixture();
            assert_eq!(
                original
                    .receipt
                    .request
                    .validate_custody(&other_custody)
                    .unwrap_err(),
                SignerFinalPromotionReceiptErrorV1::WrongPurpose
            );
            let prepared =
                prepare_final_promotion_statement_v1(&original.message, &original.binding).unwrap();
            assert_eq!(
                SignerFinalPromotionRequestV1::new(&other_custody, &f.expected, &prepared)
                    .unwrap_err(),
                SignerFinalPromotionReceiptErrorV1::WrongPurpose
            );
            assert_eq!(
                verify_receipt(&f).unwrap_err(),
                SignerFinalPromotionReceiptErrorV1::InvalidStatement
            );
        }
    }
}

#[test]
fn original_receipt_cannot_be_relabelled_after_valid_same_key_custody_renewal() {
    let mut f = receipt_fixture();
    let old = f.current.active_head.record_digest;
    let mut record: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).unwrap();
    record.statement.sequence += 1;
    record.statement.predecessor_digest = old;
    record.statement.anchor = f.current.current_anchor;
    record.statement.issued_at_unix_ms = 155_000;
    record.attestation = Signature::new(
        f.attester.private_key(),
        &record.statement.signing_payload().unwrap(),
    )
    .payload()
    .try_into()
    .unwrap();
    let renewed = norito::encode_canonical(&record).unwrap();
    let context = SignerCustodyEnrollmentContextV1 {
        now_unix_ms: 160_000,
        anchor_observed_at_unix_ms: 160_000,
        current_anchor: f.current.current_anchor,
        next_sequence: 2,
        predecessor_digest: old,
        signer_revoked: false,
        attester_revoked: false,
    };
    let verified =
        verify_signer_custody_enrollment_v1(&renewed, &f.binding, &f.trust, &context).unwrap();
    f.current.active_head.record_digest = verified.record_digest();
    f.current.active_head.sequence = 2;
    f.current.active_head.approved_anchor = record.statement.anchor;
    f.current.current_anchor.height += 1;
    f.current.current_anchor.block_hash = [0x75; 32];
    f.current.current_anchor.state_digest = [0x76; 32];
    assert!(verify_receipt(&f).is_err());
    f.receipt.custody_record = renewed;
    assert!(verify_receipt(&f).is_err()); // Same role key cannot relabel the old request/signatures/completed row.
}

#[test]
fn canonical_receipt_and_statement_bounds_and_detached_bytes_fail_closed() {
    let f = receipt_fixture();
    let bytes = norito::encode_canonical(&f.receipt).unwrap();
    for invalid in [
        vec![],
        bytes[..bytes.len() - 1].to_vec(),
        vec![0; SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1 + 1],
    ] {
        assert!(
            verify_final_promotion_signer_receipt_v1(
                &invalid,
                &f.message,
                &f.receipt.signatures[0].signature,
                &f.expected,
                &f.binding,
                &f.trust,
                &f.current,
                &f.completion
            )
            .is_err()
        );
    }
    let mut wrong = f.receipt.signatures[0].signature.clone();
    wrong[0] ^= 1;
    assert!(
        verify_final_promotion_signer_receipt_v1(
            &bytes,
            &f.message,
            &wrong,
            &f.expected,
            &f.binding,
            &f.trust,
            &f.current,
            &f.completion
        )
        .is_err()
    );
    let custody =
        verify_signer_custody_use_v1(&f.receipt.custody_record, &f.binding, &f.trust, &f.current)
            .unwrap();
    for message in [
        vec![],
        vec![b'a'; SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 + 1],
    ] {
        assert!(prepare_final_promotion_statement_v1(&message, &f.binding).is_err());
        let mut expected = f.expected;
        expected.statement_digest = signer_final_promotion_digest_v1(&message);
        expected.statement_size = message.len() as u64;
        assert!(
            validate_final_promotion_signatures_v1(
                &f.receipt,
                &message,
                &f.receipt.signatures[0].signature,
                &expected,
                &custody,
            )
            .is_err()
        );
    }
}

#[test]
fn canonical_signatures_and_receipts_ignore_ambient_layout_and_reject_other_layouts() {
    let f = receipt_fixture();
    let canonical = norito::encode_canonical(&f.receipt).unwrap();
    let canonical_intent = f.receipt.intent.digest().unwrap();
    let canonical_request = f.receipt.request.digest().unwrap();
    let canonical_provenance = f.receipt.provenance.signing_message().unwrap();
    let canonical_signatures =
        signer_operation_signatures_digest_v1(&f.receipt.signatures).unwrap();
    for layout in crate::canonical_test_support::supported_layouts() {
        let alternate = {
            let _layout = norito::core::DecodeFlagsGuard::enter(layout);
            assert_eq!(norito::encode_canonical(&f.receipt).unwrap(), canonical);
            assert_eq!(f.receipt.intent.digest().unwrap(), canonical_intent);
            assert_eq!(f.receipt.request.digest().unwrap(), canonical_request);
            assert_eq!(
                f.receipt.provenance.signing_message().unwrap(),
                canonical_provenance
            );
            assert_eq!(
                signer_operation_signatures_digest_v1(&f.receipt.signatures).unwrap(),
                canonical_signatures
            );
            assert_eq!(
                signer_final_promotion_audit_v1(
                    &f.receipt.request,
                    &f.receipt.intent,
                    f.receipt.reservation,
                    &f.receipt.signatures[0].signature
                )
                .unwrap(),
                f.receipt.commitment.audit
            );
            assert_eq!(
                signer_final_promotion_response_digest_v1(
                    &f.receipt.request,
                    &f.receipt.provenance,
                    &f.receipt.signatures[..3]
                )
                .unwrap(),
                f.receipt.commitment.response_digest
            );
            verify_receipt(&f).expect("canonical receipt under any ambient layout");
            norito::core::to_bytes(&f.receipt).unwrap()
        };
        if layout != norito::core::default_encode_flags() {
            assert_ne!(alternate, canonical);
            assert_eq!(
                verify_final_promotion_signer_receipt_v1(
                    &alternate,
                    &f.message,
                    &f.receipt.signatures[0].signature,
                    &f.expected,
                    &f.binding,
                    &f.trust,
                    &f.current,
                    &f.completion
                )
                .unwrap_err(),
                SignerFinalPromotionReceiptErrorV1::InvalidReceipt
            );
        }
    }
}

#[test]
fn prepared_statement_cannot_move_to_another_complete_custody_binding() {
    let f = receipt_fixture();
    let active =
        verify_signer_custody_use_v1(&f.receipt.custody_record, &f.binding, &f.trust, &f.current)
            .unwrap();
    // This handle is outside the public JSON statement, but the prepared capability pins it too.
    let mut other_binding = f.binding.clone();
    other_binding.runtime_handle = "software://sorafs/final-promotion-provenance/secondary".into();
    let prepared = prepare_final_promotion_statement_v1(&f.message, &other_binding).unwrap();
    assert_eq!(
        SignerFinalPromotionRequestV1::new(&active, &f.expected, &prepared).unwrap_err(),
        SignerFinalPromotionReceiptErrorV1::StatementMismatch
    );
    let prepared = prepare_final_promotion_statement_v1(&f.message, &f.binding).unwrap();
    assert_eq!(prepared.message(), f.message);
    assert_eq!(prepared.sha256(), iroha_crypto::sha256(&f.message));
    assert_eq!(prepared.len(), f.message.len());
    for mutation in 0..3 {
        let mut expected = f.expected;
        match mutation {
            0 => expected.operation_id = [0; 32],
            1 => expected.statement_digest[0] ^= 1,
            _ => expected.statement_size += 1,
        }
        assert_eq!(
            SignerFinalPromotionRequestV1::new(&active, &expected, &prepared).unwrap_err(),
            if mutation == 0 {
                SignerFinalPromotionReceiptErrorV1::InvalidReceipt
            } else {
                SignerFinalPromotionReceiptErrorV1::StatementMismatch
            }
        );
    }
}

#[test]
fn request_audit_and_response_cannot_change_actions_order_or_domain() {
    let f = receipt_fixture();
    for mutation in 0..5 {
        let mut intent = f.receipt.intent.clone();
        let mut signature = f.receipt.signatures[0].signature.clone();
        match mutation {
            0 => intent.action = SignerOperationActionV1::Status,
            1 => intent.operation_id[0] ^= 1,
            2 => intent.request_digest[0] ^= 1,
            3 => intent.previous_audit.sequence = u64::MAX,
            _ => {
                signature.pop();
            }
        }
        assert!(
            signer_final_promotion_audit_v1(
                &f.receipt.request,
                &intent,
                f.receipt.reservation,
                &signature
            )
            .is_err()
        );
    }
    let mut first = f.receipt.signatures[..3].to_vec();
    first.swap(1, 2);
    assert_eq!(
        signer_final_promotion_response_digest_v1(
            &f.receipt.request,
            &f.receipt.provenance,
            &first
        )
        .unwrap_err(),
        SignerFinalPromotionReceiptErrorV1::Operation(SignerReceiptErrorV1::InvalidSignature)
    );
    assert!(
        signer_final_promotion_response_digest_v1(
            &f.receipt.request,
            &f.receipt.provenance,
            &first[..2]
        )
        .is_err()
    );
    assert_ne!(
        signer_final_promotion_digest_v1(&f.message),
        crate::signer::receipt::signer_release_manifest_digest_v1(&f.message)
    );
    let error = SignerFinalPromotionReceiptErrorV1::from(SignerReceiptErrorV1::InvalidSignature);
    assert_eq!(
        error,
        SignerFinalPromotionReceiptErrorV1::Operation(SignerReceiptErrorV1::InvalidSignature)
    );
    assert_eq!(
        error.to_string(),
        "final promotion operation verification failed"
    );
}
