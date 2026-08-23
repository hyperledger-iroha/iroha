use super::*;

use sha2::{Digest as _, Sha256};

use super::super::{
    registered_platform_p256_circuit_source::{
        REGISTERED_PLATFORM_P256_CIRCUIT_WITNESS_LOGICAL_BYTES_V2,
        REGISTERED_PLATFORM_P256_CONTEXTUAL_CANDIDATES_LOGICAL_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_ARTIFACT_DELTA_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_PARAMS_DELTA_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_PEAK_LOGICAL_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_PROOF_DELTA_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_SCRATCH_LOGICAL_BYTES_V2,
        REGISTERED_PLATFORM_P256_SOURCE_TRACE_ROW_DELTA_V2,
        REGISTERED_PLATFORM_P256_SOURCE_WIRE_DELTA_BYTES_V2,
        assemble_unverified_registered_platform_p256_circuit_candidates_v2,
    },
    state_terminal_candidate::STATE_TERMINAL_CANDIDATE_ORDER_V2,
};

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("fixture is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("fixture has exactly {N} bytes"))
}

fn rfc6979_public_key() -> [u8; 65] {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&x);
    sec1[33..].copy_from_slice(&y);
    sec1
}

fn rfc6979_low_s_signature() -> [u8; 64] {
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let low_s =
        decode_hex::<32>("0834E36AD29A83BF2BC9385E491D6099C8FDF9D1ED67AA7EA5F51F93782857A9");
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&r);
    signature[32..].copy_from_slice(&low_s);
    signature
}

fn rfc6979_high_s_signature() -> [u8; 64] {
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let high_s =
        decode_hex::<32>("F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8");
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&r);
    signature[32..].copy_from_slice(&high_s);
    signature
}

fn identity_for_key(sec1: [u8; 65]) -> DurableRegistrationIdentityProjectionV2 {
    let key_id: [u8; 32] = Sha256::digest(sec1).into();
    DurableRegistrationIdentityProjectionV2::from_test_parts(
        9, [0x17; 32], [0x21; 32], [0x18; 32], key_id, sec1, [0x22; 32],
    )
}

fn current_helper_for_key(sec1: &[u8; 65]) -> RegisteredPlatformP256CurrentHelperFieldsV2 {
    let platform_key_digest: [u8; 32] = Sha256::digest(sec1).into();
    let mut current = RegisteredPlatformP256CurrentHelperFieldsV2 {
        operation: 1,
        release_id: [0x11; 32],
        context_digest: [0x12; 32],
        current_head: [0x13; 32],
        current_lineage_digest: [0x14; 32],
        transition_digest: [0x15; 32],
        wallet_binding: [0x16; 32],
        hardware_policy_id: [0x17; 32],
        guard_device_id: [0x18; 32],
        current_guard_binding: [0x19; 32],
        next_guard_binding: [0x1A; 32],
        from_sequence: 7,
        to_sequence: 8,
        platform_key_digest,
        platform_message_digest: [0xFF; 32],
    };
    let message = exact_current_platform_message(&current).expect("exact platform message");
    current.platform_message_digest = Sha256::digest(message).into();
    current
}

fn candidate(
    identity: DurableRegistrationIdentityProjectionV2,
    current: RegisteredPlatformP256CurrentHelperFieldsV2,
) -> AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
    AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
        registration_identity: identity,
        current,
    }
}

fn valid_parts() -> (
    DurableRegistrationIdentityProjectionV2,
    AuthenticatedRegisteredPlatformCurrentHelperCandidateV2,
    [u8; 64],
    [u8; 32],
) {
    let sec1 = rfc6979_public_key();
    let identity = identity_for_key(sec1);
    let current = current_helper_for_key(&sec1);
    let prehash = current.platform_message_digest;
    (
        identity.clone(),
        candidate(identity, current),
        rfc6979_low_s_signature(),
        prehash,
    )
}

