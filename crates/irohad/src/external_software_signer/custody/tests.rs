//! Adversarial tests of independent hardware-custody attestations, not hardware qualification.

use super::*;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::block::BlockHeader;

struct Fixture {
    signer: KeyPair,
    attester: KeyPair,
    statement: HardwareSignerCustodyStatementV1,
    trust: HardwareSignerCustodyTrustV1,
    context: HardwareSignerCustodyVerificationContextV1,
}

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test Ed25519 key")
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([seed; 32]),
    ))
}

fn fixture() -> Fixture {
    let signer = key(0x21);
    let attester = key(0x31);
    let authority = HardwareSignerCustodyAuthorityV1 {
        service_id: "custody-authority-primary".into(),
        administrator_id: "custody-security-primary".into(),
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x41; 32],
    };
    let anchor = HardwareSignerCustodyAnchorV1 {
        height: 90,
        block_hash: [0x43; 32],
        state_digest: [0x45; 32],
    };
    let statement = HardwareSignerCustodyStatementV1 {
        magic: HARDWARE_SIGNER_CUSTODY_MAGIC_V1,
        version: HARDWARE_SIGNER_CUSTODY_VERSION_V1,
        binding: HardwareSignerCustodyBindingV1 {
            chain_id: "sorafs-reference".parse().expect("canonical chain id"),
            network_id: network(0x11),
            runtime_handle: "hsm://sorafs/promotion/primary".into(),
            key_handle: "pkcs11:production/promotion/key-7".into(),
            service_id: "promotion-primary".into(),
            administrator_id: "promotion-security-primary".into(),
            role: SoftwareSignerRoleV1::Promotion,
            purpose: SoftwareSignerPurposeBindingV1::NativeOrPromotion,
            algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
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
    let trust = HardwareSignerCustodyTrustV1 {
        authority,
        public_key: attester.public_key().clone(),
        active_from_unix_ms: 900,
        active_until_unix_ms: 3_000,
        max_validity_ms: 1_000,
        max_anchor_age_ms: 100,
    };
    let context = HardwareSignerCustodyVerificationContextV1 {
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

fn attest_unchecked(statement: HardwareSignerCustodyStatementV1, attester: &KeyPair) -> Vec<u8> {
    let mut bytes = HARDWARE_SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1.to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(&statement).expect("encode test statement"));
    let signature = Signature::try_new(attester.private_key(), &bytes).expect("test attestation");
    norito::encode_canonical(&HardwareSignerCustodyRecordV1 {
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
) -> Result<VerifiedSignerCustodyV1, HardwareSignerCustodyErrorV1> {
    verify_hardware_signer_custody_v1(
        bytes,
        &fixture.statement.binding,
        &fixture.trust,
        &fixture.context,
    )
}

fn assert_error(bytes: &[u8], fixture: &Fixture, expected: HardwareSignerCustodyErrorV1) {
    assert_eq!(
        verify(bytes, fixture).expect_err("must reject unqualified custody"),
        expected
    );
}

#[test]
fn canonical_attestation_is_byte_identical_and_has_private_verified_observation() {
    let fixture = fixture();
    let first = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let second = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    assert_eq!(first, second);
    let record: HardwareSignerCustodyRecordV1 =
        norito::decode_canonical(&first).expect("canonical record");
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
    assert!(first.len() < HARDWARE_SIGNER_CUSTODY_MAX_BYTES_V1);
}

#[test]
fn tampering_and_wrong_attestation_key_fail_even_when_the_record_is_canonical() {
    let fixture = fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let mut record: HardwareSignerCustodyRecordV1 =
        norito::decode_canonical(&bytes).expect("record");
    record.statement.evidence_digest[0] ^= 1;
    assert_error(
        &norito::encode_canonical(&record).expect("tampered canonical record"),
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidAttestation,
    );
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &key(0x71)),
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidAttestation,
    );
    record.statement = fixture.statement.clone();
    record.attestation = [0; 64];
    assert_error(
        &norito::encode_canonical(&record).expect("zero signature record"),
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidAttestation,
    );
}

#[test]
fn an_independently_signed_substituted_binding_never_qualifies() {
    let fixture = fixture();
    let mutations: &[fn(&mut HardwareSignerCustodyBindingV1)] = &[
        |binding| binding.chain_id = "another-chain".parse().expect("chain"),
        |binding| binding.network_id = network(0x17),
        |binding| binding.role = SoftwareSignerRoleV1::Repair,
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
            HardwareSignerCustodyErrorV1::BindingMismatch,
        );
    }
}

#[test]
fn wrong_role_purpose_algorithm_or_key_shape_is_rejected_before_attestation() {
    let fixture = fixture();
    let mutations: &[fn(&mut HardwareSignerCustodyBindingV1)] = &[
        |binding| binding.purpose = SoftwareSignerPurposeBindingV1::EvidenceViewer,
        |binding| binding.algorithm = SoftwareSignerKeyAlgorithmV1::MlDsa,
        |binding| binding.role = SoftwareSignerRoleV1::PotrProvider,
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
            HardwareSignerCustodyErrorV1::InvalidRecord,
        );
    }
}

