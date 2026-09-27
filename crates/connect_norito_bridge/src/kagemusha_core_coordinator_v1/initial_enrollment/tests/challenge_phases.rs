//! Consuming enrollment phases retain native pins, exact signing bytes and one deadline.
//! Fixed test signers exercise cryptographic bindings; they confer no hardware qualification.

use super::super::super::signed_app_preparation::{
    SignedAppPreparationPinsV1, verify_signed_app_preparation_v1,
};
use super::*;

fn preparation_fixture() -> Fixture {
    let mut f = Fixture::new();
    Arc::get_mut(&mut f.policy).unwrap().expires_at_ms = 200_000;
    f
}

fn signed_preparation_pins<'a>(
    f: &'a Fixture,
    platform_class: KagemushaHardwarePlatformClassV1,
    trusted_now_ms: u64,
) -> SignedAppPreparationPinsV1<'a> {
    SignedAppPreparationPinsV1 {
        policy: &f.policy,
        account_id: &f.owner.account_id,
        platform_class,
        selected_attested_key_id: match platform_class {
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                Sha256::digest(f.qualification.credential.device_public_key.as_sec1_bytes()).into()
            }
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => [0; 32],
            _ => panic!("test only prepares ordinary apps"),
        },
        client_nonce: [90; 32],
        release_id: f.release.release_id(),
        profile_id: f.qualification.credential.hardware_profile_id,
        lane_id: f.owner.lane_id,
        trusted_now_ms,
    }
}

fn sign_preparation_body(f: &Fixture, token: &mut Vec<u8>) {
    token.truncate(209);
    let mut message = b"iroha:kagemusha:v1:app-enrollment-preparation\0".to_vec();
    message.extend_from_slice(&token[1..209]);
    message.extend_from_slice(&f.policy.issuer_policy_id);
    message.extend_from_slice(&Sha256::digest(f.owner.account_id.to_string().as_bytes()));
    let signature = Signature::try_new(f.issuer.private_key(), &message).unwrap();
    token.extend_from_slice(signature.payload());
    assert_eq!(token.len(), 273);
}

fn signed_preparation_token(
    f: &Fixture,
    platform_class: KagemushaHardwarePlatformClassV1,
) -> Vec<u8> {
    let mut token = Vec::with_capacity(273);
    token.push(1);
    token.extend_from_slice(&1_000_u64.to_le_bytes());
    token.extend_from_slice(&121_000_u64.to_le_bytes());
    token.extend_from_slice(&[90; 32]);
    token.extend_from_slice(&[91; 32]);
    token.extend_from_slice(&f.release.release_id());
    token.extend_from_slice(&f.qualification.credential.hardware_profile_id);
    token.extend_from_slice(&match platform_class {
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            Sha256::digest(f.qualification.credential.device_public_key.as_sec1_bytes()).into()
        }
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => [0; 32],
        _ => panic!("test only prepares ordinary apps"),
    });
    token.extend_from_slice(&f.owner.lane_id);
    sign_preparation_body(f, &mut token);
    token
}

#[test]
fn signed_app_preparation_accepts_exact_apple_key_id_and_android_zero_sentinel() {
    let f = preparation_fixture();
    for platform in [
        KagemushaHardwarePlatformClassV1::AppleAppAttest,
        KagemushaHardwarePlatformClassV1::AndroidKeyMint,
    ] {
        let token = signed_preparation_token(&f, platform);
        let verified =
            verify_signed_app_preparation_v1(&token, signed_preparation_pins(&f, platform, 1_000))
                .unwrap();
        assert_eq!(verified.server_nonce, [91; 32]);
        assert_eq!(verified.client_nonce, [90; 32]);
        assert_eq!(verified.release_id, f.release.release_id());
        assert_eq!(
            verified.profile_id,
            f.qualification.credential.hardware_profile_id
        );
        assert_eq!(verified.lane_id, f.owner.lane_id);
        assert_eq!(verified.issued_at_ms, 1_000);
        assert_eq!(verified.expires_at_ms, 121_000);
        assert_eq!(token.len(), 273);
    }
}

