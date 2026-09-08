//! Shared checks for independently trusted, purpose-bound signer state observations.
//!
//! Purpose owners authenticate their own source pins, canonical wire, exact subject and signing
//! domain. They borrow common fields from that same decoded observation and apply these private
//! checks before verifying custody and a completed receipt. No raw state or successful individual
//! check is a public qualification result. A signed observation is accountable observer evidence,
//! not a consensus proof, hardware attestation or substitute for a genuine authoritative source.

use super::{
    custody::{
        SignerCustodyAnchorV1, SignerCustodyAuthorityV1, SignerCustodyBindingV1,
        SignerCustodyTrustV1,
    },
    protocol::valid_identity,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};

/// Maximum age and lifetime of an independently signed current-state observation: five minutes.
pub const SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1: u64 = 300_000;

/// Independently configured observer identity, signing key and eligibility policy.
///
/// No wire decoder or default is provided. Configuration/governance must pin these values
/// independently of the observation, receipt, role signer and hardware attester. Constructing
/// this configuration does not establish custody, finality, completion or observer independence.
#[derive(Clone, Debug)]
pub struct SignerStateObserverTrustV1 {
    /// Exact observer service, administrator and governed key/policy generation.
    pub authority: SignerCustodyAuthorityV1,
    /// Pinned Ed25519 observer key, separate from the role and hardware-attestation keys.
    pub public_key: PublicKey,
    /// Inclusive beginning of observer-key eligibility, in Unix milliseconds.
    pub active_from_unix_ms: u64,
    /// Exclusive end of observer-key eligibility, in Unix milliseconds.
    pub active_until_unix_ms: u64,
    /// Positive observation age/lifetime bound, at most [`SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1`].
    pub max_state_age_ms: u64,
}

/// Bounded internal failures; each purpose owner preserves its own public error mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SignerStateObservationErrorV1 {
    InvalidTrust,
    InvalidState,
}

impl SignerStateObserverTrustV1 {
    /// Validate independently pinned observer authority without moving custody checks earlier.
    ///
    /// The custody verifier still owns the attester's algorithm, signer/attester independence and
    /// record eligibility. This check adds the observer's disjoint identities/key and eligibility.
    pub(super) fn validate(
        &self,
        binding: &SignerCustodyBindingV1,
        custody: &SignerCustodyTrustV1,
        now: u64,
    ) -> Result<(), SignerStateObservationErrorV1> {
        let authority = &self.authority;
        let other_identities = [
            binding.service_id.as_str(),
            binding.administrator_id.as_str(),
            custody.authority.service_id.as_str(),
            custody.authority.administrator_id.as_str(),
        ];
        if !valid_identity(&authority.service_id)
            || !valid_identity(&authority.administrator_id)
            || authority.key_revision == 0
            || authority.policy_revision == 0
            || authority.policy_digest == [0; 32]
            || authority.service_id == authority.administrator_id
            || [
                authority.service_id.as_str(),
                authority.administrator_id.as_str(),
            ]
            .iter()
            .any(|identity| other_identities.contains(identity))
            || self.public_key.algorithm() != Algorithm::Ed25519
            || self.public_key == binding.public_key
            || self.public_key == custody.public_key
            || self.max_state_age_ms == 0
            || self.max_state_age_ms > SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1
            || now < self.active_from_unix_ms
            || now >= self.active_until_unix_ms
        {
            return Err(SignerStateObservationErrorV1::InvalidTrust);
        }
        Ok(())
    }

    /// Verify the exact bounded, domain-separated message built by its purpose owner.
    ///
    /// The owner must build this message from the same canonical observation whose borrowed
    /// fields it checked. Signature validity alone cannot authenticate a different subject or
    /// establish custody/completion; this helper deliberately returns no verified-state marker.
    pub(super) fn verify_signature(
        &self,
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), SignerStateObservationErrorV1> {
        let signature = Signature::try_from_bytes(signature)
            .map_err(|_| SignerStateObservationErrorV1::InvalidState)?;
        signature
            .verify(&self.public_key, message)
            .map_err(|_| SignerStateObservationErrorV1::InvalidState)
    }
}

/// Borrowed common fields from one purpose-owned, bounded canonical signed observation.
///
/// This view has no wire implementation or verification status. The purpose owner retains its
/// subject, ACTIVE head and completed row and binds all fields through its exact signing message.
/// Check groups are separate so purpose-specific checks retain their established error order.
pub(super) struct SignerStateObservationViewV1<'a> {
    pub authority: &'a SignerCustodyAuthorityV1,
    pub chain_id: &'a str,
    pub network_id: &'a [u8; 32],
    pub observed_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub current_anchor: SignerCustodyAnchorV1,
    pub signer_revoked: bool,
    pub attester_revoked: bool,
}

impl SignerStateObservationViewV1<'_> {
    /// Compare observer and network identity before any purpose-specific deployment check.
    pub(super) fn matches_identity(
        &self,
        trust: &SignerStateObserverTrustV1,
        binding: &SignerCustodyBindingV1,
    ) -> bool {
        self.authority == &trust.authority
            && self.chain_id == binding.chain_id
            && self.network_id == &binding.network_id
    }

    /// Validate observation timing against independently supplied trust and current time.
    pub(super) fn validate_freshness(
        &self,
        trust: &SignerStateObserverTrustV1,
        now: u64,
    ) -> Result<(), SignerStateObservationErrorV1> {
        // Preserve short-circuit ordering: neither subtraction is reached before its guard.
        if self.observed_at_unix_ms > now
            || now >= self.expires_at_unix_ms
            || now - self.observed_at_unix_ms > trust.max_state_age_ms
            || self.expires_at_unix_ms <= self.observed_at_unix_ms
            || self.expires_at_unix_ms - self.observed_at_unix_ms > trust.max_state_age_ms
            || self.observed_at_unix_ms < trust.active_from_unix_ms
        {
            return Err(SignerStateObservationErrorV1::InvalidState);
        }
        Ok(())
    }

    /// Validate current status and the independently retained, nonzero finalized lower bound.
    ///
    /// The purpose owner validates the lower bound before this check. Its own completed-row
    /// timing check, if applicable, runs before this group's observer-expiration check.
    pub(super) fn validate_finality(
        &self,
        trust: &SignerStateObserverTrustV1,
        minimum_anchor: &SignerCustodyAnchorV1,
    ) -> Result<(), SignerStateObservationErrorV1> {
        let anchor = self.current_anchor;
        if self.expires_at_unix_ms > trust.active_until_unix_ms
            || self.signer_revoked
            || self.attester_revoked
            || anchor.height < minimum_anchor.height
            || (anchor.height == minimum_anchor.height && &anchor != minimum_anchor)
            || anchor.block_hash == [0; 32]
            || anchor.state_digest == [0; 32]
        {
            return Err(SignerStateObservationErrorV1::InvalidState);
        }
        Ok(())
    }
}
