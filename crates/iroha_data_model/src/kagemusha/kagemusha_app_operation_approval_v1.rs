//! Operation-bound approval from an enrolled ordinary app key.
//!
//! This codec authenticates app approval, never hardware counters, rollback
//! resistance, a financial secret, or monetary authority. A native owner must
//! independently derive the financial subject, reserve the nonce durably, and
//! consume it under its original journal. No network request is required while
//! signing an already admitted offline operation.

use super::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaHardwareTransitionSelectionV1,
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
/// Digest domain joining exact wrapper signing bytes and original platform evidence for proofs.
/// It is not a platform signing domain and grants no admission.
pub const KAGEMUSHA_ORDINARY_APP_APPROVAL_PROOF_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-app-approval-proof-binding\0";
/// Proof transcript joining platform approval and its exact selected Integrity lease original.
/// This is not a signing domain and does not authenticate either input.
pub const KAGEMUSHA_ORDINARY_AUTHORIZATION_PROOF_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-authorization-proof-binding\0";

/// Bind platform approval to the full original of the independently admitted Integrity lease.
/// `None` denotes an original credential verdict or Apple approval. Native callers select
/// the lease from their retained reservation; this data helper grants no authority.
/// # Errors
/// Rejects an absent platform identity or a zero digest offered as a selected lease.
pub fn kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
    platform_proof_binding: [u8; 32],
    selected_lease_original_digest: Option<[u8; 32]>,
) -> Result<[u8; 32], String> {
    if platform_proof_binding == [0; 32]
        || selected_lease_original_digest.is_some_and(|digest| digest == [0; 32])
    {
        return Err("ordinary authorization proof-binding identity absent".into());
    }
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_AUTHORIZATION_PROOF_BINDING_DOMAIN_V1);
    hash.update(64_u64.to_le_bytes());
    hash.update(platform_proof_binding);
    hash.update(selected_lease_original_digest.unwrap_or([0; 32]));
    Ok(hash.finalize().into())
}

/// Borrow exact authData and DER slices using the maintained bounded CBOR parser.
/// This is data-only parsing; callers separately verify the original signature.
/// # Errors
/// Rejects malformed, oversized or trailing App Attest assertion originals.
pub fn kagemusha_app_attest_original_parts_v1(
    raw_assertion: &[u8],
) -> Result<(&[u8], &[u8]), String> {
    super::kagemusha_v1::parse_app_attest_assertion(raw_assertion)
        .map_err(|error| format!("App Attest assertion parse failed: {error:?}"))
}

/// Decode the counter from the complete original App Attest assertion.
///
/// This is a bounded data projection, not signature verification, enrollment
/// admission or ownership of a durable counter floor. Financial logical indexes
/// are independent of this Apple assertion counter.
/// # Errors
/// Rejects malformed or trailing CBOR, a different authenticator shape or flag.
pub fn kagemusha_app_attest_original_counter_v1(raw_assertion: &[u8]) -> Result<u32, String> {
    let (auth_data, _) = kagemusha_app_attest_original_parts_v1(raw_assertion)?;
    if raw_assertion.len() > KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1
        || !(37..=206).contains(&auth_data.len())
        || auth_data[..32] == [0; 32]
    {
        return Err("App Attest authenticator shape differs".into());
    }
    if auth_data.len() == 37 {
        if auth_data[32] != 0x40 {
            return Err("limited App Attest flags differ".into());
        }
    } else {
        if !matches!(auth_data[32], 0x40 | 0xc0) {
            return Err("extended App Attest flags differ".into());
        }
        super::kagemusha_v1::parse_app_attest_assertion_extensions(auth_data)
            .map_err(|_| "App Attest release extension shape differs")?;
    }
    Ok(u32::from_be_bytes(
        auth_data[33..37]
            .try_into()
            .map_err(|_| "App Attest counter malformed")?,
    ))
}

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
    /// Approve the exact native prepared State before candidate and terminal generation.
    PrepareTransition,
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

    /// Return the exact bytes consumed by Android `SHA256withECDSA`.
    ///
    /// `DOMAIN || LE64(275) || LE16(version) || purpose(01/02) || eight raw32 fields
    /// || LE64(issued) || LE64(expires)`. The subject digest binds the full
    /// existing S, including its domain and length, rather than a Norito frame.
    /// # Errors
    /// Rejects missing selectors, another version, invalid subject or time interval.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let subject = self.canonical_subject_signing_bytes()?;
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
            KagemushaAppOperationApprovalPurposeV1::PrepareTransition => bytes.push(2),
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
    /// Sole purpose-selected signing subject, preserving the existing domain and fixed layout.
    /// # Errors
    /// Rejects a terminal subject in the preparation phase or an incomplete terminal subject.
    pub fn canonical_subject_signing_bytes(&self) -> Result<Vec<u8>, String> {
        match self.purpose {
            KagemushaAppOperationApprovalPurposeV1::MonetaryTransition => {
                if matches!(
                    self.subject.operation_kind,
                    super::KagemushaOperationKindV1::MintFold
                        | super::KagemushaOperationKindV1::ReceiveFold
                ) {
                    self.subject
                        .canonical_ordinary_incoming_terminal_signing_bytes()
                } else {
                    self.subject.canonical_signing_bytes()
                }
            }
            KagemushaAppOperationApprovalPurposeV1::PrepareTransition => {
                self.subject.canonical_prepare_signing_bytes()
            }
        }
        .map_err(|error| error.to_string())
    }
}

