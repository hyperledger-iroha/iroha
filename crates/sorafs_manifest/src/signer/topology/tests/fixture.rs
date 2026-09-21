//! Synthetic software keys and supplied state for consistency tests; no native execution evidence.
// These deterministic software keys simulate independent statements; they never qualify deployment custody.

use crate::signer::{
    custody::*,
    protocol::*,
    receipt::{
        SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1, SignerOperationProvenanceV1,
    },
    topology::{subject::*, *},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};

/// Deterministically signed synthetic custody and receipt for cryptographic regression tests.
pub(crate) struct ReceiptFixture {
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) signer: KeyPair,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) attester: KeyPair,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) binding: SignerCustodyBindingV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) trust: SignerCustodyTrustV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) current: SignerCustodyUseContextV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) subject: TopologyApprovalSubjectV1,
    /// Independently assigned operation id.
    pub(crate) operation_id: [u8; 32],
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) completion: SignerCompletedOperationV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) receipt: SignerTopologyReceiptV1,
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
struct Authorization {
    binding: SignerCustodyBindingV1,
    trust: SignerCustodyTrustV1,
    current: SignerCustodyUseContextV1,
    record: Vec<u8>,
}
fn binding(signer: &KeyPair) -> SignerCustodyBindingV1 {
    SignerCustodyBindingV1 {
        chain_id: "promotion-chain".into(),
        network_id: [0x11; 32],
        runtime_handle: "software://sorafs/topology-approval/primary".into(),
        key_handle: "software://sorafs/topology-approval/key-7".into(),
        service_id: "promotion-primary".into(),
        administrator_id: "promotion-security-primary".into(),
        role: SignerRoleV1::TopologyApproval,
        purpose: SignerPurposeBindingV1::TopologyApproval {
            deployment_id: "production-primary".into(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: signer.public_key().clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x41; 32],
    }
}
fn authorization(signer: &KeyPair, attester: &KeyPair) -> Authorization {
    let binding = binding(signer);
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
        evidence_digest: [0x46; 32],
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

    Authorization {
        binding,
        trust,
        current,
        record,
    }
}
fn subject(binding: &SignerCustodyBindingV1) -> TopologyApprovalSubjectV1 {
    TopologyApprovalSubjectV1 {
        deployment_id: "production-primary".into(),
        network_id: binding.network_id,
        chain_id: binding.chain_id.clone(),
        chain_discriminant: 42,
        release_manifest_sha256: [0x81; 32],
        qualification_summary_sha256: [0x82; 32],
        manifest_sha256: [0x83; 32],
        canonical_manifest_sha256: [0x84; 32],
        validator_ids_sha256: [0x85; 32],
        reviewed_at_unix_ms: 115_000,
        expires_at_unix_ms: 180_000,
    }
}
fn signed_operation(
    signer: &KeyPair,
    record: Vec<u8>,
    request: SignerTopologyRequestV1,
    prepared: &PreparedTopologyApprovalV1,
    signing_anchor: SignerCustodyAnchorV1,
) -> (SignerTopologyReceiptV1, SignerCompletedOperationV1) {
    let intent = SignerOperationIntentV1 {
        action: SignerOperationActionV1::Sign,
        operation_id: request.operation_id,
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
    let raw = sign(
        signer,
        SignerKeyOperationPurposeV1::RolePayload,
        prepared.message(),
    );
    let audit = signer_topology_audit_v1(&request, &intent, reservation, &raw.signature).unwrap();
    let provenance = SignerOperationProvenanceV1 {
        original_custody: request.original_custody,
        signing_anchor,
        intent_digest: intent.digest().unwrap(),
        reservation,
        audit,
    };
    let mut signatures = vec![
        raw,
        sign(
            signer,
            SignerKeyOperationPurposeV1::AuditRecord,
            &audit.signing_message(),
        ),
        sign(
            signer,
            SignerKeyOperationPurposeV1::Provenance,
            &provenance.signing_message().unwrap(),
        ),
    ];
    let commitment = SignerOperationCommitmentV1 {
        audit,
        response_digest: signer_topology_response_digest_v1(&request, &provenance, &signatures)
            .unwrap(),
    };
    signatures.push(sign(
        signer,
        SignerKeyOperationPurposeV1::Response,
        &commitment.response_signing_message(),
    ));
    let completion = SignerCompletedOperationV1 {
        operation_id: request.operation_id,
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
    let receipt = SignerTopologyReceiptV1 {
        magic: TOPOLOGY_RECEIPT_MAGIC_V1,
        version: 1,
        custody_record: record,
        request,
        intent,
        reservation,
        provenance,
        commitment,
        signatures,
    };

    (receipt, completion)
}
/// Build a canonical synthetic receipt without claiming native execution authority.
pub(crate) fn receipt_fixture() -> ReceiptFixture {
    let signer = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519).unwrap();
    let attester = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).unwrap();
    let Authorization {
        binding,
        trust,
        current,
        record,
    } = authorization(&signer, &attester);
    let active = verify_signer_custody_use_v1(&record, &binding, &trust, &current).unwrap();
    let subject = subject(&binding);
    let operation_id = [0x61; 32];
    let prepared = prepare_topology_approval_v1(&subject, &binding).unwrap();
    let request = SignerTopologyRequestV1::new(&active, operation_id, &prepared).unwrap();
    let (receipt, completion) =
        signed_operation(&signer, record, request, &prepared, current.current_anchor);
    let mut fixture = ReceiptFixture {
        signer,
        attester,
        binding,
        trust,
        current,
        subject,
        operation_id,
        completion,
        receipt,
    };
    // Recovery is allowed after a timely original completion even when its reservation expires.
    fixture.current.now_unix_ms = 160_000;
    fixture.current.anchor_observed_at_unix_ms = 160_000;
    fixture.current.current_anchor.height = 110;
    fixture.current.current_anchor.block_hash = [0x73; 32];
    fixture
}
/// Check test-only consistency; these injected rows never prove native execution or deployment.
pub(crate) fn verify_receipt(f: &ReceiptFixture) -> Result<(), SignerTopologyReceiptErrorV1> {
    check_topology_receipt_consistency_v1(
        &norito::encode_canonical(&f.receipt).unwrap(),
        &f.receipt.signatures[0].signature,
        &TopologyReceiptReferenceV1 {
            subject: &f.subject,
            operation_id: f.operation_id,
            binding: &f.binding,
            trust: &f.trust,
            current: &f.current,
            completion: &f.completion,
        },
    )
}
