//! Exact receipt verification simulations; these fixtures do not qualify any actual hardware.

use super::*;
use crate::signer::custody::*;
use iroha_crypto::{Algorithm, KeyPair};

pub(crate) struct Fixture {
    pub(crate) signer: KeyPair,
    pub(crate) attester: KeyPair,
    pub(crate) binding: SignerCustodyBindingV1,
    pub(crate) trust: SignerCustodyTrustV1,
    pub(crate) current: SignerCustodyUseContextV1,
    pub(crate) expected: SignerReleaseManifestExpectedV1,
    pub(crate) completion: SignerCompletedOperationV1,
    pub(crate) receipt: SignerReleaseManifestReceiptV1,
    pub(crate) manifest: Vec<u8>,
}
fn sign(
    key: &KeyPair,
    purpose: SignerKeyOperationPurposeV1,
    message: &[u8],
) -> SignerOperationSignatureV1 {
    SignerOperationSignatureV1 {
        purpose,
        message_digest: signer_operation_message_digest_v1(message),
        signature: Signature::new(key.private_key(), message)
            .payload()
            .to_vec(),
    }
}
pub(crate) fn fixture() -> Fixture {
    let signer = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519).unwrap();
    let attester = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).unwrap();
    let binding = SignerCustodyBindingV1 {
        chain_id: "release-chain".into(),
        network_id: [0x11; 32],
        runtime_handle: "hsm://sorafs/release/primary".into(),
        key_handle: "pkcs11:production/release/key-7".into(),
        service_id: "release-primary".into(),
        administrator_id: "release-security-primary".into(),
        role: SignerRoleV1::ReleaseManifest,
        purpose: SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: signer.public_key().clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x41; 32],
    };
    let authority = SignerCustodyAuthorityV1 {
        service_id: "custody-authority-primary".into(),
        administrator_id: "custody-security-primary".into(),
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x42; 32],
    };
    let approved = SignerCustodyAnchorV1 {
        height: 90,
        block_hash: [0x43; 32],
        state_digest: [0x44; 32],
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: 1,
        binding: binding.clone(),
        authority: authority.clone(),
        anchor: approved,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: 100_000,
        expires_at_unix_ms: 200_000,
        hardware_identity_digest: [0x45; 32],
        evidence_digest: [0x46; 32],
        generated_in_hardware: true,
        exportable: false,
        ever_exported: false,
        revoked: false,
    };
    let attestation = Signature::new(
        attester.private_key(),
        &statement.signing_payload().unwrap(),
    )
    .payload()
    .try_into()
    .unwrap();
    let record = norito::encode_canonical(&SignerCustodyRecordV1 {
        statement,
        attestation,
    })
    .unwrap();
    let trust = SignerCustodyTrustV1 {
        authority,
        public_key: attester.public_key().clone(),
        active_from_unix_ms: 90_000,
        active_until_unix_ms: 300_000,
        max_validity_ms: 200_000,
        max_anchor_age_ms: 10_000,
    };
    let enrollment = SignerCustodyEnrollmentContextV1 {
        now_unix_ms: 110_000,
        anchor_observed_at_unix_ms: 110_000,
        current_anchor: approved,
        next_sequence: 1,
        predecessor_digest: [0; 32],
        signer_revoked: false,
        attester_revoked: false,
    };
    let enrolled =
        verify_signer_custody_enrollment_v1(&record, &binding, &trust, &enrollment).unwrap();
    let current = SignerCustodyUseContextV1 {
        now_unix_ms: 120_000,
        anchor_observed_at_unix_ms: 120_000,
        current_anchor: SignerCustodyAnchorV1 {
            height: 100,
            block_hash: [0x51; 32],
            state_digest: [0x52; 32],
        },
        active_head: SignerCustodyActiveHeadV1 {
            record_digest: enrolled.record_digest(),
            sequence: 1,
            approved_anchor: approved,
            key_revision: 7,
            policy_revision: 9,
            policy_digest: binding.policy_digest,
        },
        signer_revoked: false,
        attester_revoked: false,
    };
    let active = verify_signer_custody_use_v1(&record, &binding, &trust, &current).unwrap();
    let manifest = b"{\"schema\":\"iroha.release-manifest.v1\",\"version\":\"1.0.0\"}\n".to_vec();
    let expected = SignerReleaseManifestExpectedV1 {
        operation_id: [0x61; 32],
        manifest_digest: signer_release_manifest_digest_v1(&manifest),
        manifest_size: manifest.len() as u64,
    };
    let request = SignerReleaseManifestRequestV1::new(&active, &expected, &manifest).unwrap();
    let intent = SignerOperationIntentV1 {
        action: SignerOperationActionV1::Sign,
        operation_id: expected.operation_id,
        request_digest: request.digest().unwrap(),
        previous_audit: SignerOperationAuditHeadV1 {
            sequence: 4,
            digest: [0x62; 32],
        },
    };
    let reservation = SignerOperationReservationV1 {
        reservation_id: [0x63; 32],
        fence: 8,
        expires_at_unix_ms: 150_000,
    };
    let raw = sign(&signer, SignerKeyOperationPurposeV1::RolePayload, &manifest);
    let audit =
        signer_release_manifest_audit_v1(&request, &intent, reservation, &raw.signature).unwrap();
    let provenance = SignerOperationProvenanceV1 {
        original_custody: request.original_custody,
        signing_anchor: current.current_anchor,
        intent_digest: intent.digest().unwrap(),
        reservation,
        audit,
    };
    let mut signatures = vec![
        raw,
        sign(
            &signer,
            SignerKeyOperationPurposeV1::AuditRecord,
            &audit.signing_message(),
        ),
        sign(
            &signer,
            SignerKeyOperationPurposeV1::Provenance,
            &provenance.signing_message().unwrap(),
        ),
    ];
    let commitment = SignerOperationCommitmentV1 {
        audit,
        response_digest: signer_release_manifest_response_digest_v1(
            &request,
            &provenance,
            &signatures,
        )
        .unwrap(),
    };
    signatures.push(sign(
        &signer,
        SignerKeyOperationPurposeV1::Response,
        &commitment.response_signing_message(),
    ));
    let completion = SignerCompletedOperationV1 {
        operation_id: expected.operation_id,
        intent_digest: intent.digest().unwrap(),
        original_custody: request.original_custody,
        reservation,
        commitment,
        signatures_digest: signer_operation_signatures_digest_v1(&signatures).unwrap(),
        completed_at_unix_ms: 125_000,
        anchor: SignerOperationFinalizedAnchorV1 {
            height: 101,
            block_hash: [0x71; 32],
            operation_state_digest: [0x72; 32],
        },
    };
    let receipt = SignerReleaseManifestReceiptV1 {
        magic: SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1,
        version: 1,
        custody_record: record,
        request,
        intent,
        reservation,
        provenance,
        commitment,
        signatures,
    };
    let mut fixture = Fixture {
        signer,
        attester,
        binding,
        trust,
        current,
        expected,
        completion,
        receipt,
        manifest,
    };
    fixture.current.now_unix_ms = 160_000; // Completion may be recovered after the reservation expires.
    fixture.current.anchor_observed_at_unix_ms = 160_000;
    fixture.current.current_anchor.height = 110;
    fixture.current.current_anchor.block_hash = [0x73; 32];
    fixture
}
fn verify(
    fixture: &Fixture,
) -> Result<VerifiedReleaseManifestSignerReceiptV1, SignerReceiptErrorV1> {
    verify_release_manifest_signer_receipt_v1(
        &norito::encode_canonical(&fixture.receipt).unwrap(),
        &fixture.manifest,
        &fixture.receipt.signatures[0].signature,
        &fixture.expected,
        &fixture.binding,
        &fixture.trust,
        &fixture.current,
        &fixture.completion,
    )
}

