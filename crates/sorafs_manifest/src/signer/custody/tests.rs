//! Adversarial tests of independent hardware-custody attestations, not hardware qualification.

mod active_use;

use super::*;
use iroha_crypto::KeyPair;

struct Fixture {
    signer: KeyPair,
    attester: KeyPair,
    statement: SignerCustodyStatementV1,
    trust: SignerCustodyTrustV1,
    context: SignerCustodyEnrollmentContextV1,
}

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test Ed25519 key")
}

fn network(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn custody_fixture() -> Fixture {
    let signer = key(0x21);
    let attester = key(0x31);
    let authority = SignerCustodyAuthorityV1 {
        service_id: "custody-authority-primary".into(),
        administrator_id: "custody-security-primary".into(),
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x41; 32],
    };
    let anchor = SignerCustodyAnchorV1 {
        height: 90,
        block_hash: [0x43; 32],
        state_digest: [0x45; 32],
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: SignerCustodyBindingV1 {
            chain_id: "sorafs-reference".parse().expect("canonical chain id"),
            network_id: network(0x11),
            runtime_handle: "hsm://sorafs/promotion/primary".into(),
            key_handle: "pkcs11:production/promotion/key-7".into(),
            service_id: "promotion-primary".into(),
            administrator_id: "promotion-security-primary".into(),
            role: SignerRoleV1::Promotion,
            purpose: SignerPurposeBindingV1::NativeOrPromotion,
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: signer.public_key().clone(),
            key_revision: 7,
            policy_revision: 9,
            policy_digest: [0x51; 32],
        },
        authority: authority.clone(),
        anchor,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
        hardware_identity_digest: [0x53; 32],
        evidence_digest: [0x55; 32],
        generated_in_hardware: true,
        exportable: false,
        ever_exported: false,
        revoked: false,
    };
    let trust = SignerCustodyTrustV1 {
        authority,
        public_key: attester.public_key().clone(),
        active_from_unix_ms: 900,
        active_until_unix_ms: 3_000,
        max_validity_ms: 1_000,
        max_anchor_age_ms: 100,
    };
    let context = SignerCustodyEnrollmentContextV1 {
        now_unix_ms: 1_500,
        anchor_observed_at_unix_ms: 1_450,
        current_anchor: anchor,
        next_sequence: 1,
        predecessor_digest: [0; 32],
        signer_revoked: false,
        attester_revoked: false,
    };
    Fixture {
        signer,
        attester,
        statement,
        trust,
        context,
    }
}

fn attest_unchecked(statement: SignerCustodyStatementV1, attester: &KeyPair) -> Vec<u8> {
    let mut bytes = SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1.to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(&statement).expect("encode test statement"));
    let signature = Signature::try_new(attester.private_key(), &bytes).expect("test attestation");
    norito::encode_canonical(&SignerCustodyRecordV1 {
        statement,
        attestation: signature
            .payload()
            .try_into()
            .expect("Ed25519 signature size"),
    })
    .expect("encode test custody record")
}

fn verify(
    bytes: &[u8],
    fixture: &Fixture,
) -> Result<VerifiedSignerCustodyEnrollmentV1, SignerCustodyErrorV1> {
    verify_signer_custody_enrollment_v1(
        bytes,
        &fixture.statement.binding,
        &fixture.trust,
        &fixture.context,
    )
}

fn assert_error(bytes: &[u8], fixture: &Fixture, expected: SignerCustodyErrorV1) {
    assert_eq!(
        verify(bytes, fixture).expect_err("must reject unqualified custody"),
        expected
    );
}

#[test]
fn canonical_attestation_is_byte_identical_and_has_private_verified_observation() {
    let fixture = custody_fixture();
    let first = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let second = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    assert_eq!(first, second);
    let record: SignerCustodyRecordV1 = norito::decode_canonical(&first).expect("canonical record");
    assert_eq!(
        norito::encode_canonical(&record).expect("canonical re-encode"),
        first
    );
    Signature::try_from_bytes(&record.attestation)
        .expect("attestation signature")
        .verify(
            fixture.attester.public_key(),
            &record.statement.signing_payload().expect("signing bytes"),
        )
        .expect("same exact signing bytes");
    let verified =
        verify(&first, &fixture).expect("independent test authority verifies fixture only");
    assert_eq!(verified.statement(), &fixture.statement);
    assert_eq!(
        verified.record_digest(),
        digest_parts(CUSTODY_RECORD_DIGEST_DOMAIN_V1, &[&first])
    );
    assert_eq!(verified.verified_at_unix_ms(), fixture.context.now_unix_ms);
    assert!(first.len() < SIGNER_CUSTODY_MAX_BYTES_V1);
}

