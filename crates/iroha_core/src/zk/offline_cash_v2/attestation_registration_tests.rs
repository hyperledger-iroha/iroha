//! Behavioral and static tests for the private native V2 registration hard cut.

use core::mem::size_of;

use iroha_data_model::offline::OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1;
use rcgen::{
    BasicConstraints, CertificateParams, CustomExtension, DistinguishedName, DnType, IsCa, Issuer,
    KeyPair, KeyUsagePurpose, PublicKeyData,
};
use x509_parser::prelude::{FromDer as _, X509Certificate};

use crate::zk::offline_cash_v1::{
    OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
    offline_cash_halo2_protocol_identity_v1,
};

use super::attestation_registration::{
    AuthenticatedRegistrationLeafV2, GovernedHostRegistrationAuthorityV2,
    GovernedOfflineInclusionAuthorityV2, NATIVE_REGISTRATION_ACTIVATION_BLOCKERS_V2,
    NATIVE_REGISTRATION_ACTIVATION_READY_V2, NATIVE_REGISTRATION_ARTIFACT_EVIDENCE_AVAILABLE_V2,
    NATIVE_REGISTRATION_CANONICAL_DECODER_AVAILABLE_V2, NATIVE_REGISTRATION_DECLARED_V2,
    NATIVE_REGISTRATION_EVIDENCE_CONTRACT_V2, NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2,
    NATIVE_REGISTRATION_FIXED_MESSAGE_BYTES_V2,
    NATIVE_REGISTRATION_FRESHNESS_PROJECTION_AVAILABLE_V2,
    NATIVE_REGISTRATION_HASH_FRAME_CONTRACT_V2, NATIVE_REGISTRATION_HELPER_WORDS_V2,
    NATIVE_REGISTRATION_KEYMINT_ADAPTER_AVAILABLE_V2,
    NATIVE_REGISTRATION_MAX_ATTESTATION_EXTENSION_BYTES_V2,
    NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2, NATIVE_REGISTRATION_MINIMUM_ADAPTER_DELTA_V2,
    NATIVE_REGISTRATION_OFFLINE_ENVELOPE_MAX_BYTES_V2,
    NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2,
    NATIVE_REGISTRATION_PERSISTENCE_AVAILABLE_V2, NATIVE_REGISTRATION_PRODUCTION_AVAILABLE_V2,
    NATIVE_REGISTRATION_PROJECTION_IDENTITY_BINDING_AVAILABLE_V2,
    NATIVE_REGISTRATION_RAW_DIGEST_CONTRACT_V2, NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2,
    NATIVE_REGISTRATION_RECEIPT_CONTRACT_V2, NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2,
    NATIVE_REGISTRATION_RELEASE_ELIGIBLE_V2,
    NATIVE_REGISTRATION_REVOCATION_CAP_ADAPTER_EVIDENCE_AVAILABLE_V2,
    NATIVE_REGISTRATION_ROOT_PROVIDER_AVAILABLE_V2, NATIVE_REGISTRATION_SOURCE_IMPLEMENTED_V2,
    NATIVE_REGISTRATION_TERMINAL_ADAPTER_AVAILABLE_V2, NATIVE_REGISTRY_ALLOCATION_CONTRACT_V2,
    NATIVE_REGISTRY_CHECKPOINT_ANTI_ROLLBACK_AVAILABLE_V2, NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2,
    NATIVE_REGISTRY_DEPTH_V2, NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2,
    NativeAttestationRegistrationInputV2, NativeRegistrationErrorV2, OfflineRegistrationReceiptV2,
    RegistryInclusionProofV2, TestHostRegistrationAuthorityV2, TestOfflineInclusionAuthorityV2,
    duplicate_input_for_test_v2, governed_policy_for_test_v2, issuer_key_id_for_test_v2,
    mutate_checkpoint_commitment_for_test_v2, mutate_receipt_commitment_for_test_v2,
    native_fixed_messages_v2, native_registration_source_helper_protocol_digest_v1,
    registry_leaf_index_for_test_v2, registry_root_for_test_v2, replace_revocations_for_test_v2,
    revoke_certificate_for_test_v2, set_policy_evaluation_time_for_test_v2, sha256_for_test_v2,
    unordered_governed_issuers_for_test_v2, verify_offline_registration_receipt_v2,
    write_digest_words_for_test_v2,
};

const POLICY_EPOCH: u64 = 17;
const POLICY_DIGEST: [u8; 32] = [0x31; 32];
const REGISTRY_INTENT: [u8; 32] = [0x52; 32];
const EVALUATION_UNIX_SECONDS: u64 = 1_800_000_000;
const STANDARD_ATTESTATION_DER: &[u8] = &[0x30, 0x03, 0x02, 0x01, 0x01];
const PROTOCOL_WORD_START: usize = 16;

