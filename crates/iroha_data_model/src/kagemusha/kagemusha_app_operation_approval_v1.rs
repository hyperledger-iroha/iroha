//! Operation-bound approval from an enrolled ordinary app key.
//!
//! This codec authenticates app approval, never hardware counters, rollback
//! resistance, a financial secret, or monetary authority. A native owner must
//! independently derive the financial subject, reserve the nonce durably, and
//! consume it under its original journal. No network request is required while
//! signing an already admitted offline operation.

use super::{
    KagemushaAppAttestHardwareTransitionSelectionV1, KagemushaDevicePublicKeyV1,
    KagemushaDeviceSignatureV1, KagemushaHardwareTransitionSelectionV1,
    KagemushaVerifiedOrdinaryAppCredentialV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Exact signing domain, including its final NUL.
pub const KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:app-operation-approval\0";
/// Hard canonical archive ceiling; it carries one fixed subject and DER signature.
pub const KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1: usize = 4096;
/// Maximum interval of one already authorized native approval attempt.
/// This bound does not create an offline epoch, a time source, or a spending lease.
pub const KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1: u64 = 120_000;
const BODY_BYTES: usize = 2 + 1 + 8 * 32 + 2 * 8;
const ACCOUNT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-approval-account\0";
const ORIGINAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-operation-approval-original\0";

/// Model-owned absolute ranges in the sole platform approval signing message.
#[derive(Debug, Clone, Copy)]
pub struct KagemushaAppOperationApprovalSigningLayoutV1;
impl KagemushaAppOperationApprovalSigningLayoutV1 {
    /// Exact signing domain range, including NUL.
    pub const DOMAIN: core::ops::Range<usize> = 0..KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1.len();
    /// LE64(275) fixed body-length range.
    pub const BODY_LENGTH: core::ops::Range<usize> = Self::DOMAIN.end..Self::DOMAIN.end + 8;
    /// Complete unsigned body range.
    pub const BODY: core::ops::Range<usize> =
        Self::BODY_LENGTH.end..Self::BODY_LENGTH.end + BODY_BYTES;
    /// Complete signature message width.
    pub const TOTAL_BYTES: usize = Self::BODY.end;
    /// LE16 first-release version.
    pub const VERSION: core::ops::Range<usize> = Self::BODY.start..Self::BODY.start + 2;
    /// One-byte monetary purpose tag.
    pub const PURPOSE: core::ops::Range<usize> = Self::BODY.start + 2..Self::BODY.start + 3;
    /// Exact raw32 `operation_id` slot.
    pub const OPERATION_ID: core::ops::Range<usize> = Self::BODY.start + 3..Self::BODY.start + 35;
    /// Exact raw32 `nonce` slot.
    pub const NONCE: core::ops::Range<usize> = Self::BODY.start + 35..Self::BODY.start + 67;
    /// Exact raw32 `account_binding` slot.
    pub const ACCOUNT_BINDING: core::ops::Range<usize> =
        Self::BODY.start + 67..Self::BODY.start + 99;
    /// Exact raw32 `authority_policy_digest` slot.
    pub const AUTHORITY_POLICY_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 99..Self::BODY.start + 131;
    /// Exact raw32 `attested_key_id` slot.
    pub const ATTESTED_KEY_ID: core::ops::Range<usize> =
        Self::BODY.start + 131..Self::BODY.start + 163;
    /// Exact raw32 `enrollment_digest` slot.
    pub const ENROLLMENT_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 163..Self::BODY.start + 195;
    /// Exact raw32 `subject_signing_digest` slot.
    pub const SUBJECT_SIGNING_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 195..Self::BODY.start + 227;
    /// Exact raw32 `normalized_guard_digest` slot.
    pub const NORMALIZED_GUARD_DIGEST: core::ops::Range<usize> =
        Self::BODY.start + 227..Self::BODY.start + 259;
    /// Exact LE64 `issued_at_ms` slot.
    pub const ISSUED_AT_MS: core::ops::Range<usize> =
        Self::BODY.start + 259..Self::BODY.start + 267;
    /// Exact LE64 `expires_at_ms` slot.
    pub const EXPIRES_AT_MS: core::ops::Range<usize> =
        Self::BODY.start + 267..Self::BODY.start + 275;
}

/// Purpose of the sole first-release monetary approval wrapper.
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
#[norito(
    tag = "purpose",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppOperationApprovalPurposeV1")]
pub enum KagemushaAppOperationApprovalPurposeV1 {
    /// Approve the exact native monetary transition; its operation is in the signed subject.
    MonetaryTransition,
}

/// Public native challenge. Decoding or constructing it grants no authority.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppOperationApprovalChallengeV1")]
pub struct KagemushaAppOperationApprovalChallengeV1 {
    /// Sole first-release wrapper version.
    pub version: u16,
    /// Domain purpose selected by the original native operation owner.
    pub purpose: KagemushaAppOperationApprovalPurposeV1,
    /// Exact independently retained native operation identity.
    pub operation_id: [u8; 32],
    /// Fresh native nonce reserved durably before any app signature exposure.
    pub nonce: [u8; 32],
    /// Domain-separated digest of the independently held wallet account.
    pub account_binding: [u8; 32],
    /// Exact independently governed app-attestation authority policy digest.
    pub authority_policy_digest: [u8; 32],
    /// SHA-256 of the original enrolled uncompressed SEC1 P-256 public key.
    pub attested_key_id: [u8; 32],
    /// Model-owned digest of the original verified app enrollment certificate.
    pub enrollment_digest: [u8; 32],
    /// SHA-256 of the exact existing hardware-selection signing message S.
    pub subject_signing_digest: [u8; 32],
    /// Exact native normalized monetary Guard statement digest.
    pub normalized_guard_digest: [u8; 32],
    /// Start selected under the original native time/epoch/lease policy.
    pub issued_at_ms: u64,
    /// Exclusive expiry under that same original policy.
    pub expires_at_ms: u64,
    /// Sole existing financial subject, including release, key, operation and logical indexes.
    pub subject: KagemushaHardwareTransitionSelectionV1,
}

impl KagemushaAppOperationApprovalChallengeV1 {
    /// Bind an actual wallet account without introducing another account text grammar.
    #[must_use]
    pub fn account_binding(account: &AccountId) -> [u8; 32] {
        let bytes = account.encode();
        let mut hash = Sha256::new();
        hash.update(ACCOUNT_DOMAIN);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        hash.finalize().into()
    }

    /// Return the exact bytes consumed by Android SHA256withECDSA.
    ///
    /// `DOMAIN || LE64(275) || LE16(version) || 01 || eight raw32 fields
    /// || LE64(issued) || LE64(expires)`. The subject digest binds the full
    /// existing S, including its domain and length, rather than a Norito frame.
    /// # Errors
    /// Rejects missing selectors, another version, invalid subject or time interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let subject = self
            .subject
            .canonical_signing_bytes()
            .map_err(|e| e.to_string())?;
        if self.version != 1
            || [
                self.operation_id,
                self.nonce,
                self.account_binding,
                self.authority_policy_digest,
                self.attested_key_id,
                self.enrollment_digest,
                self.subject_signing_digest,
                self.normalized_guard_digest,
            ]
            .contains(&[0; 32])
            || self.subject_signing_digest != <[u8; 32]>::from(Sha256::digest(&subject))
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("invalid native app approval challenge".into());
        }
        let mut bytes =
            Vec::with_capacity(KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1.len() + 8 + BODY_BYTES);
        bytes.extend_from_slice(KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1);
        bytes.extend_from_slice(&(BODY_BYTES as u64).to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        match self.purpose {
            KagemushaAppOperationApprovalPurposeV1::MonetaryTransition => bytes.push(1),
        }
        for field in [
            self.operation_id,
            self.nonce,
            self.account_binding,
            self.authority_policy_digest,
            self.attested_key_id,
            self.enrollment_digest,
            self.subject_signing_digest,
            self.normalized_guard_digest,
        ] {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        Ok(bytes)
    }
}

/// Original ordinary-app platform approval. These bytes do not carry a StateGuard.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppOperationApprovalV1")]
pub struct KagemushaAppOperationApprovalV1 {
    /// Exact original native-owned challenge.
    pub challenge: KagemushaAppOperationApprovalChallengeV1,
    /// Exact original platform evidence under its own equation.
    pub evidence: KagemushaAppOperationApprovalEvidenceV1,
}

/// Original platform equation; these variants cannot be relabeled.
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
#[norito(
    tag = "platform",
    content = "evidence",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppOperationApprovalEvidenceV1")]
pub enum KagemushaAppOperationApprovalEvidenceV1 {
    /// Original SHA256withECDSA DER from the enrolled Android key.
    AndroidKeystore {
        /// Unmodified canonical DER, not a software-normalized replacement original.
        signature_der: Vec<u8>,
    },
    /// Original App Attest CBOR with authenticatorData and DER signature.
    AppleAppAttest {
        /// Full bounded original assertion returned by the platform.
        raw_assertion: Vec<u8>,
    },
}

/// Successful signature and original enrollment correlation, without a spending grant.
/// It has no public constructor, decoder or clone implementation. The genuine
/// native Guard owner must additionally match its current release/credential,
/// financial subject, normalized statement, time authority and replay journal.
pub struct KagemushaVerifiedAppOperationApprovalV1 {
    challenge: KagemushaAppOperationApprovalChallengeV1,
    original: Vec<u8>,
    digest: [u8; 32],
    app_attest_counter: Option<u32>,
}
impl KagemushaVerifiedAppOperationApprovalV1 {
    /// Borrow the exact checked challenge for independent native comparisons.
    #[must_use]
    pub const fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        &self.challenge
    }
    /// Borrow the canonical original archive, including the unmodified DER signature.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Domain-separated digest of that exact original archive.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Original verified Apple counter, separate from financial logical indexes.
    #[must_use]
    pub const fn app_attest_counter(&self) -> Option<u32> {
        self.app_attest_counter
    }
    /// Check the original exclusive interval without renewing it.
    /// # Errors
    /// Rejects an observation before issue or at/after expiry.
    pub fn recheck_at_trusted_time(&self, trusted_now_ms: u64) -> Result<(), String> {
        if trusted_now_ms < self.challenge.issued_at_ms
            || trusted_now_ms >= self.challenge.expires_at_ms
        {
            return Err("native app approval expired".into());
        }
        Ok(())
    }
}

