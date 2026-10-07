//! Exact Core complete-request DATA binding before the provider's create-new E5 publication.
//! Core independently authenticates certificate and Google/App Attest originals.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use norito::json::Value;

fn invalid() -> Failure {
    Failure::code(INVALID)
}
fn exact(value: &Value, keys: &[&str]) -> Result<()> {
    let map = value.as_object().ok_or_else(invalid)?;
    if map.len() != keys.len() || keys.iter().any(|key| !map.contains_key(*key)) {
        return Err(invalid());
    }
    Ok(())
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .as_object()
        .and_then(|map| map.get(key))
        .ok_or_else(invalid)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    field(value, key)?.as_str().ok_or_else(invalid)
}
fn binary(value: &str, bound: usize) -> Result<Vec<u8>> {
    if value.is_empty() || value.len() > bound.div_ceil(3) * 4 {
        return Err(invalid());
    }
    let bytes = STANDARD.decode(value).map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() > bound || STANDARD.encode(&bytes) != value {
        return Err(invalid());
    }
    Ok(bytes)
}
pub(super) fn verify(
    original: &[u8],
    scope: &Scope,
    key: &iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
    account_frame: &[u8],
    chain: &[Vec<u8>],
) -> Result<()> {
    let max = advance::KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1;
    if original.is_empty() || original.len() > max {
        return Err(invalid());
    }
    let limits = norito::json::JsonPreflightLimits::new(
        max, 65_536, max, max, max, 16_384, 32_768, 32_768, 65_536, 32,
    );
    norito::json::preflight_slice(original, limits).map_err(|_| invalid())?;
    let value: Value = norito::json::from_slice(original).map_err(|_| invalid())?;
    exact(
        &value,
        &[
            "operation_id",
            "payment_key_sec1_base64",
            "existing_account_signature_base64",
            "evidence",
        ],
    )?;
    if text(&value, "operation_id")? != hex::encode(scope.challenge.challenge_digest())
        || binary(text(&value, "payment_key_sec1_base64")?, 65)?.as_slice() != key.as_sec1_bytes()
    {
        return Err(invalid());
    }
    let signature = binary(text(&value, "existing_account_signature_base64")?, 64)?;
    if signature.len() != 64 {
        return Err(invalid());
    }
    iroha_crypto::Signature::try_from_bytes(&signature)
        .map_err(|_| invalid())?
        .verify(&scope.account_key, account_frame)
        .map_err(|_| invalid())?;
    let evidence = field(&value, "evidence")?;
    if scope.android {
        exact(
            evidence,
            &["platform", "chain_base64", "opaque_play_integrity_token"],
        )?;
        if text(evidence, "platform")? != "android" {
            return Err(invalid());
        }
        let token = text(evidence, "opaque_play_integrity_token")?;
        if token.is_empty()
            || token.len() > 65_536
            || !token.bytes().all(|byte| (33..=126).contains(&byte))
        {
            return Err(invalid());
        }
        let supplied = field(evidence, "chain_base64")?
            .as_array()
            .ok_or_else(invalid)?;
        if !(2..=8).contains(&supplied.len()) || supplied.len() != chain.len() {
            return Err(invalid());
        }
        for (certificate, actual) in supplied.iter().zip(chain) {
            if binary(certificate.as_str().ok_or_else(invalid)?, 16_384)? != *actual {
                return Err(invalid());
            }
        }
    } else {
        exact(
            evidence,
            &[
                "platform",
                "attestation_base64",
                "assertion_base64",
                "app_attest_key_id_hex",
            ],
        )?;
        if text(evidence, "platform")? != "apple" {
            return Err(invalid());
        }
        binary(text(evidence, "attestation_base64")?, 65_536)?;
        binary(text(evidence, "assertion_base64")?, 4_096)?;
        let key_id = text(evidence, "app_attest_key_id_hex")?;
        if key_id.len() != 64
            || !key_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || key_id.bytes().all(|byte| byte == b'0')
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use p256::ecdsa::SigningKey;
    fn fixture() -> (
        Scope,
        iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
        Vec<u8>,
        Vec<Vec<u8>>,
        Value,
    ) {
        let account = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        let payment = SigningKey::from_slice(&[4; 32]).unwrap();
        let key = iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(
            payment.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let scope = Scope {
            challenge: KagemushaWalletEnrollmentChallengeV1 {
                version: 1,
                scheme_id: [1; 32],
                asset_digest: [2; 32],
                account_digest: [3; 32],
                app_policy: [4; 32],
                enrollment_policy: [5; 32],
                issuer_nonce: [6; 32],
            },
            profile: advance::KagemushaWalletKeyProfileV1::SecureElementOrTee,
            policy: KagemushaWalletEnrollmentPolicyV1 {
                version: 1,
                scheme_id: [1; 32],
                asset_digest: [2; 32],
                app_policy: [4; 32],
                platform: KagemushaWalletEnrollmentPlatformV1::Android {
                    attestation_root_sha256: [7; 32],
                    hardware: KagemushaWalletAndroidHardwareV1::TeeOrStrongBox,
                    patch_floor_yyyymm: 202610,
                    play_integrity_maximum_age_ms: 600_000,
                    require_play_recognized: true,
                    require_licensed: true,
                    minimum_device_integrity: KagemushaWalletPlayIntegrityLevelV1::Device,
                },
                regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
                challenge_lifetime_ms: 600_000,
                attestation_lease_lifetime_ms: 0,
            },
            network: [7; 32],
            account_key: account.public_key().clone(),
            android: true,
        };
        let input = Input {
            selector: 0,
            challenge: &[],
            policy: &[],
            account: &[],
            original: &[],
            certificates: &[],
            issued_at_ms: 1,
            expires_at_ms: 600_001,
        };
        let frame = scope.account_frame(&key, &input).unwrap();
        let signature = Signature::new(account.private_key(), &frame);
        let chain = vec![vec![9; 2], vec![10; 3]];
        let value = norito::json!({"operation_id":(hex::encode(scope.challenge.challenge_digest())),
            "payment_key_sec1_base64":(STANDARD.encode(key.as_sec1_bytes())),
            "existing_account_signature_base64":(STANDARD.encode(signature.payload())),
            "evidence":{"platform":"android","chain_base64":(chain.iter().map(|v|STANDARD.encode(v)).collect::<Vec<_>>()),"opaque_play_integrity_token":"actual-token-DATA"}});
        (scope, key, frame, chain, value)
    }
    fn bytes(value: &Value) -> Vec<u8> {
        norito::json::to_json(value).unwrap().into_bytes()
    }
    #[test]
    fn existing_account_signature_binds_exact_native_frame_and_actual_chain_data() {
        let (scope, key, frame, chain, value) = fixture();
        assert!(verify(&bytes(&value), &scope, &key, &frame, &chain).is_ok());
        let mut changed = frame.clone();
        changed[384] ^= 1;
        assert_eq!(
            verify(&bytes(&value), &scope, &key, &changed, &chain)
                .unwrap_err()
                .status,
            INVALID
        );
        assert_eq!(
            verify(&bytes(&value), &scope, &key, &frame, &[vec![9], vec![10]])
                .unwrap_err()
                .status,
            INVALID
        );
    }
    #[test]
    fn changed_operation_or_signature_and_unknown_or_duplicate_properties_refuse_publication() {
        let (scope, key, frame, chain, value) = fixture();
        for (name, changed) in [
            ("operation_id", Value::String("00".repeat(32))),
            (
                "existing_account_signature_base64",
                Value::String(STANDARD.encode([0; 64])),
            ),
            ("new_freshness_flag", Value::Bool(true)),
        ] {
            let mut v = value.clone();
            v.as_object_mut().unwrap().insert(name.into(), changed);
            assert_eq!(
                verify(&bytes(&v), &scope, &key, &frame, &chain)
                    .unwrap_err()
                    .status,
                INVALID
            );
        }
        let raw = bytes(&value);
        let mut duplicate = b"{\"operation_id\":\"00\",".to_vec();
        duplicate.extend_from_slice(&raw[1..]);
        assert_eq!(
            verify(&duplicate, &scope, &key, &frame, &chain)
                .unwrap_err()
                .status,
            INVALID
        );
    }

    #[test]
    fn play_integrity_original_has_the_exact_core_ascii_and_size_contract() {
        let (scope, key, frame, chain, mut value) = fixture();
        for token in ["!".repeat(65_536), "a.b_c-9".into()] {
            value
                .as_object_mut()
                .unwrap()
                .get_mut("evidence")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("opaque_play_integrity_token".into(), Value::String(token));
            assert!(verify(&bytes(&value), &scope, &key, &frame, &chain).is_ok());
        }
        for token in [
            "!".repeat(65_537),
            "".into(),
            "token with space".into(),
            "token\tvalue".into(),
            "token\u{7f}".into(),
            "tokén".into(),
        ] {
            value
                .as_object_mut()
                .unwrap()
                .get_mut("evidence")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("opaque_play_integrity_token".into(), Value::String(token));
            assert_eq!(
                verify(&bytes(&value), &scope, &key, &frame, &chain)
                    .unwrap_err()
                    .status,
                INVALID
            );
        }
    }

    #[test]
    fn apple_original_requires_closed_canonical_bounded_core_evidence() {
        let (mut scope, key, frame, chain, mut value) = fixture();
        scope.android = false;
        let evidence = norito::json!({"platform":"apple",
            "attestation_base64":(STANDARD.encode(vec![1;65_536])),
            "assertion_base64":(STANDARD.encode(vec![2;4_096])),
            "app_attest_key_id_hex":("ab".repeat(32))});
        value
            .as_object_mut()
            .unwrap()
            .insert("evidence".into(), evidence.clone());
        assert!(verify(&bytes(&value), &scope, &key, &frame, &chain).is_ok());
        for (name, changed) in [
            (
                "attestation_base64",
                Value::String(STANDARD.encode(vec![1; 65_537])),
            ),
            (
                "assertion_base64",
                Value::String(STANDARD.encode(vec![2; 4_097])),
            ),
            ("attestation_base64", Value::String("".into())),
            ("assertion_base64", Value::String("AQ".into())),
            ("assertion_base64", Value::String("AR==".into())),
            ("app_attest_key_id_hex", Value::String("00".repeat(32))),
            ("app_attest_key_id_hex", Value::String("AB".repeat(32))),
            ("app_attest_key_id_hex", Value::String("ab".repeat(31))),
            ("new_verdict", Value::Bool(true)),
        ] {
            let mut offered = evidence.clone();
            offered
                .as_object_mut()
                .unwrap()
                .insert(name.into(), changed);
            value
                .as_object_mut()
                .unwrap()
                .insert("evidence".into(), offered);
            assert_eq!(
                verify(&bytes(&value), &scope, &key, &frame, &chain)
                    .unwrap_err()
                    .status,
                INVALID,
                "{name}"
            );
        }
        let mut missing = evidence;
        missing.as_object_mut().unwrap().remove("assertion_base64");
        value
            .as_object_mut()
            .unwrap()
            .insert("evidence".into(), missing);
        assert_eq!(
            verify(&bytes(&value), &scope, &key, &frame, &chain)
                .unwrap_err()
                .status,
            INVALID
        );
    }
}