const DIGEST_OFFSETS: [usize; 20] = [
    24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152, 160, 168, 176,
];

struct FixtureParts {
    words: [u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
    device_key: [u8; 65],
    device_key_id: [u8; 32],
    certificate_der: Vec<u8>,
    chain_der: Vec<Vec<u8>>,
    attestation_der: Vec<u8>,
}

#[derive(Clone, Copy)]
struct FixtureExpected {
    device_key: [u8; 65],
    device_key_id: [u8; 32],
    certificate_digest: [u8; 32],
    tbs_digest: [u8; 32],
    issuer_key_digest: [u8; 32],
    attestation_digest: [u8; 32],
}

fn certificate_material(
    attestation_der: &[u8],
    include_attestation_extension: bool,
) -> (Vec<u8>, Vec<u8>, [u8; 65], [u8; 65]) {
    certificate_material_with_root_profile(attestation_der, include_attestation_extension, true)
}

fn certificate_material_with_root_profile(
    attestation_der: &[u8],
    include_attestation_extension: bool,
    root_is_ca: bool,
) -> (Vec<u8>, Vec<u8>, [u8; 65], [u8; 65]) {
    let root_key = KeyPair::generate().expect("P-256 root key generation");
    let root_public_key: [u8; 65] = root_key
        .der_bytes()
        .try_into()
        .expect("rcgen P-256 key is uncompressed SEC1");
    let mut root_params =
        CertificateParams::new(Vec::<String>::new()).expect("empty subjectAltName set is valid");
    root_params.distinguished_name = DistinguishedName::new();
    root_params
        .distinguished_name
        .push(DnType::CommonName, "Offline Cash V2 Test Root");
    root_params.is_ca = if root_is_ca {
        IsCa::Ca(BasicConstraints::Unconstrained)
    } else {
        IsCa::NoCa
    };
    root_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let root_certificate = root_params
        .self_signed(&root_key)
        .expect("self-signed test root");
    let root_der = root_certificate.der().as_ref().to_vec();
    let issuer = Issuer::new(root_params, root_key);

    let leaf_key = KeyPair::generate().expect("P-256 leaf key generation");
    let leaf_public_key: [u8; 65] = leaf_key
        .der_bytes()
        .try_into()
        .expect("rcgen P-256 key is uncompressed SEC1");
    let mut leaf_params =
        CertificateParams::new(Vec::<String>::new()).expect("empty subjectAltName set is valid");
    leaf_params.distinguished_name = DistinguishedName::new();
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, "Offline Cash V2 Test Device");
    leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    if include_attestation_extension {
        leaf_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                &[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17],
                attestation_der.to_vec(),
            ));
    } else {
        // Keep an extensions wrapper so rcgen emits the requested KeyUsage while deliberately
        // omitting only the Android Key Attestation OID.
        leaf_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                &[1, 3, 6, 1, 4, 1, 55555, 1],
                vec![0x05, 0x00],
            ));
    }
    let leaf_certificate = leaf_params
        .signed_by(&leaf_key, &issuer)
        .expect("issuer-signed test leaf");
    (
        leaf_certificate.der().as_ref().to_vec(),
        root_der,
        leaf_public_key,
        root_public_key,
    )
}

fn helper_words(
    certificate_der: &[u8],
    attestation_der: &[u8],
    device_key: &[u8; 65],
    issuer_key: &[u8; 65],
) -> [u32; NATIVE_REGISTRATION_HELPER_WORDS_V2] {
    let (_, certificate) =
        X509Certificate::from_der(certificate_der).expect("fixture certificate parses");
    let mut words = [1_u32; NATIVE_REGISTRATION_HELPER_WORDS_V2];
    words[0] = 1;
    words[1] = 1;
    words[2] = 16;
    words[3] = 1;
    words[4] = 2;
    words[5] = 1;
    words[6] = 1;
    words[7] = 8;
    words[8] = 41;
    words[9] = 0;
    words[10] = 42;
    words[11] = 0;
    words[12] = 21;
    words[13] = 7;
    words[14] = 27;
    words[15] = 0;
    let protocol = native_registration_source_helper_protocol_digest_v1(words[3], words[4])
        .expect("fixture selects an exact supported V1 helper identity");
    write_digest_words_for_test_v2(&mut words, PROTOCOL_WORD_START, protocol);
    for (index, offset) in DIGEST_OFFSETS.into_iter().enumerate() {
        write_digest_words_for_test_v2(
            &mut words,
            offset,
            [u8::try_from(index + 1).expect("small fixture index"); 32],
        );
    }
    write_digest_words_for_test_v2(&mut words, 72, POLICY_DIGEST);
    write_digest_words_for_test_v2(&mut words, 104, sha256_for_test_v2(device_key));
    write_digest_words_for_test_v2(&mut words, 136, sha256_for_test_v2(certificate_der));
    write_digest_words_for_test_v2(
        &mut words,
        144,
        sha256_for_test_v2(certificate.tbs_certificate.as_ref()),
    );
    write_digest_words_for_test_v2(&mut words, 152, sha256_for_test_v2(issuer_key));
    write_digest_words_for_test_v2(&mut words, 160, sha256_for_test_v2(attestation_der));
    for (job, offset) in NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2
        .into_iter()
        .enumerate()
    {
        let messages = native_fixed_messages_v2(&words, device_key, issuer_key)
            .expect("intermediate fixed batch framing");
        write_digest_words_for_test_v2(&mut words, offset, sha256_for_test_v2(&messages[job]));
    }
    words
}