fn assert_exact_authenticated_context(
    context: &AuthenticatedRegisteredPlatformCurrentHelperCandidateV2,
) {
    let expected_key_id: [u8; 32] = Sha256::digest(rfc6979_public_key()).into();
    let identity = context.durable_identity();
    assert_eq!(identity.policy_epoch(), 9);
    assert_eq!(identity.policy_digest(), &[0x17; 32]);
    assert_eq!(identity.registry_root_intent(), &[0x21; 32]);
    assert_eq!(identity.device_descriptor_digest(), &[0x18; 32]);
    assert_eq!(identity.device_key_id(), &expected_key_id);
    assert_eq!(identity.device_public_key_sec1(), &rfc6979_public_key());
    assert_eq!(identity.receipt_commitment(), &[0x22; 32]);

    let current = context.current_helper_fields();
    assert_eq!(current.operation, 1);
    assert_eq!(current.release_id, [0x11; 32]);
    assert_eq!(current.context_digest, [0x12; 32]);
    assert_eq!(current.current_head, [0x13; 32]);
    assert_eq!(current.current_lineage_digest, [0x14; 32]);
    assert_eq!(current.transition_digest, [0x15; 32]);
    assert_eq!(current.wallet_binding, [0x16; 32]);
    assert_eq!(current.hardware_policy_id, [0x17; 32]);
    assert_eq!(current.guard_device_id, [0x18; 32]);
    assert_eq!(current.current_guard_binding, [0x19; 32]);
    assert_eq!(current.next_guard_binding, [0x1A; 32]);
    assert_eq!(current.from_sequence, 7);
    assert_eq!(current.to_sequence, 8);
    assert_eq!(current.platform_key_digest, expected_key_id);
    assert_eq!(
        current.platform_message_digest,
        decode_hex::<32>("90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC")
    );
}

fn changed_identity(
    identity: &DurableRegistrationIdentityProjectionV2,
    index: usize,
) -> DurableRegistrationIdentityProjectionV2 {
    let mut policy_epoch = identity.policy_epoch();
    let mut policy_digest = *identity.policy_digest();
    let mut registry_root_intent = *identity.registry_root_intent();
    let mut device_descriptor_digest = *identity.device_descriptor_digest();
    let mut device_key_id = *identity.device_key_id();
    let mut device_public_key_sec1 = *identity.device_public_key_sec1();
    let mut receipt_commitment = *identity.receipt_commitment();
    match index {
        0 => policy_epoch += 1,
        1 => policy_digest[0] ^= 1,
        2 => registry_root_intent[0] ^= 1,
        3 => device_descriptor_digest[0] ^= 1,
        4 => device_key_id[0] ^= 1,
        5 => device_public_key_sec1[64] ^= 1,
        6 => receipt_commitment[0] ^= 1,
        _ => panic!("identity field index is in range"),
    }
    DurableRegistrationIdentityProjectionV2::from_test_parts(
        policy_epoch,
        policy_digest,
        registry_root_intent,
        device_descriptor_digest,
        device_key_id,
        device_public_key_sec1,
        receipt_commitment,
    )
}

#[test]
fn exact_statement_offsets_and_platform_prehash_are_frozen() {
    let (identity, candidate, signature, prehash) = valid_parts();
    let receipt_commitment = *identity.receipt_commitment();
    let sec1 = *identity.device_public_key_sec1();
    let owner = UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
        identity, candidate, signature,
    )
    .expect("structural owner binding");

    assert_eq!(REGISTERED_PLATFORM_P256_PLATFORM_MESSAGE_BYTES_V2, 494);
    assert_eq!(REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2, 161);
    assert_eq!(REGISTERED_PLATFORM_P256_SEC1_OFFSET_V2, 0);
    assert_eq!(REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2, 65);
    assert_eq!(REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2, 97);
    assert_eq!(REGISTERED_PLATFORM_P256_STATEMENT_END_V2, 161);
    assert_eq!(owner.durable_receipt_commitment(), &receipt_commitment);
    assert_eq!(
        &owner.statement_bytes()[..REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2],
        &sec1
    );
    assert_eq!(
        &owner.statement_bytes()[REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2
            ..REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2],
        &prehash
    );
    assert_eq!(
        &owner.statement_bytes()[REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2..],
        &signature
    );
}

#[test]
fn exact_platform_message_has_a_fixed_independent_sha256_kat() {
    let current = current_helper_for_key(&rfc6979_public_key());
    let message = exact_current_platform_message(&current).expect("exact platform message");
    let actual: [u8; 32] = Sha256::digest(&message).into();
    assert_eq!(message.len(), 494);
    assert_eq!(
        actual,
        decode_hex::<32>("90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC")
    );
    assert_eq!(
        current.platform_message_digest,
        decode_hex::<32>("90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC")
    );
}

#[test]
fn every_durable_identity_mutation_is_rejected() {
    for index in 0..7 {
        let (identity, candidate, signature, _) = valid_parts();
        let changed = changed_identity(&identity, index);
        assert_eq!(
            UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
                changed, candidate, signature,
            )
            .err(),
            Some(RegisteredPlatformP256StatementErrorV2::DurableIdentityMismatch),
            "identity field {index}"
        );
    }
}

