//! Adversarial consistency controls; supplied fixture state is not production authorization.
use super::*;
use crate::signer::protocol::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use subject::*;
mod fixture;
use fixture::{ReceiptFixture, receipt_fixture, verify_receipt};

#[test]
fn topology_role_has_one_isolated_wire_identity_and_no_alias() {
    let role = SignerRoleV1::TopologyApproval;
    assert_eq!(role as u8, 16);
    assert_eq!(role.as_str(), "topology_approval");
    assert_eq!(
        role.domain(),
        "sorafs.production-readiness.topology-approval.v1"
    );
    assert_eq!(role.to_string().parse::<SignerRoleV1>().unwrap(), role);
    assert!(role.allows_algorithm(SignerKeyAlgorithmV1::Ed25519));
    assert!(!role.allows_algorithm(SignerKeyAlgorithmV1::MlDsa));
    for label in [
        "proof_outcome",
        "repair",
        "reserve",
        "orderbook",
        "promotion",
        "governance_dag",
        "potr_gateway",
        "potr_provider",
        "billing_statement",
        "evidence_viewer",
        "stream_token",
        "pop_credentials",
        "release_manifest",
        "final_promotion_provenance",
        "final_promotion_account_transaction",
    ] {
        let other: SignerRoleV1 = label.parse().unwrap();
        assert_ne!(role as u8, other as u8);
        assert_ne!(role.domain(), other.domain());
        assert!(
            !SignerPurposeBindingV1::TopologyApproval {
                deployment_id: "production-primary".into()
            }
            .validates_role(other)
        );
    }
    for alias in [
        "topology",
        "TopologyApproval",
        "topology-approval",
        "topology_approval ",
        " topology_approval",
        "16",
    ] {
        assert!(alias.parse::<SignerRoleV1>().is_err());
    }
    for id in [
        "",
        " production",
        "production/other",
        "test-primary",
        "production ",
    ] {
        assert!(
            !SignerPurposeBindingV1::TopologyApproval {
                deployment_id: id.into()
            }
            .validates_role(role)
        );
    }
    assert!(!SignerPurposeBindingV1::NativeOrPromotion.validates_role(role));
    let encoded = norito::encode_canonical(&role).unwrap();
    assert_eq!(
        norito::decode_canonical::<SignerRoleV1>(&encoded).unwrap(),
        role
    );
}

#[test]
fn canonical_subject_request_and_receipt_roundtrip_without_a_promotable_token() {
    let f = receipt_fixture();
    verify_receipt(&f).unwrap(); // Original timely completion may be recovered after reservation expiry.
    let subject = norito::encode_canonical(&f.subject).unwrap();
    assert_eq!(
        norito::decode_canonical::<TopologyApprovalSubjectV1>(&subject).unwrap(),
        f.subject
    );
    let request = norito::encode_canonical(&f.receipt.request).unwrap();
    assert_eq!(
        norito::decode_canonical::<SignerTopologyRequestV1>(&request).unwrap(),
        f.receipt.request
    );
    let receipt = norito::encode_canonical(&f.receipt).unwrap();
    assert_eq!(
        norito::decode_canonical::<SignerTopologyReceiptV1>(&receipt).unwrap(),
        f.receipt
    );
    let prepared = prepare_topology_approval_v1(&f.subject, &f.binding).unwrap();
    assert_eq!(prepared.subject(), &f.subject);
    assert_eq!(
        prepared
            .message()
            .strip_prefix(TOPOLOGY_APPROVAL_DOMAIN_V1)
            .unwrap(),
        subject
    );
    assert!(!format!("{:?}", f.receipt).contains("key-7"));
}

