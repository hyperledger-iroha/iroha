//! Whole ordinary Guard component fixtures with real Ed/P-256 signatures.
//!
//! These public synthetic originals create no installed owner, physical attestation, monetary
//! authorization or release qualification. The production state-protection gates stay closed.

use super::super::super::{
    KagemushaNormalizedGuardStatementV1, KagemushaOperationV1,
    guard_bundle::{KagemushaPlatformCredentialStatementV1, device_authority_commitment_v1},
};
use super::*;
use halo2_proofs::{dev::MockProver, poly::commitment::ParamsProver as _};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalEvidenceV1,
        KagemushaAppOperationApprovalPurposeV1, KagemushaHardwareTransitionSelectionV1,
        KagemushaOperationKindV1, kagemusha_ordinary_app_approval_proof_binding_digest_v1,
        kagemusha_ordinary_financial_authorization_proof_binding_digest_v1,
        kagemusha_ordinary_financial_epoch_id_v1,
    },
    testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

// The actual first-release API accepts a typed marked genesis hash. These are explicit
// synthetic fixture identities; the constructor does not authenticate a running network.
fn fixture_network(bytes: [u8; 32]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::from_marked_bytes(bytes).expect("marked native network fixture bytes"),
    ))
}

struct OriginalFixture {
    relation: KagemushaGuardBundleRelationWitnessV1,
    credential: KagemushaOrdinaryAppCredentialV1,
    approval: KagemushaAppOperationApprovalV1,
    floor: Option<u32>,
    issuer_table: OrdinaryIssuerTableV1,
}

fn sign(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
    apple: bool,
) -> KagemushaAppOperationApprovalEvidenceV1 {
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let message = challenge.canonical_signing_bytes().unwrap();
    if !apple {
        let signature: Signature = key.sign(&message);
        return KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        };
    }
    let mut auth = [0; 37];
    auth[..32].fill(2);
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&17_u32.to_be_bytes());
    let mut nonce = Sha256::new();
    nonce.update(auth);
    nonce.update(Sha256::digest(message));
    // This invokes the same ECDSA-SHA256 message equation as the model's nonce verifier.
    let signature: Signature = key.sign(&nonce.finalize());
    let der = signature.to_der();
    let mut raw = vec![0xa2, 0x71];
    raw.extend_from_slice(b"authenticatorData");
    raw.extend_from_slice(&[0x58, 37]);
    raw.extend(auth);
    raw.push(0x69);
    raw.extend_from_slice(b"signature");
    raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
    raw.extend_from_slice(der.as_bytes());
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
}