#[test]
fn ambient_layout_cannot_change_signing_bytes_or_admit_alternate_wire_layouts() {
    let fixture = custody_fixture();
    let canonical = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let record: SignerCustodyRecordV1 = norito::decode_canonical(&canonical).expect("record");
    let payload = fixture.statement.signing_payload().expect("signing bytes");
    let alternate = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(
            norito::core::header_flags::PACKED_SEQ | norito::core::header_flags::COMPACT_LEN,
        );
        assert_eq!(
            fixture.statement.signing_payload().expect("fixed layout"),
            payload
        );
        assert_eq!(
            attest_unchecked(fixture.statement.clone(), &fixture.attester),
            canonical
        );
        norito::core::to_bytes(&record).expect("alternate noncanonical record")
    };
    assert_ne!(alternate, canonical);
    assert_error(&alternate, &fixture, SignerCustodyErrorV1::InvalidRecord);
}

#[test]
fn independent_attestation_binds_ml_dsa_provider_key_and_exact_provider_identity() {
    let mut fixture = custody_fixture();
    fixture.signer =
        KeyPair::try_from_seed(vec![0x29; 32], Algorithm::MlDsa).expect("test ML-DSA-65 key");
    fixture.statement.binding.role = SignerRoleV1::PotrProvider;
    fixture.statement.binding.purpose = SignerPurposeBindingV1::PotrProvider {
        signer_id: [0x61; 32],
        provider_id: [0x63; 32],
    };
    fixture.statement.binding.algorithm = SignerKeyAlgorithmV1::MlDsa;
    fixture.statement.binding.public_key = fixture.signer.public_key().clone();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    verify(&bytes, &fixture).expect("independent Ed25519 authority attests ML-DSA subject");
    let mut statement = fixture.statement.clone();
    statement.binding.purpose = SignerPurposeBindingV1::PotrProvider {
        signer_id: [0x61; 32],
        provider_id: [0x67; 32],
    };
    assert_error(
        &attest_unchecked(statement, &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::BindingMismatch,
    );
}

#[test]
fn tampering_and_wrong_attestation_key_fail_even_when_the_record_is_canonical() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let mut record: SignerCustodyRecordV1 = norito::decode_canonical(&bytes).expect("record");
    record.statement.evidence_digest[0] ^= 1;
    assert_error(
        &norito::encode_canonical(&record).expect("tampered canonical record"),
        &fixture,
        SignerCustodyErrorV1::InvalidAttestation,
    );
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &key(0x71)),
        &fixture,
        SignerCustodyErrorV1::InvalidAttestation,
    );
    record.statement = fixture.statement.clone();
    record.attestation = [0; 64];
    assert_error(
        &norito::encode_canonical(&record).expect("zero signature record"),
        &fixture,
        SignerCustodyErrorV1::InvalidAttestation,
    );
}

#[test]
fn an_independently_signed_substituted_binding_never_qualifies() {
    let fixture = custody_fixture();
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |binding| binding.chain_id = "another-chain".parse().expect("chain"),
        |binding| binding.network_id = network(0x17),
        |binding| binding.role = SignerRoleV1::Repair,
        |binding| binding.public_key = key(0x19).public_key().clone(),
        |binding| binding.runtime_handle = "hsm://sorafs/promotion/secondary".into(),
        |binding| binding.key_handle = "pkcs11:production/promotion/key-8".into(),
        |binding| binding.service_id = "promotion-secondary".into(),
        |binding| binding.administrator_id = "promotion-security-secondary".into(),
        |binding| binding.key_revision += 1,
        |binding| binding.policy_revision += 1,
        |binding| binding.policy_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement.binding);
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::BindingMismatch,
        );
    }
}

