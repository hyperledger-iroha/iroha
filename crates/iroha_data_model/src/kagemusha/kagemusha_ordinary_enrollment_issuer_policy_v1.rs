//! Sole bounded ordinary enrollment issuer-policy original and wallet lane derivation.
//! A decoded policy gains authority only through the independently threshold-checked policy.
use super::KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1;
use crate::account::AccountId;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_model_base::name::Name;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Exact maximum canonical issuer-policy archive, checked before decoding.
pub const KAGEMUSHA_ORDINARY_ENROLLMENT_ISSUER_POLICY_MAX_BYTES_V1: usize = 16 * 1024;
const POLICY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-enrollment-issuer-policy\0";
const LANE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-enrollment-lane\0";

/// Complete issuer configuration committed by the threshold policy, without a circular policy ID.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryEnrollmentIssuerPolicyV1")]
pub struct KagemushaOrdinaryEnrollmentIssuerPolicyV1 {
    /// Fixed original codec version.
    pub version: u16,
    /// Exact independently selected network.
    pub network_id: [u8; 32],
    /// Independently installed namespace; account-specific lanes use the sole method below.
    pub lane_namespace_id: [u8; 32],
    /// Complete threshold profile ID, whose body excludes this issuer policy digest.
    pub identity_profile_id: [u8; 32],
    /// Exact planned profile policy epoch.
    pub planned_policy_epoch: u64,
    /// Governed planned hardware epoch; a caller cannot select it when preparing C.
    pub planned_hardware_epoch: u64,
    /// Actual Core enrollment issuer key.
    pub enrollment_issuer_key: PublicKey,
    /// Actual independently selected raw app-attestation authority key.
    pub app_authority_key: PublicKey,
    /// Original maximum pending C lifetime; never greater than 120 seconds.
    pub maximum_pending_lifetime_ms: u64,
    /// Original maximum current issuer-state reply lifetime; never greater than 120 seconds.
    pub maximum_current_state_lifetime_ms: u64,
    /// Exact credential lifetime selected by the signed trust policy and app authority.
    pub maximum_credential_lifetime_ms: u64,
}

impl KagemushaOrdinaryEnrollmentIssuerPolicyV1 {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || [
                self.network_id,
                self.lane_namespace_id,
                self.identity_profile_id,
            ]
            .contains(&[0; 32])
            || self.planned_policy_epoch == 0
            || self.planned_hardware_epoch == 0
            || self.enrollment_issuer_key.algorithm() != Algorithm::Ed25519
            || self.app_authority_key.algorithm() != Algorithm::Ed25519
            || self.enrollment_issuer_key == self.app_authority_key
            || !(1..=120000).contains(&self.maximum_pending_lifetime_ms)
            || !(1..=120000).contains(&self.maximum_current_state_lifetime_ms)
            || self.maximum_credential_lifetime_ms == 0
        {
            return Err("ordinary issuer policy original malformed".into());
        }
        Ok(())
    }
    /// Sole exact bounded canonical original; encoding alone grants no authority.
    /// # Errors
    /// Rejects malformed fields and an archive exceeding its bound.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        if norito::canonical_frame_len(self).map_err(|e| e.to_string())?
            > KAGEMUSHA_ORDINARY_ENROLLMENT_ISSUER_POLICY_MAX_BYTES_V1
        {
            return Err("ordinary issuer policy archive bound".into());
        }
        norito::encode_canonical(self).map_err(|e| e.to_string())
    }
    /// Decode only the sole exact canonical archive with a predecode resource limit.
    /// # Errors
    /// Rejects unknown versions, malformed fields, oversized archives and trailing/noncanonical bytes.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_ORDINARY_ENROLLMENT_ISSUER_POLICY_MAX_BYTES_V1
        {
            return Err("ordinary issuer policy archive bound".into());
        }
        let v: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        if v.canonical_bytes()? != bytes {
            return Err("ordinary issuer policy not canonical".into());
        }
        Ok(v)
    }
    /// Sole domain-separated digest committed by the threshold identity policy.
    /// # Errors
    /// Rejects malformed or oversized original fields.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        let b = self.canonical_bytes()?;
        let mut h = Sha256::new();
        h.update(POLICY_DOMAIN);
        h.update((b.len() as u64).to_le_bytes());
        h.update(b);
        Ok(h.finalize().into())
    }
    /// Sole account/FI lane preimage. Only a checked issuer owner may install the result.
    /// # Errors
    /// Rejects a oversized canonical FI identifier or an oversized canonical account archive.
    pub fn derive_enrollment_lane(
        &self,
        fi_id: &Name,
        account: &AccountId,
    ) -> Result<[u8; 32], String> {
        self.validate()?;
        if norito::canonical_frame_len(fi_id).map_err(|e| e.to_string())? > 1024 {
            return Err("ordinary issuer canonical FI bound".into());
        }
        let fi = norito::encode_canonical(fi_id).map_err(|e| e.to_string())?;
        if norito::canonical_frame_len(account).map_err(|e| e.to_string())? > 16 * 1024 {
            return Err("ordinary issuer canonical account bound".into());
        }
        let a = norito::encode_canonical(account).map_err(|e| e.to_string())?;
        let mut h = Sha256::new();
        h.update(LANE_DOMAIN);
        h.update(self.network_id);
        h.update(self.lane_namespace_id);
        h.update((fi.len() as u64).to_le_bytes());
        h.update(fi);
        h.update((a.len() as u64).to_le_bytes());
        h.update(a);
        Ok(h.finalize().into())
    }
    /// Authenticate the complete issuer original against current independent threshold policy and namespace.
    /// # Errors
    /// Rejects any original/digest/key/profile/network/epoch/namespace/lifetime mismatch or expired policy.
    pub fn authenticate_under_policy(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        now: u64,
    ) -> Result<KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1, String> {
        policy.recheck_at_trusted_time(now)?;
        self.validate()?;
        let p = policy.policy();
        if self.canonical_digest()? != p.enrollment_issuer_policy_digest
            || self.network_id != *p.network_id.as_bytes()
            || self.lane_namespace_id != independently_installed_lane_namespace
            || independently_installed_lane_namespace == [0; 32]
            || self.identity_profile_id != p.profile.identity_profile_id
            || self.planned_policy_epoch != p.profile.policy_epoch
            || self.enrollment_issuer_key != p.enrollment_issuer_key
            || self.app_authority_key != p.app_authority_key
            || self.maximum_credential_lifetime_ms != p.trust.maximum_credential_lifetime_ms
            || self.maximum_credential_lifetime_ms > p.app_authority_maximum_lifetime_ms
        {
            return Err("ordinary issuer complete original policy join differs".into());
        }
        Ok(KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1 {
            subject: self.clone(),
            original: self.canonical_bytes()?,
            identity_policy_original: policy.original().to_vec(),
            identity_authority_original: policy.authority_original().to_vec(),
            authenticated_at_ms: now,
        })
    }
}