fn fixture_with(
    attestation_der: Vec<u8>,
    include_attestation_extension: bool,
    mutate: impl FnOnce(&mut FixtureParts),
) -> (
    NativeAttestationRegistrationInputV2,
    super::attestation_registration::GovernedRegistrationPolicyV2,
    FixtureExpected,
) {
    let (leaf_der, root_der, device_key, issuer_key) =
        certificate_material(&attestation_der, include_attestation_extension);
    let words = helper_words(&leaf_der, &attestation_der, &device_key, &issuer_key);
    let device_key_id = sha256_for_test_v2(&device_key);
    let mut parts = FixtureParts {
        words,
        device_key,
        device_key_id,
        certificate_der: leaf_der.clone(),
        chain_der: vec![leaf_der, root_der.clone()],
        attestation_der,
    };
    mutate(&mut parts);
    let (_, parsed_leaf) = X509Certificate::from_der(&parts.chain_der[0])
        .expect("unmutated fixture leaf is parseable");
    let expected = FixtureExpected {
        device_key: parts.device_key,
        device_key_id: parts.device_key_id,
        certificate_digest: sha256_for_test_v2(&parts.certificate_der),
        tbs_digest: sha256_for_test_v2(parsed_leaf.tbs_certificate.as_ref()),
        issuer_key_digest: sha256_for_test_v2(&issuer_key),
        attestation_digest: sha256_for_test_v2(&parts.attestation_der),
    };
    let input = NativeAttestationRegistrationInputV2::new(
        parts.words,
        parts.device_key,
        parts.device_key_id,
        POLICY_EPOCH,
        REGISTRY_INTENT,
        parts.certificate_der,
        parts.chain_der,
        parts.attestation_der,
    )
    .expect("fixture input passes public bounds");
    let issuer_id = issuer_key_id_for_test_v2(&issuer_key);
    let policy = governed_policy_for_test_v2(
        POLICY_EPOCH,
        POLICY_DIGEST,
        REGISTRY_INTENT,
        EVALUATION_UNIX_SECONDS,
        vec![root_der],
        Vec::new(),
        vec![issuer_id, [0xF4; 32]],
    );
    (input, policy, expected)
}

fn fixture() -> (
    NativeAttestationRegistrationInputV2,
    super::attestation_registration::GovernedRegistrationPolicyV2,
    FixtureExpected,
) {
    fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |_| {})
}