#[test]
fn software_imported_exportable_and_previously_exported_keys_cannot_qualify() {
    let fixture = fixture();
    let mutations: [fn(&mut HardwareSignerCustodyStatementV1); 3] = [
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
            HardwareSignerCustodyErrorV1::HardwareCustodyRequired
        );
        assert_error(
            &attest_unchecked(statement, &fixture.attester),
            &fixture,
            HardwareSignerCustodyErrorV1::HardwareCustodyRequired,
        );
    }
}

#[test]
fn authority_identity_policy_and_public_key_are_independently_pinned() {
    let fixture = fixture();
    let mutations: &[fn(&mut HardwareSignerCustodyAuthorityV1)] = &[
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
            HardwareSignerCustodyErrorV1::UntrustedAuthority,
        );
    }
}

#[test]
fn matching_software_key_self_attestation_is_rejected_even_if_trust_is_misconfigured() {
    let mut fixture = fixture();
    fixture.trust.public_key = fixture.signer.public_key().clone();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.signer);
    assert_error(
        &bytes,
        &fixture,
        HardwareSignerCustodyErrorV1::SelfAttestation,
    );
    fixture.trust.public_key = fixture.attester.public_key().clone();
    for identity in [
        &fixture.statement.binding.service_id,
        &fixture.statement.binding.administrator_id,
    ] {
        let mut fixture = self::fixture();
        fixture.statement.authority.administrator_id = identity.clone();
        fixture.trust.authority = fixture.statement.authority.clone();
        assert_error(
            &attest_unchecked(fixture.statement.clone(), &fixture.attester),
            &fixture,
            HardwareSignerCustodyErrorV1::SelfAttestation,
        );
    }
}

#[test]
fn exact_finalized_anchor_is_required_and_self_selected_forks_fail() {
    let fixture = fixture();
    let mutations: [fn(&mut HardwareSignerCustodyAnchorV1); 3] = [
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
            HardwareSignerCustodyErrorV1::AnchorMismatch,
        );
    }
}

#[test]
fn predecessor_advancement_accepts_one_successor_and_rejects_replay_or_substitution() {
    let mut fixture = fixture();
    let first = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let first_digest = verify(&first, &fixture)
        .expect("first observation")
        .record_digest();
    fixture.context.next_sequence = 2;
    fixture.context.predecessor_digest = first_digest;
    assert_error(
        &first,
        &fixture,
        HardwareSignerCustodyErrorV1::ReplayOrRollback,
    );
    fixture.statement.sequence = 2;
    fixture.statement.predecessor_digest = first_digest;
    let second = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    verify(&second, &fixture).expect("exact successor");
    fixture.statement.predecessor_digest[0] ^= 1;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        HardwareSignerCustodyErrorV1::ReplayOrRollback,
    );
    fixture.statement.sequence = 3;
    fixture.statement.predecessor_digest = first_digest;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        HardwareSignerCustodyErrorV1::ReplayOrRollback,
    );
}

#[test]
fn signed_or_current_signer_and_attester_revocations_all_fail_closed() {
    let mut fixture = fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    fixture.context.signer_revoked = true;
    assert_error(&bytes, &fixture, HardwareSignerCustodyErrorV1::Revoked);
    fixture.context.signer_revoked = false;
    fixture.context.attester_revoked = true;
    assert_error(&bytes, &fixture, HardwareSignerCustodyErrorV1::Revoked);
    fixture.context.attester_revoked = false;
    fixture.statement.revoked = true;
    assert_error(
        &attest_unchecked(fixture.statement.clone(), &fixture.attester),
        &fixture,
        HardwareSignerCustodyErrorV1::Revoked,
    );
}

