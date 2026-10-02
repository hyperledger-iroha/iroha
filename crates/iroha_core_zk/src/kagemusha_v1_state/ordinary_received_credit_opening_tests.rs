//! Actual maintained AEAD over explicit synthetic originals; no Native custody is constructed.

use super::*;
use crate::kagemusha_v1_crypto::seal_kagemusha_credit_v1_with_rng;
use iroha_data_model::{
    kagemusha::{KagemushaAppOperationApprovalEvidenceV1, kagemusha_ordinary_credit_id_v1},
    testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
};
use p256::ecdsa::{SigningKey, signature::Signer as _};

fn sign(body: KagemushaOrdinaryPaymentRequestBodyV1) -> Vec<u8> {
    let key = SigningKey::from_slice(&[7; 32]).unwrap();
    let signature: p256::ecdsa::Signature = key.sign(&body.canonical_signing_bytes().unwrap());
    KagemushaOrdinaryPaymentRequestV1 {
        body,
        evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        },
    }
    .canonical_bytes()
    .unwrap()
}

struct Specimen {
    request_original: Vec<u8>,
    output: KagemushaOrdinaryPaymentOutputV1,
    encrypted_credit: Vec<u8>,
    opening: PrivateReceivedCreditOpening,
    preparation_clock: KagemushaOrdinaryCashClockContextV1,
}

fn seal(
    request: &KagemushaOrdinaryPaymentRequestV1,
    output: &KagemushaOrdinaryPaymentOutputV1,
    clock: &KagemushaOrdinaryCashClockContextV1,
    opening: &KagemushaCreditOpeningV1,
) -> Vec<u8> {
    let aad = output.encrypted_credit_aad_against(request, clock).unwrap();
    seal_kagemusha_credit_v1_with_rng(
        opening,
        &aad,
        request.body.recipient_encryption_key,
        &mut rand::rngs::OsRng,
    )
    .unwrap()
    .canonical_bytes_against_recipient_key(request.body.recipient_encryption_key)
    .unwrap()
}

fn specimen() -> Specimen {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(400).unwrap();
    let credential = enrollment.app_credential().subject();
    let runtime = &enrollment.certificate().subject.owner.runtime;
    let request_original = sign(KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: credential.release_id,
        network_id: credential.network_id,
        normalized_asset_id: kagemusha_asset_identity_digest_v1(&runtime.asset).unwrap(),
        asset_incarnation: *runtime.asset_incarnation.as_bytes(),
        scale: runtime.scale,
        reserve_pool_id: [31; 32],
        recipient_account_binding: credential.account_binding,
        amount: 17,
        recipient_encryption_key: kagemusha_x25519_public_key_v1(&[32; 32]).unwrap(),
        recipient_credential_digest: enrollment.app_credential().digest(),
        recipient_lane_id: credential.lane_id,
        request_id: [33; 32],
        clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [34; 32],
            signed_observations_original_digest: [35; 32],
            lower_at_ms: 400,
            upper_at_ms: 401,
        },
        issued_at_ms: 400,
        expires_at_ms: 900,
    });
    let request =
        KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&request_original).unwrap();
    // The specimen really verifies the sole request signature; its issuer/time are synthetic.
    request
        .authenticate_receiver_signature(enrollment.app_credential(), None)
        .unwrap();
    let request_digest = request.canonical_original_digest().unwrap();
    let nullifier = [36; 32];
    let opening = PrivateReceivedCreditOpening(KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: kagemusha_ordinary_credit_id_v1(nullifier, request_digest),
        amount: 17,
        credit_commitment_opening: [37; 32],
        recipient_binding_opening: [38; 32],
        recovery_nonce: [39; 32],
    });
    let commitment = kagemusha_peer_credit_opening_commitment_v1(
        request_digest,
        request.body.recipient_encryption_key,
        opening.0.amount,
        opening.0.credit_commitment_opening,
        opening.0.recipient_binding_opening,
        opening.0.recovery_nonce,
    )
    .unwrap();
    let preparation_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [40; 32],
        signed_observations_original_digest: [41; 32],
        lower_at_ms: 450,
        upper_at_ms: 451,
    };
    let mut output = KagemushaOrdinaryPaymentOutputV1 {
        version: 1,
        request_digest,
        amount: 17,
        sender_before_commitment: [42; 32],
        sender_after_commitment: [43; 32],
        transition_nullifier: nullifier,
        credit_id: opening.0.credit_id,
        ciphertext_commitment: commitment,
        encrypted_credit_digest: [44; 32],
        clock_context_digest: preparation_clock.binding_digest().unwrap(),
        prepared_at_ms: preparation_clock.upper_at_ms,
    };
    let encrypted_credit = seal(&request, &output, &preparation_clock, &opening.0);
    output.encrypted_credit_digest = kagemusha_ciphertext_digest_v1(&encrypted_credit);
    Specimen {
        request_original,
        output,
        encrypted_credit,
        opening,
        preparation_clock,
    }
}

