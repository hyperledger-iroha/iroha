//! Exact initial app-key possession, separate from financial transition approval.
//!
//! Signature authentication produces a non-monetary cryptographic original. The native
//! enrollment owner must independently hold the pending issuer scope, reserve this attempt
//! durably, consume the original once, and activate identity only after durable completion.

use super::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaAppOperationApprovalEvidenceV1,
    KagemushaDevicePublicKeyV1, KagemushaHardwarePlatformClassV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Sole signing domain, including the final NUL.
pub const KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:app-enrollment-possession\0";
/// Maximum original platform assertion bytes before parsing or retaining it.
pub const KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1: usize = 4096;
const BODY_BYTES: usize = 371;

/// Native-selected enrollment possession transcript; decoding does not admit its scope.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppEnrollmentPossessionChallengeV1")]
pub struct KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Sole first-release version.
    pub version: u16,
    /// SHA-256 of the full original C signing message; distinct from the stable enrollment ID.
    pub enrollment_attempt_id: [u8; 32],
    /// Original client nonce from C.
    pub client_nonce: [u8; 32],
    /// Original server nonce durably reserved for C.
    pub server_nonce: [u8; 32],
    /// Model-owned binding of the independently selected account.
    pub account_binding: [u8; 32],
    /// Exact network from original C.
    pub network_id: [u8; 32],
    /// Independently admitted app-attestation authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Actual release associated with this pending identity policy.
    pub release_id: [u8; 32],
    /// Associated profile; this association grants no financial capability.
    pub hardware_profile_id: [u8; 32],
    /// Original enrollment lane.
    pub lane_id: [u8; 32],
    /// SHA256 of the original attested uncompressed SEC1 key.
    pub attested_key_id: [u8; 32],
    /// Digest of the original raw platform enrollment evidence, not a verdict DTO.
    pub raw_platform_evidence_digest: [u8; 32],
    /// Inclusive original native issue time.
    pub issued_at_ms: u64,
    /// Exclusive original expiry, never renewed on retry.
    pub expires_at_ms: u64,
}
impl KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Return E: domain || LE64(371) || version || purpose01 || eleven raw32 || times.
    /// # Errors
    /// Rejects missing scope, equal nonces, invalid version or original lifetime.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = [
            self.enrollment_attempt_id,
            self.client_nonce,
            self.server_nonce,
            self.account_binding,
            self.network_id,
            self.app_authority_policy_digest,
            self.release_id,
            self.hardware_profile_id,
            self.lane_id,
            self.attested_key_id,
            self.raw_platform_evidence_digest,
        ];
        if self.version != 1
            || fields.contains(&[0; 32])
            || self.client_nonce == self.server_nonce
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("app enrollment possession scope invalid".into());
        }
        let mut bytes = Vec::with_capacity(
            KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1.len() + 8 + BODY_BYTES,
        );
        bytes.extend_from_slice(KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1);
        bytes.extend_from_slice(&(BODY_BYTES as u64).to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(1);
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        Ok(bytes)
    }
}

/// Exact E and platform original. The native expected E is an independent argument.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppEnrollmentPossessionV1")]
pub struct KagemushaAppEnrollmentPossessionV1 {
    /// Exact native selected E.
    pub challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    /// Original platform assertion, not a callback boolean.
    pub evidence: KagemushaAppOperationApprovalEvidenceV1,
}