fn fixture(apple: bool) -> OriginalFixture {
    fixture_with_apple_release(apple, None)
}
fn fixture_with_apple_release(apple: bool, version: Option<&str>) -> OriginalFixture {
    let financial_secret = [0x41; 32];
    let financial_commitment = device_authority_commitment_v1(financial_secret);
    let fixture = match version {
        Some(version) => {
            KagemushaOrdinaryRetailEnrollmentFixtureV1::measured_apple_with_financial_commitment(
                2,
                version,
                financial_commitment,
            )
            .unwrap()
        }
        None => KagemushaOrdinaryRetailEnrollmentFixtureV1::with_financial_commitment(
            apple,
            financial_commitment,
        ),
    };
    let issuer_table = OrdinaryIssuerTableV1::from_release(&fixture.release).unwrap();
    let preparation = fixture.checked_preparation().unwrap();
    let credential = fixture.selection.issuance.credential.clone();
    // Exercise actual governed issuer verification with the separate financial commitment.
    let verified = credential
        .authenticate(
            fixture.ordinary_policy.identity_policy(),
            &preparation,
            &credential.subject.app_public_key,
            300,
        )
        .unwrap();
    let c = &credential.subject;
    let epoch = kagemusha_ordinary_financial_epoch_id_v1(c).unwrap();
    let provider = fixture.release.provider_policy_root();
    let empty = [11; 32];
    let platform = KagemushaPlatformCredentialStatementV1 {
        version: 1,
        protocol_version: 1,
        suite_id: c.suite_id,
        release_id: c.release_id,
        network_id: c.network_id,
        asset_id: [21; 32],
        asset_incarnation: fixture.selection.owner.runtime.asset_incarnation,
        asset_scale: 2,
        liability_pool_id: [22; 32],
        lane_id: c.lane_id,
        hardware_epoch_generation: u128::from(c.hardware_epoch),
        hardware_epoch_id: epoch,
        key_reference: c.app_key_reference,
        device_public_key: c.app_public_key,
        hardware_policy_id: provider,
        device_authority_commitment: c.financial_authority_commitment,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        platform_class: if apple { 4 } else { 5 },
        capability_mask: c.platform_class.required_guarantees(),
        provider_authority_commitment: [24; 32],
        platform_attestation_digest: c.platform_evidence_digest,
        app_policy_binding_digest: verified.static_binding_digest(),
        credential_issuance_digest: verified.digest(),
        canonical_empty_effect_digest: empty,
        provider_profile_index: 0,
    };
    let relation = KagemushaGuardBundleRelationWitnessV1 {
        statement: KagemushaNormalizedGuardStatementV1 {
            version: 1,
            protocol_version: 1,
            predecessor_suite_id: c.suite_id,
            predecessor_vk_digest: [0x32; 32],
            successor_suite_id: c.suite_id,
            successor_vk_digest: [0x32; 32],
            operation: KagemushaOperationV1::SendSplit,
            amount: 7,
            peer_credit_id: [20; 32],
            recipient_encryption_key_binding: [21; 32],
            mint_finality_proof_binding_digest: [0; 32],
            predecessor_release_id: c.release_id,
            release_id: c.release_id,
            network_id: c.network_id,
            asset_id: platform.asset_id,
            asset_incarnation: platform.asset_incarnation,
            asset_scale: platform.asset_scale,
            liability_pool_id: platform.liability_pool_id,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
            lane_id: c.lane_id,
            predecessor_state_commitment: [12; 32],
            successor_state_commitment: [13; 32],
            predecessor_state_nonce_commitment: [14; 32],
            successor_state_nonce_commitment: [15; 32],
            predecessor_logical_sequence: 10,
            successor_logical_sequence: 11,
            predecessor_hardware_epoch_generation: u128::from(c.hardware_epoch),
            successor_hardware_epoch_generation: u128::from(c.hardware_epoch),
            predecessor_hardware_epoch_id: epoch,
            successor_hardware_epoch_id: epoch,
            predecessor_key_reference: c.app_key_reference,
            successor_key_reference: c.app_key_reference,
            predecessor_hardware_policy_id: provider,
            successor_hardware_policy_id: provider,
            journal_revision_before: 20,
            journal_revision_after: 21,
            lifecycle_binding_digest: [0x33; 32],
            prepared_transition_binding_digest: [0x34; 32],
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: [0; 32],
            receive_credit_binding_digest: [0; 32],
            transition_intent_digest: [16; 32],
            transition_effect_digest: [17; 32],
            recovery_record_digest: [18; 32],
            durable_inbox_effect_digest: empty,
            durable_outbox_effect_digest: [19; 32],
        },
        canonical_empty_effect_digest: empty,
        predecessor_credential: platform,
        successor_credential: platform,
        predecessor_device_authority_secret: financial_secret,
        successor_device_authority_secret: financial_secret,
    };
    relation.validate().unwrap();
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: c.release_id,
        provider_policy_root: provider,
        app_policy_digest: verified.static_binding_digest(),
        credential_id: verified.digest(),
        network_id: fixture_network(c.network_id),
        lane_commitment: c.lane_id,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        hardware_epoch_id: epoch,
        hardware_epoch_generation: c.hardware_epoch,
        operation_kind: KagemushaOperationKindV1::SendSplit,
        transition_statement_digest: [61; 32],
        candidate_envelope_digest: [62; 32],
        terminal_body_commitment: [63; 32],
        secure_index_before: 10,
        secure_index_after: 11,
    };
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: [64; 32],
        nonce: [65; 32],
        account_binding: c.account_binding,
        authority_policy_digest: c.app_authority_policy_digest,
        attested_key_id: c.attested_key_id,
        enrollment_digest: verified.digest(),
        subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap()).into(),
        normalized_guard_digest: relation.statement_digest(),
        issued_at_ms: 300,
        expires_at_ms: 9000,
        subject,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        challenge,
        evidence: version.map_or_else(
            || sign(&challenge, apple),
            |version| sign_measured(&challenge, version, true, true),
        ),
    };
    let floor = apple.then_some(c.app_attest_counter_floor);
    approval
        .authenticate(&challenge, &verified, floor, 301)
        .unwrap();
    OriginalFixture {
        relation,
        credential,
        approval,
        floor,
        issuer_table,
    }
}

