//! Prepared-subject adversarial tests with simulated independent custody, not hardware evidence.

use super::*;
use crate::signer::custody::{
    SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyActiveHeadV1,
    SignerCustodyAnchorV1, SignerCustodyAuthorityV1, SignerCustodyEnrollmentContextV1,
    SignerCustodyRecordV1, SignerCustodyStatementV1, SignerCustodyTrustV1,
    SignerCustodyUseContextV1, verify_signer_custody_enrollment_v1, verify_signer_custody_use_v1,
};
use iroha_crypto::{Algorithm, KeyPair, Signature};

fn layouts() -> [u8; 10] {
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

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("simulated key")
}

fn fixture() -> (StreamTokenBodyV1, SignerCustodyBindingV1) {
    let body = StreamTokenBodyV1 {
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
    let binding = SignerCustodyBindingV1 {
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
        public_key: key(0x21).public_key().clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x51; 32],
    };
    (body, binding)
}

// Deliberately independent of protocol::digest_parts/digest_canonical and the constants above.
fn oracle_digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
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

fn oracle_payload(body: &StreamTokenBodyV1) -> Vec<u8> {
    let mut bytes = b"sorafs.stream-token.signature.v1\0".to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(body).expect("canonical body oracle"));
    bytes
}

fn assert_subject_oracle(body: &StreamTokenBodyV1, binding: &SignerCustodyBindingV1) {
    let payload = oracle_payload(body);
    let frame = norito::encode_canonical(binding).expect("canonical binding oracle");
    let binding_digest = oracle_digest(b"iroha.sorafs.signer.custody-binding.v1", &[&frame]);
    let expected = SignerStreamTokenExpectedV1::new(body, binding).expect("valid prepared body");
    assert_eq!(body.signing_payload_bytes().expect("role bytes"), payload);
    assert_eq!(expected.binding_digest(), binding_digest);
    assert_eq!(expected.signing_payload_size(), payload.len() as u64);
    assert_eq!(
        expected.issued_at_unix_ms(),
        body.issued_at.checked_mul(1_000).unwrap()
    );
    assert_eq!(
        expected.expires_at_unix_ms(),
        body.ttl_epoch.checked_mul(1_000).unwrap()
    );
    assert_eq!(
        expected.signing_payload_digest(),
        oracle_digest(b"iroha.sorafs.signer.stream-token.payload.v1", &[&payload])
    );
    assert_eq!(
        expected.operation_id(),
        oracle_digest(
            b"iroha.sorafs.signer.stream-token.operation.v1",
            &[&binding_digest, &payload]
        )
    );
    assert_ne!(
        expected.operation_id(),
        oracle_digest(
            b"iroha.sorafs.signer.stream-token.operation.v1",
            &[&payload, &binding_digest]
        )
    );
    assert_ne!(
        expected.signing_payload_digest(),
        oracle_digest(
            b"iroha.sorafs.signer.stream-token.payload.v1",
            &[&payload[b"sorafs.stream-token.signature.v1\0".len()..]]
        )
    );
}