#[test]
fn wrong_role_purpose_algorithm_or_key_shape_is_rejected_before_attestation() {
    let fixture = custody_fixture();
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |binding| binding.purpose = SignerPurposeBindingV1::EvidenceViewer,
        |binding| binding.algorithm = SignerKeyAlgorithmV1::MlDsa,
        |binding| binding.role = SignerRoleV1::PotrProvider,
        |binding| binding.key_revision = 0,
        |binding| binding.policy_revision = 0,
        |binding| binding.policy_digest = [0; 32],
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement.binding);
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::InvalidRecord,
        );
    }
}

#[test]
fn software_imported_exportable_and_previously_exported_keys_cannot_qualify() {
    let fixture = custody_fixture();
    let mutations: &[fn(&mut SignerCustodyStatementV1)] = &[
        |statement| statement.generated_in_hardware = false,
        |statement| statement.exportable = true,
        |statement| statement.ever_exported = true,
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement);
        assert_eq!(
            statement
                .signing_payload()
                .expect_err("invalid signing profile"),
            SignerCustodyErrorV1::HardwareCustodyRequired
        );
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::HardwareCustodyRequired,
        );
    }
}

#[test]
fn authority_identity_policy_and_public_key_are_independently_pinned() {
    let fixture = custody_fixture();
    let mutations: &[fn(&mut SignerCustodyAuthorityV1)] = &[
        |authority| authority.service_id = "custody-authority-secondary".into(),
        |authority| authority.administrator_id = "custody-security-secondary".into(),
        |authority| authority.key_revision += 1,
        |authority| authority.policy_revision += 1,
        |authority| authority.policy_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement.authority);
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::UntrustedAuthority,
        );
    }
}

#[test]
fn matching_software_key_self_attestation_is_rejected_even_if_trust_is_misconfigured() {
    let mut fixture = custody_fixture();
    fixture.trust.public_key = fixture.signer.public_key().clone();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.signer);
    assert_error(&bytes, &fixture, SignerCustodyErrorV1::SelfAttestation);
    fixture.trust.public_key = fixture.attester.public_key().clone();
    for identity in [
        &fixture.statement.binding.service_id,
        &fixture.statement.binding.administrator_id,
    ] {
        let mut fixture = custody_fixture();
        fixture.statement.authority.administrator_id = identity.clone();
        fixture.trust.authority = fixture.statement.authority.clone();
        assert_error(
            &attest_unchecked(fixture.statement.clone(), &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::SelfAttestation,
        );
    }
}

#[test]
fn exact_finalized_anchor_is_required_and_self_selected_forks_fail() {
    let fixture = custody_fixture();
    let mutations: &[fn(&mut SignerCustodyAnchorV1)] = &[
        |anchor| anchor.height += 1,
        |anchor| anchor.block_hash[0] ^= 1,
        |anchor| anchor.state_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement.anchor);
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::AnchorMismatch,
        );
    }
}

#[test]
fn predecessor_advancement_accepts_one_successor_and_rejects_replay_or_substitution() {
    let mut fixture = custody_fixture();
    let first = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let first_digest = verify(&first, &fixture)
        .expect("first observation")
        .record_digest();
    fixture.context.next_sequence = 2;
    fixture.context.predecessor_digest = first_digest;
    assert_error(&first, &fixture, SignerCustodyErrorV1::ReplayOrRollback);
    fixture.statement.sequence = 2;
    fixture.statement.predecessor_digest = first_digest;
    let second = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    verify(&second, &fixture).expect("exact successor");
    fixture.statement.predecessor_digest[0] ^= 1;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::ReplayOrRollback,
    );
    fixture.statement.sequence = 3;
    fixture.statement.predecessor_digest = first_digest;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::ReplayOrRollback,
    );
}

#[test]
fn signed_or_current_signer_and_attester_revocations_all_fail_closed() {
    let mut fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    fixture.context.signer_revoked = true;
    assert_error(&bytes, &fixture, SignerCustodyErrorV1::Revoked);
    fixture.context.signer_revoked = false;
    fixture.context.attester_revoked = true;
    assert_error(&bytes, &fixture, SignerCustodyErrorV1::Revoked);
    fixture.context.attester_revoked = false;
    fixture.statement.revoked = true;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::Revoked,
    );
}

