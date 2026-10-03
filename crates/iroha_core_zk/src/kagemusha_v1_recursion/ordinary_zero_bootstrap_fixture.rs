//! Bare public mathematical originals. No release, FI, phone or Native owner is admitted.

use crate::kagemusha_v1_poseidon::{
    KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonFieldV1, digest_limbs, empty_replay_root, encode,
    from_u128, hash, paired_commitment,
};
use crate::kagemusha_v1_recursion::KagemushaGuardBundleRelationWitnessV1;
use crate::kagemusha_v1_state::{BootstrapStatementV1, DigestV1, KagemushaStateV1};
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_crypto::{Algorithm, KeyPair, Signature as EdSignature};
use iroha_data_model::kagemusha::*;
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

pub(in super::super) struct Fixture {
    pub(in super::super) state: KagemushaStateV1,
    // Keep the complete constructed Bootstrap statement with its original fixture owner.
    pub(in super::super) _statement: BootstrapStatementV1,
    pub(in super::super) relation: KagemushaGuardBundleRelationWitnessV1,
    pub(in super::super) credential: KagemushaOrdinaryAppCredentialV1,
    pub(in super::super) approval: KagemushaAppOperationApprovalV1,
    pub(in super::super) issuer_table:
        super::super::super::ordinary_issuer_config::OrdinaryIssuerTableV1,
    pub(in super::super) previous_counter: Option<u32>,
}

pub(in super::super) fn resign_credential(c: &mut KagemushaOrdinaryAppCredentialV1) {
    // Known public fixture keys re-sign a foreign complete original. This ensures
    // substitution reaches the real relation rather than failing a data-only codec.
    let ed = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
    c.signature = EdSignature::try_new(
        ed.private_key(),
        &c.subject.canonical_signing_bytes().unwrap(),
    )
    .unwrap();
    let subject =
        KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(&c.subject, &c.signature)
            .unwrap();
    let issuer = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
    let signature: P256Signature = issuer.sign(&subject.canonical_signing_bytes().unwrap());
    let signature = signature.normalize_s().unwrap_or(signature);
    c.circuit_admission = KagemushaOrdinaryIssuerCircuitAdmissionV1 {
        subject,
        signature: KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .unwrap(),
    };
    c.canonical_bytes().unwrap();
}

fn sha_original(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn parity_commitment<F: KagemushaPoseidonFieldV1>(s: &KagemushaStateV1) -> F {
    let mut cells = vec![
        F::from(u64::from(s.version)),
        F::from(u64::from(s.protocol_version)),
    ];
    for d in [
        s.suite_id,
        s.vk_digest,
        s.release_id,
        *s.asset_incarnation.as_bytes(),
        s.liability_pool_id,
        s.hardware_profile_id,
    ] {
        cells.extend(digest_limbs::<F>(d));
    }
    cells.push(F::from(s.policy_epoch));
    cells.extend(digest_limbs::<F>(s.lane.normalized_network_id()));
    cells.extend(digest_limbs::<F>(s.lane.normalized_asset_id().unwrap()));
    cells.push(F::from(u64::from(s.lane.scale)));
    cells.extend(digest_limbs::<F>(s.lane.device_lane_id));
    cells.extend([
        from_u128::<F>(s.balance),
        from_u128::<F>(s.logical_sequence),
        from_u128::<F>(s.secure_index),
        from_u128::<F>(s.hardware_epoch.generation),
    ]);
    for d in [
        s.hardware_epoch.epoch_id,
        s.device_policy_binding.device_key_reference,
        s.device_policy_binding.hardware_policy_id,
        s.next_one_use_key_reference,
        s.state_nonce_commitment,
    ] {
        cells.extend(digest_limbs::<F>(d));
    }
    cells.push(empty_replay_root::<F>());
    hash(KAGEMUSHA_STATE_DOMAIN_V1, &cells)
}

fn seal_state(s: &mut KagemushaStateV1) {
    s.consumed_credit_root = KagemushaPastaStateCommitmentV1 {
        eq: encode(empty_replay_root::<Fp>()),
        ep: encode(empty_replay_root::<Fq>()),
    };
    let (components, head) =
        paired_commitment(parity_commitment::<Fp>(s), parity_commitment::<Fq>(s));
    s.state_commitment_components = components;
    s.state_commitment = head;
    s.validate()
        .expect("actual complete Native zero-State commitments");
}

pub(in super::super) fn sign_approval(
    c: &KagemushaAppOperationApprovalChallengeV1,
    apple: bool,
) -> KagemushaAppOperationApprovalEvidenceV1 {
    let app = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let message = c.canonical_signing_bytes().unwrap();
    if !apple {
        let s: P256Signature = app.sign(&message);
        let s = s.normalize_s().unwrap_or(s);
        return KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: s.to_der().as_bytes().to_vec(),
        };
    }
    let mut auth = [0; 37];
    auth[..32].fill(2);
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&17u32.to_be_bytes());
    let mut nonce = Sha256::new();
    nonce.update(auth);
    nonce.update(Sha256::digest(message));
    let s: P256Signature = app.sign(&nonce.finalize());
    let s = s.normalize_s().unwrap_or(s);
    let der = s.to_der();
    let mut raw = vec![0xa2, 0x71];
    raw.extend_from_slice(b"authenticatorData");
    raw.extend_from_slice(&[0x58, 37]);
    raw.extend_from_slice(&auth);
    raw.push(0x69);
    raw.extend_from_slice(b"signature");
    raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
    raw.extend_from_slice(der.as_bytes());
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
}