/// Original ordinary-app platform approval. These bytes do not carry a `StateGuard`.
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
    /// Original `SHA256withECDSA` DER from the enrolled Android key.
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

/// Bind exact canonical wrapper signing bytes and original platform evidence for a proof.
/// The original Norito/WAL digest remains separate; this data helper is neither signature
/// verification nor a new platform signing grammar.
/// # Errors
/// Rejects another wrapper shape or missing/oversized original evidence.
pub fn kagemusha_ordinary_app_approval_proof_binding_digest_v1(
    approval: &KagemushaAppOperationApprovalV1,
) -> Result<[u8; 32], String> {
    let signing = approval.challenge.canonical_signing_bytes()?;
    let (tag, original) = match &approval.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der }
            if (8..=72).contains(&signature_der.len()) =>
        {
            (1_u8, signature_der.as_slice())
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion }
            if !raw_assertion.is_empty()
                && raw_assertion.len() <= KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1 =>
        {
            (2_u8, raw_assertion.as_slice())
        }
        _ => return Err("ordinary app proof-binding evidence bound differs".into()),
    };
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_APP_APPROVAL_PROOF_BINDING_DOMAIN_V1);
    hash.update((signing.len() as u64).to_le_bytes());
    hash.update(signing);
    hash.update([tag]);
    hash.update((original.len() as u64).to_le_bytes());
    hash.update(original);
    Ok(hash.finalize().into())
}

/// Fixed ordinary Apple profile bounds derived from the exact two release-extension keys.
/// A 128-byte UTF-8 bundle version gives a 169-byte suffix and 206-byte authenticator data.
/// The outer two-key map with maximum canonical DER72 is at most 311 bytes.
pub const KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1: usize = 311;

/// Parsed same-original Apple data, without signature, release or native authority.
/// The release measurement describes only the signed fields checked against the supplied digest.
#[derive(Debug, Clone, Copy)]
pub struct KagemushaOrdinaryAppleOriginalPartsV1<'a> {
    /// Entire original authenticator byte string, including any signed extension map.
    pub authenticator_data: &'a [u8],
    /// Entire original canonical DER byte string; high-S is not normalized here.
    pub signature_der: &'a [u8],
    /// Explicitly unavailable for the limited 37-byte form.
    pub release_measurement: super::KagemushaAppAttestReleaseMeasurementV1,
    /// Parsed category only when extensions were checked against the supplied release digest.
    pub validation_category: Option<u32>,
    /// Parsed original version only when extensions were checked against that same digest.
    pub bundle_version: Option<&'a str>,
}

/// Parse exact bounded Apple originals and correlate signed release fields to an external digest.
/// This formatter/parser never authenticates a signature or constructs an enrolled/native owner.
/// # Errors
/// Rejects malformed/nonminimal/duplicate/trailing CBOR, wrong flags or a different measured
/// release. The limited form returns explicit unavailable measurement even without a release
/// digest; signature authentication separately requires its independently governed policy.
pub fn kagemusha_ordinary_apple_original_parts_v1(
    raw: &[u8],
    expected_app_release_digest: [u8; 32],
) -> Result<KagemushaOrdinaryAppleOriginalPartsV1<'_>, String> {
    if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1 {
        return Err("ordinary Apple original/release bound differs".into());
    }
    let (auth, der) = super::kagemusha_v1::parse_app_attest_assertion(raw)
        .map_err(|_| "ordinary Apple original CBOR differs")?;
    if der.len() > 72 || !(37..=206).contains(&auth.len()) {
        return Err("ordinary Apple original fields exceed fixed profile".into());
    }
    let (measurement, category, version) = if auth.len() == 37 {
        if auth[32] != 0x40 {
            return Err("ordinary Apple limited flags differ".into());
        }
        (
            super::KagemushaAppAttestReleaseMeasurementV1::Unavailable,
            None,
            None,
        )
    } else {
        if expected_app_release_digest == [0; 32] {
            return Err("ordinary Apple measured release policy absent".into());
        }
        if !matches!(auth[32], 0x40 | 0xc0) {
            return Err("ordinary Apple extension flag absent".into());
        }
        let ext = super::kagemusha_v1::parse_app_attest_assertion_extensions(auth)
            .map_err(|_| "ordinary Apple release extensions malformed")?;
        ext.verify_release_digest(expected_app_release_digest)
            .map_err(|_| "ordinary Apple signed release differs")?;
        (
            super::KagemushaAppAttestReleaseMeasurementV1::SignedExtensions,
            Some(ext.validation_category()),
            Some(ext.bundle_version()),
        )
    };
    Ok(KagemushaOrdinaryAppleOriginalPartsV1 {
        authenticator_data: auth,
        signature_der: der,
        release_measurement: measurement,
        validation_category: category,
        bundle_version: version,
    })
}