fn independently_qualified(
    binding: &SignerCustodyBindingV1,
    sequence: u64,
    predecessor_digest: [u8; 32],
    state_digest: [u8; 32],
) -> VerifiedSignerCustodyV1 {
    let attester = key(0x31);
    let authority = SignerCustodyAuthorityV1 {
        service_id: "custody-authority-primary".into(),
        administrator_id: "custody-security-primary".into(),
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x41; 32],
    };
    let approved_anchor = SignerCustodyAnchorV1 {
        height: 90 + sequence,
        block_hash: [0x43; 32],
        state_digest: [0x45; 32],
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: binding.clone(),
        authority: authority.clone(),
        anchor: approved_anchor,
        sequence,
        predecessor_digest,
        issued_at_unix_ms: 900_000,
        expires_at_unix_ms: 1_900_000,
        hardware_identity_digest: [0x53; 32],
        evidence_digest: [0x55; 32],
        generated_in_hardware: true,
        exportable: false,
        ever_exported: false,
        revoked: false,
    };
    // The independent test authority signs the actual StreamToken statement. No release fixture
    // is relabelled, and no private Verified constructor or candidate-created trust is used.
    let mut payload = b"iroha:sorafs:hardware-signer-custody:v1\0".to_vec();
    payload.extend_from_slice(&norito::encode_canonical(&statement).expect("statement oracle"));
    let signature = Signature::try_new(attester.private_key(), &payload).expect("attestation");
    let bytes = norito::encode_canonical(&SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().expect("Ed25519 width"),
    })
    .expect("record");
    let trust = SignerCustodyTrustV1 {
        authority,
        public_key: attester.public_key().clone(),
        active_from_unix_ms: 800_000,
        active_until_unix_ms: 2_000_000,
        max_validity_ms: 1_000_000,
        max_anchor_age_ms: 100,
    };
    let enrollment_context = SignerCustodyEnrollmentContextV1 {
        now_unix_ms: 1_050_000,
        anchor_observed_at_unix_ms: 1_049_950,
        current_anchor: approved_anchor,
        next_sequence: sequence,
        predecessor_digest,
        signer_revoked: false,
        attester_revoked: false,
    };
    let admitted =
        verify_signer_custody_enrollment_v1(&bytes, binding, &trust, &enrollment_context)
            .expect("independent enrollment simulation");
    let context = SignerCustodyUseContextV1 {
        now_unix_ms: enrollment_context.now_unix_ms,
        anchor_observed_at_unix_ms: enrollment_context.anchor_observed_at_unix_ms,
        current_anchor: SignerCustodyAnchorV1 {
            height: approved_anchor.height + 1,
            block_hash: [0x81; 32],
            state_digest,
        },
        active_head: SignerCustodyActiveHeadV1 {
            record_digest: admitted.record_digest(),
            sequence,
            approved_anchor,
            key_revision: binding.key_revision,
            policy_revision: binding.policy_revision,
            policy_digest: binding.policy_digest,
        },
        signer_revoked: false,
        attester_revoked: false,
    };
    verify_signer_custody_use_v1(&bytes, binding, &trust, &context)
        .expect("independently simulated ACTIVE head, not production qualification")
}

#[test]
fn prepared_subject_and_request_match_independent_preimages_in_all_ten_layouts() {
    let (body, binding) = fixture();
    let custody = independently_qualified(&binding, 1, [0; 32], [0x83; 32]);
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("subject");
    let baseline = SignerStreamTokenRequestV1::new(&custody, &expected, &body).expect("request");
    let canonical = norito::encode_canonical(&baseline).expect("canonical request oracle");
    let digest = oracle_digest(
        b"iroha.sorafs.signer.stream-token.request.v1",
        &[&canonical],
    );
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_subject_oracle(&body, &binding);
        let request = SignerStreamTokenRequestV1::new(&custody, &expected, &body).expect("request");
        assert_eq!(request, baseline);
        assert_eq!(request.digest().expect("request digest"), digest);
        assert_eq!(
            norito::encode_canonical(&request).expect("canonical request"),
            canonical
        );
        assert_eq!(
            request.original_custody.record_digest,
            custody.record_digest()
        );
        assert_eq!(
            request.original_custody.control_state_digest,
            custody.current_anchor().state_digest
        );
    }
}

