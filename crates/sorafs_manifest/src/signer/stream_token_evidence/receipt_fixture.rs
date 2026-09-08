//! Independent signed stream-receipt simulations; no fixture qualifies physical hardware.

use super::*;

pub(super) struct Fixture {
    pub(super) signer: KeyPair,
    pub(super) attester: KeyPair,
    pub(super) binding: SignerCustodyBindingV1,
    pub(super) trust: SignerCustodyTrustV1,
    pub(super) current: SignerCustodyUseContextV1,
    pub(super) expected: SignerStreamTokenExpectedV1,
    pub(super) completion: SignerCompletedOperationV1,
    pub(super) receipt: SignerStreamTokenReceiptV1,
    pub(super) token: StreamTokenV1,
}

pub(super) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("simulated Ed25519 key")
}

pub(super) fn layouts() -> [u8; 10] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}

// Explicit domains and length prefixes deliberately do not call the production digest helpers.
pub(super) fn oracle_digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(
            &u64::try_from(part.len())
                .expect("bounded fixture")
                .to_be_bytes(),
        );
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

pub(super) fn oracle_canonical<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> [u8; 32] {
    oracle_digest(
        domain,
        &[&norito::encode_canonical(value).expect("canonical oracle")],
    )
}

pub(super) fn oracle_payload(body: &StreamTokenBodyV1) -> Vec<u8> {
    let mut payload = b"sorafs.stream-token.signature.v1\0".to_vec();
    payload.extend(norito::encode_canonical(body).expect("body oracle"));
    payload
}

pub(super) fn oracle_intent(intent: &SignerOperationIntentV1) -> [u8; 32] {
    oracle_canonical(b"iroha.sorafs.signer.operation.intent.v1", intent)
}

pub(super) fn oracle_audit(
    receipt: &SignerStreamTokenReceiptV1,
    signature: &[u8],
) -> SignerOperationAuditHeadV1 {
    SignerOperationAuditHeadV1 {
        sequence: receipt
            .intent
            .previous_audit
            .sequence
            .checked_add(1)
            .expect("valid predecessor"),
        digest: oracle_canonical(
            b"iroha.sorafs.signer.stream-token.audit.v1",
            &(
                receipt.request,
                oracle_intent(&receipt.intent),
                receipt.reservation,
                oracle_digest(b"iroha.sorafs.signer.operation.signature.v1", &[signature]),
            ),
        ),
    }
}

pub(super) fn oracle_signatures(signatures: &[SignerOperationSignatureV1]) -> [u8; 32] {
    let manifest: Vec<_> = signatures
        .iter()
        .map(|signature| {
            (
                signature.purpose,
                signature.message_digest,
                oracle_digest(
                    b"iroha.sorafs.signer.operation.signature.v1",
                    &[&signature.signature],
                ),
            )
        })
        .collect();
    oracle_canonical(b"iroha.sorafs.signer.operation.signatures.v1", &manifest)
}

pub(super) fn oracle_response(receipt: &SignerStreamTokenReceiptV1) -> [u8; 32] {
    oracle_canonical(
        b"iroha.sorafs.signer.stream-token.response.v1",
        &(
            receipt.request,
            receipt.provenance,
            oracle_signatures(&receipt.signatures[..3]),
        ),
    )
}

pub(super) fn sign(
    key: &KeyPair,
    purpose: SignerKeyOperationPurposeV1,
    message: &[u8],
) -> SignerOperationSignatureV1 {
    SignerOperationSignatureV1 {
        purpose,
        message_digest: oracle_digest(b"iroha.sorafs.signer.operation.message.v1", &[message]),
        signature: Signature::try_new(key.private_key(), message)
            .expect("Ed25519 signature")
            .payload()
            .to_vec(),
    }
}

pub(super) fn attest_unchecked(statement: SignerCustodyStatementV1, attester: &KeyPair) -> Vec<u8> {
    let mut payload = b"iroha:sorafs:hardware-signer-custody:v1\0".to_vec();
    payload.extend(norito::encode_canonical(&statement).expect("statement oracle"));
    let signature =
        Signature::try_new(attester.private_key(), &payload).expect("independent attestation");
    norito::encode_canonical(&SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().expect("Ed25519 width"),
    })
    .expect("signed custody record")
}

pub(super) fn fixture() -> Fixture {
    fixture_with(0x21, |_, _, _| {})
}