fn witness(f: &OriginalFixture) -> OrdinaryGuardWitnessV1<'_> {
    OrdinaryGuardWitnessV1 {
        relation: &f.relation,
        credential: &f.credential,
        approval: &f.approval,
        previous_app_attest_counter: f.floor,
        // This fixture's genuinely signed trust original has no Integrity requirement.
        integrity_lease: None,
    }
}

fn pair_satisfied(f: &OriginalFixture) -> Result<bool, String> {
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (eq, ep) = build_ordinary_app_guard_pair_v1(
        &eq_parameters,
        &ep_parameters,
        witness(f),
        f.relation.statement.successor_hardware_policy_id,
        &f.issuer_table,
    )?;
    assert_eq!(eq.builder.config_params.k, KAGEMUSHA_HALO2_K_V1 as usize);
    assert_eq!(ep.builder.config_params.k, KAGEMUSHA_HALO2_K_V1 as usize);
    let digests = [
        f.relation.statement_digest(),
        f.credential.canonical_digest()?,
        // This signed fixture has no selected Integrity lease; the exact native formatter
        // still binds the zero lease slot in the same authorization column.
        kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            kagemusha_ordinary_app_approval_proof_binding_digest_v1(&f.approval)?,
            None,
        )?,
        Sha256::digest(f.approval.challenge.canonical_subject_signing_bytes()?).into(),
        f.relation.statement.successor_hardware_policy_id,
    ];
    let eq_history =
        initial_kagemusha_eq_accumulator_v1(&eq_parameters).map_err(|e| e.to_string())?;
    let ep_history =
        initial_kagemusha_ep_accumulator_v1(&ep_parameters).map_err(|e| e.to_string())?;
    let eq_public = super::super::super::ordinary_guard_verifier::public_column::<Fp>(
        digests,
        eq_history.as_bytes(),
    );
    let ep_public = super::super::super::ordinary_guard_verifier::public_column::<Fq>(
        digests,
        ep_history.as_bytes(),
    );
    assert_eq!(eq_public.len(), 44);
    assert_eq!(ep_public.len(), 44);
    let eq_pass = MockProver::run(KAGEMUSHA_HALO2_K_V1, &eq, vec![eq_public])
        .map_err(|e| format!("Eq original Guard synthesis: {e:?}"))?
        .verify()
        .is_ok();
    let ep_pass = MockProver::run(KAGEMUSHA_HALO2_K_V1, &ep, vec![ep_public])
        .map_err(|e| format!("Ep original Guard synthesis: {e:?}"))?
        .verify()
        .is_ok();
    assert_eq!(
        eq_pass, ep_pass,
        "Eq and Ep must independently agree on every original or mutation"
    );
    Ok(eq_pass && ep_pass)
}

#[test]
fn complete_android_and_apple_guard_match_native_columns_in_both_pasta_fields() {
    for apple in [false, true] {
        assert!(
            pair_satisfied(&fixture(apple))
                .expect("whole original relation fits actual release k16")
        );
    }
}