#[test]
fn source_is_privately_declared_uninhabited_and_all_release_gates_are_false() {
    assert!(NATIVE_REGISTRATION_SOURCE_IMPLEMENTED_V2);
    assert!(NATIVE_REGISTRATION_DECLARED_V2);
    assert!(!NATIVE_REGISTRATION_KEYMINT_ADAPTER_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_ROOT_PROVIDER_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_PERSISTENCE_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_TERMINAL_ADAPTER_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_FRESHNESS_PROJECTION_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_PROJECTION_IDENTITY_BINDING_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRY_CHECKPOINT_ANTI_ROLLBACK_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_CANONICAL_DECODER_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_REVOCATION_CAP_ADAPTER_EVIDENCE_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_ARTIFACT_EVIDENCE_AVAILABLE_V2);
    assert!(!NATIVE_REGISTRATION_ACTIVATION_READY_V2);
    assert!(!NATIVE_REGISTRATION_RELEASE_ELIGIBLE_V2);
    assert!(!NATIVE_REGISTRATION_PRODUCTION_AVAILABLE_V2);
    assert_eq!(size_of::<GovernedHostRegistrationAuthorityV2>(), 0);
    assert_eq!(size_of::<GovernedOfflineInclusionAuthorityV2>(), 0);
    assert!(NATIVE_REGISTRATION_MINIMUM_ADAPTER_DELTA_V2.starts_with(b"make the existing"));
    assert!(NATIVE_REGISTRATION_ACTIVATION_BLOCKERS_V2.contains(&b'2'));
    assert!(
        NATIVE_REGISTRATION_ACTIVATION_BLOCKERS_V2
            .windows(13)
            .any(|window| window == b"anti-rollback")
    );
    assert!(
        NATIVE_REGISTRATION_ACTIVATION_BLOCKERS_V2
            .windows(20)
            .any(|window| window == b"freshness/projection")
    );
    assert_eq!(
        NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1
    );
    assert!(NATIVE_REGISTRATION_EVIDENCE_CONTRACT_V2.contains(&b'1'));
    assert!(NATIVE_REGISTRATION_RECEIPT_CONTRACT_V2.ends_with(b"/v2"));
    assert!(NATIVE_REGISTRATION_HASH_FRAME_CONTRACT_V2.starts_with(b"u64le-domain"));
    assert!(NATIVE_REGISTRATION_RAW_DIGEST_CONTRACT_V2.starts_with(b"device-key-id=SHA256"));
    assert!(NATIVE_REGISTRY_ALLOCATION_CONTRACT_V2.starts_with(b"index=first8"));
    assert_eq!(NATIVE_REGISTRY_DEPTH_V2, 64);
    assert_eq!(NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2, 2_090);
    assert_eq!(NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2, 595);
    assert_eq!(NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2, 138);
    assert_eq!(NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2, 2_823);
    assert!(
        NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2
            <= NATIVE_REGISTRATION_OFFLINE_ENVELOPE_MAX_BYTES_V2
    );

    let source = include_str!("attestation_registration.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod attestation_registration;")
            .count(),
        1
    );
    assert!(parent.contains("#[path = \"offline_cash_v2/attestation_registration.rs\"]"));
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod attestation_registration_tests;")
            .count(),
        1
    );
    assert!(parent.contains("#[path = \"offline_cash_v2/attestation_registration_tests.rs\"]"));
    assert!(!parent.contains("pub mod attestation_registration"));
    assert!(source.contains("pub(super) enum GovernedHostRegistrationAuthorityV2 {}"));
    assert!(source.contains("pub(super) enum GovernedOfflineInclusionAuthorityV2 {}"));
    assert!(!source.contains("impl Clone for AuthenticatedRegistrationLeafV2"));
    assert!(!source.contains("impl Clone for OfflineRegistrationReceiptV2"));
}

#[test]
fn helper_protocol_and_inequality_gate_matches_all_eight_live_v1_identities() {
    let (certificate_der, _, device_key, issuer_key) =
        certificate_material(STANDARD_ATTESTATION_DER, true);
    let base_words = helper_words(
        &certificate_der,
        STANDARD_ATTESTATION_DER,
        &device_key,
        &issuer_key,
    );
    let roles = [
        OfflineCashHalo2CircuitRoleV1::GuardUse,
        OfflineCashHalo2CircuitRoleV1::PlatformBind,
        OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
        OfflineCashHalo2CircuitRoleV1::GuardBundle,
    ];
    for parity in OfflineCashHalo2ParityV1::ALL {
        for role in roles {
            let expected = offline_cash_halo2_protocol_identity_v1(parity, role).digest();
            assert_eq!(
                native_registration_source_helper_protocol_digest_v1(parity as u32, role as u32),
                Some(expected)
            );
            let mut words = base_words;
            words[3] = parity as u32;
            words[4] = role as u32;
            write_digest_words_for_test_v2(&mut words, PROTOCOL_WORD_START, expected);
            native_fixed_messages_v2(&words, &device_key, &issuer_key)
                .expect("exact live V1 parity/role identity is accepted");

            words[PROTOCOL_WORD_START] ^= 1;
            assert_eq!(
                native_fixed_messages_v2(&words, &device_key, &issuer_key).map(|_| ()),
                Err(NativeRegistrationErrorV2::InvalidHelperProtocolIdentity)
            );
        }
    }
    assert_eq!(
        native_registration_source_helper_protocol_digest_v1(1, 1),
        None
    );
    assert_eq!(
        native_registration_source_helper_protocol_digest_v1(3, 2),
        None
    );
    let mut cross_parity = base_words;
    cross_parity[3] = 2;
    assert_eq!(
        native_fixed_messages_v2(&cross_parity, &device_key, &issuer_key).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidHelperProtocolIdentity)
    );
    let mut state_role = base_words;
    state_role[4] = OfflineCashHalo2CircuitRoleV1::State as u32;
    assert_eq!(
        native_fixed_messages_v2(&state_role, &device_key, &issuer_key).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidHelperHeader)
    );

    let mut equal_heads = base_words;
    equal_heads[56..64].copy_from_slice(&base_words[40..48]);
    assert_eq!(
        native_fixed_messages_v2(&equal_heads, &device_key, &issuer_key).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidHelperDigestEquality)
    );
    let mut equal_guards = base_words;
    equal_guards[96..104].copy_from_slice(&base_words[88..96]);
    assert_eq!(
        native_fixed_messages_v2(&equal_guards, &device_key, &issuer_key).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidHelperDigestEquality)
    );
}