#[test]
fn exact_canonical_receipt_roundtrips_and_recovers_after_timely_completion() {
    let fixture = fixture();
    let bytes = norito::encode_canonical(&fixture.receipt).unwrap();
    let decoded: SignerReleaseManifestReceiptV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, fixture.receipt);
    let verified = verify(&fixture).unwrap();
    assert_eq!(verified.completion(), &fixture.completion);
    assert_eq!(verified.manifest_digest(), fixture.expected.manifest_digest);
    assert_eq!(
        verified.custody().record_digest(),
        fixture.current.active_head.record_digest
    );
    assert!(!format!("{verified:?}").contains("key-7"));
    assert!(!format!("{:?}", fixture.receipt).contains("production-primary"));
}

#[test]
fn independent_reviewed_manifest_operation_and_binding_cannot_be_replaced() {
    for mutate in [
        |f: &mut Fixture| f.manifest[0] ^= 1,
        |f: &mut Fixture| f.expected.manifest_digest[0] ^= 1,
        |f: &mut Fixture| f.expected.manifest_size += 1,
        |f: &mut Fixture| f.expected.operation_id[0] ^= 1,
        |f: &mut Fixture| f.binding.chain_id = "another-chain".into(),
        |f: &mut Fixture| f.binding.network_id[0] ^= 1,
        |f: &mut Fixture| f.binding.key_handle = "hsm:another-key".into(),
        |f: &mut Fixture| f.binding.policy_digest[0] ^= 1,
        |f: &mut Fixture| {
            f.binding.purpose = SignerPurposeBindingV1::ReleaseManifest {
                deployment_id: "another-deployment".into(),
            }
        },
    ] {
        let mut f = fixture();
        mutate(&mut f);
        assert!(verify(&f).is_err());
    }
}

#[test]
fn independent_authority_active_head_revocation_and_time_are_mandatory() {
    for mutate in [
        |f: &mut Fixture| f.trust.public_key = f.signer.public_key().clone(),
        |f: &mut Fixture| f.trust.authority.policy_digest[0] ^= 1,
        |f: &mut Fixture| f.current.active_head.record_digest[0] ^= 1,
        |f: &mut Fixture| f.current.active_head.sequence += 1,
        |f: &mut Fixture| f.current.signer_revoked = true,
        |f: &mut Fixture| f.current.attester_revoked = true,
        |f: &mut Fixture| f.current.now_unix_ms = 200_000,
        |f: &mut Fixture| f.current.anchor_observed_at_unix_ms = 1,
        |f: &mut Fixture| f.current.current_anchor.state_digest[0] ^= 1,
    ] {
        let mut f = fixture();
        mutate(&mut f);
        assert!(verify(&f).is_err());
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
        let mut f = fixture();
        mutate(&mut f.completion);
        assert_eq!(
            verify(&f).unwrap_err(),
            SignerReceiptErrorV1::CompletionMismatch
        );
    }
}