#[test]
fn changed_normalized_guard_and_re_signed_foreign_subject_fail_both_pasta_fields() {
    let mut f = fixture(false);
    f.relation.statement.amount += 1;
    assert!(!pair_satisfied(&f).unwrap());
    let mut f = fixture(false);
    f.approval.challenge.subject.network_id = fixture_network([0x71; 32]);
    f.approval.challenge.subject_signing_digest = Sha256::digest(
        f.approval
            .challenge
            .subject
            .canonical_signing_bytes()
            .unwrap(),
    )
    .into();
    f.approval.evidence = sign(&f.approval.challenge, false);
    assert!(!pair_satisfied(&f).unwrap());
}

#[test]
fn changed_original_nonce_and_apple_counter_floor_fail_both_pasta_fields() {
    let mut f = fixture(false);
    f.approval.challenge.nonce[0] ^= 1;
    assert!(!pair_satisfied(&f).unwrap());
    let mut f = fixture(true);
    f.floor = Some(17);
    assert!(!pair_satisfied(&f).unwrap());
}

#[test]
fn foreign_app_key_and_changed_issuer_signature_reject_in_both_parities() {
    let mut f = fixture(false);
    let foreign = SigningKey::from_bytes((&[8; 32]).into()).unwrap();
    let signature: Signature =
        foreign.sign(&f.approval.challenge.canonical_signing_bytes().unwrap());
    f.approval.evidence = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
        signature_der: signature.to_der().as_bytes().to_vec(),
    };
    // The same W under a different actual app key cannot replace the enrolled original key.
    assert!(!pair_satisfied(&f).unwrap());

    let mut f = fixture(false);
    let mut raw = *f.credential.circuit_admission.signature.as_raw_bytes();
    raw[31] ^= 1;
    f.credential.circuit_admission.signature =
        iroha_data_model::kagemusha::KagemushaDeviceSignatureV1::from_raw_bytes(&raw).unwrap();
    // Keep the admission subject and every original Ed field fixed; this changes only the
    // admitted public signature carrier and must fail its actual governed P256 equation.
    assert!(!pair_satisfied(&f).unwrap());
}

#[test]
fn host_original_admission_rejects_platform_scalar_as_financial_opening() {
    for apple in [false, true] {
        let mut f = fixture(apple);
        f.relation.predecessor_device_authority_secret = [7; 32];
        f.relation.successor_device_authority_secret = [7; 32];
        // This is the explicit host original-opening validation boundary, not an
        // assigned-cell mutation or a claim that a failed build executed either circuit.
        assert!(pair_satisfied(&f).is_err());
    }
}

fn cbor(major: u8, bytes: &[u8]) -> Vec<u8> {
    let mut v = if bytes.len() < 24 {
        vec![(major << 5) | bytes.len() as u8]
    } else {
        vec![(major << 5) | 24, bytes.len() as u8]
    };
    v.extend_from_slice(bytes);
    v
}
fn sign_measured(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
    version: &str,
    cat_first: bool,
    auth_first: bool,
) -> KagemushaAppOperationApprovalEvidenceV1 {
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let mut auth = vec![2; 32];
    auth.push(0xc0);
    auth.extend(17u32.to_be_bytes());
    let mut cat = cbor(3, b"validationCategory");
    cat.extend(cbor(2, &2u32.to_le_bytes()));
    let mut ver = cbor(3, b"bundleVersion");
    ver.extend(cbor(3, version.as_bytes()));
    auth.push(0xa2);
    if cat_first {
        auth.extend(cat);
        auth.extend(ver);
    } else {
        auth.extend(ver);
        auth.extend(cat);
    }
    let mut nonce = Sha256::new();
    nonce.update(&auth);
    nonce.update(Sha256::digest(challenge.canonical_signing_bytes().unwrap()));
    let signature: Signature = key.sign(&nonce.finalize());
    let mut a = cbor(3, b"authenticatorData");
    a.extend(cbor(2, &auth));
    let mut sig = cbor(3, b"signature");
    sig.extend(cbor(2, signature.to_der().as_bytes()));
    let mut raw = vec![0xa2];
    if auth_first {
        raw.extend(a);
        raw.extend(sig);
    } else {
        raw.extend(sig);
        raw.extend(a);
    }
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
}
#[test]
fn measured_apple_whole_guard_uses_exact_governed_release_and_full_original_in_both_parities() {
    let version = "é".repeat(64);
    for cat in [false, true] {
        for auth in [false, true] {
            let mut f = fixture_with_apple_release(true, Some(&version));
            f.approval.evidence = sign_measured(&f.approval.challenge, &version, cat, auth);
            let parts = iroha_data_model::kagemusha::kagemusha_ordinary_apple_original_parts_v1(
                match &f.approval.evidence {
                    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                        raw_assertion
                    }
                    _ => unreachable!(),
                },
                f.credential.subject.app_release_digest,
            )
            .unwrap();
            assert_eq!(parts.authenticator_data.len(), 206);
            assert_eq!(parts.bundle_version, Some(version.as_str()));
            assert!(pair_satisfied(&f).unwrap());
        }
    }
}