type BodyMutation = (&'static str, fn(&mut StreamTokenBodyV1));

#[test]
fn every_valid_body_slot_changes_the_prepared_operation_and_denies_original_request() {
    let (body, binding) = fixture();
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("subject");
    let custody = independently_qualified(&binding, 1, [0; 32], [0x83; 32]);
    let mutations: &[BodyMutation] = &[
        ("token_id", |v| v.token_id.replace_range(..1, "f")),
        ("manifest_cid", |v| v.manifest_cid[0] ^= 1),
        ("provider_id", |v| v.provider_id[0] ^= 1),
        ("profile_handle", |v| v.profile_handle.push('a')),
        ("max_streams", |v| v.max_streams += 1),
        ("ttl_epoch", |v| v.ttl_epoch += 1),
        ("rate_limit_bytes", |v| v.rate_limit_bytes += 1),
        ("issued_at", |v| v.issued_at += 1),
        ("requests_per_minute", |v| v.requests_per_minute += 1),
        ("token_pk_version", |v| v.token_pk_version += 1),
    ];
    for (name, mutate) in mutations {
        let mut changed = body.clone();
        mutate(&mut changed);
        let mut authorized = binding.clone();
        authorized.purpose = SignerPurposeBindingV1::StreamToken {
            provider_id: changed.provider_id,
        };
        authorized.key_revision = u64::from(changed.token_pk_version);
        let other = SignerStreamTokenExpectedV1::new(&changed, &authorized).expect(name);
        assert_ne!(other.operation_id(), expected.operation_id(), "{name}");
        assert_ne!(
            other.signing_payload_digest(),
            expected.signing_payload_digest(),
            "{name}"
        );
        assert_subject_oracle(&changed, &authorized);
        let error = if *name == "provider_id" {
            SignerStreamTokenReceiptErrorV1::WrongPurpose
        } else {
            SignerStreamTokenReceiptErrorV1::TokenMismatch
        };
        assert_eq!(
            SignerStreamTokenRequestV1::new(&custody, &expected, &changed),
            Err(error),
            "{name}"
        );
    }
}

type BindingMutation = (&'static str, fn(&mut SignerCustodyBindingV1));

#[test]
fn every_valid_binding_slot_is_committed_and_cannot_replace_prepared_authority() {
    let (body, binding) = fixture();
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("subject");
    let mutations: &[BindingMutation] = &[
        ("chain_id", |v| v.chain_id.push('a')),
        ("network_id", |v| v.network_id[0] ^= 1),
        ("runtime_handle", |v| v.runtime_handle.push('a')),
        ("key_handle", |v| v.key_handle.push('a')),
        ("service_id", |v| v.service_id.push('a')),
        ("administrator_id", |v| v.administrator_id.push('a')),
        ("public_key", |v| {
            v.public_key = key(0x23).public_key().clone()
        }),
        ("key_revision", |v| v.key_revision += 1),
        ("policy_revision", |v| v.policy_revision += 1),
        ("policy_digest", |v| v.policy_digest[0] ^= 1),
        ("purpose.provider_id", |v| {
            v.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: [0x63; 32],
            }
        }),
    ];
    for (name, mutate) in mutations {
        let mut changed = binding.clone();
        mutate(&mut changed);
        let mut matching_body = body.clone();
        matching_body.token_pk_version =
            u32::try_from(changed.key_revision).expect("fixture revision");
        if let SignerPurposeBindingV1::StreamToken { provider_id } = &changed.purpose {
            matching_body.provider_id = *provider_id;
        }
        let other = SignerStreamTokenExpectedV1::new(&matching_body, &changed).expect(name);
        assert_ne!(other.binding_digest(), expected.binding_digest(), "{name}");
        assert_ne!(other.operation_id(), expected.operation_id(), "{name}");
        assert_subject_oracle(&matching_body, &changed);
        let custody = independently_qualified(&changed, 1, [0; 32], [0x83; 32]);
        assert_eq!(
            SignerStreamTokenRequestV1::new(&custody, &expected, &matching_body),
            Err(SignerStreamTokenReceiptErrorV1::TokenMismatch),
            "{name}"
        );
    }
}

#[test]
fn body_structural_leaves_and_limits_are_rejected_before_preparation() {
    let (body, binding) = fixture();
    let invalid: &[BodyMutation] = &[
        ("empty token", |v| v.token_id.clear()),
        ("long token", |v| v.token_id.push('0')),
        ("uppercase token", |v| v.token_id.make_ascii_uppercase()),
        ("nonhex token", |v| v.token_id.replace_range(..1, "g")),
        ("empty CID", |v| v.manifest_cid.clear()),
        ("large CID", |v| v.manifest_cid = vec![1; 129]),
        ("zero provider", |v| v.provider_id = [0; 32]),
        ("empty profile", |v| v.profile_handle.clear()),
        ("large profile", |v| v.profile_handle = "a".repeat(129)),
        ("spaced profile", |v| v.profile_handle.push(' ')),
        ("NUL profile", |v| v.profile_handle.push('\0')),
        ("non-ASCII profile", |v| v.profile_handle.push('é')),
        ("zero streams", |v| v.max_streams = 0),
        ("large streams", |v| v.max_streams = 1_025),
        ("zero issuance", |v| v.issued_at = 0),
        ("empty lifetime", |v| v.ttl_epoch = v.issued_at),
        ("inverted lifetime", |v| v.ttl_epoch = v.issued_at - 1),
        ("large lifetime", |v| v.ttl_epoch = v.issued_at + 3_601),
        ("zero rate", |v| v.rate_limit_bytes = 0),
        ("large rate", |v| v.rate_limit_bytes = 1_073_741_825),
        ("zero requests", |v| v.requests_per_minute = 0),
        ("large requests", |v| v.requests_per_minute = 10_001),
        ("zero revision", |v| v.token_pk_version = 0),
    ];
    for (name, mutate) in invalid {
        let mut changed = body.clone();
        mutate(&mut changed);
        assert_eq!(
            SignerStreamTokenExpectedV1::new(&changed, &binding),
            Err(SignerStreamTokenReceiptErrorV1::TokenMismatch),
            "{name}"
        );
    }
}

#[test]
fn binding_structural_leaves_wrong_role_and_provider_fail_closed() {
    let (body, binding) = fixture();
    let invalid: &[BindingMutation] = &[
        ("empty chain", |v| v.chain_id.clear()),
        ("large chain", |v| v.chain_id = "a".repeat(129)),
        ("noncanonical chain", |v| v.chain_id.push(' ')),
        ("zero network", |v| v.network_id = [0; 32]),
        ("runtime credentials", |v| {
            v.runtime_handle.push_str("?password=secret")
        }),
        ("empty runtime", |v| v.runtime_handle.clear()),
        ("large runtime", |v| {
            v.runtime_handle = format!("hsm:{}", "a".repeat(125))
        }),
        ("software runtime", |v| {
            v.runtime_handle = "software:primary".into()
        }),
        ("mock key", |v| v.key_handle = "hsm:mock/primary".into()),
        ("empty key", |v| v.key_handle.clear()),
        ("large key", |v| {
            v.key_handle = format!("kms:{}", "a".repeat(125))
        }),
        ("empty service", |v| v.service_id.clear()),
        ("large service", |v| v.service_id = "a".repeat(129)),
        ("invalid service", |v| v.service_id.push('\0')),
        ("empty administrator", |v| v.administrator_id.clear()),
        ("large administrator", |v| {
            v.administrator_id = "a".repeat(129)
        }),
        ("shared administrator", |v| {
            v.administrator_id = v.service_id.clone()
        }),
        ("wrong role", |v| v.role = SignerRoleV1::Promotion),
        ("wrong purpose", |v| {
            v.purpose = SignerPurposeBindingV1::NativeOrPromotion
        }),
        ("zero purpose provider", |v| {
            v.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: [0; 32],
            }
        }),
        ("wrong algorithm", |v| {
            v.algorithm = SignerKeyAlgorithmV1::MlDsa
        }),
        ("public key algorithm mismatch", |v| {
            v.public_key = KeyPair::try_from_seed(vec![0x29; 32], Algorithm::Secp256k1)
                .expect("different public-key algorithm")
                .public_key()
                .clone()
        }),
        ("zero key revision", |v| v.key_revision = 0),
        ("zero policy revision", |v| v.policy_revision = 0),
        ("zero policy digest", |v| v.policy_digest = [0; 32]),
    ];
    for (name, mutate) in invalid {
        let mut changed = binding.clone();
        mutate(&mut changed);
        assert_eq!(
            SignerStreamTokenExpectedV1::new(&body, &changed),
            Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt),
            "{name}"
        );
    }
    let mut changed = binding.clone();
    changed.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x63; 32],
    };
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&body, &changed),
        Err(SignerStreamTokenReceiptErrorV1::WrongPurpose)
    );
    // A coherent, independently attested alternate role must reach the purpose guard.
    changed.role = SignerRoleV1::Promotion;
    changed.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    let custody = independently_qualified(&changed, 1, [0; 32], [0x83; 32]);
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("stream subject");
    assert_eq!(
        SignerStreamTokenRequestV1::new(&custody, &expected, &body),
        Err(SignerStreamTokenReceiptErrorV1::WrongPurpose)
    );
}