#[test]
fn explicit_time_enforces_exclusive_expiry_future_and_current_anchor_freshness() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    for (now, observed) in [
        (999, 950),
        (2_000, 1_950),
        (1_500, 1_399),
        (1_500, 1_501),
        (0, 0),
        (1_500, 0),
    ] {
        let context = SignerCustodyEnrollmentContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: observed,
            ..fixture.context
        };
        assert_eq!(
            verify_signer_custody_enrollment_v1(
                &bytes,
                &fixture.statement.binding,
                &fixture.trust,
                &context
            )
            .expect_err("time must fail"),
            SignerCustodyErrorV1::Freshness
        );
    }
    for now in [1_000, 1_999] {
        let context = SignerCustodyEnrollmentContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: now - 100,
            ..fixture.context
        };
        verify_signer_custody_enrollment_v1(
            &bytes,
            &fixture.statement.binding,
            &fixture.trust,
            &context,
        )
        .expect("inclusive issuance and anchor-age bound");
    }
}

#[test]
fn invalid_trust_and_key_activation_intervals_cannot_extend_qualification() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let invalid_trust: &[fn(&mut SignerCustodyTrustV1)] = &[
        |trust| trust.max_validity_ms = 0,
        |trust| trust.max_anchor_age_ms = MAX_VALIDITY_MS_V1 + 1,
        |trust| trust.active_from_unix_ms = trust.active_until_unix_ms,
    ];
    for mutate in invalid_trust {
        let mut trust = fixture.trust.clone();
        mutate(&mut trust);
        assert_eq!(
            verify_signer_custody_enrollment_v1(
                &bytes,
                &fixture.statement.binding,
                &trust,
                &fixture.context
            )
            .expect_err("invalid trust"),
            SignerCustodyErrorV1::UntrustedAuthority
        );
    }
    let invalid_interval: &[fn(&mut SignerCustodyTrustV1)] = &[
        |trust| trust.max_validity_ms = 999,
        |trust| trust.active_from_unix_ms = 1_001,
        |trust| trust.active_until_unix_ms = 1_999,
    ];
    for mutate in invalid_interval {
        let mut trust = fixture.trust.clone();
        mutate(&mut trust);
        assert_eq!(
            verify_signer_custody_enrollment_v1(
                &bytes,
                &fixture.statement.binding,
                &trust,
                &fixture.context
            )
            .expect_err("ineligible interval"),
            SignerCustodyErrorV1::Freshness
        );
    }
}

#[test]
fn canonical_bounds_reject_truncated_trailing_oversized_and_unknown_version_records() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    assert_error(&[], &fixture, SignerCustodyErrorV1::InvalidRecord);
    assert_error(
        &bytes[..bytes.len() - 1],
        &fixture,
        SignerCustodyErrorV1::InvalidRecord,
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_error(&trailing, &fixture, SignerCustodyErrorV1::InvalidRecord);
    assert_error(
        &vec![0; SIGNER_CUSTODY_MAX_BYTES_V1 + 1],
        &fixture,
        SignerCustodyErrorV1::InvalidRecord,
    );
    let mut statement = fixture.statement.clone();
    statement.version += 1;
    assert_error(
        &attest_unchecked(statement, &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::InvalidRecord,
    );
    let mut statement = fixture.statement.clone();
    statement.binding.runtime_handle = "hsm:".to_owned() + &"a".repeat(4096);
    assert_error(
        &attest_unchecked(statement, &fixture.attester),
        &fixture,
        SignerCustodyErrorV1::InvalidRecord,
    );
    let mutations: &[fn(&mut SignerCustodyStatementV1)] = &[
        |statement| statement.magic[0] ^= 1,
        |statement| statement.binding.key_handle = format!("hsm:{}", "a".repeat(125)),
        |statement| statement.binding.service_id = "a".repeat(129),
        |statement| statement.anchor.height = 0,
        |statement| statement.anchor.state_digest = [0; 32],
        |statement| statement.hardware_identity_digest = [0; 32],
        |statement| statement.evidence_digest = [0; 32],
        |statement| statement.predecessor_digest = [0x69; 32],
        |statement| statement.sequence = 0,
    ];
    for mutate in mutations {
        let mut statement = fixture.statement.clone();
        mutate(&mut statement);
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            SignerCustodyErrorV1::InvalidRecord,
        );
    }
}

