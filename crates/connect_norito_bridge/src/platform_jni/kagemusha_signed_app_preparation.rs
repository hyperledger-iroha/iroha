//! Android pre-KeyMint check for the exact issuer-signed app preparation.
//!
//! This endpoint only exposes a verified challenge nonce. The native enrollment owner
//! independently retains its selection, policy, deadline and trusted service time,
//! and rechecks all of them before granting a credential.

use super::*;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1, KagemushaHardwarePlatformClassV1,
        KagemushaRetailEnrollmentIssuerPolicyV1,
    },
};
use sha2::{Digest as _, Sha256};

use crate::{SignedAppPreparationPinsV1, verify_signed_app_preparation_v1};

const TOKEN_BYTES: usize = 273;
const ACCOUNT_I105_MAX_BYTES: usize = 512;
const DIGEST_BYTES: usize = 32;
const CONTRACT: [jni::sys::jint; 4] = [
    1,
    TOKEN_BYTES as jni::sys::jint,
    KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1 as jni::sys::jint,
    ACCOUNT_I105_MAX_BYTES as jni::sys::jint,
];

fn bounded_bytes(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    minimum: usize,
    maximum: usize,
) -> Option<Vec<u8>> {
    let size = usize::try_from(env.get_array_length(value).ok()?).ok()?;
    if size < minimum || size > maximum {
        return None;
    }
    let bytes = env.convert_byte_array(value).ok()?;
    (bytes.len() == size).then_some(bytes)
}

fn exact_digest(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
) -> Option<[u8; DIGEST_BYTES]> {
    bounded_bytes(env, value, DIGEST_BYTES, DIGEST_BYTES)?
        .try_into()
        .ok()
}

#[allow(clippy::too_many_arguments)]
fn verified_android_server_nonce(
    token: &[u8],
    canonical_policy: &[u8],
    pinned_policy_sha256: [u8; 32],
    account_i105: &[u8],
    client_nonce: [u8; 32],
    release_id: [u8; 32],
    profile_id: [u8; 32],
    lane_id: [u8; 32],
    trusted_now_ms: u64,
) -> Option<[u8; 32]> {
    if token.len() != TOKEN_BYTES
        || canonical_policy.is_empty()
        || canonical_policy.len() > KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1
        || pinned_policy_sha256 == [0; 32]
        || account_i105.is_empty()
        || account_i105.len() > ACCOUNT_I105_MAX_BYTES
        || trusted_now_ms == 0
    {
        return None;
    }
    let actual_policy_sha256: [u8; 32] = Sha256::digest(canonical_policy).into();
    if actual_policy_sha256 != pinned_policy_sha256 {
        return None;
    }
    let policy =
        KagemushaRetailEnrollmentIssuerPolicyV1::decode_canonical_exact(canonical_policy).ok()?;
    let account_i105 = std::str::from_utf8(account_i105).ok()?;
    let account = AccountId::parse_encoded(account_i105).ok()?;
    if account.canonical_i105().ok()?.as_bytes() != account_i105.as_bytes() {
        return None;
    }
    let verified = verify_signed_app_preparation_v1(
        token,
        SignedAppPreparationPinsV1 {
            policy: &policy,
            account_id: &account,
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            selected_attested_key_id: [0; 32],
            client_nonce,
            release_id,
            profile_id,
            lane_id,
            trusted_now_ms,
        },
    )
    .ok()?;
    Some(verified.server_nonce)
}

/// Expose the sole exact Android pre-KeyMint verifier contract.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaSignedAppPreparationJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let Ok(output) = env.new_int_array(CONTRACT.len() as jni::sys::jsize) else {
        return ptr::null_mut();
    };
    if env.set_int_array_region(&output, 0, &CONTRACT).is_err() {
        return ptr::null_mut();
    }
    output.into_raw()
}