pub(super) fn fixture_with(
    signer_seed: u8,
    configure: impl FnOnce(
        &mut StreamTokenBodyV1,
        &mut SignerCustodyBindingV1,
        &mut SignerCustodyAuthorityV1,
    ),
) -> Fixture {
    let signer = key(signer_seed);
    let attester = key(0x31);
    let mut body = StreamTokenBodyV1 {
        token_id: "0123456789abcdef0123456789abcdef".into(),
        manifest_cid: vec![0x01, 0x55, 0x01],
        provider_id: [0x61; 32],
        profile_handle: "sorafs.sf1@1.0.0".into(),
        max_streams: 4,
        ttl_epoch: 1_200,
        rate_limit_bytes: 1_024,
        issued_at: 1_000,
        requests_per_minute: 120,
        token_pk_version: 7,
    };
    let mut binding = SignerCustodyBindingV1 {
        chain_id: "sorafs-reference".into(),
        network_id: [0x11; 32],
        runtime_handle: "hsm://sorafs/stream/primary".into(),
        key_handle: "pkcs11:production/stream/key-7".into(),
        service_id: "stream-primary".into(),
        administrator_id: "stream-security-primary".into(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: body.provider_id,
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: signer.public_key().clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x51; 32],
    };
    let mut authority = SignerCustodyAuthorityV1 {
        service_id: "custody-authority-primary".into(),
        administrator_id: "custody-security-primary".into(),
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x41; 32],
    };
    configure(&mut body, &mut binding, &mut authority);
    // Trust is independently configured before any candidate statement or receipt exists.
    let trust = SignerCustodyTrustV1 {
        authority: authority.clone(),
        public_key: attester.public_key().clone(),
        active_from_unix_ms: 800_000,
        active_until_unix_ms: 2_000_000,
        max_validity_ms: 1_000_000,
        max_anchor_age_ms: 10_000,
    };
    let approved = SignerCustodyAnchorV1 {
        height: 90,
        block_hash: [0x43; 32],
        state_digest: [0x45; 32],
    };
    let record = attest_unchecked(
        SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: binding.clone(),
            authority,
            anchor: approved,
            sequence: 1,
            predecessor_digest: [0; 32],
            issued_at_unix_ms: 900_000,
            expires_at_unix_ms: 1_900_000,
            hardware_identity_digest: [0x53; 32],
            evidence_digest: [0x55; 32],
            generated_in_hardware: true,
            exportable: false,
            ever_exported: false,
            revoked: false,
        },
        &attester,
    );
    let admitted = verify_signer_custody_enrollment_v1(
        &record,
        &binding,
        &trust,
        &SignerCustodyEnrollmentContextV1 {
            now_unix_ms: 1_110_000,
            anchor_observed_at_unix_ms: 1_110_000,
            current_anchor: approved,
            next_sequence: 1,
            predecessor_digest: [0; 32],
            signer_revoked: false,
            attester_revoked: false,
        },
    )
    .expect("independent stream custody enrollment simulation");
    let mut current = SignerCustodyUseContextV1 {
        now_unix_ms: 1_120_000,
        anchor_observed_at_unix_ms: 1_120_000,
        current_anchor: SignerCustodyAnchorV1 {
            height: 100,
            block_hash: [0x81; 32],
            state_digest: [0x83; 32],
        },
        active_head: SignerCustodyActiveHeadV1 {
            record_digest: admitted.record_digest(),
            sequence: 1,
            approved_anchor: approved,
            key_revision: binding.key_revision,
            policy_revision: binding.policy_revision,
            policy_digest: binding.policy_digest,
        },
        signer_revoked: false,
        attester_revoked: false,
    };
    let custody = verify_signer_custody_use_v1(&record, &binding, &trust, &current)
        .expect("independently simulated ACTIVE stream custody");
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("prepared subject");
    let request =
        SignerStreamTokenRequestV1::new(&custody, &expected, &body).expect("original request");
    let intent = SignerOperationIntentV1 {
        action: SignerOperationActionV1::Sign,
        operation_id: expected.operation_id(),
        request_digest: oracle_canonical(b"iroha.sorafs.signer.stream-token.request.v1", &request),
        previous_audit: SignerOperationAuditHeadV1 {
            sequence: 4,
            digest: [0x62; 32],
        },
    };
    let reservation = SignerOperationReservationV1 {
        reservation_id: [0x63; 32],
        fence: 8,
        expires_at_unix_ms: 1_150_000,
    };
    let raw = sign(
        &signer,
        SignerKeyOperationPurposeV1::RolePayload,
        &oracle_payload(&body),
    );
    let token = StreamTokenV1 {
        body,
        signature: raw.signature.clone(),
    };
    let initial_audit = SignerOperationAuditHeadV1 {
        sequence: 0,
        digest: [0; 32],
    };
    let commitment = SignerOperationCommitmentV1 {
        audit: initial_audit,
        response_digest: [0; 32],
    };
    let receipt = SignerStreamTokenReceiptV1 {
        magic: *b"IRSTKR01",
        version: 1,
        custody_record: record,
        request,
        intent,
        reservation,
        provenance: SignerOperationProvenanceV1 {
            original_custody: request.original_custody,
            signing_anchor: current.current_anchor,
            intent_digest: oracle_intent(&intent),
            reservation,
            audit: initial_audit,
        },
        commitment,
        signatures: vec![raw],
    };
    let completion = SignerCompletedOperationV1 {
        operation_id: expected.operation_id(),
        intent_digest: oracle_intent(&intent),
        original_custody: request.original_custody,
        reservation,
        commitment,
        signatures_digest: [0; 32],
        completed_at_unix_ms: 1_125_000,
        anchor: SignerOperationFinalizedAnchorV1 {
            height: 101,
            block_hash: [0x71; 32],
            operation_state_digest: [0x72; 32],
        },
    };
    current.now_unix_ms = 1_160_000;
    current.anchor_observed_at_unix_ms = 1_160_000;
    current.current_anchor.height = 110;
    current.current_anchor.block_hash = [0x73; 32];
    let mut fixture = Fixture {
        signer,
        attester,
        binding,
        trust,
        current,
        expected,
        completion,
        receipt,
        token,
    };
    rebuild_operation(&mut fixture);
    fixture
}