#[test]
fn signed_app_preparation_rejects_field_account_policy_and_signature_substitution() {
    let f = preparation_fixture();
    let apple = KagemushaHardwarePlatformClassV1::AppleAppAttest;
    let token = signed_preparation_token(&f, apple);
    for offset in [17, 81, 113, 145, 177] {
        let mut changed = token.clone();
        changed[offset] ^= 1;
        sign_preparation_body(&f, &mut changed);
        assert!(
            verify_signed_app_preparation_v1(&changed, signed_preparation_pins(&f, apple, 1_000),)
                .is_err()
        );
    }
    for offset in [49, 209, 272] {
        let mut changed = token.clone();
        changed[offset] ^= 1;
        assert!(
            verify_signed_app_preparation_v1(&changed, signed_preparation_pins(&f, apple, 1_000),)
                .is_err()
        );
    }
    let other_account = AccountId::new(
        KeyPair::from_seed(vec![111; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let mut account_pins = signed_preparation_pins(&f, apple, 1_000);
    account_pins.account_id = &other_account;
    assert!(verify_signed_app_preparation_v1(&token, account_pins).is_err());
    let mut other_policy = (*f.policy).clone();
    other_policy.issuer_policy_id[0] ^= 1;
    let mut policy_pins = signed_preparation_pins(&f, apple, 1_000);
    policy_pins.policy = &other_policy;
    assert!(verify_signed_app_preparation_v1(&token, policy_pins).is_err());
    let mut wrong_signer = (*f.policy).clone();
    wrong_signer.issuer_public_key = KeyPair::from_seed(vec![112; 32], Algorithm::Ed25519)
        .public_key()
        .clone();
    let mut signer_pins = signed_preparation_pins(&f, apple, 1_000);
    signer_pins.policy = &wrong_signer;
    assert!(verify_signed_app_preparation_v1(&token, signer_pins).is_err());
}

#[test]
fn signed_app_preparation_rejects_wrong_platform_key_id_and_time_interval() {
    let f = preparation_fixture();
    let apple = KagemushaHardwarePlatformClassV1::AppleAppAttest;
    let android = KagemushaHardwarePlatformClassV1::AndroidKeyMint;
    let apple_token = signed_preparation_token(&f, apple);
    let android_token = signed_preparation_token(&f, android);
    let mut substituted_key = signed_preparation_pins(&f, apple, 1_000);
    substituted_key.selected_attested_key_id[0] ^= 1;
    assert!(verify_signed_app_preparation_v1(&apple_token, substituted_key).is_err());
    let mut android_nonzero = signed_preparation_pins(&f, android, 1_000);
    android_nonzero.selected_attested_key_id = [1; 32];
    assert!(verify_signed_app_preparation_v1(&android_token, android_nonzero).is_err());
    assert!(verify_signed_app_preparation_v1(
        &apple_token,
        signed_preparation_pins(&f, android, 1_000),
    )
    .is_err());
    assert!(verify_signed_app_preparation_v1(
        &android_token,
        signed_preparation_pins(&f, apple, 1_000),
    )
    .is_err());
    for now in [999, 121_000] {
        assert!(verify_signed_app_preparation_v1(
            &apple_token,
            signed_preparation_pins(&f, apple, now),
        )
        .is_err());
    }
    for (offset, value) in [(1, 0_u64), (9, 120_999_u64), (9, u64::MAX)] {
        let mut changed = apple_token.clone();
        changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        sign_preparation_body(&f, &mut changed);
        assert!(
            verify_signed_app_preparation_v1(&changed, signed_preparation_pins(&f, apple, 1_000),)
                .is_err()
        );
    }
    let mut zero_server = apple_token.clone();
    zero_server[49..81].fill(0);
    sign_preparation_body(&f, &mut zero_server);
    assert!(
        verify_signed_app_preparation_v1(&zero_server, signed_preparation_pins(&f, apple, 1_000),)
            .is_err()
    );
    let mut repeated_server = apple_token.clone();
    repeated_server[49..81].copy_from_slice(&[90; 32]);
    sign_preparation_body(&f, &mut repeated_server);
    assert!(
        verify_signed_app_preparation_v1(
            &repeated_server,
            signed_preparation_pins(&f, apple, 1_000),
        )
        .is_err()
    );
    for truncated in [&apple_token[..272], &apple_token[..209]] {
        assert!(
            verify_signed_app_preparation_v1(truncated, signed_preparation_pins(&f, apple, 1_000),)
                .is_err()
        );
    }
    let mut wrong_version = apple_token;
    wrong_version[0] = 2;
    assert!(verify_signed_app_preparation_v1(
        &wrong_version,
        signed_preparation_pins(&f, apple, 1_000),
    )
    .is_err());
}

fn projection<'a>(
    challenge: &KagemushaRetailEnrollmentChallengeV1,
    command: &'a [u8],
) -> IssuerChallengeProjectionV1<'a> {
    IssuerChallengeProjectionV1 {
        challenge_id: challenge.device_request_id().unwrap(),
        account_signing_message: challenge.account_signing_message().unwrap(),
        device_request_id: challenge.device_request_id().unwrap(),
        canonical_device_command: command,
        expires_at_ms: challenge.expires_at_ms,
    }
}

fn accept(
    f: &Fixture,
    pending: PendingIssuerEnrollmentV1,
    challenge: &KagemushaRetailEnrollmentChallengeV1,
) -> AcceptedIssuerChallengeV1 {
    let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
    pending
        .accept_challenge(
            &challenge.canonical_bytes().unwrap(),
            projection(challenge, &command),
            f.verified_app(challenge),
        )
        .unwrap()
}

fn account_signature(proof: &KagemushaRetailEnrollmentPossessionProofV1) -> Vec<u8> {
    Signature::from(proof.account_signature.clone())
        .payload()
        .to_vec()
}

fn prepare(
    f: &Fixture,
) -> (
    PreparedIssuerProofV1,
    KagemushaRetailEnrollmentPossessionProofV1,
) {
    let pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let prepared = accept(f, pending, &proof.challenge)
        .prepare_proof(&account_signature(&proof), &proof.device_response)
        .unwrap();
    (prepared, proof)
}

fn resign_credential(f: &mut Fixture) {
    let profile = f.qualification.profile;
    f.qualification.credential = f.qualification.credential.seal_credential_id().unwrap();
    let seed = profile.provider_id[0].wrapping_add(5);
    let governance = SigningKey::from_bytes((&[seed; 32]).into()).unwrap();
    let signature: p256::ecdsa::Signature = governance.sign(
        &f.qualification
            .credential
            .canonical_signing_bytes()
            .unwrap(),
    );
    f.qualification.credential.governance_signature = KagemushaDeviceSignatureV1::from_raw_bytes(
        &signature.normalize_s().unwrap_or(signature).to_bytes(),
    )
    .unwrap();
    f.qualification
        .credential
        .validate_against_profile(&profile)
        .unwrap();
}

#[test]
fn phases_retain_exact_qualification_signing_inputs_proof_and_issuer_response() {
    let f = Fixture::new();
    let pending = f.begin();
    let nonce = pending.client_nonce().unwrap();
    assert_eq!(
        pending.canonical_qualification().unwrap(),
        norito::encode_canonical(&f.qualification).unwrap()
    );
    let proof = f.proof(nonce);
    let accepted = accept(&f, pending, &proof.challenge);
    assert_eq!(
        accepted.account_signing_message().unwrap(),
        proof.challenge.account_signing_message().unwrap()
    );
    assert_eq!(
        accepted.device_request_id().unwrap(),
        proof.challenge.device_request_id().unwrap()
    );
    assert_eq!(
        accepted.canonical_device_command().unwrap(),
        KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap()
    );
    let prepared = accepted
        .prepare_proof(&account_signature(&proof), &proof.device_response)
        .unwrap();
    let expected_proof = proof.canonical_bytes().unwrap();
    for _ in 0..3 {
        assert_eq!(prepared.canonical_proof().unwrap(), expected_proof);
        assert_eq!(
            prepared.challenge_id().unwrap(),
            proof.challenge.device_request_id().unwrap()
        );
        assert_eq!(
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(
                prepared.canonical_proof().unwrap()
            )
            .unwrap()
            .canonical_evidence_digest()
            .unwrap(),
            proof.canonical_evidence_digest().unwrap()
        );
    }
    let certificate = f.certificate(&proof);
    let certificate_bytes = certificate.canonical_bytes().unwrap();
    let admission = prepared.complete(&certificate_bytes).unwrap();
    assert_eq!(admission.canonical_proof(), expected_proof);
    assert_eq!(admission.canonical_certificate(), certificate_bytes);
    assert_eq!(admission.evidence().client_nonce(), nonce);
    assert_eq!(admission.evidence().certificate(), &certificate);
    // Historical fixture times are deliberately accepted; these phases never claim UTC now.
    assert_eq!(
        admission.evidence().certificate().subject.issued_at_ms,
        1000
    );
}

#[test]
fn begin_rejects_self_consistent_qualification_outside_native_owner_and_release() {
    for mutation in 0..6 {
        let mut f = Fixture::new();
        match mutation {
            0 => f.owner.lane_id = [88; 32],
            1 => {
                f.qualification.credential.network_id = NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other-qualification-network")),
                );
                resign_credential(&mut f);
            }
            2 => f.qualification.release_id = [88; 32],
            3 => f.qualification.hardware_policy_digest = [88; 32],
            4 => f.qualification.core_authorization_key_reference = [88; 32],
            _ => {
                // Reseal and sign a valid embedded profile/credential chain. Its own
                // consistency cannot substitute for inclusion in the authenticated release.
                f.qualification.profile.qualification_report_digest = [88; 32];
                f.qualification.profile =
                    f.qualification.profile.seal_hardware_profile_id().unwrap();
                f.qualification.credential.hardware_profile_id =
                    f.qualification.profile.hardware_profile_id;
                resign_credential(&mut f);
            }
        }
        let bytes = norito::encode_canonical(&f.qualification).unwrap();
        KagemushaDeviceQualificationReplyV1::decode_canonical_exact(&bytes).unwrap();
        assert_eq!(
            PendingIssuerEnrollmentV1::begin(
                f.policy.clone(),
                f.app_policy.clone(),
                f.release.clone(),
                f.owner.clone(),
                f.native_key,
                &bytes,
            )
            .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "qualification mutation {mutation}"
        );
    }
}