#[test]
fn every_independently_reviewed_coordinate_rejects_substitution() {
    for mutate in [
        |f: &mut ReceiptFixture| f.operation_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.deployment_id = "production-other".into(),
        |f: &mut ReceiptFixture| f.subject.chain_id = "another-chain".into(),
        |f: &mut ReceiptFixture| f.subject.chain_discriminant += 1,
        |f: &mut ReceiptFixture| f.subject.network_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.release_manifest_sha256[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.qualification_summary_sha256[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.manifest_sha256[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.canonical_manifest_sha256[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.validator_ids_sha256[0] ^= 1,
        |f: &mut ReceiptFixture| f.subject.reviewed_at_unix_ms += 1,
        |f: &mut ReceiptFixture| f.subject.expires_at_unix_ms += 1,
        |f: &mut ReceiptFixture| {
            f.binding.key_handle = "software://sorafs/topology-approval/key-8".into()
        },
        |f: &mut ReceiptFixture| f.binding.key_revision += 1,
        |f: &mut ReceiptFixture| f.binding.policy_revision += 1,
        |f: &mut ReceiptFixture| f.binding.policy_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.binding.public_key = f.attester.public_key().clone(),
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn independent_custody_revocation_active_head_and_clock_are_required() {
    for mutate in [
        |f: &mut ReceiptFixture| f.trust.public_key = f.signer.public_key().clone(),
        |f: &mut ReceiptFixture| f.trust.authority.policy_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.current.active_head.record_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.current.active_head.sequence += 1,
        |f: &mut ReceiptFixture| f.current.signer_revoked = true,
        |f: &mut ReceiptFixture| f.current.attester_revoked = true,
        |f: &mut ReceiptFixture| {
            f.current.now_unix_ms = f.subject.expires_at_unix_ms;
            f.current.anchor_observed_at_unix_ms = f.current.now_unix_ms;
        },
        |f: &mut ReceiptFixture| f.current.now_unix_ms = f.subject.reviewed_at_unix_ms - 1,
        |f: &mut ReceiptFixture| f.current.anchor_observed_at_unix_ms = 1,
        |f: &mut ReceiptFixture| f.current.current_anchor.state_digest[0] ^= 1,
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn reservation_fence_completion_and_every_signature_are_bound() {
    for mutate in [
        |f: &mut ReceiptFixture| f.receipt.request.original_custody.record_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.intent.action = SignerOperationActionV1::Status,
        |f: &mut ReceiptFixture| f.receipt.intent.previous_audit.sequence = u64::MAX,
        |f: &mut ReceiptFixture| f.receipt.reservation.fence += 1,
        |f: &mut ReceiptFixture| f.receipt.reservation.reservation_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.provenance.signing_anchor.height = 111,
        |f: &mut ReceiptFixture| f.receipt.commitment.response_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures[0].signature[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures[1].signature[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures[2].signature[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures[3].signature[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures[3].message_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.receipt.signatures.swap(1, 2),
        |f: &mut ReceiptFixture| {
            f.receipt.signatures.pop();
        },
        |f: &mut ReceiptFixture| f.completion.operation_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.intent_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.original_custody.record_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.reservation.fence += 1,
        |f: &mut ReceiptFixture| f.completion.reservation.reservation_id[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.commitment.audit.digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.signatures_digest[0] ^= 1,
        |f: &mut ReceiptFixture| f.completion.anchor.operation_state_digest = [0; 32],
        |f: &mut ReceiptFixture| f.completion.anchor.height = 111,
        |f: &mut ReceiptFixture| {
            f.completion.completed_at_unix_ms = f.subject.reviewed_at_unix_ms - 1
        },
        |f: &mut ReceiptFixture| f.completion.completed_at_unix_ms = 150_000,
    ] {
        let mut f = receipt_fixture();
        mutate(&mut f);
        assert!(verify_receipt(&f).is_err());
    }
}

#[test]
fn prepared_owner_rejects_same_deployment_with_substituted_full_binding() {
    let f = receipt_fixture();
    let active =
        verify_signer_custody_use_v1(&f.receipt.custody_record, &f.binding, &f.trust, &f.current)
            .unwrap();
    let mut other = f.binding.clone();
    other.key_revision += 1;
    let prepared = prepare_topology_approval_v1(&f.subject, &other).unwrap();
    assert_eq!(
        SignerTopologyRequestV1::new(&active, f.operation_id, &prepared),
        Err(Error::SubjectMismatch)
    );
    let prepared = prepare_topology_approval_v1(&f.subject, &f.binding).unwrap();
    assert_eq!(
        SignerTopologyRequestV1::new(&active, [0; 32], &prepared),
        Err(Error::InvalidReceipt)
    );
    let mut wrong = f.binding.clone();
    wrong.role = SignerRoleV1::Promotion;
    wrong.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    assert!(matches!(
        prepare_topology_approval_v1(&f.subject, &wrong),
        Err(Error::WrongPurpose)
    ));
}

#[test]
fn subject_bounds_and_receipt_frame_reject_missing_or_retired_layouts() {
    let f = receipt_fixture();
    for mutate in [
        |s: &mut TopologyApprovalSubjectV1| s.release_manifest_sha256 = [0; 32],
        |s: &mut TopologyApprovalSubjectV1| s.qualification_summary_sha256 = [0; 32],
        |s: &mut TopologyApprovalSubjectV1| s.manifest_sha256 = [0; 32],
        |s: &mut TopologyApprovalSubjectV1| s.canonical_manifest_sha256 = [0; 32],
        |s: &mut TopologyApprovalSubjectV1| s.validator_ids_sha256 = [0; 32],
        |s: &mut TopologyApprovalSubjectV1| s.reviewed_at_unix_ms = 0,
        |s: &mut TopologyApprovalSubjectV1| s.expires_at_unix_ms = s.reviewed_at_unix_ms,
        |s: &mut TopologyApprovalSubjectV1| {
            s.expires_at_unix_ms = s.reviewed_at_unix_ms + TOPOLOGY_APPROVAL_MAX_VALIDITY_MS_V1 + 1
        },
    ] {
        let mut subject = f.subject.clone();
        mutate(&mut subject);
        assert!(prepare_topology_approval_v1(&subject, &f.binding).is_err());
    }
    let reference = TopologyReceiptReferenceV1 {
        subject: &f.subject,
        operation_id: f.operation_id,
        binding: &f.binding,
        trust: &f.trust,
        current: &f.current,
        completion: &f.completion,
    };
    let signature = &f.receipt.signatures[0].signature;
    let mut truncated = norito::encode_canonical(&f.receipt).unwrap();
    truncated.pop();
    for bytes in [
        Vec::new(),
        truncated,
        vec![0; TOPOLOGY_RECEIPT_MAX_BYTES_V1 + 1],
        br#"{"schema":"sorafs.l1.deployment_qualification.signed_envelope.v1"}"#.to_vec(),
    ] {
        assert!(check_topology_receipt_consistency_v1(&bytes, signature, &reference).is_err());
    }
    let bytes = norito::encode_canonical(&f.receipt).unwrap();
    assert!(check_topology_receipt_consistency_v1(&bytes, &signature[..63], &reference).is_err());
    let other = KeyPair::try_from_seed(vec![0x92; 32], Algorithm::Ed25519).unwrap();
    let prepared = prepare_topology_approval_v1(&f.subject, &f.binding).unwrap();
    let substituted = Signature::new(other.private_key(), prepared.message());
    assert!(
        check_topology_receipt_consistency_v1(&bytes, substituted.payload(), &reference).is_err()
    );
}