#[test]
fn signatures_order_bytes_messages_provenance_and_response_are_bound() {
    for mutate in [
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures[0].signature[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures[1].signature[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures[2].signature[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures[3].signature[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures[3].message_digest[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.signatures.swap(1, 2),
        |r: &mut SignerReleaseManifestReceiptV1| {
            r.signatures.pop();
        },
        |r: &mut SignerReleaseManifestReceiptV1| r.commitment.audit.digest[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.provenance.intent_digest[0] ^= 1,
        |r: &mut SignerReleaseManifestReceiptV1| r.provenance.signing_anchor.height = 111,
        |r: &mut SignerReleaseManifestReceiptV1| {
            r.provenance.signing_anchor.height = 90;
            r.provenance.signing_anchor.block_hash = [0x99; 32];
        },
    ] {
        let mut f = fixture();
        mutate(&mut f.receipt);
        assert!(verify(&f).is_err());
    }
}

#[test]
fn other_purpose_and_exportable_attestations_never_upgrade_to_release_receipts() {
    for mutate in [
        |s: &mut SignerCustodyStatementV1| {
            s.binding.role = SignerRoleV1::Promotion;
            s.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
        },
        |s: &mut SignerCustodyStatementV1| s.exportable = true,
        |s: &mut SignerCustodyStatementV1| s.generated_in_hardware = false,
    ] {
        let mut f = fixture();
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
        if record.statement.binding.role == SignerRoleV1::Promotion {
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
            verify_signer_custody_use_v1(
                &f.receipt.custody_record,
                &f.binding,
                &f.trust,
                &f.current,
            )
            .expect("independently active valid Promotion custody");
            assert_eq!(verify(&f).unwrap_err(), SignerReceiptErrorV1::WrongPurpose);
        } else {
            assert_eq!(
                verify(&f).unwrap_err(),
                SignerReceiptErrorV1::Custody(SignerCustodyErrorV1::HardwareCustodyRequired)
            );
        }
    }
}

#[test]
fn original_receipt_cannot_be_relabelled_after_valid_same_key_custody_renewal() {
    let mut f = fixture();
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
    assert!(verify(&f).is_err());
    f.receipt.custody_record = renewed;
    assert!(verify(&f).is_err()); // Same role key cannot relabel the old request/signatures/completed row.
}

#[test]
fn canonical_receipt_and_manifest_bounds_and_detached_bytes_fail_closed() {
    let f = fixture();
    let bytes = norito::encode_canonical(&f.receipt).unwrap();
    for invalid in [
        vec![],
        bytes[..bytes.len() - 1].to_vec(),
        vec![0; SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1 + 1],
    ] {
        assert!(
            verify_release_manifest_signer_receipt_v1(
                &invalid,
                &f.manifest,
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
        verify_release_manifest_signer_receipt_v1(
            &bytes,
            &f.manifest,
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
    for manifest in [vec![], vec![b'a'; SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1 + 1]] {
        let expected = SignerReleaseManifestExpectedV1 {
            manifest_digest: signer_release_manifest_digest_v1(&manifest),
            manifest_size: manifest.len() as u64,
            ..f.expected
        };
        assert!(SignerReleaseManifestRequestV1::new(&custody, &expected, &manifest).is_err());
    }
}

#[test]
fn canonical_signatures_and_receipts_ignore_ambient_layout_and_reject_other_layouts() {
    let f = fixture();
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
                signer_release_manifest_audit_v1(
                    &f.receipt.request,
                    &f.receipt.intent,
                    f.receipt.reservation,
                    &f.receipt.signatures[0].signature
                )
                .unwrap(),
                f.receipt.commitment.audit
            );
            assert_eq!(
                signer_release_manifest_response_digest_v1(
                    &f.receipt.request,
                    &f.receipt.provenance,
                    &f.receipt.signatures[..3]
                )
                .unwrap(),
                f.receipt.commitment.response_digest
            );
            verify(&f).expect("canonical receipt under any ambient layout");
            norito::core::to_bytes(&f.receipt).unwrap()
        };
        if layout != norito::core::default_encode_flags() {
            assert_ne!(alternate, canonical);
            assert_eq!(
                verify_release_manifest_signer_receipt_v1(
                    &alternate,
                    &f.manifest,
                    &f.receipt.signatures[0].signature,
                    &f.expected,
                    &f.binding,
                    &f.trust,
                    &f.current,
                    &f.completion
                )
                .unwrap_err(),
                SignerReceiptErrorV1::InvalidReceipt
            );
        }
    }
}