#[test]
fn validated_chain_and_key_types_cannot_be_bypassed_by_well_framed_wire_bytes() {
    let mut fixture = custody_fixture();
    for invalid in ["".to_owned(), "a".repeat(129), "chain with spaces".into()] {
        assert!(iroha_primitives::chain_id::validate_chain_id(&invalid).is_err());
    }
    fixture.statement.binding.chain_id = "a".repeat(128).parse().expect("maximum chain id");
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    verify(&bytes, &fixture).expect("exact maximum-size chain identity");
    let record: SignerCustodyRecordV1 = norito::decode_canonical(&bytes).expect("record");
    let _layout = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload = record.encode();
    let chain_bytes = fixture.statement.binding.chain_id.as_str().as_bytes();
    let chain_offset = payload
        .windows(chain_bytes.len())
        .position(|window| window == chain_bytes)
        .expect("chain field inside payload");
    let mut invalid_chain = payload.clone();
    invalid_chain[chain_offset] = b' ';
    let key_bytes = fixture.statement.binding.public_key.encode();
    let key_offset = payload
        .windows(key_bytes.len())
        .position(|window| window == key_bytes)
        .expect("public key inside payload");
    let invalid_key = iroha_primitives::const_vec::ConstVec::from(vec![u8::MAX; 33]).encode();
    assert_eq!(invalid_key.len(), key_bytes.len());
    let mut invalid_key_payload = payload.clone();
    invalid_key_payload[key_offset..key_offset + key_bytes.len()].copy_from_slice(&invalid_key);
    let mut short_signature = payload;
    short_signature.pop();
    for malformed in [invalid_chain, invalid_key_payload, short_signature] {
        let framed = norito::core::frame_bare_with_header_flags::<SignerCustodyRecordV1>(
            &malformed,
            norito::core::default_encode_flags(),
        )
        .expect("authentic frame with deliberately malformed inner field");
        assert_error(&framed, &fixture, SignerCustodyErrorV1::InvalidRecord);
    }
}

#[test]
fn hardware_handles_reject_software_credentials_aliases_and_test_markers() {
    for accepted in [
        "hsm://sorafs/promotion/primary",
        "kms:production/key-1",
        "pkcs11:production/key-2",
    ] {
        assert!(valid_hardware_handle(accepted));
    }
    for rejected in [
        "software://sorafs/promotion/primary",
        "file:private-key",
        "hsm://user:secret@host",
        "pkcs11:token=production;pin-value=secret",
        "hsm:production/test",
        "kms:production/key%31",
        "hsm:production/key?token=secret",
        " hsm:production/key",
        "HSM:production/key",
        "hsm:",
        "hsm://",
        "hsm:///key",
        "hsm:production//key",
        "hsm:production/../key",
        "hsm:production/key:credential",
        "hsm:production/key/",
    ] {
        assert!(!valid_hardware_handle(rejected));
    }
}

#[test]
fn debug_and_errors_never_echo_handles_identity_strings_or_attestation_bytes() {
    let mut fixture = custody_fixture();
    let sentinel = "SECRET-CUSTODY-PIN-DO-NOT-LOG";
    fixture.statement.binding.key_handle = format!("pkcs11:{sentinel}");
    fixture.statement.binding.service_id = sentinel.into();
    fixture.statement.authority.service_id = "SECRET-AUTHORITY-ID".into();
    fixture.trust.authority = fixture.statement.authority.clone();
    let record = SignerCustodyRecordV1 {
        statement: fixture.statement.clone(),
        attestation: [0xAB; 64],
    };
    let formatted = format!(
        "{:?} {:?} {:?} {:?}",
        fixture.statement.binding, fixture.statement.authority, record, fixture.trust
    );
    assert!(!formatted.contains(sentinel));
    assert!(!formatted.contains("SECRET-AUTHORITY-ID"));
    assert!(!formatted.contains("171, 171"));
    let valid = verify(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
    )
    .expect("signed redaction fixture");
    assert!(!format!("{valid:?}").contains(sentinel));
    assert_eq!(
        SignerCustodyErrorV1::InvalidAttestation.to_string(),
        "hardware custody attestation is invalid"
    );
}
