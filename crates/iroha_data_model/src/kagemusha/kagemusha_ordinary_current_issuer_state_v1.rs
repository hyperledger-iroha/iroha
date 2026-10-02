//! Fresh issuer-signed current ordinary enrollment state, joined to complete originals.
//!
//! These types check signatures and exact original joins. Only the Core durable journal
//! may sign a reply after its current-state CAS commits. Native code must durably reserve
//! the fresh query nonce, prove current hardware-key possession over this complete reply,
//! and fsync before exposing identity. This codec grants no financial or native authority.

use super::{
    KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
    KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
    KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::Signature;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Maximum complete canonical current-state reply before decoding.
pub const KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum original signed lifetime; native retry cannot renew it.
pub const KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_LIFETIME_MS_V1: u64 = 120_000;
/// Sole fixed raw signing-body width: version, state, seventeen raw32, epoch and times.
pub const KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_BODY_BYTES_V1: usize = 571;
/// Separate issuer signature domain; a C/raw/credential signature cannot serve as state.
pub const KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-current-issuer-state\0";

/// Actual committed enrollment lifecycle, never inferred from a local credential.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(tag = "state", content = "payload")]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryEnrollmentStateV1")]
pub enum KagemushaOrdinaryEnrollmentStateV1 {
    /// Current issuer record permits a separately qualified native identity open.
    Active,
    /// Original enrollment is replaced by the exact distinct successor.
    Rotated,
    /// Original enrollment is terminally retired.
    Retired,
}

/// Model-owned current-state statement derived from already checked complete originals.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCurrentIssuerStateSubjectV1")]
pub struct KagemushaOrdinaryCurrentIssuerStateSubjectV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual lifecycle read from the durable issuer journal.
    pub state: KagemushaOrdinaryEnrollmentStateV1,
    /// Fresh nonce reserved by native OS custody before the current query.
    pub query_nonce: [u8; 32],
    /// Original C/credential enrollment identity.
    pub enrollment_id: [u8; 32],
    /// Model-owned account binding, separate from any caller label.
    pub account_binding: [u8; 32],
    /// Independently checked genesis-derived network.
    pub network_id: [u8; 32],
    /// Exact original enrollment lane.
    pub lane_id: [u8; 32],
    /// Independently governed current policy identity.
    pub identity_policy_id: [u8; 32],
    /// SHA256 of the complete current signed policy archive.
    pub policy_original_digest: [u8; 32],
    /// SHA256 of the independently installed complete governance root archive.
    pub authority_original_digest: [u8; 32],
    /// SHA256 of complete original C515, including its issuer signature.
    pub preparation_original_digest: [u8; 32],
    /// SHA256 of complete original raw314, including its issuer signature.
    pub raw_admission_original_digest: [u8; 32],
    /// SHA256 of the complete canonical untouched platform container.
    pub platform_original_digest: [u8; 32],
    /// SHA256 of complete original E archive, including actual possession evidence.
    pub possession_original_digest: [u8; 32],
    /// SHA256 of complete original signed credential archive.
    pub credential_original_digest: [u8; 32],
    /// Actual original attested hardware key identity.
    pub attested_key_id: [u8; 32],
    /// Sole model-derived reference to that exact hardware key.
    pub app_key_reference: [u8; 32],
    /// Original separately selected financial-authority correlation; grants no spend.
    pub financial_authority_commitment: [u8; 32],
    /// Zero for Active/Retired; exact different enrollment ID for Rotated.
    pub successor_enrollment_id: [u8; 32],
    /// Positive durable monotonic lifecycle epoch; the native retained floor cannot regress.
    pub state_epoch: u64,
    /// Inclusive original issuer issue time, selected at actual durable state read.
    pub issued_at_ms: u64,
    /// Exclusive original expiry, also bounded by current credential and policy.
    pub expires_at_ms: u64,
}