impl KagemushaAppOperationApprovalV1 {
    /// Verify the exact original signature against independently held native selectors.
    ///
    /// An expected challenge must come from the native operation owner, never
    /// this response. The verified enrollment must come from the actual governed
    /// certificate. This method verifies crypto/correlation, not durable one-use
    /// consumption, hardware rollback properties, or financial proofs.
    /// # Errors
    /// Rejects substituted scope/key/certificate/subject/Guard/nonce, DER or interval.
    pub fn authenticate(
        &self,
        expected: &KagemushaAppOperationApprovalChallengeV1,
        enrollment: &KagemushaVerifiedOrdinaryAppCredentialV1,
        original_app_attest_counter_floor: Option<u32>,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, String> {
        enrollment.recheck_at_trusted_time(trusted_now_ms)?;
        let message = self.challenge.canonical_signing_bytes()?;
        let selection = enrollment.subject();
        let subject = &self.challenge.subject;
        if self.challenge != *expected
            || self.challenge.enrollment_digest != enrollment.digest()
            || self.challenge.authority_policy_digest != selection.app_authority_policy_digest
            || self.challenge.account_binding != selection.account_binding
            || self.challenge.attested_key_id != selection.attested_key_id
            || subject.credential_id != enrollment.digest()
            || subject.release_id != selection.release_id
            || subject.network_id.as_bytes() != &selection.network_id
            || subject.hardware_profile_id != selection.hardware_profile_id
            || subject.policy_epoch != selection.policy_epoch
            || subject.hardware_epoch_generation != selection.hardware_epoch
            || subject.lane_commitment != selection.lane_id
            || subject.app_policy_digest != enrollment.static_binding_digest()
        {
            return Err("native app approval original binding differs".into());
        }
        let counter = match (&self.evidence, selection.platform_class) {
            (
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der },
                super::KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            ) => {
                if original_app_attest_counter_floor.is_some()
                    || !(8..=72).contains(&signature_der.len())
                {
                    return Err("Android approval contains Apple counter or invalid DER".into());
                }
                KagemushaDeviceSignatureV1::from_der_normalizing_low_s(signature_der)
                    .map_err(|e| e.to_string())?
                    .verify(&selection.app_public_key, &message)
                    .map_err(|e| e.to_string())?;
                None
            }
            (
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion },
                super::KagemushaHardwarePlatformClassV1::AppleAppAttest,
            ) => {
                if raw_assertion.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1 {
                    return Err("App Attest original oversized".into());
                }
                let original = KagemushaAppAttestHardwareTransitionSelectionV1 {
                    subject: *subject,
                    raw_assertion: raw_assertion.clone(),
                };
                let (auth_data, der) = original
                    .original_assertion_components()
                    .map_err(|e| e.to_string())?;
                let floor = original_app_attest_counter_floor
                    .ok_or("App Attest original counter floor absent")?;
                let counter = u32::from_be_bytes(
                    auth_data[33..37]
                        .try_into()
                        .map_err(|_| "App Attest counter malformed")?,
                );
                if floor < selection.app_attest_counter_floor
                    || counter <= floor
                    || auth_data[..32] != selection.app_signing_identity_digest
                    || auth_data.len() != 37
                    || auth_data[32] != 0x40
                {
                    return Err("App Attest application, original floor or counter differs".into());
                }
                // Standard assertion without release extensions. Distribution identity is
                // checked in the independently governed original credential, not guessed here.
                let mut nonce = Sha256::new();
                nonce.update(auth_data);
                nonce.update(Sha256::digest(&message));
                let nonce: [u8; 32] = nonce.finalize().into();
                KagemushaDeviceSignatureV1::from_der_normalizing_low_s(der)
                    .map_err(|e| e.to_string())?
                    .verify(&selection.app_public_key, &nonce)
                    .map_err(|e| e.to_string())?;
                Some(counter)
            }
            _ => {
                return Err(
                    "app approval platform equation differs from original credential".into(),
                );
            }
        };
        let original = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1 {
            return Err("native app approval original oversized".into());
        }
        let mut hash = Sha256::new();
        hash.update(ORIGINAL_DOMAIN);
        hash.update((original.len() as u64).to_le_bytes());
        hash.update(&original);
        let verified = KagemushaVerifiedAppOperationApprovalV1 {
            challenge: self.challenge,
            original,
            digest: hash.finalize().into(),
            app_attest_counter: counter,
        };
        verified.recheck_at_trusted_time(trusted_now_ms)?;
        Ok(verified)
    }
}