#[test]
fn sha256_kat_and_fixed_nine_geometry_are_exact() {
    assert_eq!(
        sha256_for_test_v2(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
    let (input, policy, _) = fixture();
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    let leaf = authority
        .mint(input)
        .expect("bounded native fixture validates");
    assert_ne!(leaf.leaf_commitment(), [0; 32]);
    assert_ne!(leaf.authorization_commitment(), [0; 32]);
    assert_ne!(leaf.leaf_commitment(), leaf.authorization_commitment());
    assert_eq!(
        NATIVE_REGISTRATION_FIXED_MESSAGE_BYTES_V2,
        [355, 432, 494, 65, 533, 376, 65, 480, 619]
    );
}

#[test]
fn every_fixed_sha_job_is_checked_in_dependency_order() {
    for job in 0..NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2.len() {
        let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
            let offset = NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2[job];
            parts.words[offset] ^= 1;
        });
        let mut authority = TestHostRegistrationAuthorityV2::new(policy);
        assert_eq!(
            authority.mint(input).map(|_| ()),
            Err(NativeRegistrationErrorV2::FixedJobDigestMismatch { job })
        );
    }
}

#[test]
fn cheap_failed_attempt_poisons_the_one_shot_capability() {
    let (poisoned, mut policy, _) = fixture();
    let valid_retry = duplicate_input_for_test_v2(&poisoned);
    unordered_governed_issuers_for_test_v2(&mut policy);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(poisoned).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidGovernanceCollection)
    );
    assert_eq!(
        authority.mint(valid_retry).map(|_| ()),
        Err(NativeRegistrationErrorV2::AuthoritySpent)
    );
}

#[test]
fn exact_certificate_tbs_issuer_and_attestation_digests_reach_public_receipt() {
    let (input, policy, expected) = fixture();
    let mut host = TestHostRegistrationAuthorityV2::new(policy);
    let leaf = host.mint(input).expect("host registration");
    let siblings = core::array::from_fn(|level| {
        sha256_for_test_v2(&[u8::try_from(level + 1).expect("depth fits u8")])
    });
    let index = registry_leaf_index_for_test_v2(&expected.device_key_id);
    let root = registry_root_for_test_v2(leaf.leaf_commitment(), index, siblings);
    let proof = RegistryInclusionProofV2::new(index, siblings, root).expect("canonical proof");
    let mut inclusion =
        TestOfflineInclusionAuthorityV2::new(POLICY_EPOCH, POLICY_DIGEST, REGISTRY_INTENT, root);
    let checkpoint = inclusion.checkpoint().expect("governed checkpoint");
    let receipt = inclusion.issue(leaf, &proof).expect("persisted inclusion");
    verify_offline_registration_receipt_v2(&receipt, &proof, &checkpoint)
        .expect("offline receipt verifies");
    let receipt_bytes = receipt.canonical_bytes();
    let proof_bytes = proof.canonical_bytes();
    let checkpoint_bytes = checkpoint.canonical_bytes();
    assert_eq!(
        receipt_bytes.len(),
        NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2
    );
    assert_eq!(
        proof_bytes.len(),
        NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2
    );
    assert_eq!(
        checkpoint_bytes.len(),
        NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2
    );
    assert_eq!(&receipt_bytes[..2], &2_u16.to_le_bytes());
    assert_eq!(&receipt_bytes[74..106], &expected.device_key_id);
    assert_eq!(&receipt_bytes[523..531], &index.to_le_bytes());
    assert_eq!(&proof_bytes[2..10], &index.to_le_bytes());
    assert_eq!(&checkpoint_bytes[..2], &2_u16.to_le_bytes());
    assert_eq!(receipt.device_key_id(), expected.device_key_id);
    assert_eq!(receipt.device_public_key_sec1(), &expected.device_key);
    assert_eq!(receipt.certificate_digest(), expected.certificate_digest);
    assert_eq!(receipt.tbs_digest(), expected.tbs_digest);
    assert_eq!(receipt.issuer_key_digest(), expected.issuer_key_digest);
    assert_eq!(receipt.attestation_digest(), expected.attestation_digest);
    assert_ne!(receipt.receipt_commitment(), [0; 32]);
    let durable_identity = receipt.durable_identity_projection();
    assert_eq!(durable_identity.policy_epoch(), POLICY_EPOCH);
    assert_eq!(durable_identity.policy_digest(), &POLICY_DIGEST);
    assert_eq!(durable_identity.registry_root_intent(), &REGISTRY_INTENT);
    assert_eq!(durable_identity.device_descriptor_digest(), &[8; 32]);
    assert_eq!(durable_identity.device_key_id(), &expected.device_key_id);
    assert_eq!(
        durable_identity.device_public_key_sec1(),
        &expected.device_key
    );
    assert_eq!(
        durable_identity.receipt_commitment(),
        &receipt.receipt_commitment()
    );
}