/// Successful signature and original enrollment correlation, without a spending grant.
/// It has no public constructor, decoder or clone implementation. The genuine
/// native Guard owner must additionally match its current release/credential,
/// financial subject, normalized statement, time authority and replay journal.
pub struct KagemushaVerifiedAppOperationApprovalV1 {
    challenge: KagemushaAppOperationApprovalChallengeV1,
    original: Vec<u8>,
    digest: [u8; 32],
    proof_binding_digest: [u8; 32],
    app_attest_counter: Option<u32>,
    app_attest_release_measurement: Option<super::KagemushaAppAttestReleaseMeasurementV1>,
    authenticated_at_ms: u64,
    valid_until_ms: u64,
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
    /// Model digest of exact verified wrapper signing bytes and original platform evidence.
    #[must_use]
    pub const fn proof_binding_digest(&self) -> [u8; 32] {
        self.proof_binding_digest
    }
    /// Original verified Apple counter, separate from financial logical indexes.
    #[must_use]
    pub const fn app_attest_counter(&self) -> Option<u32> {
        self.app_attest_counter
    }
    /// Signed Apple release fields, or explicit unavailable measurement for the limited form.
    /// This fact does not qualify a monetary profile or establish native current ownership.
    #[must_use]
    pub const fn app_attest_release_measurement(
        &self,
    ) -> Option<super::KagemushaAppAttestReleaseMeasurementV1> {
        self.app_attest_release_measurement
    }
    /// Check the original exclusive interval without renewing it.
    /// # Errors
    /// Rejects an observation before issue or at/after expiry.
    pub fn recheck_at_trusted_time(&self, trusted_now_ms: u64) -> Result<(), String> {
        if trusted_now_ms < self.authenticated_at_ms
            || trusted_now_ms < self.challenge.issued_at_ms
            || trusted_now_ms >= self.valid_until_ms
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
        let selection = enrollment.subject();
        let valid_until_ms = selection
            .play_integrity
            .map_or(selection.expires_at_ms, |pi| {
                selection.expires_at_ms.min(pi.refresh_before_ms)
            });
        self.authenticate_bound(
            expected,
            enrollment,
            original_app_attest_counter_floor,
            trusted_now_ms,
            valid_until_ms,
        )
    }