/// Private-constructor, non-Clone checked complete issuer originals; no CAS or ledger admission.
pub struct KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1 {
    subject: KagemushaOrdinaryEnrollmentIssuerPolicyV1,
    original: Vec<u8>,
    identity_policy_original: Vec<u8>,
    identity_authority_original: Vec<u8>,
    authenticated_at_ms: u64,
}
impl KagemushaAuthenticatedOrdinaryEnrollmentIssuerPolicyV1 {
    /// Complete canonical issuer original for actual installation and durable recovery.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Immutable actual checked issuer fields, without a public constructor.
    #[must_use]
    pub const fn policy(&self) -> &KagemushaOrdinaryEnrollmentIssuerPolicyV1 {
        &self.subject
    }
    /// Reauthenticate exact policy/authority originals, namespace and trusted-time floor before use.
    /// # Errors
    /// Rejects drift, expiry and time regression; this floor does not authorize wall-clock time.
    pub fn recheck_current(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        now: u64,
    ) -> Result<(), String> {
        if now < self.authenticated_at_ms
            || self.identity_policy_original != policy.original()
            || self.identity_authority_original != policy.authority_original()
        {
            return Err("ordinary issuer current original policy/time differs".into());
        }
        self.subject
            .authenticate_under_policy(policy, independently_installed_lane_namespace, now)
            .map(|_| ())
    }
    /// Check a known signed archive using the same complete originals in a closed DATA view.
    /// The actual Core signature is verified before its issue time is evaluated. A later
    /// authentication floor of this live issuer is not backdated or returned to the caller.
    /// The local historical issuer/reply evaluation never escapes and grants no CURRENT owner.
    /// # Errors
    /// Refuses issuer/policy/root/namespace/cap drift, bad signature or original interval/joins.
    pub(super) fn authenticate_archived_current_reply_original_data(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        historical: &super::KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
        reply_original: &[u8],
        expected_native_query_nonce: &[u8; 32],
        minimum_native_state_epoch: u64,
    ) -> Result<(), String> {
        if self.original != self.subject.canonical_bytes()?
            || self.identity_policy_original != policy.original()
            || self.identity_authority_original != policy.authority_original()
        {
            return Err("archived current issuer complete originals differ".into());
        }
        let reply = super::KagemushaSignedOrdinaryCurrentIssuerStateV1::decode_canonical_exact(
            reply_original,
        )?;
        reply
            .signature
            .verify(
                &policy.policy().enrollment_issuer_key,
                &reply.subject.canonical_signing_bytes()?,
            )
            .map_err(|_| "archived current reply Core signature rejected")?;
        let signed_issue = reply.subject.issued_at_ms;
        let evaluation = self.subject.authenticate_under_policy(
            policy,
            independently_installed_lane_namespace,
            signed_issue,
        )?;
        if evaluation.original() != self.original() {
            return Err("archived current issuer complete original differs".into());
        }
        reply
            .authenticate(
                policy,
                &evaluation,
                independently_installed_lane_namespace,
                historical,
                expected_native_query_nonce,
                minimum_native_state_epoch,
                signed_issue,
            )
            .map(|_| ())
    }

    /// Derive the sole account/FI lane under the complete current independently installed originals.
    /// # Errors
    /// Rejects drift/time/namespace or oversized canonical FI/account originals.
    pub fn derive_enrollment_lane(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        independently_installed_lane_namespace: [u8; 32],
        fi_id: &Name,
        account: &AccountId,
        now: u64,
    ) -> Result<[u8; 32], String> {
        self.recheck_current(policy, independently_installed_lane_namespace, now)?;
        self.subject.derive_enrollment_lane(fi_id, account)
    }
}

#[cfg(test)]
#[path = "kagemusha_ordinary_enrollment_issuer_policy_v1_tests.rs"]
mod tests;
