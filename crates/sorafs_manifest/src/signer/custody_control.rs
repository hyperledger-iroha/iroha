//! Public governed signer custody control, independent of operation state.
//!
//! This canonical payload contains no application body, role signature or signing reservation. The
//! native ledger owns authorization, deterministic provenance and compare-and-swap publication.
use super::custody::{
    SignerCustodyActiveHeadV1, SignerCustodyAuthorityV1, SignerCustodyBindingV1,
    SignerCustodyErrorV1, SignerCustodyTrustV1, validate_binding, validate_trust_binding,
};
use iroha_crypto::PublicKey;
use norito::codec::{Decode, Encode};

/// Maximum complete canonical policy or control frame, including its header and padding.
pub const SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1: usize = 16 * 1024;

/// Exact signer binding and independently governed attestation trust.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::custody_control::SignerCustodyPolicyV1")]
pub struct SignerCustodyPolicyV1 {
    /// Exact chain, network, role, application purpose, key and signing-policy identity.
    pub binding: SignerCustodyBindingV1,
    /// Independent authority approving the exact signer role and purpose binding.
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
impl SignerCustodyPolicyV1 {
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
    /// Validate the common role/purpose binding and independent custody/trust contract.
    ///
    /// Native application owners additionally enforce their exact role and purpose scope.
    ///
    /// # Errors
    /// Rejects malformed identities, wrong purposes, unsupported keys or self-attestation.
    pub fn validate(&self) -> Result<(), SignerCustodyErrorV1> {
        validate_binding(&self.binding)?;
        validate_trust_binding(&self.binding, &self.custody_trust())
    }
}

/// Authoritative role state committed by native control history.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::custody_control::SignerCustodyControlStateV1")]
pub struct SignerCustodyControlStateV1 {
    /// Current governed public binding and independent attestation trust.
    pub policy: SignerCustodyPolicyV1,
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
impl SignerCustodyControlStateV1 {
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

/// Fixed runtime failures when configuring a governed custody policy.
///
/// This error has no wire representation and does not contain candidate policy data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerCustodyPolicyTransitionErrorV1 {
    /// The current control or proposed policy is structurally invalid.
    Invalid,
    /// The proposed policy exactly matches the current policy.
    Unchanged,
    /// A configuration attempts to change the immutable chain, network, role or purpose.
    BindingMismatch,
    /// Key or policy changes contradict the monotonic generation rules.
    Generation,
}
impl std::fmt::Display for SignerCustodyPolicyTransitionErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid signer custody policy transition",
            Self::Unchanged => "signer custody policy is unchanged",
            Self::BindingMismatch => "signer custody policy scope mismatch",
            Self::Generation => "signer custody policy generation mismatch",
        })
    }
}
impl std::error::Error for SignerCustodyPolicyTransitionErrorV1 {}

fn key_generation_advanced(
    old_revision: u64,
    new_revision: u64,
    old_key: &PublicKey,
    new_key: &PublicKey,
) -> Result<bool, SignerCustodyPolicyTransitionErrorV1> {
    if new_revision < old_revision
        || (new_revision == old_revision && new_key != old_key)
        || (new_revision > old_revision && new_key == old_key)
    {
        return Err(SignerCustodyPolicyTransitionErrorV1::Generation);
    }
    Ok(new_revision > old_revision)
}

/// Prepare the sole shared governed policy-configuration transition.
///
/// The initial policy starts an unenrolled sequence. Every later policy change clears the active
/// enrollment while preserving its sequence and predecessor. Each revocation remains set until
/// that exact signer or attester key advances to a genuinely different public key generation.
/// Signing and attestation policies advance independently; changing one cannot authorize a
/// revision rollback or an unversioned change to the other.
///
/// Native owners retain authorization, exact role/purpose admission, historical key-use
/// tombstones, finite journal capacity, deterministic execution provenance and atomic CAS.
/// This pure transition does not authenticate a policy or prove its publication.
///
/// # Errors
/// Rejects invalid state/policy, unchanged configuration, immutable scope substitution and
/// inconsistent key or policy generations.
pub fn configure_signer_custody_policy_v1(
    current: Option<&SignerCustodyControlStateV1>,
    policy: SignerCustodyPolicyV1,
) -> Result<SignerCustodyControlStateV1, SignerCustodyPolicyTransitionErrorV1> {
    use SignerCustodyPolicyTransitionErrorV1 as Error;
    policy.validate().map_err(|_| Error::Invalid)?;
    let Some(current) = current else {
        return Ok(SignerCustodyControlStateV1 {
            policy,
            next_sequence: 1,
            predecessor_digest: [0; 32],
            active_head: None,
            signer_revoked: false,
            attester_revoked: false,
        });
    };
    current.validate().map_err(|_| Error::Invalid)?;
    let old = &current.policy;
    if policy.binding.chain_id != old.binding.chain_id
        || policy.binding.network_id != old.binding.network_id
        || policy.binding.role != old.binding.role
        || policy.binding.purpose != old.binding.purpose
    {
        return Err(Error::BindingMismatch);
    }
    if old == &policy {
        return Err(Error::Unchanged);
    }
    let signer_changed = key_generation_advanced(
        old.binding.key_revision,
        policy.binding.key_revision,
        &old.binding.public_key,
        &policy.binding.public_key,
    )?;
    let attester_changed = key_generation_advanced(
        old.attester_authority.key_revision,
        policy.attester_authority.key_revision,
        &old.attester_public_key,
        &policy.attester_public_key,
    )?;
    if policy.binding.policy_revision < old.binding.policy_revision
        || policy.attester_authority.policy_revision < old.attester_authority.policy_revision
        || (policy.binding.policy_revision == old.binding.policy_revision
            && policy.binding.policy_digest != old.binding.policy_digest)
        || (policy.attester_authority.policy_revision == old.attester_authority.policy_revision
            && policy.attester_authority.policy_digest != old.attester_authority.policy_digest)
    {
        return Err(Error::Generation);
    }
    // A key rotation may change its handle. Other signing identity/handle changes require policy
    // advancement, and a same-key handle change is governed policy work rather than renewal.
    let mut signer_policy = policy.binding.clone();
    signer_policy.public_key = old.binding.public_key.clone();
    signer_policy.key_revision = old.binding.key_revision;
    if signer_changed {
        signer_policy.key_handle.clone_from(&old.binding.key_handle);
    }
    if signer_policy != old.binding && policy.binding.policy_revision <= old.binding.policy_revision
    {
        return Err(Error::Generation);
    }
    let mut attester_policy = policy.custody_trust();
    attester_policy.public_key = old.attester_public_key.clone();
    attester_policy.authority.key_revision = old.attester_authority.key_revision;
    let old_trust = old.custody_trust();
    if (attester_policy.authority != old_trust.authority
        || attester_policy.active_from_unix_ms != old_trust.active_from_unix_ms
        || attester_policy.active_until_unix_ms != old_trust.active_until_unix_ms
        || attester_policy.max_validity_ms != old_trust.max_validity_ms
        || attester_policy.max_anchor_age_ms != old_trust.max_anchor_age_ms)
        && policy.attester_authority.policy_revision <= old.attester_authority.policy_revision
    {
        return Err(Error::Generation);
    }
    Ok(SignerCustodyControlStateV1 {
        policy,
        next_sequence: current.next_sequence,
        predecessor_digest: current.predecessor_digest,
        active_head: None,
        signer_revoked: current.signer_revoked && !signer_changed,
        attester_revoked: current.attester_revoked && !attester_changed,
    })
}

#[cfg(test)]
#[path = "custody_control_tests.rs"]
mod tests;