impl KagemushaOrdinaryCurrentIssuerStateSubjectV1 {
    /// Derive every enrollment/root/key join under the complete independently checked issuer body.
    /// The installed namespace is independent of the reply; the governed cap only restricts DATA.
    /// This data-only builder does not read or commit state and cannot create an issuer signature.
    /// # Errors
    /// Rejects issuer/namespace/policy/history/time mismatch, a governed-cap excess,
    /// missing nonce/epoch or invalid state relation.
    #[allow(clippy::too_many_arguments)]
    pub fn for_committed_enrollment(
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        issuer: &KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        historical: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        query_nonce: [u8; 32],
        state: KagemushaOrdinaryEnrollmentStateV1,
        state_epoch: u64,
        successor_enrollment_id: [u8; 32],
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, String> {
        historical.recheck_current(policy, issued_at_ms)?;
        let value = Self::joined(
            policy,
            historical,
            query_nonce,
            state,
            state_epoch,
            successor_enrollment_id,
            issued_at_ms,
            expires_at_ms,
        );
        value.validate_issuer_lifetime(
            policy,
            issuer,
            independently_installed_lane_namespace,
            issued_at_ms,
        )?;
        value.validate_interval(policy, historical, issued_at_ms)?;
        value.canonical_signing_bytes()?;
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    fn joined(
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        historical: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        query_nonce: [u8; 32],
        state: KagemushaOrdinaryEnrollmentStateV1,
        state_epoch: u64,
        successor_enrollment_id: [u8; 32],
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Self {
        let credential = historical.credential();
        let subject = credential.subject();
        Self {
            version: 1,
            state,
            query_nonce,
            enrollment_id: subject.enrollment_id,
            account_binding: subject.account_binding,
            network_id: subject.network_id,
            lane_id: subject.lane_id,
            identity_policy_id: policy.policy_id(),
            policy_original_digest: hash(policy.original()),
            authority_original_digest: hash(policy.authority_original()),
            preparation_original_digest: hash(historical.preparation_original()),
            raw_admission_original_digest: hash(historical.raw_admission_original()),
            platform_original_digest: hash(historical.platform_original()),
            possession_original_digest: hash(historical.possession_original()),
            credential_original_digest: hash(credential.original()),
            attested_key_id: subject.attested_key_id,
            app_key_reference: subject.app_key_reference,
            financial_authority_commitment: subject.financial_authority_commitment,
            successor_enrollment_id,
            state_epoch,
            issued_at_ms,
            expires_at_ms,
        }
    }

    fn fields(&self) -> [[u8; 32]; 17] {
        [
            self.query_nonce,
            self.enrollment_id,
            self.account_binding,
            self.network_id,
            self.lane_id,
            self.identity_policy_id,
            self.policy_original_digest,
            self.authority_original_digest,
            self.preparation_original_digest,
            self.raw_admission_original_digest,
            self.platform_original_digest,
            self.possession_original_digest,
            self.credential_original_digest,
            self.attested_key_id,
            self.app_key_reference,
            self.financial_authority_commitment,
            self.successor_enrollment_id,
        ]
    }

    // Recheck the complete private-constructor issuer owner before reading its signed cap.
    // No response-selected namespace, projected digest or caller-provided cap is accepted.
    fn validate_issuer_lifetime(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        issuer: &KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        trusted_now_ms: u64,
    ) -> Result<(), String> {
        issuer.recheck_current(
            policy,
            independently_installed_lane_namespace,
            trusted_now_ms,
        )?;
        self.expires_at_ms
            .checked_sub(self.issued_at_ms)
            .filter(|duration| {
                *duration > 0 && *duration <= issuer.policy().maximum_current_state_lifetime_ms
            })
            .ok_or("current issuer state exceeds selected issuer lifetime")?;
        Ok(())
    }

    fn validate_interval(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        historical: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        now: u64,
    ) -> Result<(), String> {
        let credential = historical.credential().subject();
        if self.issued_at_ms < credential.issued_at_ms
            || now < self.issued_at_ms
            || now >= self.expires_at_ms
            || self.expires_at_ms > credential.expires_at_ms
            || self.expires_at_ms > policy.policy().profile.expires_at_ms
            || credential
                .play_integrity
                .is_some_and(|p| self.expires_at_ms > p.refresh_before_ms)
        {
            return Err("current issuer state original interval differs".into());
        }
        Ok(())
    }

    /// Return sole domain || LE64(571) || version || lifecycle || seventeen raw32 || epoch/times.
    /// This exact fixed message is shared by Core signing and native verification.
    /// # Errors
    /// Rejects incomplete selectors, another version, invalid lifecycle or lifetime.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        let fields = self.fields();
        let successor_valid = match self.state {
            KagemushaOrdinaryEnrollmentStateV1::Active
            | KagemushaOrdinaryEnrollmentStateV1::Retired => {
                self.successor_enrollment_id == [0; 32]
            }
            KagemushaOrdinaryEnrollmentStateV1::Rotated => {
                self.successor_enrollment_id != [0; 32]
                    && self.successor_enrollment_id != self.enrollment_id
            }
        };
        if self.version != 1
            || fields[..16].contains(&[0; 32])
            || !successor_valid
            || self.state_epoch == 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_LIFETIME_MS_V1
        {
            return Err("current issuer state scope invalid".into());
        }
        let mut bytes = Vec::with_capacity(
            KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_DOMAIN_V1.len()
                + 8
                + KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_BODY_BYTES_V1,
        );
        bytes.extend_from_slice(KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_DOMAIN_V1);
        bytes.extend_from_slice(
            &(KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_BODY_BYTES_V1 as u64).to_le_bytes(),
        );
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(match self.state {
            KagemushaOrdinaryEnrollmentStateV1::Active => 1,
            KagemushaOrdinaryEnrollmentStateV1::Rotated => 2,
            KagemushaOrdinaryEnrollmentStateV1::Retired => 3,
        });
        for field in fields {
            bytes.extend_from_slice(&field);
        }
        bytes.extend_from_slice(&self.state_epoch.to_le_bytes());
        bytes.extend_from_slice(&self.issued_at_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_ms.to_le_bytes());
        Ok(bytes)
    }
}

/// Complete exact issuer signature and current-state statement; decoded data grants no identity.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryCurrentIssuerStateV1")]
pub struct KagemushaSignedOrdinaryCurrentIssuerStateV1 {
    /// Sole shared model-owned statement.
    pub subject: KagemushaOrdinaryCurrentIssuerStateSubjectV1,
    /// Exact Ed25519 signature under independently governed Core enrollment issuer.
    pub signature: Signature,
}

/// Opaque checked reply. It is neither native identity custody nor a durable-state commit owner.
pub struct KagemushaVerifiedOrdinaryCurrentIssuerStateV1 {
    subject: KagemushaOrdinaryCurrentIssuerStateSubjectV1,
    original: Vec<u8>,
    authenticated_at_ms: u64,
}

impl KagemushaSignedOrdinaryCurrentIssuerStateV1 {
    /// Encode exact bounded complete reply, including its real fixed-width signature.
    /// # Errors
    /// Rejects malformed shape, signature width or complete framing bound.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.canonical_signing_bytes()?;
        if self.signature.payload().len() != 64 {
            return Err("current issuer state signature width".into());
        }
        let bytes = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_BYTES_V1 {
            return Err("current issuer state resource bound".into());
        }
        Ok(bytes)
    }
    /// Decode sole complete canonical archive before actual signature verification.
    /// # Errors
    /// Rejects resource excess, unknown/duplicate fields, invalid lifecycle or any tail.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_BYTES_V1 {
            return Err("current issuer state resource bound".into());
        }
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != bytes {
            return Err("current issuer state noncanonical archive".into());
        }
        Ok(value)
    }
    /// Verify genuine Core signature and every complete original join under current policy/time.
    /// The native caller must own the fresh expected nonce and retained epoch floor; neither
    /// may be derived from the response. A valid Retired/Rotated reply remains non-active.
    /// # Errors
    /// Rejects foreign issuer/namespace/signer, governed-cap excess, nonce, original, key,
    /// scope, interval, epoch or trusted-time regression.
    #[expect(
        clippy::too_many_arguments,
        reason = "independent current policy, issuer, installed namespace, historical original, native nonce, retained epoch floor and trusted time must remain explicit"
    )]
    pub fn authenticate(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        issuer: &KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        historical: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        expected_native_query_nonce: &[u8; 32],
        minimum_native_state_epoch: u64,
        trusted_now_ms: u64,
    ) -> Result<KagemushaVerifiedOrdinaryCurrentIssuerStateV1, String> {
        self.subject.validate_issuer_lifetime(
            policy,
            issuer,
            independently_installed_lane_namespace,
            trusted_now_ms,
        )?;
        historical.recheck_current(policy, trusted_now_ms)?;
        self.subject
            .validate_interval(policy, historical, trusted_now_ms)?;
        if expected_native_query_nonce == &[0; 32]
            || minimum_native_state_epoch == 0
            || self.subject.query_nonce != *expected_native_query_nonce
            || self.subject.state_epoch < minimum_native_state_epoch
            || self.subject
                != KagemushaOrdinaryCurrentIssuerStateSubjectV1::joined(
                    policy,
                    historical,
                    *expected_native_query_nonce,
                    self.subject.state,
                    self.subject.state_epoch,
                    self.subject.successor_enrollment_id,
                    self.subject.issued_at_ms,
                    self.subject.expires_at_ms,
                )
        {
            return Err("current issuer state native nonce/epoch/original join differs".into());
        }
        let original = self.canonical_bytes()?;
        self.signature
            .verify(
                &policy.policy().enrollment_issuer_key,
                &self.subject.canonical_signing_bytes()?,
            )
            .map_err(|_| "current issuer state Core signature rejected")?;
        Ok(KagemushaVerifiedOrdinaryCurrentIssuerStateV1 {
            subject: self.subject,
            original,
            authenticated_at_ms: trusted_now_ms,
        })
    }
}