#[test]
fn begin_rejects_noncanonical_oversized_or_unsigned_qualification() {
    let f = Fixture::new();
    for mutation in 0..4 {
        let mut qualification = f.qualification.clone();
        if mutation == 3 {
            qualification.credential.governance_signature =
                KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap();
        }
        let mut bytes = norito::encode_canonical(&qualification).unwrap();
        match mutation {
            0 => bytes.clear(),
            1 => bytes.push(0),
            2 => bytes = vec![0; 2049],
            _ => {}
        }
        assert_eq!(
            PendingIssuerEnrollmentV1::begin(
                f.policy.clone(),
                f.app_policy.clone(),
                f.release.clone(),
                f.owner.clone(),
                f.native_key,
                &bytes,
            )
            .err(),
            Some(InitialEnrollmentErrorV1::Encoding)
        );
    }
}

#[test]
fn challenge_cannot_replace_any_retained_native_selection() {
    for mutation in 0..10 {
        let mut f = Fixture::new();
        let pending = f.begin();
        let nonce = pending.client_nonce().unwrap();
        if mutation == 8 {
            f.qualification.credential.hardware_epoch_id = [88; 32];
            resign_credential(&mut f);
        } else if mutation == 9 {
            f.qualification.credential.hardware_epoch_generation += 1;
            resign_credential(&mut f);
        }
        let mut challenge = f.proof(nonce).challenge;
        match mutation {
            0 => challenge.client_nonce[0] ^= 1,
            1 => {
                challenge.owner.account_id = AccountId::new(
                    KeyPair::from_seed(vec![14; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                );
            }
            2 => challenge.owner.runtime.authentication_namespace = "other".parse().unwrap(),
            3 => challenge.issuer_policy_id = [88; 32],
            4 => challenge.issuer_audience = "other-enrollment".parse().unwrap(),
            5 => challenge.issuance.release_id = [88; 32],
            6 => challenge.issuance.hardware_policy_digest = [88; 32],
            7 => challenge.issuance.core_authorization_key_reference = [88; 32],
            _ => {}
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        assert_eq!(
            pending
                .accept_challenge(
                    &challenge.canonical_bytes().unwrap(),
                    projection(&challenge, &command),
                    f.verified_app(&challenge),
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "challenge mutation {mutation}"
        );
    }
}

#[test]
fn challenge_rejects_a_substituted_or_replayed_verified_app_result() {
    let f = Fixture::new();
    for mutation in 0..3 {
        let pending = f.begin();
        let challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        let mut other = challenge.clone();
        let token = match mutation {
            0 => {
                other.server_nonce[0] ^= 1;
                f.verified_app(&other)
            }
            1 => {
                other.client_nonce[0] ^= 1;
                f.verified_app(&other)
            }
            _ => f.verified_app(&challenge),
        };
        let mut presented = challenge;
        if mutation == 2 {
            presented.app_attestation_digest[0] ^= 1;
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        assert_eq!(
            pending
                .accept_challenge(
                    &presented.canonical_bytes().unwrap(),
                    projection(&presented, &command),
                    token,
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "app binding mutation {mutation}"
        );
    }
}

#[test]
fn challenge_requires_the_independently_pinned_app_authority_policy() {
    let f = Fixture::new();
    for mutation in 0..4 {
        let mut pending = f.begin();
        let challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        let token = f.verified_app(&challenge);
        let pinned = Arc::make_mut(&mut pending.app_policy);
        match mutation {
            0 => {
                pinned.authority_key = KeyPair::from_seed(vec![78; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            }
            1 => pinned.app_signing_identity_digest = [79; 32],
            2 => pinned.app_release_digest = [80; 32],
            _ => pinned.maximum_lifetime_ms += 1,
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        assert_eq!(
            pending
                .accept_challenge(
                    &challenge.canonical_bytes().unwrap(),
                    projection(&challenge, &command),
                    token,
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "authority policy mutation {mutation}"
        );
    }
}

#[test]
fn each_server_projection_must_equal_its_local_derivation() {
    let f = Fixture::new();
    for mutation in 0..5 {
        let pending = f.begin();
        let challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        let mut changed_command = command.clone();
        changed_command.push(0);
        let mut projected = projection(&challenge, &command);
        match mutation {
            0 => projected.challenge_id[0] ^= 1,
            1 => projected.account_signing_message[0] ^= 1,
            2 => projected.device_request_id[0] ^= 1,
            3 => projected.canonical_device_command = &changed_command,
            _ => projected.expires_at_ms += 1,
        }
        assert_eq!(
            pending
                .accept_challenge(
                    &challenge.canonical_bytes().unwrap(),
                    projected,
                    f.verified_app(&challenge),
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "projection mutation {mutation}"
        );
    }
}

#[test]
fn challenge_interval_must_fit_both_policy_and_credential_without_a_utc_claim() {
    for mutation in 0..4 {
        let mut f = Fixture::new();
        match mutation {
            0 => Arc::make_mut(&mut f.policy).valid_from_ms = 1500,
            1 => Arc::make_mut(&mut f.policy).expires_at_ms = 6000,
            3 => {
                f.qualification.credential.expires_at_ms = 5000;
                resign_credential(&mut f);
            }
            _ => {}
        }
        let pending = f.begin();
        let mut challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        match mutation {
            0 => {}
            1 => challenge.expires_at_ms = 6500,
            2 => challenge.issued_at_ms = 199,
            _ => challenge.expires_at_ms = 5001,
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        assert_eq!(
            pending
                .accept_challenge(
                    &challenge.canonical_bytes().unwrap(),
                    projection(&challenge, &command),
                    f.verified_app(&challenge),
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding)
        );
    }
}

#[test]
fn challenge_decode_rejects_trailing_bytes_and_bounds_before_signing() {
    let f = Fixture::new();
    for mutation in 0..3 {
        let pending = f.begin();
        let challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        let mut bytes = challenge.canonical_bytes().unwrap();
        match mutation {
            0 => bytes.clear(),
            1 => bytes.push(0),
            _ => bytes = vec![0; KAGEMUSHA_RETAIL_ENROLLMENT_CHALLENGE_MAX_BYTES_V1 + 1],
        }
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        assert_eq!(
            pending
                .accept_challenge(
                    &bytes,
                    projection(&challenge, &command),
                    f.verified_app(&challenge),
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Encoding)
        );
    }
}

#[test]
fn account_and_device_proofs_cannot_replay_another_challenge_with_the_same_client_nonce() {
    let f = Fixture::new();
    for mutation in 0..4 {
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let accepted = accept(&f, pending, &proof.challenge);
        let mut different_challenge = proof.challenge.clone();
        if mutation < 2 {
            different_challenge.server_nonce[0] ^= 1;
        } else {
            different_challenge.expires_at_ms += 1;
        }
        let different = f.sign_proof(different_challenge);
        assert_eq!(
            different.challenge.client_nonce,
            proof.challenge.client_nonce
        );
        let (account, response) = if mutation % 2 == 0 {
            (account_signature(&different), &proof.device_response)
        } else {
            (account_signature(&proof), &different.device_response)
        };
        assert_eq!(
            accepted.prepare_proof(&account, response).err(),
            Some(InitialEnrollmentErrorV1::Authority)
        );
    }
}

#[test]
fn account_proof_requires_the_exact_controller_typed_hash_and_purpose() {
    let f = Fixture::new();
    for mutation in 0..4 {
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let accepted = accept(&f, pending, &proof.challenge);
        let payload = proof.challenge.account_signing_payload().unwrap();
        let bytes = norito::encode_canonical(&payload).unwrap();
        let wrong_signer = KeyPair::from_seed(vec![14; 32], Algorithm::Ed25519);
        let signature = match mutation {
            0 => Signature::try_new(
                wrong_signer.private_key(),
                &proof.challenge.account_signing_message().unwrap(),
            ),
            1 => Signature::try_new(f.account.private_key(), &bytes),
            2 => Signature::try_new(f.account.private_key(), &Sha256::digest(&bytes)),
            _ => {
                let mut wrong_purpose = payload;
                wrong_purpose.domain = "iroha:other-purpose".to_owned();
                Signature::try_new(
                    f.account.private_key(),
                    HashOf::new(&wrong_purpose).as_ref(),
                )
            }
        }
        .unwrap();
        assert_eq!(
            accepted
                .prepare_proof(signature.payload(), &proof.device_response)
                .err(),
            Some(InitialEnrollmentErrorV1::Authority)
        );
    }
}

#[test]
fn a_valid_device_signature_cannot_replace_the_retained_qualification() {
    for mutation in 0..4 {
        let mut f = Fixture::new();
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let accepted = accept(&f, pending, &proof.challenge);
        match mutation {
            0 => f.qualification.release_id = [88; 32],
            1 => f.qualification.core_authorization_key_reference = [88; 32],
            2 => {
                f.qualification.credential.hardware_epoch_id = [88; 32];
                resign_credential(&mut f);
            }
            _ => {
                f.qualification.credential.hardware_epoch_generation += 1;
                resign_credential(&mut f);
            }
        }
        let changed = f.device_response(proof.challenge.device_request_id().unwrap());
        assert_eq!(
            accepted
                .prepare_proof(&account_signature(&proof), &changed)
                .err(),
            Some(InitialEnrollmentErrorV1::Binding)
        );
    }
}

#[test]
fn preparation_rejects_signature_and_device_frame_bounds() {
    let f = Fixture::new();
    for mutation in 0..6 {
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let accepted = accept(&f, pending, &proof.challenge);
        let mut signature = account_signature(&proof);
        let mut response = proof.device_response.clone();
        match mutation {
            0 => signature.clear(),
            1 => {
                signature.pop();
            }
            2 => signature.push(0),
            3 => response.clear(),
            4 => response = vec![0; KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1 + 1],
            _ => response.push(0),
        }
        assert_eq!(
            accepted.prepare_proof(&signature, &response).err(),
            Some(if mutation == 5 {
                InitialEnrollmentErrorV1::Authority
            } else {
                InitialEnrollmentErrorV1::Encoding
            })
        );
    }
}

#[test]
fn prepared_proof_cannot_accept_a_valid_certificate_committing_to_another_proof() {
    let f = Fixture::new();
    let (prepared, proof) = prepare(&f);
    let mut different_challenge = proof.challenge.clone();
    different_challenge.server_nonce[0] ^= 1;
    let different_proof = f.sign_proof(different_challenge);
    let certificate = f.certificate(&different_proof);
    assert_ne!(
        certificate.subject.challenge_evidence_digest,
        proof.canonical_evidence_digest().unwrap()
    );
    assert_eq!(
        prepared
            .complete(&certificate.canonical_bytes().unwrap())
            .err(),
        Some(InitialEnrollmentErrorV1::Authority)
    );
}

#[test]
fn issuer_signed_time_must_be_within_the_retained_challenge_interval() {
    let f = Fixture::new();
    for issued_at in [999, 2000] {
        let (prepared, proof) = prepare(&f);
        let mut certificate = f.certificate(&proof);
        certificate.subject.issued_at_ms = issued_at;
        certificate.signature = SignatureOf::try_new(
            f.issuer.private_key(),
            &certificate.subject.approval_payload().unwrap(),
        )
        .unwrap();
        assert_eq!(
            prepared
                .complete(&certificate.canonical_bytes().unwrap())
                .err(),
            Some(InitialEnrollmentErrorV1::Authority)
        );
    }
}

#[test]
fn expiry_precedes_parsing_and_signing_in_every_retained_phase() {
    let f = Fixture::new();
    let mut pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
    pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        pending.canonical_qualification().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        pending
            .accept_challenge(
                &[],
                projection(&proof.challenge, &command),
                f.verified_app(&proof.challenge),
            )
            .err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );

    let pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let mut accepted = accept(&f, pending, &proof.challenge);
    accepted.pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        accepted.account_signing_message().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        accepted.device_request_id().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        accepted.canonical_device_command().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        accepted.prepare_proof(&[], &[]).err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );

    let (mut prepared, _) = prepare(&f);
    prepared.pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        prepared.challenge_id().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        prepared.canonical_proof().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        prepared.complete(&[]).err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
}

#[test]
fn all_challenge_transitions_share_the_original_continuous_deadline() {
    let f = Fixture::new();
    let mut pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let certificate = f.certificate(&proof).canonical_bytes().unwrap();
    let original = NativeDeadlineV1::start(Duration::from_secs(2)).unwrap();
    pending.deadline = original.clone();
    let prepared = accept(&f, pending, &proof.challenge)
        .prepare_proof(&account_signature(&proof), &proof.device_response)
        .unwrap();
    let admission = prepared.complete(&certificate).unwrap();
    while original.check().is_ok() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        admission.deadline().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
}