#[test]
fn explicit_time_enforces_exclusive_expiry_future_and_current_anchor_freshness() {
    let fixture = fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    for (now, observed) in [
        (999, 950),
        (2_000, 1_950),
        (1_500, 1_399),
        (1_500, 1_501),
        (0, 0),
        (1_500, 0),
    ] {
        let context = HardwareSignerCustodyVerificationContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: observed,
            ..fixture.context
        };
        assert_eq!(
            verify_hardware_signer_custody_v1(
                &bytes,
                &fixture.statement.binding,
                &fixture.trust,
                &context
            )
            .expect_err("time must fail"),
            HardwareSignerCustodyErrorV1::Freshness
        );
    }
    for now in [1_000, 1_999] {
        let context = HardwareSignerCustodyVerificationContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: now - 100,
            ..fixture.context
        };
        verify_hardware_signer_custody_v1(
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
    let fixture = fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let mutations: [fn(&mut HardwareSignerCustodyTrustV1); 3] = [
        |trust| trust.max_validity_ms = 0,
        |trust| trust.max_anchor_age_ms = MAX_VALIDITY_MS_V1 + 1,
        |trust| trust.active_from_unix_ms = trust.active_until_unix_ms,
    ];
    for mutate in mutations {
        let mut trust = fixture.trust.clone();
        mutate(&mut trust);
        assert_eq!(
            verify_hardware_signer_custody_v1(
                &bytes,
                &fixture.statement.binding,
                &trust,
                &fixture.context
            )
            .expect_err("invalid trust"),
            HardwareSignerCustodyErrorV1::UntrustedAuthority
        );
    }
    let mutations: [fn(&mut HardwareSignerCustodyTrustV1); 3] = [
        |trust| trust.max_validity_ms = 999,
        |trust| trust.active_from_unix_ms = 1_001,
        |trust| trust.active_until_unix_ms = 1_999,
    ];
    for mutate in mutations {
        let mut trust = fixture.trust.clone();
        mutate(&mut trust);
        assert_eq!(
            verify_hardware_signer_custody_v1(
                &bytes,
                &fixture.statement.binding,
                &trust,
                &fixture.context
            )
            .expect_err("ineligible interval"),
            HardwareSignerCustodyErrorV1::Freshness
        );
    }
}

#[test]
fn canonical_bounds_reject_truncated_trailing_oversized_and_unknown_version_records() {
    let fixture = fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    assert_error(&[], &fixture, HardwareSignerCustodyErrorV1::InvalidRecord);
    assert_error(
        &bytes[..bytes.len() - 1],
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidRecord,
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_error(
        &trailing,
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidRecord,
    );
    assert_error(
        &vec![0; HARDWARE_SIGNER_CUSTODY_MAX_BYTES_V1 + 1],
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidRecord,
    );
    let mut statement = fixture.statement.clone();
    statement.version += 1;
    assert_error(
        &attest_unchecked(statement, &fixture.attester),
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidRecord,
    );
    let mut statement = fixture.statement.clone();
    statement.binding.runtime_handle = "hsm:".to_owned() + &"a".repeat(4096);
    assert_error(
        &attest_unchecked(statement, &fixture.attester),
        &fixture,
        HardwareSignerCustodyErrorV1::InvalidRecord,
    );
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
    ] {
        assert!(!valid_hardware_handle(rejected));
    }
}

#[test]
fn debug_and_errors_never_echo_handles_identity_strings_or_attestation_bytes() {
    let mut fixture = fixture();
    let sentinel = "SECRET-CUSTODY-PIN-DO-NOT-LOG";
    fixture.statement.binding.key_handle = format!("pkcs11:{sentinel}");
    fixture.statement.binding.service_id = sentinel.into();
    fixture.statement.authority.service_id = "SECRET-AUTHORITY-ID".into();
    fixture.trust.authority = fixture.statement.authority.clone();
    let record = HardwareSignerCustodyRecordV1 {
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
        HardwareSignerCustodyErrorV1::InvalidAttestation.to_string(),
        "hardware custody attestation is invalid"
    );
}
