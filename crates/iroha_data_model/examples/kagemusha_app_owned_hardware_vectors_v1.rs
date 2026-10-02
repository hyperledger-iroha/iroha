//! Generate shared ordinary-app signing vectors using the actual Rust model.
//!
//! The repeated selectors and public fixture account are codec test material.
//! They carry no native admission, issuer enrollment, hardware signature or
//! monetary authority. Run this example and retain stdout for Kotlin/Swift
//! conformance; applications must obtain signing operations from Core.

use base64::{Engine as _, engine::general_purpose};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    kagemusha::{
        KagemushaAppEnrollmentPossessionChallengeV1, KagemushaAppOperationApprovalChallengeV1,
        KagemushaAppOperationApprovalPurposeV1, KagemushaDevicePublicKeyV1,
        KagemushaHardwarePlatformClassV1, KagemushaHardwareTransitionSelectionV1,
        KagemushaOperationKindV1, KagemushaOrdinaryAppEnrollmentChallengeV1,
        KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        kagemusha_ordinary_android_app_key_alias_v1,
    },
};
use sha2::{Digest as _, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let account_key = KeyPair::from_private_key(PrivateKey::from_bytes(
        Algorithm::Ed25519,
        &[0x42; 32], // Public, deterministic test material only.
    )?)?;
    let account = AccountId::new(account_key.public_key().clone());
    let account_binding = KagemushaAppOperationApprovalChallengeV1::account_binding(&account);
    let mut vectors = Vec::new();
    for (name, tag, operation) in [
        ("mint_fold", 1_u8, KagemushaOperationKindV1::MintFold),
        ("send_split", 2, KagemushaOperationKindV1::SendSplit),
        ("receive_fold", 3, KagemushaOperationKindV1::ReceiveFold),
        ("redeem_split", 4, KagemushaOperationKindV1::RedeemSplit),
        ("rotate", 5, KagemushaOperationKindV1::Rotate),
    ] {
        for index in [9_u128, u128::MAX - 1] {
            let outgoing = matches!(
                operation,
                KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
            );
            let subject = KagemushaHardwareTransitionSelectionV1 {
                version: 1,
                release_id: [1; 32],
                provider_policy_root: [2; 32],
                app_policy_digest: [3; 32],
                credential_id: [4; 32],
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::prehashed([5; 32]),
                )),
                lane_commitment: [6; 32],
                hardware_profile_id: [7; 32],
                policy_epoch: 8,
                hardware_epoch_id: [9; 32],
                hardware_epoch_generation: 10,
                operation_kind: operation,
                transition_statement_digest: [11; 32],
                candidate_envelope_digest: [if outgoing { 12 } else { 0 }; 32],
                terminal_body_commitment: [if outgoing { 13 } else { 0 }; 32],
                secure_index_before: index,
                secure_index_after: index + 1,
            };
            let s = subject.canonical_signing_bytes()?;
            let challenge = KagemushaAppOperationApprovalChallengeV1 {
                version: 1,
                purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
                operation_id: [0x20 + tag; 32],
                nonce: [0x22; 32],
                account_binding,
                authority_policy_digest: [0x24; 32],
                attested_key_id: [0x25; 32],
                enrollment_digest: [0x26; 32],
                subject_signing_digest: Sha256::digest(&s).into(),
                normalized_guard_digest: [0x28; 32],
                issued_at_ms: 1000,
                expires_at_ms: 2000,
                subject,
            };
            let w = challenge.canonical_signing_bytes()?;
            vectors.push(norito::json!({
                "operation": name,
                "operation_tag": tag,
                "secure_index_before": (index.to_string()),
                "secure_index_after": ((index + 1).to_string()),
                "subject_signing_hex": (hex::encode(&s)),
                "subject_sha256_hex": (hex::encode(Sha256::digest(&s))),
                "approval_signing_hex": (hex::encode(&w)),
                "approval_sha256_hex": (hex::encode(Sha256::digest(&w))),
                "challenge_archive_hex": (hex::encode(norito::encode_canonical(&challenge)?)),
            }));
        }
    }
    // Separate public fixture issuer and app key, never ledger-account keys.
    let issuer =
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[0x43; 32])?)?;
    let app_fixture = p256::ecdsa::SigningKey::from_bytes((&[17; 32]).into())?;
    let app_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        app_fixture
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )?;
    let key_id: [u8; 32] = Sha256::digest(app_key.as_sec1_bytes()).into();
    let mut enrollment_vectors = Vec::new();
    for (name, tag, platform) in [
        (
            "android_keymint",
            1_u8,
            KagemushaHardwarePlatformClassV1::AndroidKeyMint,
        ),
        (
            "apple_app_attest",
            2,
            KagemushaHardwarePlatformClassV1::AppleAppAttest,
        ),
    ] {
        let c = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: platform,
            enrollment_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding,
            network_id: [5; 32],
            lane_id: [6; 32],
            release_id: [7; 32],
            hardware_profile_id: [8; 32],
            suite_id: [9; 32],
            trust_policy_digest: [10; 32],
            app_authority_policy_digest: [11; 32],
            financial_authority_commitment: [12; 32],
            issuer_policy_digest: [13; 32],
            policy_epoch: 1,
            hardware_epoch: 10,
            issued_at_ms: 1000,
            expires_at_ms: 121_000,
        };
        let signing = c.canonical_signing_bytes()?;
        let signed = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge: c,
            signature: Signature::new(issuer.private_key(), &signing),
        };
        signed.signature.verify(issuer.public_key(), &signing)?;
        let transport = signed.to_transport_bytes()?;
        assert_eq!(
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&transport)?,
            signed
        );
        let e = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            &c, &app_key, [14; 32],
        )?;
        let possession = e.canonical_signing_bytes()?;
        let integrity = c.play_integrity_request_hash(key_id)?;
        let alias = if platform == KagemushaHardwarePlatformClassV1::AndroidKeyMint {
            kagemusha_ordinary_android_app_key_alias_v1(&c)?
        } else {
            general_purpose::STANDARD.encode(key_id)
        };
        enrollment_vectors.push(norito::json!({
            "platform": name,
            "platform_tag": tag,
            "challenge_signing_hex": (hex::encode(&signing)),
            "challenge_transport_hex": (hex::encode(&transport)),
            "challenge_archive_hex": (hex::encode(norito::encode_canonical(&c)?)),
            "issuer_public_key_hex": (hex::encode(issuer.public_key().to_bytes().1)),
            "issuer_signature_hex": (hex::encode(signed.signature.payload())),
            "attestation_challenge_hex": (hex::encode(c.attestation_challenge()?)),
            "attested_public_key_sec1_hex": (hex::encode(app_key.as_sec1_bytes())),
            "attested_key_id_hex": (hex::encode(key_id)),
            "key_alias": alias,
            "raw_platform_evidence_digest_hex": (hex::encode([14; 32])),
            "play_integrity_request_hash_hex": (hex::encode(integrity)),
            "play_integrity_request_hash_base64url": (general_purpose::URL_SAFE_NO_PAD.encode(integrity)),
            "possession_signing_hex": (hex::encode(&possession)),
            "possession_sha256_hex": (hex::encode(Sha256::digest(&possession))),
            "possession_archive_hex": (hex::encode(norito::encode_canonical(&e)?)),
        }));
    }
    let result = norito::json!({
        "schema": "iroha.kagemusha.app-owned-hardware.signing-vectors.v1",
        "codec_only": true,
        "native_authority": false,
        "hardware_qualified": false,
        "monetary_authority": false,
        "account_binding_hex": (hex::encode(account_binding)),
        "vectors": vectors,
        "enrollment_vectors": enrollment_vectors,
    });
    println!("{}", norito::json::to_json_pretty(&result)?);
    Ok(())
}
