// Shared test-only fixture include. The containing module supplies a lexical `manifest` alias.
// These deterministic software keys simulate independent statements; they never qualify deployment custody.

use iroha_crypto::{Algorithm, KeyPair, Signature, sha256};
use manifest::signer::{
    custody::*,
    final_promotion::{evidence::*, statement::prepare_final_promotion_statement_v1, *},
    protocol::*,
    receipt::{
        SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1, SignerOperationProvenanceV1,
    },
};

include!("statement_fixture_support.rs");

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
    pub(crate) expected: SignerFinalPromotionExpectedV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) completion: SignerCompletedOperationV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) receipt: SignerFinalPromotionReceiptV1,
    /// Synthetic test input; never deployment evidence or runtime credentials.
    pub(crate) message: Vec<u8>,
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
/// Build or verify a deterministic synthetic receipt through the public contract.
pub(crate) fn receipt_fixture() -> ReceiptFixture {
    let signer = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519).unwrap();
    let attester = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).unwrap();
    let binding = SignerCustodyBindingV1 {
        chain_id: "promotion-chain".into(),
        network_id: [0x11; 32],
        runtime_handle: "software://sorafs/final-promotion-provenance/primary".into(),
        key_handle: "software://sorafs/final-promotion-provenance/key-7".into(),
        service_id: "promotion-primary".into(),
        administrator_id: "promotion-security-primary".into(),
        role: SignerRoleV1::FinalPromotionProvenance,
        purpose: SignerPurposeBindingV1::FinalPromotionProvenance {
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
    let active = verify_signer_custody_use_v1(&record, &binding, &trust, &current).unwrap();
    let message = statement_message(&binding);
    let expected = SignerFinalPromotionExpectedV1 {
        operation_id: [0x61; 32],
        statement_digest: signer_final_promotion_digest_v1(&message),
        statement_size: message.len() as u64,
    };
    let prepared = prepare_final_promotion_statement_v1(&message, &binding).unwrap();
    let request = SignerFinalPromotionRequestV1::new(&active, &expected, &prepared).unwrap();
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
    let raw = sign(&signer, SignerKeyOperationPurposeV1::RolePayload, &message);
    let audit =
        signer_final_promotion_audit_v1(&request, &intent, reservation, &raw.signature).unwrap();
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
        response_digest: signer_final_promotion_response_digest_v1(
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
    let receipt = SignerFinalPromotionReceiptV1 {
        magic: SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1,
        version: 1,
        custody_record: record,
        request,
        intent,
        reservation,
        provenance,
        commitment,
        signatures,
    };
    let mut fixture = ReceiptFixture {
        signer,
        attester,
        binding,
        trust,
        current,
        expected,
        completion,
        receipt,
        message,
    };
    fixture.current.now_unix_ms = 160_000; // Completion may be recovered after the reservation expires.
    fixture.current.anchor_observed_at_unix_ms = 160_000;
    fixture.current.current_anchor.height = 110;
    fixture.current.current_anchor.block_hash = [0x73; 32];
    fixture
}
/// Build or verify a deterministic synthetic receipt through the public contract.
pub(crate) fn verify_receipt(
    fixture: &ReceiptFixture,
) -> Result<VerifiedFinalPromotionSignerReceiptV1, SignerFinalPromotionReceiptErrorV1> {
    verify_final_promotion_signer_receipt_v1(
        &norito::encode_canonical(&fixture.receipt).unwrap(),
        &fixture.message,
        &fixture.receipt.signatures[0].signature,
        &fixture.expected,
        &fixture.binding,
        &fixture.trust,
        &fixture.current,
        &fixture.completion,
    )
}

/// Synthetic receipt plus independently signed observer inputs for full consumer verification.
pub(crate) struct EvidenceFixture {
    /// Synthetic independently pinned test input.
    pub(crate) receipt: ReceiptFixture,
    /// Synthetic independently pinned test input.
    pub(crate) observer: KeyPair,
    /// Synthetic independently pinned test input.
    pub(crate) policy: SignerFinalPromotionEvidencePolicyV1,
    /// Synthetic independently pinned test input.
    pub(crate) trust: SignerFinalPromotionEvidenceTrustV1,
    /// Synthetic independently pinned test input.
    pub(crate) state: SignerFinalPromotionStateObservationV1,
    /// Synthetic independently pinned test input.
    pub(crate) expected: SignerFinalPromotionEvidenceExpectedV1,
}
impl EvidenceFixture {
    /// Build, resign or verify this deterministic synthetic evidence fixture.
    pub(crate) fn new() -> Self {
        let receipt = receipt_fixture();
        let observer = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
        let policy = SignerFinalPromotionEvidencePolicyV1 {
            magic: SignerFinalPromotionEvidencePolicyV1::magic(),
            binding: receipt.binding.clone(),
            operation_id: receipt.expected.operation_id,
            statement_sha256: sha256(&receipt.message),
            statement_size: receipt.expected.statement_size,
            minimum_anchor: receipt.current.current_anchor,
        };
        let trust = SignerFinalPromotionEvidenceTrustV1 {
            magic: SignerFinalPromotionEvidenceTrustV1::magic(),
            custody_authority: receipt.trust.authority.clone(),
            custody_public_key: receipt.trust.public_key.clone(),
            custody_active_from_unix_ms: receipt.trust.active_from_unix_ms,
            custody_active_until_unix_ms: receipt.trust.active_until_unix_ms,
            custody_max_validity_ms: receipt.trust.max_validity_ms,
            state_authority: SignerCustodyAuthorityV1 {
                service_id: "finalized-state-observer".into(),
                administrator_id: "independent-state-reviewer".into(),
                key_revision: 2,
                policy_revision: 4,
                policy_digest: [0x74; 32],
            },
            state_public_key: observer.public_key().clone(),
            state_active_from_unix_ms: 90_000,
            state_active_until_unix_ms: 300_000,
            max_state_age_ms: 10_000,
        };
        let expected = SignerFinalPromotionEvidenceExpectedV1 {
            policy_sha256: sha256(norito::encode_canonical(&policy).unwrap()),
            trust_sha256: sha256(norito::encode_canonical(&trust).unwrap()),
            public_key_fingerprint_sha256: sha256(receipt.signer.public_key().to_bytes().1),
            now_unix_ms: receipt.current.now_unix_ms,
        };
        let state = SignerFinalPromotionStateObservationV1 {
            body: SignerFinalPromotionStateObservationBodyV1 {
                magic: SignerFinalPromotionStateObservationBodyV1::magic(),
                reviewed_policy_sha256: expected.policy_sha256,
                statement_sha256: policy.statement_sha256,
                statement_size: policy.statement_size,
                authority: trust.state_authority.clone(),
                chain_id: policy.binding.chain_id.clone(),
                network_id: policy.binding.network_id,
                deployment_id: "production-primary".into(),
                observed_at_unix_ms: receipt.current.anchor_observed_at_unix_ms,
                expires_at_unix_ms: receipt.current.now_unix_ms + 5_000,
                current_anchor: receipt.current.current_anchor,
                active_head: receipt.current.active_head,
                signer_revoked: false,
                attester_revoked: false,
                completed_operation: receipt.completion,
            },
            signature: [0; 64],
        };
        let mut result = Self {
            receipt,
            observer,
            policy,
            trust,
            state,
            expected,
        };
        result.sign_state();
        result
    }
    /// Build, resign or verify this deterministic synthetic evidence fixture.
    pub(crate) fn sign_state(&mut self) {
        self.state.signature = Signature::new(
            self.observer.private_key(),
            &self.state.body.signing_payload().unwrap(),
        )
        .payload()
        .try_into()
        .unwrap();
    }
    /// Build, resign or verify this deterministic synthetic evidence fixture.
    pub(crate) fn verify(
        &self,
    ) -> Result<VerifiedFinalPromotionSignerReceiptV1, SignerFinalPromotionEvidenceErrorV1> {
        verify_final_promotion_evidence_v1(
            &norito::encode_canonical(&self.policy).unwrap(),
            &norito::encode_canonical(&self.trust).unwrap(),
            &norito::encode_canonical(&self.state).unwrap(),
            &norito::encode_canonical(&self.receipt.receipt).unwrap(),
            &self.receipt.message,
            &self.receipt.receipt.signatures[0].signature,
            &self
                .receipt
                .signer
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .unwrap(),
            &self.expected,
        )
    }
}

impl EvidenceFixture {
    /// Encode the exact independently pinned policy document.
    pub(crate) fn policy_bytes(&self) -> Vec<u8> {
        norito::encode_canonical(&self.policy).unwrap()
    }
    /// Encode the exact independently pinned trust document.
    pub(crate) fn trust_bytes(&self) -> Vec<u8> {
        norito::encode_canonical(&self.trust).unwrap()
    }
    /// Encode the signed observation, including deliberate test mutations.
    pub(crate) fn state_bytes(&self) -> Vec<u8> {
        norito::encode_canonical(&self.state).unwrap()
    }
    /// Encode the candidate complete receipt.
    pub(crate) fn receipt_bytes(&self) -> Vec<u8> {
        norito::encode_canonical(&self.receipt.receipt).unwrap()
    }
    /// Return the synthetic role key's raw public bytes.
    pub(crate) fn raw_public_key(&self) -> [u8; 32] {
        self.receipt
            .signer
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .unwrap()
    }
}