impl KagemushaVerifiedOrdinaryCurrentIssuerStateV1 {
    /// Borrow the actual checked lifecycle statement; reading it creates no native owner.
    #[must_use]
    pub const fn subject(&self) -> &KagemushaOrdinaryCurrentIssuerStateSubjectV1 {
        &self.subject
    }
    /// Retain the entire exact signed archive for native key proof and durable journal binding.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Recheck current active status and original lifetime at the actual native completion boundary.
    /// # Errors
    /// Rejects retirement/rotation, issuer/namespace/cap drift, changed nonce/floor,
    /// current-policy/history or time regression.
    #[expect(
        clippy::too_many_arguments,
        reason = "completion independently rechecks current policy, issuer, installed namespace, historical original, native nonce, retained epoch floor and trusted time"
    )]
    pub fn require_active(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        issuer: &KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        historical: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        expected_native_query_nonce: &[u8; 32],
        minimum_native_state_epoch: u64,
        trusted_now_ms: u64,
    ) -> Result<(), String> {
        if trusted_now_ms < self.authenticated_at_ms
            || self.subject.state != KagemushaOrdinaryEnrollmentStateV1::Active
        {
            return Err("current issuer state is not active at completion".into());
        }
        KagemushaSignedOrdinaryCurrentIssuerStateV1::decode_canonical_exact(&self.original)?
            .authenticate(
                policy,
                issuer,
                independently_installed_lane_namespace,
                historical,
                expected_native_query_nonce,
                minimum_native_state_epoch,
                trusted_now_ms,
            )?;
        Ok(())
    }
}

fn hash(original: &[u8]) -> [u8; 32] {
    Sha256::digest(original).into()
}

#[cfg(test)]
#[path = "kagemusha_ordinary_current_issuer_state_v1_tests.rs"]
mod tests;