#[test]
fn every_exact_current_helper_field_mutation_is_rejected_before_pairing() {
    for index in 0..15 {
        let (identity, mut candidate, signature, _) = valid_parts();
        match index {
            0 => candidate.current.operation = 2,
            1 => candidate.current.release_id[0] ^= 1,
            2 => candidate.current.context_digest[0] ^= 1,
            3 => candidate.current.current_head[0] ^= 1,
            4 => candidate.current.current_lineage_digest[0] ^= 1,
            5 => candidate.current.transition_digest[0] ^= 1,
            6 => candidate.current.wallet_binding[0] ^= 1,
            7 => candidate.current.hardware_policy_id[0] ^= 1,
            8 => candidate.current.guard_device_id[0] ^= 1,
            9 => candidate.current.current_guard_binding[0] ^= 1,
            10 => candidate.current.next_guard_binding[0] ^= 1,
            11 => candidate.current.from_sequence += 1,
            12 => candidate.current.to_sequence += 1,
            13 => candidate.current.platform_key_digest[0] ^= 1,
            14 => candidate.current.platform_message_digest[0] ^= 1,
            _ => unreachable!("current-helper field index is bounded"),
        }
        assert!(
            UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
                identity, candidate, signature,
            )
            .is_err(),
            "current-helper field {index}"
        );
    }
}

#[test]
fn durable_projection_getters_and_receipt_mapping_are_exact() {
    let identity = identity_for_key(rfc6979_public_key());
    let expected_key_id: [u8; 32] = Sha256::digest(rfc6979_public_key()).into();
    assert_eq!(identity.policy_epoch(), 9);
    assert_eq!(identity.policy_digest(), &[0x17; 32]);
    assert_eq!(identity.registry_root_intent(), &[0x21; 32]);
    assert_eq!(identity.device_descriptor_digest(), &[0x18; 32]);
    assert_eq!(identity.device_key_id(), &expected_key_id);
    assert_eq!(identity.device_public_key_sec1(), &rfc6979_public_key());
    assert_eq!(identity.receipt_commitment(), &[0x22; 32]);

    let registration_source = include_str!("attestation_registration.rs");
    for mapping in [
        "policy_epoch: self.payload.policy_epoch",
        "policy_digest: self.payload.policy_digest",
        "registry_root_intent: self.payload.registry_root_intent",
        "device_descriptor_digest: self.payload.device_descriptor_digest",
        "device_key_id: self.payload.device_key_id",
        "device_public_key_sec1: self.payload.device_public_key_sec1",
        "receipt_commitment: self.receipt_commitment",
    ] {
        assert!(registration_source.contains(mapping), "missing {mapping}");
    }
    let projection = registration_source
        .split_once("pub(super) struct DurableRegistrationIdentityProjectionV2 {")
        .and_then(|(_, tail)| tail.split_once("impl DurableRegistrationIdentityProjectionV2"))
        .map(|(fields, _)| fields)
        .expect("projection remains source-visible");
    assert!(!projection.contains("guard_bundle_digest"));
    assert!(!projection.contains("certificate_digest"));
    assert!(!projection.contains("attestation_digest"));
}

#[test]
fn policy_device_key_and_message_mismatches_fail_closed() {
    let (identity, mut candidate, signature, _) = valid_parts();
    candidate.current.hardware_policy_id[0] ^= 1;
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::PolicyIdentityMismatch)
    );

    let (identity, mut candidate, signature, _) = valid_parts();
    candidate.current.guard_device_id[0] ^= 1;
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::DeviceIdentityMismatch)
    );

    let (identity, mut candidate, signature, _) = valid_parts();
    candidate.current.platform_key_digest[0] ^= 1;
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::PlatformKeyIdentityMismatch)
    );

    let (identity, mut candidate, signature, _) = valid_parts();
    candidate.current.platform_message_digest[0] ^= 1;
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::PlatformMessageDigestMismatch)
    );

    let (identity, mut candidate, signature, _) = valid_parts();
    candidate.current.to_sequence += 1;
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::MalformedCurrentHelper)
    );
}