/// Verify the signed preparation before creating the KeyMint key; return only its server nonce.
///
/// The caller must supply a policy SHA-256 anchored outside the issuer response and a
/// service-trusted time. This preflight confers no device or monetary authority.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaSignedAppPreparationJniV1_nativeVerifyV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    token: jni::objects::JByteArray<'_>,
    canonical_policy: jni::objects::JByteArray<'_>,
    pinned_policy_sha256: jni::objects::JByteArray<'_>,
    account_i105: jni::objects::JByteArray<'_>,
    client_nonce: jni::objects::JByteArray<'_>,
    release_id: jni::objects::JByteArray<'_>,
    profile_id: jni::objects::JByteArray<'_>,
    lane_id: jni::objects::JByteArray<'_>,
    trusted_now_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let token = bounded_bytes(&mut env, &token, TOKEN_BYTES, TOKEN_BYTES)?;
        let canonical_policy = bounded_bytes(
            &mut env,
            &canonical_policy,
            1,
            KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1,
        )?;
        let pinned_policy_sha256 = exact_digest(&mut env, &pinned_policy_sha256)?;
        let account_i105 = bounded_bytes(&mut env, &account_i105, 1, ACCOUNT_I105_MAX_BYTES)?;
        let client_nonce = exact_digest(&mut env, &client_nonce)?;
        let release_id = exact_digest(&mut env, &release_id)?;
        let profile_id = exact_digest(&mut env, &profile_id)?;
        let lane_id = exact_digest(&mut env, &lane_id)?;
        let trusted_now_ms = u64::try_from(trusted_now_ms).ok()?;
        let server_nonce = verified_android_server_nonce(
            &token,
            &canonical_policy,
            pinned_policy_sha256,
            &account_i105,
            client_nonce,
            release_id,
            profile_id,
            lane_id,
            trusted_now_ms,
        )?;
        Some(env.byte_array_from_slice(&server_nonce).ok()?.into_raw())
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        NetworkId, asset::AssetDefinitionId, kagemusha::KagemushaRetailEnrollmentRuntimeV1,
        nexus::AxtAssetIncarnationV1,
    };
    use iroha_model_base::topology::DataSpaceId;

    #[test]
    fn exact_preparation_contract_is_bounded() {
        assert_eq!(CONTRACT, [1, 273, 8192, 512]);
    }

    #[test]
    fn untrusted_policy_or_account_never_yields_a_keymint_challenge() {
        let token = [1_u8; TOKEN_BYTES];
        let pins = [2_u8; 32];
        assert!(
            verified_android_server_nonce(
                &token,
                b"not a policy",
                pins,
                b"not-i105",
                pins,
                pins,
                pins,
                pins,
                1_000,
            )
            .is_none()
        );
        assert!(
            verified_android_server_nonce(
                &token,
                b"not a policy",
                [0; 32],
                b"not-i105",
                pins,
                pins,
                pins,
                pins,
                1_000,
            )
            .is_none()
        );
    }

    #[test]
    fn signed_android_preparation_requires_policy_account_selection_and_time() {
        let issuer = KeyPair::from_seed(vec![81; 32], Algorithm::Ed25519);
        let account_key = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        let account = AccountId::new(account_key.public_key().clone());
        let account_i105 = account.canonical_i105().unwrap();
        let policy = KagemushaRetailEnrollmentIssuerPolicyV1 {
            version: 1,
            issuer_policy_id: [71; 32],
            issuer_public_key: issuer.public_key().clone(),
            issuer_audience: "test-issuer".parse().unwrap(),
            runtime: KagemushaRetailEnrollmentRuntimeV1 {
                fi_id: "test-bank".parse().unwrap(),
                ledger_dataspace_id: DataSpaceId::new(7),
                authentication_namespace: "test.bank".parse().unwrap(),
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"signed-android-preparation-test-network"),
                )),
                asset: AssetDefinitionId::from_uuid_bytes([
                    0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84,
                    0xfd, 0xcd, 0x2f,
                ])
                .unwrap(),
                asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                    *Hash::new(b"signed-android-preparation-test-asset").as_ref(),
                )
                .unwrap(),
                scale: 2,
            },
            valid_from_ms: 100,
            expires_at_ms: 200_000,
            maximum_certificate_lifetime_ms: 4_000,
        };
        let canonical_policy = norito::encode_canonical(&policy).unwrap();
        let policy_pin: [u8; 32] = Sha256::digest(&canonical_policy).into();
        let mut token = vec![1_u8];
        token.extend_from_slice(&1_000_u64.to_le_bytes());
        token.extend_from_slice(&121_000_u64.to_le_bytes());
        token.extend_from_slice(&[90; 32]);
        token.extend_from_slice(&[91; 32]);
        token.extend_from_slice(&[92; 32]);
        token.extend_from_slice(&[93; 32]);
        token.extend_from_slice(&[0; 32]);
        token.extend_from_slice(&[94; 32]);
        let mut message = b"iroha:kagemusha:v1:app-enrollment-preparation\0".to_vec();
        message.extend_from_slice(&token[1..209]);
        message.extend_from_slice(&policy.issuer_policy_id);
        message.extend_from_slice(&Sha256::digest(account_i105.as_bytes()));
        token.extend_from_slice(
            Signature::try_new(issuer.private_key(), &message)
                .unwrap()
                .payload(),
        );
        assert_eq!(token.len(), TOKEN_BYTES);
        let verify = |candidate: &[u8], pin, account: &[u8], now| {
            verified_android_server_nonce(
                candidate,
                &canonical_policy,
                pin,
                account,
                [90; 32],
                [92; 32],
                [93; 32],
                [94; 32],
                now,
            )
        };
        assert_eq!(
            verify(&token, policy_pin, account_i105.as_bytes(), 1_000),
            Some([91; 32])
        );
        assert_eq!(
            verify(&token, [0; 32], account_i105.as_bytes(), 1_000),
            None
        );
        assert_eq!(verify(&token, policy_pin, b"not-i105", 1_000), None);
        assert_eq!(
            verify(&token, policy_pin, account_i105.as_bytes(), 121_000),
            None
        );
        token[209] ^= 1;
        assert_eq!(
            verify(&token, policy_pin, account_i105.as_bytes(), 1_000),
            None
        );
    }
}