/// Non-cloneable checked cryptographic original; it is not a native identity or monetary owner.
pub struct KagemushaVerifiedAppEnrollmentPossessionV1 {
    challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    original: Vec<u8>,
    digest: [u8; 32],
    app_attest_counter: Option<u32>,
    app_attest_release_measurement: Option<super::KagemushaAppAttestReleaseMeasurementV1>,
}
impl KagemushaVerifiedAppEnrollmentPossessionV1 {
    /// Borrow exact E for native scope checks.
    #[must_use]
    pub const fn challenge(&self) -> &KagemushaAppEnrollmentPossessionChallengeV1 {
        &self.challenge
    }
    /// Borrow the original canonical evidence archive.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Digest of the original archive, never a capability constructor.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Checked Apple assertion counter; unrelated to a financial secure index.
    #[must_use]
    pub const fn app_attest_counter(&self) -> Option<u32> {
        self.app_attest_counter
    }
    /// Actual signed release measurement; limited37-byte assertions explicitly report unavailable.
    #[must_use]
    pub const fn app_attest_release_measurement(
        &self,
    ) -> Option<super::KagemushaAppAttestReleaseMeasurementV1> {
        self.app_attest_release_measurement
    }
}
impl KagemushaAppEnrollmentPossessionV1 {
    /// Authenticate E under independently held pending enrollment selectors.
    ///
    /// Expected scope/key/platform/RP/floor/time must come from actual admitted issuer originals,
    /// not this archive. This method verifies crypto only. Native durable consumption and identity
    /// activation remain separate, and no financial capability is returned.
    /// # Errors
    /// Rejects changed E, key, purpose, platform equation, counter, application or expiry.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        &self,
        expected: &KagemushaAppEnrollmentPossessionChallengeV1,
        original_key: &KagemushaDevicePublicKeyV1,
        platform: KagemushaHardwarePlatformClassV1,
        app_signing_identity_digest: [u8; 32],
        app_release_digest: [u8; 32],
        original_apple_counter_floor: Option<u32>,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedAppEnrollmentPossessionV1, String> {
        let message = self.challenge.canonical_signing_bytes()?;
        if self.challenge != *expected
            || trusted_now_ms < expected.issued_at_ms
            || trusted_now_ms >= expected.expires_at_ms
            || app_signing_identity_digest == [0; 32]
            || expected.attested_key_id
                != <[u8; 32]>::from(Sha256::digest(original_key.as_sec1_bytes()))
        {
            return Err("app enrollment possession original binding differs".into());
        }
        let (counter, release_measurement) = self.evidence.authenticate_signature(
            platform,
            original_key,
            app_signing_identity_digest,
            app_release_digest,
            original_apple_counter_floor,
            &message,
        )?;
        let original =
            norito::encode_canonical(self).map_err(|_| "possession canonical encoding failed")?;
        if original.len() > KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1024 {
            return Err("possession archive oversized".into());
        }
        let mut digest = Sha256::new();
        digest.update(b"iroha:kagemusha:v1:app-enrollment-possession-original\0");
        digest.update((original.len() as u64).to_le_bytes());
        digest.update(&original);
        Ok(KagemushaVerifiedAppEnrollmentPossessionV1 {
            challenge: self.challenge,
            original,
            digest: digest.finalize().into(),
            app_attest_counter: counter,
            app_attest_release_measurement: release_measurement,
        })
    }
}

impl KagemushaAppEnrollmentPossessionChallengeV1 {
    /// Construct E from exact original C, point and full raw-attestation digest.
    ///
    /// This is a codec formatter, not issuer, native identity or monetary admission. The
    /// original C interval is retained exactly and cannot be renewed by a retry.
    /// # Errors
    /// Rejects malformed C, original point or missing raw platform attestation digest.
    pub fn from_original_enrollment(
        original: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
        key: &KagemushaDevicePublicKeyV1,
        raw_platform_evidence_digest: [u8; 32],
    ) -> Result<Self, String> {
        original.canonical_signing_bytes()?;
        key.validate().map_err(|_| "E original point rejected")?;
        let challenge = Self {
            version: 1,
            enrollment_attempt_id: original.attestation_challenge()?,
            client_nonce: original.client_nonce,
            server_nonce: original.server_nonce,
            account_binding: original.account_binding,
            network_id: original.network_id,
            app_authority_policy_digest: original.app_authority_policy_digest,
            release_id: original.release_id,
            hardware_profile_id: original.hardware_profile_id,
            lane_id: original.lane_id,
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            raw_platform_evidence_digest,
            issued_at_ms: original.issued_at_ms,
            expires_at_ms: original.expires_at_ms,
        };
        challenge.canonical_signing_bytes()?;
        Ok(challenge)
    }
}