#[test]
fn revisions_and_unix_milliseconds_are_checked_without_narrowing_or_overflow() {
    let (mut body, mut binding) = fixture();
    body.token_pk_version = u32::MAX;
    binding.key_revision = u64::from(u32::MAX);
    SignerStreamTokenExpectedV1::new(&body, &binding).expect("maximum representable revision");
    for revision in [u64::from(u32::MAX) + 1, u64::MAX] {
        binding.key_revision = revision;
        assert_eq!(
            SignerStreamTokenExpectedV1::new(&body, &binding),
            Err(SignerStreamTokenReceiptErrorV1::TokenMismatch)
        );
    }
    binding.key_revision = u64::from(body.token_pk_version);
    let maximum_seconds = u64::MAX / 1_000;
    body.issued_at = maximum_seconds - 3_600;
    body.ttl_epoch = maximum_seconds;
    SignerStreamTokenExpectedV1::new(&body, &binding)
        .expect("exact representable millisecond range");
    body.issued_at = maximum_seconds;
    body.ttl_epoch = maximum_seconds + 1;
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&body, &binding),
        Err(SignerStreamTokenReceiptErrorV1::InvalidTime)
    );
    body.issued_at += 1;
    body.ttl_epoch += 1;
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&body, &binding),
        Err(SignerStreamTokenReceiptErrorV1::InvalidTime)
    );
}