pub(in super::super) fn fixture(
    apple: bool,
    release_id: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
) -> Fixture {
    fixture_with_account_binding(apple, release_id, suite_id, vk_digest, [15; 32])
}

/// Same bare mathematical credential/State fixture with the sole account binding formula.
/// This supplies no release, enrollment, FI, source finality or Native owner capability.
pub(in super::super) fn fixture_for_account(
    apple: bool,
    release_id: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
    account: &iroha_data_model::account::AccountId,
) -> Fixture {
    fixture_with_account_binding(
        apple,
        release_id,
        suite_id,
        vk_digest,
        kagemusha_ordinary_app_account_binding_v1(account),
    )
}

fn fixture_with_account_binding(
    apple: bool,
    release_id: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
    account_binding: DigestV1,
) -> Fixture {
    use super::super::super::ordinary_issuer_config::{
        OrdinaryIssuerProfileV1, OrdinaryIssuerTableV1,
    };
    let mut state = crate::kagemusha_v1_recursion::tests::state_verification_fixture()
        .0
        .successor;
    state.release_id = release_id;
    state.suite_id = suite_id;
    state.vk_digest = vk_digest;
    let app = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let app_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        app.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap();
    let financial_secret = [0x41; 32];
    let subject = KagemushaOrdinaryAppCredentialSubjectV1 {
        version: 1,
        platform_class: if apple {
            KagemushaHardwarePlatformClassV1::AppleAppAttest
        } else {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint
        },
        security_level: if apple {
            KagemushaAppKeySecurityLevelV1::AppleAppAttest
        } else {
            KagemushaAppKeySecurityLevelV1::StrongBox
        },
        enrollment_id: [12; 32],
        client_nonce: [13; 32],
        server_nonce: [14; 32],
        account_binding,
        network_id: state.lane.normalized_network_id(),
        lane_id: state.lane.device_lane_id,
        release_id,
        hardware_profile_id: state.hardware_profile_id,
        suite_id,
        trust_policy_digest: [16; 32],
        app_authority_policy_digest: [17; 32],
        app_signing_identity_digest: [2; 32],
        app_release_digest: [3; 32],
        attested_key_id: Sha256::digest(app_public_key.as_sec1_bytes()).into(),
        app_key_reference: kagemusha_device_key_reference_v1(&app_public_key),
        financial_authority_commitment:
            super::super::super::guard_bundle::device_authority_commitment_v1(financial_secret),
        platform_evidence_digest: [18; 32],
        enrollment_challenge_digest: [19; 32],
        app_public_key,
        policy_epoch: state.policy_epoch,
        hardware_epoch: 1,
        issued_at_ms: 100,
        expires_at_ms: 20000,
        app_attest_counter_floor: if apple { 5 } else { 0 },
        play_integrity: None,
    };
    let ed = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
    let signature = EdSignature::try_new(
        ed.private_key(),
        &subject.canonical_signing_bytes().unwrap(),
    )
    .unwrap();
    let admission_subject =
        KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(&subject, &signature)
            .unwrap();
    let issuer = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
    let sig: P256Signature = issuer.sign(&admission_subject.canonical_signing_bytes().unwrap());
    let sig = sig.normalize_s().unwrap_or(sig);
    let credential = KagemushaOrdinaryAppCredentialV1 {
        subject,
        signature,
        circuit_admission: KagemushaOrdinaryIssuerCircuitAdmissionV1 {
            subject: admission_subject,
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(sig.to_bytes().as_ref()).unwrap(),
        },
    };
    let credential_digest = sha_original(
        b"iroha:kagemusha:v1:ordinary-app-credential-original\0",
        &credential.canonical_bytes().unwrap(),
    );
    let c = &credential.subject;
    let static_body = [
        c.account_binding,
        c.network_id,
        c.lane_id,
        c.release_id,
        c.hardware_profile_id,
        c.suite_id,
        c.trust_policy_digest,
        c.app_authority_policy_digest,
        c.app_signing_identity_digest,
        c.app_release_digest,
        c.attested_key_id,
        c.app_key_reference,
        c.financial_authority_commitment,
    ]
    .concat();
    let static_binding = sha_original(
        b"iroha:kagemusha:v1:ordinary-app-static-binding\0",
        &static_body,
    );
    state.hardware_epoch.epoch_id = kagemusha_ordinary_financial_epoch_id_v1(c).unwrap();
    state.device_policy_binding.device_key_reference = c.app_key_reference;
    seal_state(&mut state);
    let statement = BootstrapStatementV1 {
        version: 1,
        protocol_version: 1,
        suite_id,
        vk_digest,
        release_id,
        asset_incarnation: state.asset_incarnation,
        liability_pool_id: state.liability_pool_id,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        lane: state.lane.clone(),
        hardware_epoch: state.hardware_epoch,
        device_policy_binding: state.device_policy_binding,
        next_one_use_key_reference: [0; 32],
        state_nonce_commitment: state.state_nonce_commitment,
        state_commitment: state.state_commitment,
    };
    let empty = [11; 32];
    let normalized =
        super::super::super::KagemushaNormalizedGuardStatementV1::from_bootstrap_state(
            &statement,
            super::super::super::KagemushaGuardContextV1 {
                release_id,
                liability_pool_id: state.liability_pool_id,
                lifecycle_binding_digest: [24; 32],
                prepared_transition_binding_digest: [0; 32],
                terminal_commit_binding_digest: [0; 32],
                sender_one_time_authorization_digest: [0; 32],
                receive_credit_binding_digest: [0; 32],
                transition_intent_digest: [25; 32],
                transition_effect_digest: [26; 32],
                recovery_record_digest: [27; 32],
                durable_inbox_effect_digest: empty,
                durable_outbox_effect_digest: empty,
                canonical_empty_effect_digest: empty,
            },
        )
        .unwrap();
    let platform = super::super::super::guard_bundle::KagemushaPlatformCredentialStatementV1 {
        version: 1,
        protocol_version: 1,
        suite_id,
        release_id,
        network_id: c.network_id,
        asset_id: state.lane.normalized_asset_id().unwrap(),
        asset_incarnation: state.asset_incarnation,
        asset_scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        lane_id: c.lane_id,
        hardware_epoch_generation: 1,
        hardware_epoch_id: state.hardware_epoch.epoch_id,
        key_reference: c.app_key_reference,
        device_public_key: c.app_public_key,
        hardware_policy_id: state.device_policy_binding.hardware_policy_id,
        device_authority_commitment: c.financial_authority_commitment,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        platform_class: if apple { 4 } else { 5 },
        capability_mask: c.platform_class.required_guarantees(),
        provider_authority_commitment: [28; 32],
        platform_attestation_digest: c.platform_evidence_digest,
        app_policy_binding_digest: static_binding,
        credential_issuance_digest: credential_digest,
        canonical_empty_effect_digest: empty,
        provider_profile_index: 0,
    };
    let relation = KagemushaGuardBundleRelationWitnessV1 {
        statement: normalized,
        canonical_empty_effect_digest: empty,
        predecessor_credential: platform,
        successor_credential: platform,
        predecessor_device_authority_secret: financial_secret,
        successor_device_authority_secret: financial_secret,
    };
    relation.validate().unwrap();
    let selection = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id,
        provider_policy_root: state.device_policy_binding.hardware_policy_id,
        app_policy_digest: static_binding,
        credential_id: credential_digest,
        network_id: state.lane.network_id,
        lane_commitment: c.lane_id,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        hardware_epoch_id: state.hardware_epoch.epoch_id,
        hardware_epoch_generation: 1,
        operation_kind: KagemushaOperationKindV1::Bootstrap,
        transition_statement_digest: statement.proof_statement_digest().unwrap(),
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: 0,
        secure_index_after: 0,
    };
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: [31; 32],
        nonce: [32; 32],
        account_binding: c.account_binding,
        authority_policy_digest: c.app_authority_policy_digest,
        attested_key_id: c.attested_key_id,
        enrollment_digest: credential_digest,
        subject_signing_digest: Sha256::digest(selection.canonical_signing_bytes().unwrap()).into(),
        normalized_guard_digest: relation.statement_digest(),
        issued_at_ms: 300,
        expires_at_ms: 9000,
        subject: selection,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        challenge,
        evidence: sign_approval(&challenge, apple),
    };
    let mut issuer_table = OrdinaryIssuerTableV1::default();
    issuer_table.slots[0] = OrdinaryIssuerProfileV1 {
        profile_id: c.hardware_profile_id,
        issuer_sec1: issuer
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .try_into()
            .unwrap(),
    };
    Fixture {
        state,
        _statement: statement,
        relation,
        credential,
        approval,
        issuer_table,
        previous_counter: apple.then_some(5),
    }
}