/// Derive the single ordinary Android alias from exact C451, including its original domain.
/// This name is correlation data; native custody independently retains the generation original.
/// # Errors
/// Rejects malformed C or another platform. No retired preparation/layout is accepted.
pub fn kagemusha_ordinary_android_app_key_alias_v1(
    original: &super::KagemushaOrdinaryAppEnrollmentChallengeV1,
) -> Result<String, String> {
    use core::fmt::Write as _;

    if original.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint {
        return Err("ordinary Android alias platform differs".into());
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-app-key-alias\0");
    hash.update(original.canonical_signing_bytes()?);
    let digest: [u8; 32] = hash.finalize().into();
    let mut alias = String::from("kagemusha-ordinary-app-v1-");
    for byte in digest {
        write!(&mut alias, "{byte:02x}").map_err(|_| "ordinary alias encoding failed")?;
    }
    Ok(alias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{SigningKey, signature::Signer as _};
    fn challenge(key: &KagemushaDevicePublicKeyV1) -> KagemushaAppEnrollmentPossessionChallengeV1 {
        KagemushaAppEnrollmentPossessionChallengeV1 {
            version: 1,
            enrollment_attempt_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding: [4; 32],
            network_id: [5; 32],
            app_authority_policy_digest: [6; 32],
            release_id: [7; 32],
            hardware_profile_id: [8; 32],
            lane_id: [9; 32],
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            raw_platform_evidence_digest: [11; 32],
            issued_at_ms: 1000,
            expires_at_ms: 121_000,
        }
    }
    #[test]
    fn possession_exact_body_and_purpose_are_not_financial_approval() {
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let c = challenge(&key);
        let bytes = c.canonical_signing_bytes().unwrap();
        let offset = KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1.len();
        assert_eq!(bytes.len(), offset + 8 + 371);
        assert_eq!(&bytes[offset..offset + 8], &371u64.to_le_bytes());
        assert_eq!(bytes[offset + 10], 1);
        assert_ne!(
            KAGEMUSHA_APP_ENROLLMENT_POSSESSION_DOMAIN_V1,
            super::super::KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1
        );
        assert_eq!(
            norito::decode_from_bytes::<KagemushaAppEnrollmentPossessionChallengeV1>(
                &norito::encode_canonical(&c).unwrap()
            )
            .unwrap(),
            c
        );
    }
    #[test]
    fn possession_android_keeps_raw_signature_and_rejects_substitution_and_expiry() {
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let c = challenge(&key);
        let signature: p256::ecdsa::Signature = signing.sign(&c.canonical_signing_bytes().unwrap());
        let proof = KagemushaAppEnrollmentPossessionV1 {
            challenge: c,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        };
        let checked = proof
            .authenticate(
                &c,
                &key,
                KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                [12; 32],
                [13; 32],
                None,
                1000,
            )
            .unwrap();
        assert_eq!(
            checked.original(),
            norito::encode_canonical(&proof).unwrap()
        );
        assert_eq!(checked.app_attest_counter(), None);
        assert!(
            proof
                .authenticate(
                    &c,
                    &key,
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                    [12; 32],
                    [13; 32],
                    None,
                    121_000
                )
                .is_err()
        );
        let mut wrong = c;
        wrong.server_nonce = [13; 32];
        assert!(
            proof
                .authenticate(
                    &wrong,
                    &key,
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                    [12; 32],
                    [13; 32],
                    None,
                    1000
                )
                .is_err()
        );
        assert!(
            proof
                .authenticate(
                    &c,
                    &key,
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                    [12; 32],
                    [13; 32],
                    Some(0),
                    1000
                )
                .is_err()
        );
    }
    #[test]
    fn possession_apple_uses_exact_e_and_original_counter_and_rp() {
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let c = challenge(&key);
        let mut authenticator = vec![12; 32];
        authenticator.push(0x40);
        authenticator.extend_from_slice(&1u32.to_be_bytes());
        let mut hash = Sha256::new();
        hash.update(&authenticator);
        hash.update(Sha256::digest(c.canonical_signing_bytes().unwrap()));
        let nonce: [u8; 32] = hash.finalize().into();
        let signature: p256::ecdsa::Signature = signing.sign(&nonce);
        let der = signature.to_der();
        let mut raw = vec![0xa2, 0x71];
        raw.extend_from_slice(b"authenticatorData");
        raw.extend_from_slice(&[0x58, 37]);
        raw.extend_from_slice(&authenticator);
        raw.push(0x69);
        raw.extend_from_slice(b"signature");
        raw.extend_from_slice(&[0x58, u8::try_from(der.as_bytes().len()).unwrap()]);
        raw.extend_from_slice(der.as_bytes());
        let proof = KagemushaAppEnrollmentPossessionV1 {
            challenge: c,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                raw_assertion: raw,
            },
        };
        let checked = proof
            .authenticate(
                &c,
                &key,
                KagemushaHardwarePlatformClassV1::AppleAppAttest,
                [12; 32],
                [13; 32],
                Some(0),
                1000,
            )
            .unwrap();
        assert_eq!(checked.app_attest_counter(), Some(1));
        assert_eq!(
            checked.original(),
            norito::encode_canonical(&proof).unwrap()
        );
        for (rp, floor) in [([13; 32], Some(0)), ([12; 32], Some(1)), ([12; 32], None)] {
            assert!(
                proof
                    .authenticate(
                        &c,
                        &key,
                        KagemushaHardwarePlatformClassV1::AppleAppAttest,
                        rp,
                        [13; 32],
                        floor,
                        1000
                    )
                    .is_err()
            );
        }
        let mut changed = proof.clone();
        changed.challenge.raw_platform_evidence_digest = [14; 32];
        assert!(
            changed
                .authenticate(
                    &changed.challenge,
                    &key,
                    KagemushaHardwarePlatformClassV1::AppleAppAttest,
                    [12; 32],
                    [13; 32],
                    Some(0),
                    1000
                )
                .is_err()
        );
        changed.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
            raw_assertion: vec![0; KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1],
        };
        assert!(
            changed
                .authenticate(
                    &changed.challenge,
                    &key,
                    KagemushaHardwarePlatformClassV1::AppleAppAttest,
                    [12; 32],
                    [13; 32],
                    Some(0),
                    1000
                )
                .is_err()
        );
        assert!(
            proof
                .authenticate(
                    &c,
                    &key,
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                    [12; 32],
                    [13; 32],
                    None,
                    1000
                )
                .is_err()
        );
    }

    #[test]
    fn possession_formatter_and_alias_retain_original_c_scope_and_reject_other_roles() {
        use p256::ecdsa::signature::Verifier as _;

        use super::super::KagemushaOrdinaryAppEnrollmentChallengeV1;
        let signing = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let mut c = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            enrollment_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding: [4; 32],
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
            hardware_epoch: 1,
            issued_at_ms: 1000,
            expires_at_ms: 121_000,
        };
        let e = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            &c, &key, [14; 32],
        )
        .unwrap();
        assert_eq!(
            (e.issued_at_ms, e.expires_at_ms),
            (c.issued_at_ms, c.expires_at_ms)
        );
        assert_eq!(e.enrollment_attempt_id, c.attestation_challenge().unwrap());
        assert_ne!(e.enrollment_attempt_id, c.enrollment_id);
        assert_eq!(e.raw_platform_evidence_digest, [14; 32]);
        assert_eq!(e.app_authority_policy_digest, c.app_authority_policy_digest);
        assert_eq!(
            super::super::kagemusha_ordinary_app_enrollment_possession_message_v1(
                &c, &key, [14; 32]
            )
            .unwrap(),
            e.canonical_signing_bytes().unwrap()
        );
        let original_message = e.canonical_signing_bytes().unwrap();
        let original_signature: p256::ecdsa::Signature = signing.sign(&original_message);
        for field in 0..4 {
            let mut substituted = c;
            match field {
                0 => substituted.hardware_epoch += 1,
                1 => substituted.financial_authority_commitment[0] ^= 1,
                2 => substituted.suite_id[0] ^= 1,
                _ => substituted.trust_policy_digest[0] ^= 1,
            }
            let changed = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &substituted,
                &key,
                [14; 32],
            )
            .unwrap();
            assert_ne!(changed.canonical_signing_bytes().unwrap(), original_message);
            assert!(
                signing
                    .verifying_key()
                    .verify(
                        &changed.canonical_signing_bytes().unwrap(),
                        &original_signature,
                    )
                    .is_err()
            );
            assert_ne!(
                substituted.attestation_challenge().unwrap(),
                c.attestation_challenge().unwrap()
            );
        }
        let mut stable_id = e;
        stable_id.enrollment_attempt_id = c.enrollment_id;
        let stable_signature: p256::ecdsa::Signature =
            signing.sign(&stable_id.canonical_signing_bytes().unwrap());
        assert!(
            signing
                .verifying_key()
                .verify(&original_message, &stable_signature)
                .is_err()
        );
        let alias = kagemusha_ordinary_android_app_key_alias_v1(&c).unwrap();
        assert!(alias.starts_with("kagemusha-ordinary-app-v1-"));
        assert_eq!(alias.len(), "kagemusha-ordinary-app-v1-".len() + 64);
        assert!(
            alias["kagemusha-ordinary-app-v1-".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        c.server_nonce = [15; 32];
        assert_ne!(
            kagemusha_ordinary_android_app_key_alias_v1(&c).unwrap(),
            alias
        );
        assert!(
            KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &c, &key, [0; 32]
            )
            .is_err()
        );
        c.platform_class = KagemushaHardwarePlatformClassV1::AppleAppAttest;
        assert!(kagemusha_ordinary_android_app_key_alias_v1(&c).is_err());
        c.client_nonce = c.server_nonce;
        assert!(
            KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                &c, &key, [14; 32]
            )
            .is_err()
        );
    }
}