#[test]
fn maximum_valid_body_counts_actual_payload_and_signed_frame_in_all_ten_layouts() {
    let (mut body, mut binding) = fixture();
    body.manifest_cid = vec![0xff; 128];
    body.profile_handle = "a".repeat(128);
    body.max_streams = 1_024;
    body.ttl_epoch = u64::MAX / 1_000;
    body.issued_at = body.ttl_epoch - 3_600;
    body.rate_limit_bytes = 1_073_741_824;
    body.requests_per_minute = 10_000;
    body.token_pk_version = u32::MAX;
    binding.key_revision = u64::from(u32::MAX);
    let payload = oracle_payload(&body);
    let token = StreamTokenV1 {
        body: body.clone(),
        signature: vec![0; 64],
    };
    let wire = norito::encode_canonical(&token).expect("maximum legal signed schema");
    assert!(payload.len() <= 2_048);
    assert!(wire.len() <= 2_048);
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let subject =
            SignerStreamTokenExpectedV1::new(&body, &binding).expect("maximum valid body");
        assert_eq!(subject.signing_payload_size(), payload.len() as u64);
        assert_eq!(
            norito::canonical_frame_len(&body).expect("body count")
                + b"sorafs.stream-token.signature.v1\0".len(),
            payload.len()
        );
        assert_eq!(
            norito::canonical_frame_len(&token).expect("signed frame count"),
            wire.len()
        );
        assert_subject_oracle(&body, &binding);
    }
    // These are actual schema maxima, not padding to manufacture a nominal 2,048-byte body.
    body.manifest_cid.push(0xff);
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&body, &binding),
        Err(SignerStreamTokenReceiptErrorV1::TokenMismatch)
    );
}