#[test]
fn malformed_key_signature_and_high_s_are_rejected() {
    let mut malformed_key = rfc6979_public_key();
    malformed_key[0] = 2;
    let identity = identity_for_key(malformed_key);
    let current = current_helper_for_key(&malformed_key);
    let candidate = candidate(identity.clone(), current);
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity,
            candidate,
            rfc6979_low_s_signature(),
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::InvalidPlatformPublicKey)
    );

    let malformed_signatures = {
        let mut zero_r = rfc6979_low_s_signature();
        zero_r[..32].fill(0);
        let mut zero_s = rfc6979_low_s_signature();
        zero_s[32..].fill(0);
        [zero_r, zero_s, [0xFF; 64]]
    };
    for signature in malformed_signatures {
        let (identity, candidate, _, _) = valid_parts();
        assert_eq!(
            UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
                identity, candidate, signature,
            )
            .err(),
            Some(RegisteredPlatformP256StatementErrorV2::InvalidPlatformSignature)
        );
    }

    let (identity, candidate, _, _) = valid_parts();
    assert_eq!(
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity,
            candidate,
            rfc6979_high_s_signature(),
        )
        .err(),
        Some(RegisteredPlatformP256StatementErrorV2::HighSPlatformSignature)
    );
}

#[test]
fn eq_and_ep_emit_one_shared_role_six_transaction_statement() {
    let (identity, candidate, signature, _) = valid_parts();
    let owner = UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
        identity, candidate, signature,
    )
    .expect("structural owner binding");
    let source_pair = owner.into_eq_ep_source_pair();
    let [eq, ep] = source_pair.statements();
    assert_eq!(eq.parity(), OfflineCashHalo2ParityV2::Eq);
    assert_eq!(ep.parity(), OfflineCashHalo2ParityV2::Ep);
    assert_eq!(eq.role(), OfflineCashHalo2CircuitRoleV2::P256Signature);
    assert_eq!(ep.role(), OfflineCashHalo2CircuitRoleV2::P256Signature);
    assert_eq!(eq.role() as u8, 6);
    assert_eq!(ep.role() as u8, 6);
    assert_eq!(eq.statement_bytes(), ep.statement_bytes());
    assert_eq!(eq.statement_bytes().len(), 161);
    assert_eq!(
        REGISTERED_PLATFORM_P256_ROLE_CONTRACT_V2,
        b"role-6=transaction-platform-signature-only/certificate-signatures=native-registration-only/v2"
    );
}

#[test]
fn authenticated_context_survives_owner_pair_and_opaque_candidate_by_value() {
    let (identity, candidate, signature, _) = valid_parts();
    let owner = UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
        identity, candidate, signature,
    )
    .expect("structural owner binding");
    assert_exact_authenticated_context(owner.authenticated_current_helper());

    let source_pair = owner.into_eq_ep_source_pair();
    assert_exact_authenticated_context(source_pair.authenticated_current_helper());
    let expected_statement = *source_pair.statements()[0].statement_bytes();

    let candidates =
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(source_pair)
            .expect("context-preserving opaque candidates");
    assert_exact_authenticated_context(candidates.source_pair().authenticated_current_helper());
    let [eq, ep] = candidates.provenance();
    assert_eq!(eq.statement_bytes(), &expected_statement);
    assert_eq!(ep.statement_bytes(), &expected_statement);
}