#[test]
fn certificate_trailing_bytes_are_rejected_before_hash_admission() {
    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
        parts.certificate_der.push(0);
        parts.chain_der[0].push(0);
    });
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::TrailingCertificateDer { index: 0 })
    );
}

#[test]
fn ambiguous_inner_der_and_missing_attestation_extension_are_rejected() {
    let mut trailing = STANDARD_ATTESTATION_DER.to_vec();
    trailing.push(0);
    let (input, policy, _) = fixture_with(trailing, true, |_| {});
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::NonCanonicalAttestationExtension)
    );

    let non_minimal = vec![0x30, 0x81, 0x03, 0x02, 0x01, 0x01];
    let (input, policy, _) = fixture_with(non_minimal, true, |_| {});
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::NonCanonicalAttestationExtension)
    );

    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), false, |_| {});
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidAttestationExtension)
    );
}

#[test]
fn chain_signature_leaf_binding_and_order_mutations_fail_closed() {
    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
        let last = parts.certificate_der.len() - 1;
        parts.certificate_der[last] ^= 1;
        parts.chain_der[0] = parts.certificate_der.clone();
    });
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidCertificateSignature)
    );

    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
        parts.chain_der.reverse()
    });
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::LeafCertificateMismatch)
    );
}

#[test]
fn attestation_bytes_device_key_and_policy_epoch_mutations_fail_closed() {
    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
        parts.attestation_der[4] ^= 1
    });
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::AttestationExtensionMismatch)
    );

    let replacement_key_pair = KeyPair::generate().expect("replacement P-256 key");
    let replacement_key: [u8; 65] = replacement_key_pair
        .der_bytes()
        .try_into()
        .expect("uncompressed replacement SEC1 key");
    let (input, policy, _) = fixture_with(STANDARD_ATTESTATION_DER.to_vec(), true, |parts| {
        parts.device_key = replacement_key;
        parts.device_key_id = sha256_for_test_v2(&replacement_key);
    });
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::DeviceKeyMismatch)
    );

    let (leaf_der, root_der, device_key, issuer_key) =
        certificate_material(STANDARD_ATTESTATION_DER, true);
    let words = helper_words(
        &leaf_der,
        STANDARD_ATTESTATION_DER,
        &device_key,
        &issuer_key,
    );
    assert_eq!(
        NativeAttestationRegistrationInputV2::new(
            words,
            device_key,
            sha256_for_test_v2(&device_key),
            0,
            REGISTRY_INTENT,
            leaf_der.clone(),
            vec![leaf_der, root_der],
            STANDARD_ATTESTATION_DER.to_vec(),
        )
        .map(|_| ()),
        Err(NativeRegistrationErrorV2::ZeroPolicyEpoch)
    );

    let (leaf_der, root_der, device_key, issuer_key) =
        certificate_material(STANDARD_ATTESTATION_DER, true);
    let words = helper_words(
        &leaf_der,
        STANDARD_ATTESTATION_DER,
        &device_key,
        &issuer_key,
    );
    let input = NativeAttestationRegistrationInputV2::new(
        words,
        device_key,
        sha256_for_test_v2(&device_key),
        POLICY_EPOCH + 1,
        REGISTRY_INTENT,
        leaf_der.clone(),
        vec![leaf_der, root_der.clone()],
        STANDARD_ATTESTATION_DER.to_vec(),
    )
    .expect("nonzero mismatched epoch is structurally valid");
    let policy = governed_policy_for_test_v2(
        POLICY_EPOCH,
        POLICY_DIGEST,
        REGISTRY_INTENT,
        EVALUATION_UNIX_SECONDS,
        vec![root_der],
        Vec::new(),
        vec![issuer_key_id_for_test_v2(&issuer_key)],
    );
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::PolicyEpochMismatch)
    );
}

#[test]
fn raw_tbs_cap_and_bounded_fixture_endpoints_are_enforced() {
    let (input, policy, _) = fixture();
    let mut host = TestHostRegistrationAuthorityV2::new(policy);
    host.mint(input)
        .expect("ordinary fixture is below the raw-TBS cap");

    let payload_len = NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2;
    let mut oversized_inner_der = Vec::with_capacity(payload_len + 4);
    oversized_inner_der.extend_from_slice(&[0x04, 0x82]);
    oversized_inner_der.extend_from_slice(
        &u16::try_from(payload_len)
            .expect("test payload fits u16")
            .to_be_bytes(),
    );
    oversized_inner_der.resize(payload_len + 4, 0xA5);
    assert!(oversized_inner_der.len() < NATIVE_REGISTRATION_MAX_ATTESTATION_EXTENSION_BYTES_V2);
    let (input, policy, _) = fixture_with(oversized_inner_der, true, |_| {});
    let mut host = TestHostRegistrationAuthorityV2::new(policy);
    assert!(matches!(
        host.mint(input),
        Err(NativeRegistrationErrorV2::RawTbsTooLarge {
            actual,
            maximum: NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2,
        }) if actual > NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2
    ));
}