#[test]
fn renewal_preserves_operation_identity_but_commits_original_custody_and_control() {
    let (body, binding) = fixture();
    let first = independently_qualified(&binding, 1, [0; 32], [0x83; 32]);
    let renewed = independently_qualified(&binding, 2, first.record_digest(), [0x85; 32]);
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("retained subject");
    let initial =
        SignerStreamTokenRequestV1::new(&first, &expected, &body).expect("initial request");
    let next =
        SignerStreamTokenRequestV1::new(&renewed, &expected, &body).expect("renewed request");
    assert_eq!(initial.operation_id, next.operation_id);
    assert_eq!(initial.binding_digest, next.binding_digest);
    assert_eq!(initial.signing_payload_digest, next.signing_payload_digest);
    assert_ne!(
        initial.original_custody.record_digest,
        next.original_custody.record_digest
    );
    assert_ne!(
        initial.original_custody.control_state_digest,
        next.original_custody.control_state_digest
    );
    assert_ne!(
        initial.digest().expect("old request digest"),
        next.digest().expect("new request digest")
    );
    let control_only = independently_qualified(&binding, 1, [0; 32], [0x87; 32]);
    let changed =
        SignerStreamTokenRequestV1::new(&control_only, &expected, &body).expect("new control");
    assert_eq!(
        initial.original_custody.record_digest,
        changed.original_custody.record_digest
    );
    assert_ne!(
        initial.digest().expect("old digest"),
        changed.digest().expect("control-bound digest")
    );
    // TODO: the complete receipt/coordinator integration must reject recovery of initial under
    // renewed/control_only custody and must not reserve the same operation a second time.
}

#[test]
fn each_public_request_slot_is_digested_and_debug_discloses_no_payload() {
    let (body, binding) = fixture();
    let custody = independently_qualified(&binding, 1, [0; 32], [0x83; 32]);
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).expect("subject");
    let request = SignerStreamTokenRequestV1::new(&custody, &expected, &body).expect("request");
    let original = request.digest().expect("request digest");
    let mutations: &[fn(&mut SignerStreamTokenRequestV1)] = &[
        |v| v.operation_id[0] ^= 1,
        |v| v.binding_digest[0] ^= 1,
        |v| v.original_custody.record_digest[0] ^= 1,
        |v| v.original_custody.control_state_digest[0] ^= 1,
        |v| v.signing_payload_digest[0] ^= 1,
        |v| v.signing_payload_size += 1,
    ];
    for mutate in mutations {
        let mut changed = request;
        mutate(&mut changed);
        let bytes = norito::encode_canonical(&changed).expect("changed raw request");
        let digest = changed
            .digest()
            .expect("raw claim digest is not authorization");
        assert_ne!(digest, original);
        assert_eq!(
            digest,
            oracle_digest(b"iroha.sorafs.signer.stream-token.request.v1", &[&bytes])
        );
    }
    assert_eq!(
        format!("{expected:?}"),
        "SignerStreamTokenExpectedV1 { .. }"
    );
    assert_eq!(format!("{request:?}"), "SignerStreamTokenRequestV1 { .. }");
    for error in [
        SignerStreamTokenReceiptErrorV1::InvalidReceipt,
        SignerStreamTokenReceiptErrorV1::WrongPurpose,
        SignerStreamTokenReceiptErrorV1::TokenMismatch,
        SignerStreamTokenReceiptErrorV1::InvalidTime,
    ] {
        let text = format!("{error:?}: {error}");
        for secret in [
            &body.token_id,
            &body.profile_handle,
            &binding.key_handle,
            &binding.runtime_handle,
        ] {
            assert!(!text.contains(secret));
        }
    }
}
