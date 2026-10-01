//! Generate shared ordinary-app signing vectors using the actual Rust model.
//!
//! The repeated selectors and public fixture account are codec test material.
//! They carry no native admission, issuer enrollment, hardware signature or
//! monetary authority. Run this example and retain stdout for Kotlin/Swift
//! conformance; applications must obtain signing operations from Core.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    kagemusha::{
        KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
        KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
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
    let result = norito::json!({
        "schema": "iroha.kagemusha.app-owned-hardware.signing-vectors.v1",
        "codec_only": true,
        "native_authority": false,
        "hardware_qualified": false,
        "monetary_authority": false,
        "account_binding_hex": (hex::encode(account_binding)),
        "vectors": vectors,
    });
    println!("{}", norito::json::to_json_pretty(&result)?);
    Ok(())
}