#[test]
fn governance_time_ca_revocation_and_full_cap_mutations_are_enforced() {
    let (input, mut policy, _) = fixture();
    unordered_governed_issuers_for_test_v2(&mut policy);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidGovernanceCollection)
    );

    let (input, mut policy, _) = fixture();
    set_policy_evaluation_time_for_test_v2(&mut policy, 0);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidCertificateTime)
    );

    let (input, mut policy, expected) = fixture();
    revoke_certificate_for_test_v2(&mut policy, expected.certificate_digest);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::RevokedCertificate)
    );

    let (leaf_der, root_der, device_key, issuer_key) =
        certificate_material_with_root_profile(STANDARD_ATTESTATION_DER, true, false);
    let words = helper_words(
        &leaf_der,
        STANDARD_ATTESTATION_DER,
        &device_key,
        &issuer_key,
    );
    let input = NativeAttestationRegistrationInputV2::new(
        words,
        device_key,
        sha256_for_test_v2(&device_key),
        POLICY_EPOCH,
        REGISTRY_INTENT,
        leaf_der.clone(),
        vec![leaf_der, root_der.clone()],
        STANDARD_ATTESTATION_DER.to_vec(),
    )
    .expect("non-CA root fixture is structurally bounded");
    let policy = governed_policy_for_test_v2(
        POLICY_EPOCH,
        POLICY_DIGEST,
        REGISTRY_INTENT,
        EVALUATION_UNIX_SECONDS,
        vec![root_der],
        Vec::new(),
        vec![issuer_key_id_for_test_v2(&issuer_key)],
    );
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidCaUsage)
    );

    let (input, mut policy, _) = fixture();
    let maximum_revocations = (1_u16
        ..=u16::try_from(NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2)
            .expect("governed revocation cap fits u16"))
        .map(|index| {
            let mut digest = [0xA5; 32];
            digest[..2].copy_from_slice(&index.to_le_bytes());
            digest
        })
        .collect();
    replace_revocations_for_test_v2(&mut policy, maximum_revocations);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    authority
        .mint(input)
        .expect("source accepts the complete governed revocation cap");

    let (input, mut policy, _) = fixture();
    let excessive_revocations = (1_u16
        ..=u16::try_from(NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2 + 1)
            .expect("one beyond the governed cap fits u16"))
        .map(|index| {
            let mut digest = [0x5A; 32];
            digest[..2].copy_from_slice(&index.to_le_bytes());
            digest
        })
        .collect();
    replace_revocations_for_test_v2(&mut policy, excessive_revocations);
    let mut authority = TestHostRegistrationAuthorityV2::new(policy);
    assert_eq!(
        authority.mint(input).map(|_| ()),
        Err(NativeRegistrationErrorV2::InvalidGovernanceCollection)
    );
}

#[test]
fn host_and_inclusion_authorities_are_independent_one_shot_capabilities() {
    let (input, policy, expected) = fixture();
    let mut host = TestHostRegistrationAuthorityV2::new(policy);
    let leaf = host.mint(input).expect("first host mint");
    let (second_input, _, _) = fixture();
    assert_eq!(
        host.mint(second_input).map(|_| ()),
        Err(NativeRegistrationErrorV2::AuthoritySpent)
    );

    let siblings = core::array::from_fn(|level| sha256_for_test_v2(&[0x80 | level as u8]));
    let index = registry_leaf_index_for_test_v2(&expected.device_key_id);
    let root = registry_root_for_test_v2(leaf.leaf_commitment(), index, siblings);
    let proof = RegistryInclusionProofV2::new(index, siblings, root).expect("inclusion proof");
    let mut inclusion =
        TestOfflineInclusionAuthorityV2::new(POLICY_EPOCH, POLICY_DIGEST, REGISTRY_INTENT, root);
    inclusion.issue(leaf, &proof).expect("first receipt issue");

    let (third_input, third_policy, _) = fixture();
    let mut third_host = TestHostRegistrationAuthorityV2::new(third_policy);
    let third_leaf = third_host.mint(third_input).expect("independent host mint");
    assert_eq!(
        inclusion.issue(third_leaf, &proof).map(|_| ()),
        Err(NativeRegistrationErrorV2::AuthoritySpent)
    );
}