#[test]
fn context_preservation_logical_resource_ledger_is_exact_and_non_wire() {
    assert_eq!(
        REGISTERED_PLATFORM_P256_DURABLE_IDENTITY_LOGICAL_BYTES_V2,
        233
    );
    assert_eq!(
        REGISTERED_PLATFORM_P256_CURRENT_HELPER_LOGICAL_BYTES_V2,
        401
    );
    assert_eq!(
        233 + 401,
        REGISTERED_PLATFORM_P256_AUTHENTICATED_CONTEXT_LOGICAL_BYTES_V2
    );
    assert_eq!(
        634 + 161,
        REGISTERED_PLATFORM_P256_PRE_PAIR_OWNER_LOGICAL_BYTES_V2
    );
    assert_eq!(
        1 + 1 + 161,
        REGISTERED_PLATFORM_P256_TYPED_STATEMENT_LOGICAL_BYTES_V2
    );
    assert_eq!(
        2 * 163,
        REGISTERED_PLATFORM_P256_TYPED_PAIR_LOGICAL_BYTES_V2
    );
    assert_eq!(
        634 + 326,
        REGISTERED_PLATFORM_P256_SOURCE_PAIR_LOGICAL_BYTES_V2
    );
    assert_eq!(
        2 * 161,
        REGISTERED_PLATFORM_P256_CIRCUIT_WITNESS_LOGICAL_BYTES_V2
    );
    assert_eq!(
        960 + 322,
        REGISTERED_PLATFORM_P256_CONTEXTUAL_CANDIDATES_LOGICAL_BYTES_V2
    );
    assert_eq!(
        REGISTERED_PLATFORM_P256_SOURCE_SCRATCH_LOGICAL_BYTES_V2,
        161
    );
    assert_eq!(
        1_282 + 161,
        REGISTERED_PLATFORM_P256_SOURCE_PEAK_LOGICAL_BYTES_V2
    );
    assert_eq!(REGISTERED_PLATFORM_P256_SOURCE_WIRE_DELTA_BYTES_V2, 0);
    assert_eq!(REGISTERED_PLATFORM_P256_SOURCE_PROOF_DELTA_BYTES_V2, 0);
    assert_eq!(REGISTERED_PLATFORM_P256_SOURCE_ARTIFACT_DELTA_BYTES_V2, 0);
    assert_eq!(REGISTERED_PLATFORM_P256_SOURCE_TRACE_ROW_DELTA_V2, 0);
    assert_eq!(REGISTERED_PLATFORM_P256_SOURCE_PARAMS_DELTA_BYTES_V2, 0);

    assert_eq!(OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2, 9_152);
    assert_eq!(
        REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2,
        9_184
    );
    assert!(!REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2);
}

#[test]
fn registration_envelope_stays_out_of_band_and_session_math_is_only_telemetry() {
    assert_eq!(
        REGISTERED_PLATFORM_P256_OUT_OF_BAND_REGISTRATION_BYTES_V2,
        2_823
    );
    assert_eq!(OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2, 9_152);
    assert_eq!(
        REGISTERED_PLATFORM_P256_CANDIDATE_REGISTRATION_RECEIPT_REFERENCE_BYTES_V2,
        32
    );
    assert_eq!(
        9_152 + 32,
        REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2
    );
    assert_eq!(
        REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2,
        9_184
    );
    assert_eq!(
        OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2,
        9_403
    );
    assert_eq!(9_403 - 9_184, 219);
    assert_eq!(
        REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_HEADROOM_BYTES_V2,
        219
    );
    assert_eq!(9_152 + 2_823, 11_975);
    assert_eq!(
        REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_TOTAL_BYTES_V2,
        11_975
    );
    assert_eq!(11_975 - 9_403, 2_572);
    assert_eq!(
        REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_EXCESS_BYTES_V2,
        2_572
    );
}

#[test]
fn production_boundary_and_every_capability_remain_fail_closed() {
    assert!(REGISTERED_PLATFORM_P256_DECLARED_V2);
    assert!(!REGISTERED_PLATFORM_P256_CURRENT_HELPER_AUTHENTICATION_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_FRESHNESS_AUTHORITY_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_COMPILED_PROTOCOL_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!REGISTERED_PLATFORM_P256_BACKEND_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_GUARD_BUNDLE_ADAPTER_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_READINESS_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_RELEASE_ELIGIBLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_PRODUCTION_AVAILABLE_V2);

    let (identity, candidate, signature, _) = valid_parts();
    let owner = UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
        identity, candidate, signature,
    )
    .expect("structural owner binding");
    assert!(matches!(
        fail_closed_registered_platform_p256_boundary_v2(owner),
        Err(RegisteredPlatformP256StatementErrorV2::VerificationUnavailable)
    ));
}