// Re-sign all downstream claims, retaining the caller's provenance and exact role signature.
// This is deliberately unable to replace the independently supplied completed row.
pub(super) fn resign_follow_on(f: &mut Fixture) {
    let audit = f.receipt.commitment.audit;
    let audit_message = oracle_digest(
        b"iroha.external-signer.audit-attestation.v1",
        &[&audit.sequence.to_be_bytes(), &audit.digest],
    );
    let provenance_message = oracle_canonical(
        b"iroha.sorafs.signer.operation.provenance.v1",
        &f.receipt.provenance,
    );
    f.receipt.signatures.truncate(1);
    f.receipt.signatures.push(sign(
        &f.signer,
        SignerKeyOperationPurposeV1::AuditRecord,
        &audit_message,
    ));
    f.receipt.signatures.push(sign(
        &f.signer,
        SignerKeyOperationPurposeV1::Provenance,
        &provenance_message,
    ));
    resign_response(f);
}

pub(super) fn resign_response(f: &mut Fixture) {
    f.receipt.signatures.truncate(3);
    f.receipt.commitment.response_digest = oracle_response(&f.receipt);
    let response_message = oracle_digest(
        b"iroha.external-signer.response-attestation.v1",
        &[&f.receipt.commitment.response_digest],
    );
    f.receipt.signatures.push(sign(
        &f.signer,
        SignerKeyOperationPurposeV1::Response,
        &response_message,
    ));
}

pub(super) fn rebuild_operation(f: &mut Fixture) {
    f.receipt.intent.request_digest = oracle_canonical(
        b"iroha.sorafs.signer.stream-token.request.v1",
        &f.receipt.request,
    );
    f.receipt.commitment.audit = oracle_audit(&f.receipt, &f.token.signature);
    f.receipt.provenance.original_custody = f.receipt.request.original_custody;
    f.receipt.provenance.intent_digest = oracle_intent(&f.receipt.intent);
    f.receipt.provenance.reservation = f.receipt.reservation;
    f.receipt.provenance.audit = f.receipt.commitment.audit;
    resign_follow_on(f);
    // Only fixture construction simulates a corresponding authoritative immutable row.
    f.completion.operation_id = f.receipt.request.operation_id;
    f.completion.intent_digest = oracle_intent(&f.receipt.intent);
    f.completion.original_custody = f.receipt.request.original_custody;
    f.completion.reservation = f.receipt.reservation;
    f.completion.commitment = f.receipt.commitment;
    f.completion.signatures_digest = oracle_signatures(&f.receipt.signatures);
}

pub(super) fn active(f: &Fixture) -> VerifiedSignerCustodyV1 {
    verify_signer_custody_use_v1(&f.receipt.custody_record, &f.binding, &f.trust, &f.current)
        .expect("positive independent custody")
}

pub(super) fn verify_bytes(
    f: &Fixture,
    bytes: &[u8],
) -> Result<VerifiedStreamTokenSignerReceiptV1, SignerStreamTokenReceiptErrorV1> {
    verify_stream_token_signer_receipt_v1(
        bytes,
        &f.token,
        &f.expected,
        &f.binding,
        &f.trust,
        &f.current,
        &f.completion,
    )
}

pub(super) fn verify(
    f: &Fixture,
) -> Result<VerifiedStreamTokenSignerReceiptV1, SignerStreamTokenReceiptErrorV1> {
    // Raw canonical serialization is intentional: malformed semantic fields must reach decoding
    // and verification even when the safe producer's encode_canonical() would reject them first.
    verify_bytes(
        f,
        &norito::encode_canonical(&f.receipt).expect("candidate frame"),
    )
}

pub(super) fn assert_positive(f: &Fixture) {
    let (_, public) = f
        .binding
        .public_key
        .try_to_bytes()
        .expect("checked public key");
    let public: [u8; 32] = public.try_into().expect("Ed25519 public key");
    f.token
        .verify(&ed25519_dalek::VerifyingKey::from_bytes(&public).expect("public key"))
        .expect("actual strict stream-token verifier");
    let custody = active(f);
    assert_eq!(
        validate_stream_token_signatures_v1(&f.receipt, &f.token, &f.expected, &custody)
            .expect("signature-only positive"),
        oracle_signatures(&f.receipt.signatures)
    );
    let verified = verify(f).expect("independent complete positive before mutation");
    assert_eq!(verified.completion(), &f.completion);
    assert_eq!(
        verified.signing_payload_digest(),
        f.expected.signing_payload_digest()
    );
    assert_eq!(
        verified.custody().record_digest(),
        f.current.active_head.record_digest
    );
}