#[test]
fn zero_and_mutated_inclusion_or_receipt_commitments_are_rejected() {
    let nonzero_siblings = core::array::from_fn(|level| sha256_for_test_v2(&[level as u8]));
    let mut zero_sibling = nonzero_siblings;
    zero_sibling[3] = [0; 32];
    assert_eq!(
        RegistryInclusionProofV2::new(0, zero_sibling, [1; 32]).map(|_| ()),
        Err(NativeRegistrationErrorV2::ZeroInclusionDigest)
    );
    assert_eq!(
        RegistryInclusionProofV2::new(0, nonzero_siblings, [0; 32]).map(|_| ()),
        Err(NativeRegistrationErrorV2::ZeroInclusionDigest)
    );

    let (wrong_input, wrong_policy, wrong_expected) = fixture();
    let retry_input = duplicate_input_for_test_v2(&wrong_input);
    let mut retry_host = TestHostRegistrationAuthorityV2::new(wrong_policy.clone());
    let retry_leaf = retry_host.mint(retry_input).expect("retry leaf fixture");
    let mut wrong_host = TestHostRegistrationAuthorityV2::new(wrong_policy);
    let wrong_leaf = wrong_host.mint(wrong_input).expect("host mint");
    let wrong_index = registry_leaf_index_for_test_v2(&wrong_expected.device_key_id) ^ 1;
    let wrong_root =
        registry_root_for_test_v2(wrong_leaf.leaf_commitment(), wrong_index, nonzero_siblings);
    let wrong_proof = RegistryInclusionProofV2::new(wrong_index, nonzero_siblings, wrong_root)
        .expect("structurally bounded but non-canonical index proof");
    let mut wrong_inclusion = TestOfflineInclusionAuthorityV2::new(
        POLICY_EPOCH,
        POLICY_DIGEST,
        REGISTRY_INTENT,
        wrong_root,
    );
    assert_eq!(
        wrong_inclusion.issue(wrong_leaf, &wrong_proof).map(|_| ()),
        Err(NativeRegistrationErrorV2::NonCanonicalLeafIndex)
    );
    assert_eq!(
        wrong_inclusion.issue(retry_leaf, &wrong_proof).map(|_| ()),
        Err(NativeRegistrationErrorV2::AuthoritySpent)
    );

    let (input, policy, expected) = fixture();
    let mut host = TestHostRegistrationAuthorityV2::new(policy);
    let leaf = host.mint(input).expect("host mint");
    let index = registry_leaf_index_for_test_v2(&expected.device_key_id);
    let root = registry_root_for_test_v2(leaf.leaf_commitment(), index, nonzero_siblings);
    let proof = RegistryInclusionProofV2::new(index, nonzero_siblings, root).expect("proof");
    let mut inclusion =
        TestOfflineInclusionAuthorityV2::new(POLICY_EPOCH, POLICY_DIGEST, REGISTRY_INTENT, root);
    let mut checkpoint = inclusion.checkpoint().expect("checkpoint");
    let mut receipt: OfflineRegistrationReceiptV2 = inclusion.issue(leaf, &proof).expect("receipt");
    mutate_checkpoint_commitment_for_test_v2(&mut checkpoint);
    assert_eq!(
        verify_offline_registration_receipt_v2(&receipt, &proof, &checkpoint),
        Err(NativeRegistrationErrorV2::ReceiptCommitmentMismatch)
    );
    mutate_checkpoint_commitment_for_test_v2(&mut checkpoint);
    verify_offline_registration_receipt_v2(&receipt, &proof, &checkpoint)
        .expect("restored checkpoint verifies");
    mutate_receipt_commitment_for_test_v2(&mut receipt);
    assert_eq!(
        verify_offline_registration_receipt_v2(&receipt, &proof, &checkpoint),
        Err(NativeRegistrationErrorV2::ReceiptCommitmentMismatch)
    );
}

#[test]
fn field_order_is_domain_and_position_sensitive() {
    let left = sha256_for_test_v2(b"left");
    let right = sha256_for_test_v2(b"right");
    let siblings_a = core::array::from_fn(|level| {
        if level == 0 {
            left
        } else {
            sha256_for_test_v2(&[level as u8])
        }
    });
    let siblings_b = core::array::from_fn(|level| {
        if level == 0 {
            right
        } else {
            sha256_for_test_v2(&[level as u8])
        }
    });
    let leaf = sha256_for_test_v2(b"leaf");
    assert_ne!(
        registry_root_for_test_v2(leaf, 0, siblings_a),
        registry_root_for_test_v2(leaf, 0, siblings_b)
    );
    assert_ne!(
        registry_root_for_test_v2(leaf, 0, siblings_a),
        registry_root_for_test_v2(leaf, 1, siblings_a)
    );
    assert_eq!(NATIVE_REGISTRY_DEPTH_V2, 64);
}

// Compile-time shape check: neither authenticated output implements `Copy`.  Passing by value
// below makes ownership explicit without requiring a negative trait bound.
#[allow(dead_code)]
fn consume_move_only_outputs(
    _leaf: AuthenticatedRegistrationLeafV2,
    _receipt: OfflineRegistrationReceiptV2,
) {
}
