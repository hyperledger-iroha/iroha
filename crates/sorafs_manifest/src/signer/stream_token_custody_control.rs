//! Public governed StreamToken custody control, independent of per-token operation state.
//!
//! This canonical payload contains no token body, role signature or signing reservation. The
//! native ledger owns authorization, deterministic provenance and compare-and-swap publication.
use super::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyAuthorityV1, SignerCustodyBindingV1,
        SignerCustodyErrorV1, SignerCustodyTrustV1, validate_binding, validate_trust_binding,
    },
    protocol::{SignerPurposeBindingV1, SignerRoleV1},
};
use iroha_crypto::PublicKey;
use norito::codec::{Decode, Encode};

/// Maximum complete canonical policy or control frame, including its header and padding.
pub const STREAM_TOKEN_CUSTODY_CONTROL_MAX_BYTES_V1: usize = 16 * 1024;

/// Exact signer binding and independently governed attestation trust.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::stream_token_custody_control::StreamTokenCustodyPolicyV1"
)]
pub struct StreamTokenCustodyPolicyV1 {
    /// Exact chain, network, provider purpose, key and signing-policy identity.
    pub binding: SignerCustodyBindingV1,
    /// Independent device-evidence attestation authority.
    pub attester_authority: SignerCustodyAuthorityV1,
    /// Independently governed attestation verification key.
    pub attester_public_key: PublicKey,
    /// Inclusive beginning of attestation authority eligibility.
    pub active_from_unix_ms: u64,
    /// Exclusive end of attestation authority eligibility.
    pub active_until_unix_ms: u64,
    /// Maximum signed custody statement duration.
    pub max_validity_ms: u64,
    /// Maximum authenticated observation age.
    pub max_anchor_age_ms: u64,
}
impl StreamTokenCustodyPolicyV1 {
    /// Convert governed public fields to the existing verifier's explicit trust input.
    ///
    /// This does not authenticate a candidate policy or manufacture verified evidence.
    #[must_use]
    pub fn custody_trust(&self) -> SignerCustodyTrustV1 {
        SignerCustodyTrustV1 {
            authority: self.attester_authority.clone(),
            public_key: self.attester_public_key.clone(),
            active_from_unix_ms: self.active_from_unix_ms,
            active_until_unix_ms: self.active_until_unix_ms,
            max_validity_ms: self.max_validity_ms,
            max_anchor_age_ms: self.max_anchor_age_ms,
        }
    }
    /// Validate the sole StreamToken role and the existing structural custody/trust contract.
    ///
    /// # Errors
    /// Rejects malformed identities, wrong purposes, unsupported keys or self-attestation.
    pub fn validate(&self) -> Result<(), SignerCustodyErrorV1> {
        validate_binding(&self.binding)?;
        if self.binding.role != SignerRoleV1::StreamToken
            || !matches!(self.binding.purpose, SignerPurposeBindingV1::StreamToken { provider_id } if provider_id != [0; 32])
        {
            return Err(SignerCustodyErrorV1::InvalidRecord);
        }
        validate_trust_binding(&self.binding, &self.custody_trust())
    }
}

/// Authoritative role state committed by native control history.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::stream_token_custody_control::StreamTokenCustodyControlStateV1"
)]
pub struct StreamTokenCustodyControlStateV1 {
    /// Current governed public binding and independent attestation trust.
    pub policy: StreamTokenCustodyPolicyV1,
    /// Next enrollment sequence, preserved across policy/key changes.
    pub next_sequence: u64,
    /// Last admitted signed enrollment digest, preserved across policy/key changes.
    pub predecessor_digest: [u8; 32],
    /// Exact enrolled head, absent before enrollment or after a genuine policy change.
    pub active_head: Option<SignerCustodyActiveHeadV1>,
    /// Terminal revocation for the current signer key generation.
    pub signer_revoked: bool,
    /// Terminal revocation for the current independent attester key generation.
    pub attester_revoked: bool,
}
impl StreamTokenCustodyControlStateV1 {
    /// Validate internal head, generation and initial-sequence consistency.
    ///
    /// # Errors
    /// Rejects malformed policies or impossible enrollment/head coordinates.
    pub fn validate(&self) -> Result<(), SignerCustodyErrorV1> {
        self.policy.validate()?;
        if self.next_sequence == 0
            || (self.next_sequence == 1) != (self.predecessor_digest == [0; 32])
        {
            return Err(SignerCustodyErrorV1::ReplayOrRollback);
        }
        if let Some(head) = self.active_head {
            let binding = &self.policy.binding;
            if head.sequence.checked_add(1) != Some(self.next_sequence)
                || head.record_digest != self.predecessor_digest
                || head.approved_anchor.height == 0
                || head.approved_anchor.block_hash == [0; 32]
                || head.approved_anchor.state_digest == [0; 32]
                || head.key_revision != binding.key_revision
                || head.policy_revision != binding.policy_revision
                || head.policy_digest != binding.policy_digest
            {
                return Err(SignerCustodyErrorV1::ReplayOrRollback);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "stream_token_custody_control_tests.rs"]
mod tests;