#[test]
fn actual_aead_opens_only_exact_model_request_output_clock_and_key() {
    let s = specimen();
    let opened = open_selected_original_data(
        &[32; 32],
        &s.request_original,
        &s.output,
        &s.encrypted_credit,
        &s.preparation_clock,
    )
    .unwrap();
    assert_eq!(opened.0, s.opening.0);
    assert!(
        open_selected_original_data(
            &[33; 32],
            &s.request_original,
            &s.output,
            &s.encrypted_credit,
            &s.preparation_clock,
        )
        .is_err()
    );
    let request =
        KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&s.request_original).unwrap();
    let mut changed = request.body;
    changed.request_id[0] ^= 1;
    assert!(
        open_selected_original_data(
            &[32; 32],
            &sign(changed),
            &s.output,
            &s.encrypted_credit,
            &s.preparation_clock,
        )
        .is_err()
    );
    let mut clock = s.preparation_clock;
    clock.signed_observations_original_digest[0] ^= 1;
    assert!(
        open_selected_original_data(
            &[32; 32],
            &s.request_original,
            &s.output,
            &s.encrypted_credit,
            &clock,
        )
        .is_err()
    );
}

#[test]
fn full_cipher_original_digest_and_canonical_aead_remain_separate_mandatory_checks() {
    let s = specimen();
    let mut ciphertext = s.encrypted_credit.clone();
    let last = ciphertext.len() - 1;
    ciphertext[last] ^= 1;
    assert!(
        open_selected_original_data(
            &[32; 32],
            &s.request_original,
            &s.output,
            &ciphertext,
            &s.preparation_clock,
        )
        .is_err()
    );
    // Coherently rehashing public metadata must not substitute actual authenticated encryption.
    let mut output = s.output;
    output.encrypted_credit_digest = kagemusha_ciphertext_digest_v1(&ciphertext);
    assert!(
        open_selected_original_data(
            &[32; 32],
            &s.request_original,
            &output,
            &ciphertext,
            &s.preparation_clock,
        )
        .is_err()
    );
    ciphertext = s.encrypted_credit.clone();
    ciphertext.push(0);
    output.encrypted_credit_digest = kagemusha_ciphertext_digest_v1(&ciphertext);
    assert!(
        open_selected_original_data(
            &[32; 32],
            &s.request_original,
            &output,
            &ciphertext,
            &s.preparation_clock,
        )
        .is_err()
    );
}

#[test]
fn genuine_aead_of_substituted_secret_opening_cannot_match_selected_semantic_commitment() {
    let s = specimen();
    let request =
        KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&s.request_original).unwrap();
    for field in 0..3 {
        let mut substituted = PrivateReceivedCreditOpening(s.opening.0.clone());
        match field {
            0 => substituted.0.credit_commitment_opening[0] ^= 1,
            1 => substituted.0.recipient_binding_opening[0] ^= 1,
            _ => substituted.0.recovery_nonce[0] ^= 1,
        }
        // A valid fresh ciphertext is encrypted under the actual selected AAD, but its plaintext
        // has different secrets. The post-open sole model commitment must independently refuse.
        let ciphertext = seal(&request, &s.output, &s.preparation_clock, &substituted.0);
        let mut output = s.output;
        output.encrypted_credit_digest = kagemusha_ciphertext_digest_v1(&ciphertext);
        assert!(
            open_selected_original_data(
                &[32; 32],
                &s.request_original,
                &output,
                &ciphertext,
                &s.preparation_clock,
            )
            .is_err()
        );
    }
}

#[test]
fn substituted_selected_semantics_cannot_reuse_original_encrypted_bytes() {
    let s = specimen();
    for field in 0..5 {
        let mut output = s.output;
        match field {
            0 => output.amount += 1,
            1 => output.sender_before_commitment[0] ^= 1,
            2 => output.sender_after_commitment[0] ^= 1,
            3 => output.transition_nullifier[0] ^= 1,
            _ => output.ciphertext_commitment[0] ^= 1,
        }
        assert!(
            open_selected_original_data(
                &[32; 32],
                &s.request_original,
                &output,
                &s.encrypted_credit,
                &s.preparation_clock,
            )
            .is_err()
        );
    }
}