    /// Verify the same original approval with a separately authenticated periodic Integrity lease.
    /// This accepts only an opaque lease for the same enrolled credential; it does not renew
    /// the operation nonce, financial epoch, challenge or FI certificate.
    /// # Errors
    /// Rejects an unrelated/expired lease, substituted original binding or platform signature.
    pub fn authenticate_with_integrity_lease(
        &self,
        expected: &KagemushaAppOperationApprovalChallengeV1,
        enrollment: &KagemushaVerifiedOrdinaryAppCredentialV1,
        integrity_lease: &super::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
        original_app_attest_counter_floor: Option<u32>,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, String> {
        enrollment.recheck_with_integrity_lease(integrity_lease, trusted_now_ms)?;
        let valid_until_ms = enrollment
            .subject()
            .expires_at_ms
            .min(integrity_lease.subject().expires_at_ms)
            .min(integrity_lease.subject().binding.refresh_before_ms);
        self.authenticate_bound(
            expected,
            enrollment,
            original_app_attest_counter_floor,
            trusted_now_ms,
            valid_until_ms,
        )
    }

    fn authenticate_bound(
        &self,
        expected: &KagemushaAppOperationApprovalChallengeV1,
        enrollment: &KagemushaVerifiedOrdinaryAppCredentialV1,
        original_app_attest_counter_floor: Option<u32>,
        trusted_now_ms: u64,
        valid_until_ms: u64,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, String> {
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
        let (counter, release_measurement) = self.evidence.authenticate_signature(
            selection.platform_class,
            &selection.app_public_key,
            selection.app_signing_identity_digest,
            selection.app_release_digest,
            original_app_attest_counter_floor,
            &message,
        )?;
        if counter.is_some_and(|_| {
            original_app_attest_counter_floor
                .is_none_or(|floor| floor < selection.app_attest_counter_floor)
        }) {
            return Err("App Attest retained floor precedes credential".into());
        }
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
            proof_binding_digest: kagemusha_ordinary_app_approval_proof_binding_digest_v1(self)?,
            app_attest_counter: counter,
            app_attest_release_measurement: release_measurement,
            authenticated_at_ms: trusted_now_ms,
            valid_until_ms: valid_until_ms.min(self.challenge.expires_at_ms),
        };
        verified.recheck_at_trusted_time(trusted_now_ms)?;
        Ok(verified)
    }
}

impl KagemushaAppOperationApprovalEvidenceV1 {
    /// Verify the original platform signature over one independently selected message.
    /// No financial authority, enrollment trust or durable counter ownership is established.
    /// # Errors
    /// Rejects another platform equation, original key, application, counter floor or signature.
    pub fn authenticate_signature(
        &self,
        platform: super::KagemushaHardwarePlatformClassV1,
        key: &KagemushaDevicePublicKeyV1,
        app_identity: [u8; 32],
        expected_app_release_digest: [u8; 32],
        original_app_attest_counter_floor: Option<u32>,
        message: &[u8],
    ) -> Result<
        (
            Option<u32>,
            Option<super::KagemushaAppAttestReleaseMeasurementV1>,
        ),
        String,
    > {
        key.validate().map_err(|e| e.to_string())?;
        if message.is_empty()
            || message.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
            || app_identity == [0; 32]
            || expected_app_release_digest == [0; 32]
        {
            return Err("app approval message or identity absent".into());
        }
        match (self, platform) {
            (
                Self::AndroidKeystore { signature_der },
                super::KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            ) => {
                if original_app_attest_counter_floor.is_some()
                    || !(8..=72).contains(&signature_der.len())
                {
                    return Err("Android approval contains Apple counter or invalid DER".into());
                }
                KagemushaDeviceSignatureV1::from_der_normalizing_low_s(signature_der)
                    .map_err(|e| e.to_string())?
                    .verify(key, message)
                    .map_err(|e| e.to_string())?;
                Ok((None, None))
            }
            (
                Self::AppleAppAttest { raw_assertion },
                super::KagemushaHardwarePlatformClassV1::AppleAppAttest,
            ) => {
                let parts = kagemusha_ordinary_apple_original_parts_v1(
                    raw_assertion,
                    expected_app_release_digest,
                )?;
                let auth_data = parts.authenticator_data;
                let der = parts.signature_der;
                let floor = original_app_attest_counter_floor
                    .ok_or("App Attest original counter floor absent")?;
                if auth_data[..32] != app_identity {
                    return Err("App Attest application differs".into());
                }
                let counter = u32::from_be_bytes(
                    auth_data[33..37]
                        .try_into()
                        .map_err(|_| "App Attest counter malformed")?,
                );
                if counter <= floor {
                    return Err("App Attest original counter did not advance".into());
                }
                let mut nonce = Sha256::new();
                nonce.update(auth_data);
                nonce.update(Sha256::digest(message));
                let nonce: [u8; 32] = nonce.finalize().into();
                KagemushaDeviceSignatureV1::from_der_normalizing_low_s(der)
                    .map_err(|e| e.to_string())?
                    .verify(key, &nonce)
                    .map_err(|e| e.to_string())?;
                Ok((Some(counter), Some(parts.release_measurement)))
            }
            _ => Err("app approval platform equation differs".into()),
        }
    }
}

#[cfg(test)]
#[path = "ordinary_apple_release_original_tests.rs"]
mod ordinary_apple_release_original_tests;

#[cfg(test)]
mod authorization_binding_tests {
    use super::*;

    #[test]
    fn authorization_binding_uses_exact_fixed_transcript_and_selected_original() {
        let platform = [1; 32];
        let lease = [2; 32];
        let actual = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            platform,
            Some(lease),
        )
        .unwrap();
        let mut original = KAGEMUSHA_ORDINARY_AUTHORIZATION_PROOF_BINDING_DOMAIN_V1.to_vec();
        original.extend_from_slice(&64_u64.to_le_bytes());
        original.extend_from_slice(&platform);
        original.extend_from_slice(&lease);
        assert_eq!(actual, <[u8; 32]>::from(Sha256::digest(original)));
        assert_ne!(
            actual,
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(platform, None)
                .unwrap()
        );
        assert_ne!(
            actual,
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                platform,
                Some([3; 32])
            )
            .unwrap()
        );
        assert_ne!(
            actual,
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                [4; 32],
                Some(lease)
            )
            .unwrap()
        );
    }

    #[test]
    fn authorization_binding_rejects_zero_identity_and_zero_selected_lease() {
        assert!(
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1([0; 32], None)
                .is_err()
        );
        assert!(
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                [1; 32],
                Some([0; 32])
            )
            .is_err()
        );
    }
}