#[test]
fn shared_platform_guard_accepts_genuine_preparation_original_in_both_parities() {
    for apple in [false, true] {
        let mut f = fixture(apple);
        f.approval.challenge.purpose = KagemushaAppOperationApprovalPurposeV1::PrepareTransition;
        f.approval.challenge.subject.candidate_envelope_digest = [0; 32];
        f.approval.challenge.subject.terminal_body_commitment = [0; 32];
        f.approval.challenge.subject_signing_digest = Sha256::digest(
            f.approval
                .challenge
                .canonical_subject_signing_bytes()
                .unwrap(),
        )
        .into();
        f.approval.evidence = sign(&f.approval.challenge, apple);
        // Shared crypto proves both canonical purposes. This proof is not a selected Native
        // owner; the State and terminal consumers select their one exact signed purpose.
        let bytes = f.approval.challenge.canonical_signing_bytes().unwrap();
        assert_eq!(bytes[A::PURPOSE.start], 2);
        assert_eq!(
            f.approval
                .evidence
                .authenticate_signature(
                    f.credential.subject.platform_class,
                    &f.credential.subject.app_public_key,
                    f.credential.subject.app_signing_identity_digest,
                    f.credential.subject.app_release_digest,
                    f.floor,
                    &bytes,
                )
                .unwrap()
                .0,
            if apple { Some(17) } else { None },
        );
        assert!(pair_satisfied(&f).unwrap());
        // The genuine purpose2 signature cannot approve another message by changing only its
        // purpose byte. This assertion exercises actual platform crypto, not a Native grant.
        let mut terminal_bytes = bytes;
        terminal_bytes[A::PURPOSE.start] = 1;
        assert!(
            f.approval
                .evidence
                .authenticate_signature(
                    f.credential.subject.platform_class,
                    &f.credential.subject.app_public_key,
                    f.credential.subject.app_signing_identity_digest,
                    f.credential.subject.app_release_digest,
                    f.floor,
                    &terminal_bytes,
                )
                .is_err()
        );
    }
}

#[test]
fn original_guard_preserves_secure_indexes_independent_of_logical_sequence() {
    for apple in [false, true] {
        let mut original = fixture(apple);
        assert_eq!(original.relation.statement.predecessor_logical_sequence, 10);
        assert_eq!(original.relation.statement.successor_logical_sequence, 11);
        original.approval.challenge.subject.secure_index_before = 41;
        original.approval.challenge.subject.secure_index_after = 42;
        original.approval.challenge.subject_signing_digest = Sha256::digest(
            original
                .approval
                .challenge
                .canonical_subject_signing_bytes()
                .unwrap(),
        )
        .into();
        original.approval.evidence = sign(&original.approval.challenge, apple);
        assert!(
            pair_satisfied(&original)
                .expect("both complete Guard parities bind the genuine independent-index original")
        );
    }
}