#[test]
fn namespace_privacy_and_terminal_source_guards_hold() {
    let source = include_str!("registered_platform_p256_statement.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod registered_platform_p256_statement;")
            .count(),
        1
    );
    assert!(!parent.contains("pub mod registered_platform_p256_statement"));
    assert!(source.contains(
        "const PLATFORM_MESSAGE_DOMAIN: &[u8] = b\"iroha:offline-cash:v1:helper:platform-message\";"
    ));
    assert!(!source.contains("guard_bundle_digest"));
    assert!(!source.contains("protocol_digest"));
    assert!(!source.contains("halo2_proofs"));
    assert!(!source.contains("verify_prehash"));
    assert!(!source.contains("verify_digest"));
    assert!(!source.contains("register_backend"));
    assert!(!source.contains("impl OfflineCashPairedProofVerifier"));
    assert!(source.contains("enum RegisteredPlatformP256CurrentHelperAuthorityV2 {}"));
    for empty_authority in [
        "enum RegisteredPlatformP256FreshnessAuthorityV2 {}",
        "enum RegisteredPlatformP256CircuitSourceAuthorityV2 {}",
        "enum RegisteredPlatformP256CompiledProtocolAuthorityV2 {}",
        "enum RegisteredPlatformP256ArtifactAuthorityV2 {}",
        "enum RegisteredPlatformP256VerifierAuthorityV2 {}",
        "enum RegisteredPlatformP256GuardBundleAuthorityV2 {}",
        "enum RegisteredPlatformP256WireAdapterAuthorityV2 {}",
    ] {
        assert!(
            source.contains(empty_authority),
            "missing {empty_authority}"
        );
    }
    assert!(source.contains("match authority {}"));
    assert!(source.contains("Result<Infallible, RegisteredPlatformP256StatementErrorV2>"));
    assert!(
        source.contains("Err(RegisteredPlatformP256StatementErrorV2::VerificationUnavailable)")
    );
    assert_eq!(STATE_TERMINAL_CANDIDATE_ORDER_V2.len(), 12);

    let owner = source
        .split_once("pub(super) struct UnverifiedRegisteredPlatformP256OwnerBindingV2 {")
        .and_then(|(_, tail)| {
            tail.split_once("impl UnverifiedRegisteredPlatformP256OwnerBindingV2")
        })
        .map(|(fields, _)| fields)
        .expect("move-only owner remains source-visible");
    assert!(owner.contains(
        "authenticated_current_helper: AuthenticatedRegisteredPlatformCurrentHelperCandidateV2"
    ));
    assert!(
        owner.contains(
            "statement_bytes: Zeroizing<[u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]>"
        )
    );
    assert!(!owner.contains("derive(Clone"));
    assert!(!owner.contains("derive(Copy"));
    assert!(!source.contains("impl Drop for UnverifiedRegisteredPlatformP256OwnerBindingV2"));
    assert!(!source.contains("into_eq_ep_statements"));
    assert!(source.contains("into_eq_ep_source_pair"));

    let authenticated_context = source
        .split_once("pub(super) struct AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {")
        .and_then(|(_, tail)| {
            tail.split_once("impl AuthenticatedRegisteredPlatformCurrentHelperCandidateV2")
        })
        .map(|(fields, _)| fields)
        .expect("move-only authenticated context remains source-visible");
    assert!(!authenticated_context.contains("derive(Clone"));
    assert!(!authenticated_context.contains("derive(Copy"));
    assert!(source.contains(
        "pub(super) const fn durable_identity(&self) -> &DurableRegistrationIdentityProjectionV2"
    ));
    assert!(source.contains(") -> &RegisteredPlatformP256CurrentHelperFieldsV2"));

    let source_pair = source
        .split_once("pub(super) struct UnverifiedRegisteredPlatformP256StatementSourcePairV2 {")
        .and_then(|(_, tail)| {
            tail.split_once("impl UnverifiedRegisteredPlatformP256StatementSourcePairV2")
        })
        .map(|(fields, _)| fields)
        .expect("context-preserving source pair remains source-visible");
    assert!(source_pair.contains(
        "authenticated_current_helper: AuthenticatedRegisteredPlatformCurrentHelperCandidateV2"
    ));
    assert!(source_pair.contains("statements: [UnverifiedRegisteredPlatformP256StatementV2; 2]"));
    assert!(!source_pair.contains("derive(Clone"));
    assert!(!source_pair.contains("derive(Copy"));
    assert!(source.contains("validate_registered_platform_p256_source_pair_context_v2"));
    assert!(!source.contains("into_raw"));
    assert!(!source.contains("into_parts"));

    let message_function = source
        .split_once("fn exact_current_platform_message(")
        .and_then(|(_, tail)| tail.split_once("fn framed_bytes("))
        .map(|(body, _)| body)
        .expect("platform-message reconstruction remains source-visible");
    for field in [
        "&operation",
        "&current.release_id",
        "&current.context_digest",
        "&current.current_head",
        "&current.current_lineage_digest",
        "&current.transition_digest",
        "&current.wallet_binding",
        "&current.hardware_policy_id",
        "&current.guard_device_id",
        "&current.current_guard_binding",
        "&current.next_guard_binding",
        "&from_sequence",
        "&to_sequence",
    ] {
        assert!(message_function.contains(field), "missing {field}");
    }
    assert!(!message_function.contains("receipt_commitment"));
    assert!(!message_function.contains("certificate"));
}
