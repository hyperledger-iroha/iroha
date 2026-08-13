//! Externally governed authentication for hedging feeds and billing references.
use std::cmp::Ordering;
use blake3::Hasher;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use super::{
    BillingStatementV1, HedgingPriceFeedV1, HedgingReferencePriceDecisionV1,
    HedgingValidationError, MAX_BILLING_LINES, MAX_HEDGING_IDENTIFIER_BYTES,
    MAX_HEDGING_PRICE_FEEDS, derive_reference_price_decision_v1,
};
/// Schema version for [`HedgingFeedBindingV1`].
pub const HEDGING_FEED_BINDING_VERSION_V1: u8 = 1;
/// Schema version for [`HedgingTrustedSignerV1`].
pub const HEDGING_TRUSTED_SIGNER_VERSION_V1: u8 = 1;
/// Schema version for [`HedgingFeedTrustPolicyV1`].
pub const HEDGING_FEED_TRUST_POLICY_VERSION_V1: u8 = 1;
/// Schema version for [`SignedHedgingPriceFeedV1`].
pub const SIGNED_HEDGING_PRICE_FEED_VERSION_V1: u8 = 1;
/// Schema version for [`GovernedHedgingReferencePriceDecisionV1`].
pub const GOVERNED_HEDGING_REFERENCE_PRICE_VERSION_V1: u8 = 1;
/// Schema version for [`GovernedBillingStatementV1`].
pub const GOVERNED_BILLING_STATEMENT_VERSION_V1: u8 = 1;
/// Schema version for [`SignedHedgingFeedLedgerV1`].
pub const SIGNED_HEDGING_FEED_LEDGER_VERSION_V1: u8 = 1;
/// Maximum trusted signer count in one hedging policy.
pub const MAX_HEDGING_TRUSTED_SIGNERS: usize = 64;
/// Maximum authorized feed bindings per signer.
pub const MAX_HEDGING_FEED_BINDINGS_PER_SIGNER: usize = MAX_HEDGING_PRICE_FEEDS;
/// Maximum signer identifier byte length.
pub const MAX_HEDGING_SIGNER_ID_BYTES: usize = 128;
/// Maximum sample age allowed by any V1 trust policy.
pub const MAX_HEDGING_SAMPLE_AGE_SECS: u64 = 7 * 24 * 60 * 60;
/// Maximum future clock skew allowed by any V1 trust policy.
pub const MAX_HEDGING_FUTURE_SKEW_SECS: u64 = 5 * 60;
/// Maximum canonical trust-policy bytes.
pub const MAX_HEDGING_TRUST_POLICY_BYTES: usize = 256 * 1024;
/// Maximum canonical signed feed envelope bytes.
pub const MAX_SIGNED_HEDGING_FEED_BYTES: usize = 64 * 1024;
/// Maximum canonical governed reference-price bytes.
pub const MAX_GOVERNED_HEDGING_DECISION_BYTES: usize = 8 * 1024 * 1024;
/// Maximum canonical governed billing-statement bytes.
pub const MAX_GOVERNED_BILLING_STATEMENT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum latest-feed high-water marks in one durable checkpoint.
pub const MAX_SIGNED_HEDGING_FEED_LEDGER_ENTRIES: usize = MAX_HEDGING_PRICE_FEEDS;
/// Maximum canonical durable feed-ledger checkpoint bytes.
pub const MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES: usize = 32 * 1024 * 1024;
/// Exact feed/source pair authorized for one external signer.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct HedgingFeedBindingV1 {
    /// Schema version.
    pub version: u8,
    /// Canonical feed identifier.
    pub feed_id: String,
    /// Canonical governed source identifier.
    pub source: String,
}
impl HedgingFeedBindingV1 {
    /// Validate the exact authorization pair.
    pub fn validate(&self) -> Result<(), SignedHedgingError> {
        if self.version != HEDGING_FEED_BINDING_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedFeedBindingVersion {
                found: self.version,
            });
        }
        super::validate_identifier("feed_id", &self.feed_id)?;
        super::validate_identifier("source", &self.source)?;
        Ok(())
    }
}
/// One strong Ed25519 signer and the feed identities it may attest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct HedgingTrustedSignerV1 {
    /// Schema version.
    pub version: u8,
    /// Stable governance signer identifier.
    pub signer_id: String,
    /// Canonical strong Ed25519 public key.
    pub public_key: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
    /// Authorized pairs in strictly increasing `(feed_id, source)` order.
    pub authorized_feeds: Vec<HedgingFeedBindingV1>,
}
impl HedgingTrustedSignerV1 {
    /// Validate identity, key strength, bounds, and canonical bindings.
    pub fn validate(&self) -> Result<(), SignedHedgingError> {
        if self.version != HEDGING_TRUSTED_SIGNER_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedTrustedSignerVersion {
                found: self.version,
            });
        }
        validate_signer_id(&self.signer_id)?;
        crate::checked_ed25519_verifying_key_from_bytes(&self.public_key).map_err(|reason| {
            SignedHedgingError::InvalidPublicKey {
                signer_id: self.signer_id.clone(),
                reason,
            }
        })?;
        if self.authorized_feeds.is_empty() {
            return Err(SignedHedgingError::EmptyFeedAuthorizations {
                signer_id: self.signer_id.clone(),
            });
        }
        if self.authorized_feeds.len() > MAX_HEDGING_FEED_BINDINGS_PER_SIGNER {
            return Err(SignedHedgingError::ResourceLimitExceeded {
                field: "authorized_feeds",
                count: self.authorized_feeds.len(),
                max: MAX_HEDGING_FEED_BINDINGS_PER_SIGNER,
            });
        }
        let mut previous: Option<(&str, &str)> = None;
        for binding in &self.authorized_feeds {
            binding.validate()?;
            let current = (binding.feed_id.as_str(), binding.source.as_str());
            if let Some(previous) = previous {
                match previous.cmp(&current) {
                    Ordering::Equal => {
                        return Err(SignedHedgingError::DuplicateFeedAuthorization {
                            feed_id: binding.feed_id.clone(),
                            feed_source: binding.source.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedHedgingError::NonCanonicalOrder {
                            field: "authorized_feeds",
                        });
                    }
                    Ordering::Less => {}
                }
            }
            previous = Some(current);
        }
        Ok(())
    }
    fn authorizes(&self, feed: &HedgingPriceFeedV1) -> bool {
        self.authorized_feeds
            .binary_search_by(|binding| {
                (binding.feed_id.as_str(), binding.source.as_str())
                    .cmp(&(feed.feed_id.as_str(), feed.source.as_str()))
            })
            .is_ok()
    }
}
/// External signer, identity, freshness, and revocation policy for price feeds.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct HedgingFeedTrustPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Non-zero governance policy identifier.
    pub policy_id: [u8; 32],
    /// First valid Unix second, inclusive.
    pub valid_from_unix: u64,
    /// Expiry Unix second, exclusive.
    pub valid_until_unix: u64,
    /// Maximum accepted sample age at admission.
    pub max_sample_age_secs: u64,
    /// Maximum sample clock skew into the future.
    pub max_future_skew_secs: u64,
    /// Trusted signers in strictly increasing signer-id order.
    pub signers: Vec<HedgingTrustedSignerV1>,
    /// Explicit revoked signer ids in strictly increasing order.
    pub revoked_signer_ids: Vec<String>,
}
impl HedgingFeedTrustPolicyV1 {
    /// Validate policy structure, key uniqueness, and revocation references.
    pub fn validate(&self) -> Result<(), SignedHedgingError> {
        if self.version != HEDGING_FEED_TRUST_POLICY_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedTrustPolicyVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_id) {
            return Err(SignedHedgingError::InvalidPolicyId);
        }
        if self.valid_from_unix == 0 || self.valid_until_unix <= self.valid_from_unix {
            return Err(SignedHedgingError::InvalidPolicyValidity);
        }
        if self.max_sample_age_secs == 0 || self.max_sample_age_secs > MAX_HEDGING_SAMPLE_AGE_SECS {
            return Err(SignedHedgingError::InvalidMaximumSampleAge {
                found: self.max_sample_age_secs,
                max: MAX_HEDGING_SAMPLE_AGE_SECS,
            });
        }
        if self.max_future_skew_secs > MAX_HEDGING_FUTURE_SKEW_SECS {
            return Err(SignedHedgingError::InvalidMaximumFutureSkew {
                found: self.max_future_skew_secs,
                max: MAX_HEDGING_FUTURE_SKEW_SECS,
            });
        }
        if self.signers.is_empty() {
            return Err(SignedHedgingError::EmptyTrustedSignerSet);
        }
        if self.signers.len() > MAX_HEDGING_TRUSTED_SIGNERS {
            return Err(SignedHedgingError::ResourceLimitExceeded {
                field: "signers",
                count: self.signers.len(),
                max: MAX_HEDGING_TRUSTED_SIGNERS,
            });
        }
        let mut previous_id: Option<&str> = None;
        for (index, signer) in self.signers.iter().enumerate() {
            signer.validate()?;
            if let Some(previous) = previous_id {
                match previous.cmp(signer.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedHedgingError::DuplicateTrustedSigner {
                            signer_id: signer.signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedHedgingError::NonCanonicalOrder { field: "signers" });
                    }
                    Ordering::Less => {}
                }
            }
            if self.signers[..index]
                .iter()
                .any(|previous| previous.public_key == signer.public_key)
            {
                return Err(SignedHedgingError::DuplicateTrustedPublicKey);
            }
            previous_id = Some(&signer.signer_id);
        }
        if self.revoked_signer_ids.len() > self.signers.len() {
            return Err(SignedHedgingError::ResourceLimitExceeded {
                field: "revoked_signer_ids",
                count: self.revoked_signer_ids.len(),
                max: self.signers.len(),
            });
        }
        let mut previous_revocation: Option<&str> = None;
        for signer_id in &self.revoked_signer_ids {
            validate_signer_id(signer_id)?;
            if let Some(previous) = previous_revocation {
                match previous.cmp(signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(SignedHedgingError::DuplicateRevocation {
                            signer_id: signer_id.clone(),
                        });
                    }
                    Ordering::Greater => {
                        return Err(SignedHedgingError::NonCanonicalOrder {
                            field: "revoked_signer_ids",
                        });
                    }
                    Ordering::Less => {}
                }
            }
            if self.signer(signer_id).is_none() {
                return Err(SignedHedgingError::UnknownRevokedSigner {
                    signer_id: signer_id.clone(),
                });
            }
            previous_revocation = Some(signer_id);
        }
        if self.revoked_signer_ids.len() == self.signers.len() {
            return Err(SignedHedgingError::NoActiveTrustedSigners);
        }
        Ok(())
    }
    /// Return the canonical domain-separated policy digest.
    pub fn canonical_digest(&self) -> Result<[u8; 32], SignedHedgingError> {
        self.validate()?;
        hash_canonical(
            b"sorafs-hedging-feed-trust-policy-v1",
            "hedging feed trust policy",
            self,
            MAX_HEDGING_TRUST_POLICY_BYTES,
        )
    }
    /// Return bounded canonical policy bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedHedgingError> {
        self.validate()?;
        encode_canonical_bounded(
            "hedging feed trust policy",
            self,
            MAX_HEDGING_TRUST_POLICY_BYTES,
        )
    }
    fn signer(&self, signer_id: &str) -> Option<&HedgingTrustedSignerV1> {
        self.signers
            .binary_search_by(|signer| signer.signer_id.as_str().cmp(signer_id))
            .ok()
            .map(|index| &self.signers[index])
    }
    fn is_revoked(&self, signer_id: &str) -> bool {
        self.revoked_signer_ids
            .binary_search_by(|revoked| revoked.as_str().cmp(signer_id))
            .is_ok()
    }
}
/// Canonical normalized feed plus external signer authorization.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SignedHedgingPriceFeedV1 {
    /// Schema version.
    pub version: u8,
    /// Digest of the external trust policy.
    pub policy_digest: [u8; 32],
    /// Intrinsically validated normalized feed sample.
    pub feed: HedgingPriceFeedV1,
    /// Signer id selected from the external policy.
    pub signer_id: String,
    /// Ed25519 signature over policy, signer id, and exact feed bytes.
    pub signature: [u8; ed25519_dalek::SIGNATURE_LENGTH],
}
impl SignedHedgingPriceFeedV1 {
    /// Validate policy-independent structure and canonical signature material.
    pub fn validate_structure(&self) -> Result<(), SignedHedgingError> {
        if self.version != SIGNED_HEDGING_PRICE_FEED_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedSignedFeedVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_digest) {
            return Err(SignedHedgingError::InvalidPolicyDigest);
        }
        self.feed.validate()?;
        validate_signer_id(&self.signer_id)?;
        crate::checked_ed25519_signature_from_bytes(&self.signature).map_err(|reason| {
            SignedHedgingError::InvalidSignature {
                signer_id: self.signer_id.clone(),
                reason,
            }
        })?;
        Ok(())
    }
    /// Return the exact digest signed by the feed authority.
    pub fn signing_digest(&self) -> Result<[u8; 32], SignedHedgingError> {
        self.feed.validate()?;
        if crate::inert_bytes(&self.policy_digest) {
            return Err(SignedHedgingError::InvalidPolicyDigest);
        }
        validate_signer_id(&self.signer_id)?;
        let feed_bytes = encode_canonical_bounded(
            "hedging price feed",
            &self.feed,
            MAX_SIGNED_HEDGING_FEED_BYTES,
        )?;
        let signer_len =
            u64::try_from(self.signer_id.len()).map_err(|_| SignedHedgingError::LengthOverflow)?;
        let feed_len =
            u64::try_from(feed_bytes.len()).map_err(|_| SignedHedgingError::LengthOverflow)?;
        let mut hasher = Hasher::new();
        hasher.update(b"sorafs-signed-hedging-price-feed-v1");
        hasher.update(&self.policy_digest);
        hasher.update(&signer_len.to_le_bytes());
        hasher.update(self.signer_id.as_bytes());
        hasher.update(&feed_len.to_le_bytes());
        hasher.update(&feed_bytes);
        Ok(*hasher.finalize().as_bytes())
    }
    /// Verify external policy identity, authorization, freshness, revocation, and signature.
    pub fn verify(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
        admitted_at_unix: u64,
    ) -> Result<(), SignedHedgingError> {
        self.validate_structure()?;
        policy.validate()?;
        if self.policy_digest != policy.canonical_digest()? {
            return Err(SignedHedgingError::PolicyDigestMismatch);
        }
        validate_admission_time(policy, &self.feed, admitted_at_unix)?;
        let signer =
            policy
                .signer(&self.signer_id)
                .ok_or_else(|| SignedHedgingError::UntrustedSigner {
                    signer_id: self.signer_id.clone(),
                })?;
        if policy.is_revoked(&self.signer_id) {
            return Err(SignedHedgingError::RevokedSigner {
                signer_id: self.signer_id.clone(),
            });
        }
        if !signer.authorizes(&self.feed) {
            return Err(SignedHedgingError::UnauthorizedFeedBinding {
                signer_id: self.signer_id.clone(),
                feed_id: self.feed.feed_id.clone(),
                feed_source: self.feed.source.clone(),
            });
        }
        let signature =
            crate::checked_ed25519_signature_from_bytes(&self.signature).map_err(|reason| {
                SignedHedgingError::InvalidSignature {
                    signer_id: self.signer_id.clone(),
                    reason,
                }
            })?;
        let key = crate::checked_ed25519_verifying_key_from_bytes(&signer.public_key).map_err(
            |reason| SignedHedgingError::InvalidPublicKey {
                signer_id: self.signer_id.clone(),
                reason,
            },
        )?;
        key.verify_strict(&self.signing_digest()?, &signature)
            .map_err(|error| SignedHedgingError::SignatureVerification {
                signer_id: self.signer_id.clone(),
                reason: error.to_string(),
            })?;
        Ok(())
    }
    /// Return bounded canonical envelope bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedHedgingError> {
        self.validate_structure()?;
        encode_canonical_bounded(
            "signed hedging price feed",
            self,
            MAX_SIGNED_HEDGING_FEED_BYTES,
        )
    }
}
/// Deterministic reference-price decision retaining every authenticated input.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernedHedgingReferencePriceDecisionV1 {
    /// Schema version.
    pub version: u8,
    /// Digest of the external feed trust policy.
    pub policy_digest: [u8; 32],
    /// Intrinsic deterministic weighted-price decision.
    pub decision: HedgingReferencePriceDecisionV1,
    /// Signed inputs in the same canonical order as `decision.feeds`.
    pub signed_feeds: Vec<SignedHedgingPriceFeedV1>,
    /// Canonical digest of all retained signed inputs.
    pub signed_feeds_digest: [u8; 32],
}
impl GovernedHedgingReferencePriceDecisionV1 {
    /// Validate intrinsic decision replay and exact signed-input binding.
    pub fn validate_structure(&self) -> Result<(), SignedHedgingError> {
        if self.version != GOVERNED_HEDGING_REFERENCE_PRICE_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedGovernedDecisionVersion {
                found: self.version,
            });
        }
        if crate::inert_bytes(&self.policy_digest) {
            return Err(SignedHedgingError::InvalidPolicyDigest);
        }
        self.decision.validate()?;
        if self.signed_feeds.is_empty() || self.signed_feeds.len() > MAX_HEDGING_PRICE_FEEDS {
            return Err(SignedHedgingError::ResourceLimitExceeded {
                field: "signed_feeds",
                count: self.signed_feeds.len(),
                max: MAX_HEDGING_PRICE_FEEDS,
            });
        }
        if self.signed_feeds.len() != self.decision.feeds.len() {
            return Err(SignedHedgingError::SignedFeedCountMismatch);
        }
        let mut previous_id: Option<&str> = None;
        for (index, envelope) in self.signed_feeds.iter().enumerate() {
            envelope.validate_structure()?;
            if envelope.policy_digest != self.policy_digest {
                return Err(SignedHedgingError::PolicyDigestMismatch);
            }
            if envelope.feed != self.decision.feeds[index] {
                return Err(SignedHedgingError::DecisionFeedMismatch { index });
            }
            if let Some(previous) = previous_id
                && previous >= envelope.feed.feed_id.as_str()
            {
                return Err(SignedHedgingError::NonCanonicalOrder {
                    field: "signed_feeds",
                });
            }
            previous_id = Some(&envelope.feed.feed_id);
        }
        let digest = signed_feeds_digest(&self.signed_feeds)?;
        if self.signed_feeds_digest != digest {
            return Err(SignedHedgingError::SignedFeedsDigestMismatch);
        }
        Ok(())
    }
    /// Verify policy, every feed signature, freshness, and deterministic replay.
    pub fn verify(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
        admitted_at_unix: u64,
    ) -> Result<(), SignedHedgingError> {
        self.validate_structure()?;
        policy.validate()?;
        if self.policy_digest != policy.canonical_digest()? {
            return Err(SignedHedgingError::PolicyDigestMismatch);
        }
        if self.decision.max_feed_age_secs > policy.max_sample_age_secs {
            return Err(SignedHedgingError::DecisionAgePolicyExceeded {
                decision_max_age: self.decision.max_feed_age_secs,
                policy_max_age: policy.max_sample_age_secs,
            });
        }
        if !(policy.valid_from_unix..policy.valid_until_unix)
            .contains(&self.decision.effective_at_unix)
        {
            return Err(SignedHedgingError::PolicyInactiveAtDecision);
        }
        let latest_effective = admitted_at_unix
            .checked_add(policy.max_future_skew_secs)
            .ok_or(SignedHedgingError::TimeOverflow)?;
        if self.decision.effective_at_unix > latest_effective {
            return Err(SignedHedgingError::DecisionFromFuture);
        }
        for envelope in &self.signed_feeds {
            envelope.verify(policy, admitted_at_unix)?;
        }
        Ok(())
    }
    /// Return bounded canonical governed-decision bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedHedgingError> {
        self.validate_structure()?;
        encode_canonical_bounded(
            "governed hedging reference-price decision",
            self,
            MAX_GOVERNED_HEDGING_DECISION_BYTES,
        )
    }
}
/// Billing statement paired with the authenticated price decision it embeds.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GovernedBillingStatementV1 {
    /// Schema version.
    pub version: u8,
    /// Intrinsic canonical billing statement.
    pub statement: BillingStatementV1,
    /// Authenticated reference-price evidence.
    pub governed_reference_price: GovernedHedgingReferencePriceDecisionV1,
    /// Binding digest over statement id, policy, decision, and signed-feed evidence.
    pub binding_digest: [u8; 32],
}
impl GovernedBillingStatementV1 {
    /// Validate the intrinsic statement and exact governed-price binding.
    pub fn validate_structure(&self) -> Result<(), SignedHedgingError> {
        if self.version != GOVERNED_BILLING_STATEMENT_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedGovernedBillingVersion {
                found: self.version,
            });
        }
        self.statement.validate()?;
        self.governed_reference_price.validate_structure()?;
        if self.statement.reference_price != self.governed_reference_price.decision {
            return Err(SignedHedgingError::StatementReferencePriceMismatch);
        }
        let expected =
            governed_billing_binding_digest(&self.statement, &self.governed_reference_price);
        if self.binding_digest != expected {
            return Err(SignedHedgingError::GovernedBillingDigestMismatch);
        }
        Ok(())
    }
    /// Verify the external feed policy retained by this billing statement.
    pub fn verify(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
        admitted_at_unix: u64,
    ) -> Result<(), SignedHedgingError> {
        self.validate_structure()?;
        self.governed_reference_price
            .verify(policy, admitted_at_unix)
    }
    /// Return bounded canonical governed-statement bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SignedHedgingError> {
        self.validate_structure()?;
        encode_canonical_bounded(
            "governed billing statement",
            self,
            MAX_GOVERNED_BILLING_STATEMENT_BYTES,
        )
    }
}
/// One authenticated feed sample and the exact time it passed admission.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SignedHedgingFeedAdmissionV1 {
    /// Unix second at which policy, freshness, authorization, and signature checks passed.
    admitted_at_unix: u64,
    /// Exact signed feed envelope retained for replay.
    envelope: SignedHedgingPriceFeedV1,
}
impl SignedHedgingFeedAdmissionV1 {
    /// Return the exact admission Unix second.
    #[must_use]
    pub const fn admitted_at_unix(&self) -> u64 {
        self.admitted_at_unix
    }
    /// Return the retained signed feed envelope.
    #[must_use]
    pub const fn envelope(&self) -> &SignedHedgingPriceFeedV1 {
        &self.envelope
    }
}
/// Durable latest-feed high-water marks with replay and rollback protection.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SignedHedgingFeedLedgerV1 {
    /// Schema version.
    version: u8,
    /// Digest of the external trust policy used for every admission.
    policy_digest: [u8; 32],
    /// Highest accepted admission clock across all feed updates.
    last_admitted_at_unix: u64,
    /// Latest admission per feed id in strictly increasing feed-id order.
    admissions: Vec<SignedHedgingFeedAdmissionV1>,
}
impl SignedHedgingFeedLedgerV1 {
    /// Construct an empty durable ledger bound to a validated trust policy.
    pub fn new(policy: &HedgingFeedTrustPolicyV1) -> Result<Self, SignedHedgingError> {
        policy.validate()?;
        Ok(Self {
            version: SIGNED_HEDGING_FEED_LEDGER_VERSION_V1,
            policy_digest: policy.canonical_digest()?,
            last_admitted_at_unix: 0,
            admissions: Vec::new(),
        })
    }
    /// Replay policy, signature, freshness, order, and uniqueness invariants.
    pub fn validate(&self, policy: &HedgingFeedTrustPolicyV1) -> Result<(), SignedHedgingError> {
        if self.version != SIGNED_HEDGING_FEED_LEDGER_VERSION_V1 {
            return Err(SignedHedgingError::UnsupportedFeedLedgerVersion {
                found: self.version,
            });
        }
        policy.validate()?;
        if self.policy_digest != policy.canonical_digest()? {
            return Err(SignedHedgingError::FeedLedgerPolicyDigestMismatch);
        }
        if self.admissions.len() > MAX_SIGNED_HEDGING_FEED_LEDGER_ENTRIES {
            return Err(SignedHedgingError::ResourceLimitExceeded {
                field: "feed_ledger_admissions",
                count: self.admissions.len(),
                max: MAX_SIGNED_HEDGING_FEED_LEDGER_ENTRIES,
            });
        }
        if self.admissions.is_empty() != (self.last_admitted_at_unix == 0) {
            return Err(SignedHedgingError::InvalidFeedLedgerClock);
        }
        let mut previous_feed_id: Option<&str> = None;
        let mut maximum_admitted_at = 0_u64;
        for (index, admission) in self.admissions.iter().enumerate() {
            if admission.admitted_at_unix > self.last_admitted_at_unix {
                return Err(SignedHedgingError::FeedAdmissionAfterLedgerClock {
                    admitted: admission.admitted_at_unix,
                    ledger_clock: self.last_admitted_at_unix,
                });
            }
            admission
                .envelope
                .verify(policy, admission.admitted_at_unix)?;
            let feed_id = admission.envelope.feed.feed_id.as_str();
            if previous_feed_id.is_some_and(|previous| previous >= feed_id) {
                return Err(SignedHedgingError::NonCanonicalOrder {
                    field: "feed_ledger_admissions",
                });
            }
            if self.admissions[..index].iter().any(|previous| {
                previous.envelope.feed.evidence_digest == admission.envelope.feed.evidence_digest
            }) {
                return Err(SignedHedgingError::FeedEvidenceReplay);
            }
            previous_feed_id = Some(feed_id);
            maximum_admitted_at = maximum_admitted_at.max(admission.admitted_at_unix);
        }
        if maximum_admitted_at != self.last_admitted_at_unix {
            return Err(SignedHedgingError::InvalidFeedLedgerClock);
        }
        Ok(())
    }
    /// Verify and upsert one signed feed high-water mark without partial mutation.
    pub fn admit(
        &mut self,
        policy: &HedgingFeedTrustPolicyV1,
        envelope: SignedHedgingPriceFeedV1,
        admitted_at_unix: u64,
    ) -> Result<(), SignedHedgingError> {
        self.validate(policy)?;
        if admitted_at_unix < self.last_admitted_at_unix {
            return Err(SignedHedgingError::FeedAdmissionTimeRollback {
                previous: self.last_admitted_at_unix,
                found: admitted_at_unix,
            });
        }
        envelope.verify(policy, admitted_at_unix)?;
        let feed_id = envelope.feed.feed_id.as_str();
        let position = self
            .admissions
            .binary_search_by(|entry| entry.envelope.feed.feed_id.as_str().cmp(feed_id));
        let existing_index = position.ok();
        if let Some(index) = existing_index {
            validate_feed_successor(&self.admissions[index].envelope, &envelope)?;
        }
        if self.admissions.iter().enumerate().any(|(index, previous)| {
            Some(index) != existing_index
                && previous.envelope.feed.evidence_digest == envelope.feed.evidence_digest
        }) {
            return Err(SignedHedgingError::FeedEvidenceReplay);
        }
        let admission = SignedHedgingFeedAdmissionV1 {
            admitted_at_unix,
            envelope,
        };
        match position {
            Ok(index) => self.admissions[index] = admission,
            Err(index) => {
                if self.admissions.len() == MAX_SIGNED_HEDGING_FEED_LEDGER_ENTRIES {
                    return Err(SignedHedgingError::ResourceLimitExceeded {
                        field: "feed_ledger_admissions",
                        count: self.admissions.len().saturating_add(1),
                        max: MAX_SIGNED_HEDGING_FEED_LEDGER_ENTRIES,
                    });
                }
                self.admissions.try_reserve(1).map_err(|_| {
                    SignedHedgingError::AllocationFailed {
                        context: "signed hedging feed ledger admission",
                    }
                })?;
                self.admissions.insert(index, admission);
            }
        }
        self.last_admitted_at_unix = admitted_at_unix;
        Ok(())
    }
    /// Return the external policy digest bound into this ledger.
    #[must_use]
    pub const fn policy_digest(&self) -> &[u8; 32] {
        &self.policy_digest
    }
    /// Return the highest accepted admission Unix second.
    #[must_use]
    pub const fn last_admitted_at_unix(&self) -> u64 {
        self.last_admitted_at_unix
    }
    /// Return the number of retained per-feed high-water marks.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.admissions.len()
    }
    /// Return `true` when no signed feed has been admitted.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.admissions.is_empty()
    }
    /// Return immutable per-feed high-water marks in canonical feed-id order.
    #[must_use]
    pub fn high_water_marks(&self) -> &[SignedHedgingFeedAdmissionV1] {
        &self.admissions
    }
    /// Return fallibly cloned latest samples, one per feed id, in canonical order.
    pub fn latest_signed_feeds(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
    ) -> Result<Vec<SignedHedgingPriceFeedV1>, SignedHedgingError> {
        self.validate(policy)?;
        let mut latest = Vec::new();
        latest
            .try_reserve_exact(self.admissions.len().min(MAX_HEDGING_PRICE_FEEDS))
            .map_err(|_| SignedHedgingError::AllocationFailed {
                context: "latest signed hedging feeds",
            })?;
        for admission in &self.admissions {
            latest.push(try_clone_signed_feed(&admission.envelope)?);
        }
        Ok(latest)
    }
    /// Derive a governed decision only from the latest admitted sample per feed.
    pub fn derive_latest_reference_price(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
        effective_at_unix: u64,
        admitted_at_unix: u64,
        max_feed_age_secs: u64,
        max_divergence_bps: u16,
    ) -> Result<GovernedHedgingReferencePriceDecisionV1, SignedHedgingError> {
        derive_governed_reference_price_decision_v1(
            policy,
            self.latest_signed_feeds(policy)?,
            effective_at_unix,
            admitted_at_unix,
            max_feed_age_secs,
            max_divergence_bps,
        )
    }
    /// Return a bounded canonical checkpoint for durable restart recovery.
    pub fn canonical_bytes(
        &self,
        policy: &HedgingFeedTrustPolicyV1,
    ) -> Result<Vec<u8>, SignedHedgingError> {
        self.validate(policy)?;
        encode_canonical_bounded(
            "signed hedging feed ledger",
            self,
            MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES,
        )
    }
}
fn validate_feed_successor(
    previous: &SignedHedgingPriceFeedV1,
    next: &SignedHedgingPriceFeedV1,
) -> Result<(), SignedHedgingError> {
    let previous_observed = previous.feed.observed_at_unix;
    match next.feed.observed_at_unix.cmp(&previous_observed) {
        Ordering::Less => Err(SignedHedgingError::FeedObservationRollback {
            feed_id: next.feed.feed_id.clone(),
            previous: previous_observed,
            found: next.feed.observed_at_unix,
        }),
        Ordering::Equal if previous == next => Err(SignedHedgingError::FeedReplay {
            feed_id: next.feed.feed_id.clone(),
        }),
        Ordering::Equal => Err(SignedHedgingError::FeedObservationEquivocation {
            feed_id: next.feed.feed_id.clone(),
            observed_at_unix: next.feed.observed_at_unix,
        }),
        Ordering::Greater => {
            if previous.feed.evidence_digest == next.feed.evidence_digest {
                return Err(SignedHedgingError::FeedEvidenceReplay);
            }
            Ok(())
        }
    }
}
/// Derive a deterministic reference price only from externally verified feeds.
pub fn derive_governed_reference_price_decision_v1(
    policy: &HedgingFeedTrustPolicyV1,
    mut signed_feeds: Vec<SignedHedgingPriceFeedV1>,
    effective_at_unix: u64,
    admitted_at_unix: u64,
    max_feed_age_secs: u64,
    max_divergence_bps: u16,
) -> Result<GovernedHedgingReferencePriceDecisionV1, SignedHedgingError> {
    policy.validate()?;
    if signed_feeds.is_empty() || signed_feeds.len() > MAX_HEDGING_PRICE_FEEDS {
        return Err(SignedHedgingError::ResourceLimitExceeded {
            field: "signed_feeds",
            count: signed_feeds.len(),
            max: MAX_HEDGING_PRICE_FEEDS,
        });
    }
    if max_feed_age_secs > policy.max_sample_age_secs {
        return Err(SignedHedgingError::DecisionAgePolicyExceeded {
            decision_max_age: max_feed_age_secs,
            policy_max_age: policy.max_sample_age_secs,
        });
    }
    signed_feeds.sort_by(|left, right| left.feed.feed_id.cmp(&right.feed.feed_id));
    let policy_digest = policy.canonical_digest()?;
    let mut feeds = Vec::new();
    feeds.try_reserve_exact(signed_feeds.len()).map_err(|_| {
        SignedHedgingError::AllocationFailed {
            context: "governed decision feed replay",
        }
    })?;
    for envelope in &signed_feeds {
        envelope.verify(policy, admitted_at_unix)?;
        if envelope.policy_digest != policy_digest {
            return Err(SignedHedgingError::PolicyDigestMismatch);
        }
        feeds.push(try_clone_feed(&envelope.feed)?);
    }
    let decision = derive_reference_price_decision_v1(
        effective_at_unix,
        feeds,
        max_feed_age_secs,
        max_divergence_bps,
    )?;
    let signed_feeds_digest = signed_feeds_digest(&signed_feeds)?;
    let governed = GovernedHedgingReferencePriceDecisionV1 {
        version: GOVERNED_HEDGING_REFERENCE_PRICE_VERSION_V1,
        policy_digest,
        decision,
        signed_feeds,
        signed_feeds_digest,
    };
    governed.verify(policy, admitted_at_unix)?;
    Ok(governed)
}
/// Bind a canonical statement to its exact governed reference-price evidence.
pub fn bind_governed_billing_statement_v1(
    statement: BillingStatementV1,
    governed_reference_price: GovernedHedgingReferencePriceDecisionV1,
) -> Result<GovernedBillingStatementV1, SignedHedgingError> {
    statement.validate()?;
    governed_reference_price.validate_structure()?;
    if statement.reference_price != governed_reference_price.decision {
        return Err(SignedHedgingError::StatementReferencePriceMismatch);
    }
    let binding_digest = governed_billing_binding_digest(&statement, &governed_reference_price);
    let governed = GovernedBillingStatementV1 {
        version: GOVERNED_BILLING_STATEMENT_VERSION_V1,
        statement,
        governed_reference_price,
        binding_digest,
    };
    governed.validate_structure()?;
    Ok(governed)
}
/// Decode one bounded canonical trust policy.
pub fn decode_hedging_feed_trust_policy(
    bytes: &[u8],
) -> Result<HedgingFeedTrustPolicyV1, SignedHedgingError> {
    let policy: HedgingFeedTrustPolicyV1 = decode_canonical(
        "hedging feed trust policy",
        bytes,
        MAX_HEDGING_TRUST_POLICY_BYTES,
        norito::DecodeLimits::new(
            MAX_HEDGING_IDENTIFIER_BYTES,
            MAX_HEDGING_TRUST_POLICY_BYTES,
            32_768,
            MAX_HEDGING_TRUST_POLICY_BYTES * 4,
            32,
        ),
    )?;
    policy.validate()?;
    Ok(policy)
}
/// Decode one bounded canonical signed price feed.
pub fn decode_signed_hedging_price_feed(
    bytes: &[u8],
) -> Result<SignedHedgingPriceFeedV1, SignedHedgingError> {
    let envelope: SignedHedgingPriceFeedV1 = decode_canonical(
        "signed hedging price feed",
        bytes,
        MAX_SIGNED_HEDGING_FEED_BYTES,
        norito::DecodeLimits::new(
            MAX_HEDGING_IDENTIFIER_BYTES,
            MAX_SIGNED_HEDGING_FEED_BYTES,
            8_192,
            MAX_SIGNED_HEDGING_FEED_BYTES * 4,
            32,
        ),
    )?;
    envelope.validate_structure()?;
    Ok(envelope)
}
/// Decode and fully replay one bounded canonical signed-feed ledger checkpoint.
pub fn decode_signed_hedging_feed_ledger(
    bytes: &[u8],
    policy: &HedgingFeedTrustPolicyV1,
) -> Result<SignedHedgingFeedLedgerV1, SignedHedgingError> {
    let ledger: SignedHedgingFeedLedgerV1 = decode_canonical(
        "signed hedging feed ledger",
        bytes,
        MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES,
        norito::DecodeLimits::new(
            MAX_SIGNED_HEDGING_FEED_BYTES,
            MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES,
            1_000_000,
            MAX_SIGNED_HEDGING_FEED_LEDGER_BYTES * 4,
            64,
        ),
    )?;
    ledger.validate(policy)?;
    Ok(ledger)
}
/// Decode one bounded canonical governed price decision.
pub fn decode_governed_reference_price_decision(
    bytes: &[u8],
) -> Result<GovernedHedgingReferencePriceDecisionV1, SignedHedgingError> {
    let decision: GovernedHedgingReferencePriceDecisionV1 = decode_canonical(
        "governed hedging reference-price decision",
        bytes,
        MAX_GOVERNED_HEDGING_DECISION_BYTES,
        norito::DecodeLimits::new(
            MAX_HEDGING_IDENTIFIER_BYTES,
            MAX_GOVERNED_HEDGING_DECISION_BYTES,
            65_536,
            MAX_GOVERNED_HEDGING_DECISION_BYTES * 4,
            64,
        ),
    )?;
    decision.validate_structure()?;
    Ok(decision)
}
/// Decode one bounded canonical governed billing statement.
pub fn decode_governed_billing_statement(
    bytes: &[u8],
) -> Result<GovernedBillingStatementV1, SignedHedgingError> {
    let statement: GovernedBillingStatementV1 = decode_canonical(
        "governed billing statement",
        bytes,
        MAX_GOVERNED_BILLING_STATEMENT_BYTES,
        norito::DecodeLimits::new(
            MAX_BILLING_LINES,
            MAX_GOVERNED_BILLING_STATEMENT_BYTES,
            1_000_000,
            MAX_GOVERNED_BILLING_STATEMENT_BYTES * 4,
            96,
        ),
    )?;
    statement.validate_structure()?;
    Ok(statement)
}
fn validate_admission_time(
    policy: &HedgingFeedTrustPolicyV1,
    feed: &HedgingPriceFeedV1,
    admitted_at_unix: u64,
) -> Result<(), SignedHedgingError> {
    if admitted_at_unix == 0 {
        return Err(SignedHedgingError::InvalidAdmissionTime);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&admitted_at_unix) {
        return Err(SignedHedgingError::PolicyInactiveAtAdmission);
    }
    if !(policy.valid_from_unix..policy.valid_until_unix).contains(&feed.observed_at_unix) {
        return Err(SignedHedgingError::PolicyInactiveAtObservation);
    }
    let latest = admitted_at_unix
        .checked_add(policy.max_future_skew_secs)
        .ok_or(SignedHedgingError::TimeOverflow)?;
    if feed.observed_at_unix > latest {
        return Err(SignedHedgingError::FeedFromFuture);
    }
    let expires = feed
        .observed_at_unix
        .checked_add(policy.max_sample_age_secs)
        .ok_or(SignedHedgingError::TimeOverflow)?;
    if admitted_at_unix > expires {
        return Err(SignedHedgingError::FeedTooOld);
    }
    Ok(())
}
fn signed_feeds_digest(
    signed_feeds: &[SignedHedgingPriceFeedV1],
) -> Result<[u8; 32], SignedHedgingError> {
    let count =
        u64::try_from(signed_feeds.len()).map_err(|_| SignedHedgingError::LengthOverflow)?;
    let mut total_bytes = 0_usize;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-governed-hedging-signed-feeds-v1");
    hasher.update(&count.to_le_bytes());
    for envelope in signed_feeds {
        let bytes = envelope.canonical_bytes()?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or(SignedHedgingError::LengthOverflow)?;
        if total_bytes > MAX_GOVERNED_HEDGING_DECISION_BYTES {
            return Err(SignedHedgingError::EncodedPayloadTooLarge {
                payload: "governed signed feed evidence",
                length: total_bytes,
                max: MAX_GOVERNED_HEDGING_DECISION_BYTES,
            });
        }
        let length = u64::try_from(bytes.len()).map_err(|_| SignedHedgingError::LengthOverflow)?;
        hasher.update(&length.to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(*hasher.finalize().as_bytes())
}
fn governed_billing_binding_digest(
    statement: &BillingStatementV1,
    governed: &GovernedHedgingReferencePriceDecisionV1,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-governed-billing-binding-v1");
    hasher.update(&statement.statement_id);
    hasher.update(&governed.policy_digest);
    hasher.update(&governed.decision.decision_id);
    hasher.update(&governed.signed_feeds_digest);
    *hasher.finalize().as_bytes()
}
fn try_clone_feed(feed: &HedgingPriceFeedV1) -> Result<HedgingPriceFeedV1, SignedHedgingError> {
    Ok(HedgingPriceFeedV1 {
        version: feed.version,
        feed_id: try_clone_text(&feed.feed_id, "governed feed id")?,
        source: try_clone_text(&feed.source, "governed feed source")?,
        observed_at_unix: feed.observed_at_unix,
        xor_usd_price: feed.xor_usd_price.clone(),
        weight_bps: feed.weight_bps,
        evidence_digest: feed.evidence_digest,
        status: feed.status,
    })
}
fn try_clone_signed_feed(
    envelope: &SignedHedgingPriceFeedV1,
) -> Result<SignedHedgingPriceFeedV1, SignedHedgingError> {
    Ok(SignedHedgingPriceFeedV1 {
        version: envelope.version,
        policy_digest: envelope.policy_digest,
        feed: try_clone_feed(&envelope.feed)?,
        signer_id: try_clone_text(&envelope.signer_id, "signed feed signer id")?,
        signature: envelope.signature,
    })
}
fn try_clone_text(value: &str, context: &'static str) -> Result<String, SignedHedgingError> {
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(value.len())
        .map_err(|_| SignedHedgingError::AllocationFailed { context })?;
    cloned.push_str(value);
    Ok(cloned)
}
fn validate_signer_id(signer_id: &str) -> Result<(), SignedHedgingError> {
    if signer_id.is_empty()
        || signer_id.len() > MAX_HEDGING_SIGNER_ID_BYTES
        || !signer_id.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b':')
        })
    {
        return Err(SignedHedgingError::InvalidSignerId);
    }
    Ok(())
}
fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<[u8; 32], SignedHedgingError> {
    let bytes = encode_canonical_bounded(payload, value, max_bytes)?;
    let encoded_len = u64::try_from(bytes.len()).map_err(|_| SignedHedgingError::LengthOverflow)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
fn encode_canonical_bounded<T: norito::NoritoSerialize>(
    payload: &'static str,
    value: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, SignedHedgingError> {
    let exact = norito::core::encoded_frame_len(value)
        .map_err(|error| SignedHedgingError::Encoding(error.to_string()))?;
    if exact > max_bytes {
        return Err(SignedHedgingError::EncodedPayloadTooLarge {
            payload,
            length: exact,
            max: max_bytes,
        });
    }
    let bytes =
        norito::to_bytes(value).map_err(|error| SignedHedgingError::Encoding(error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(SignedHedgingError::EncodedPayloadTooLarge {
            payload,
            length: bytes.len(),
            max: max_bytes,
        });
    }
    Ok(bytes)
}
fn decode_canonical<T>(
    payload: &'static str,
    bytes: &[u8],
    max_bytes: usize,
    limits: norito::DecodeLimits,
) -> Result<T, SignedHedgingError>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    if bytes.len() > max_bytes {
        return Err(SignedHedgingError::EncodedPayloadTooLarge {
            payload,
            length: bytes.len(),
            max: max_bytes,
        });
    }
    let decoded = norito::decode_from_bytes_with_limits(bytes, limits).map_err(|error| {
        SignedHedgingError::Decoding {
            payload,
            reason: error.to_string(),
        }
    })?;
    let canonical = encode_canonical_bounded(payload, &decoded, max_bytes)?;
    if canonical != bytes {
        return Err(SignedHedgingError::NonCanonicalEncoding { payload });
    }
    Ok(decoded)
}
/// Externally governed feed and billing validation failures.
#[derive(Debug, Error)]
pub enum SignedHedgingError {
    /// Feed-binding version is unsupported.
    #[error("unsupported hedging feed-binding version {found}")]
    UnsupportedFeedBindingVersion { found: u8 },
    /// Trusted-signer version is unsupported.
    #[error("unsupported hedging trusted-signer version {found}")]
    UnsupportedTrustedSignerVersion { found: u8 },
    /// Trust-policy version is unsupported.
    #[error("unsupported hedging trust-policy version {found}")]
    UnsupportedTrustPolicyVersion { found: u8 },
    /// Signed-feed version is unsupported.
    #[error("unsupported signed hedging feed version {found}")]
    UnsupportedSignedFeedVersion { found: u8 },
    /// Governed-decision version is unsupported.
    #[error("unsupported governed hedging decision version {found}")]
    UnsupportedGovernedDecisionVersion { found: u8 },
    /// Governed-statement version is unsupported.
    #[error("unsupported governed billing statement version {found}")]
    UnsupportedGovernedBillingVersion { found: u8 },
    /// Durable feed-ledger version is unsupported.
    #[error("unsupported signed hedging feed-ledger version {found}")]
    UnsupportedFeedLedgerVersion { found: u8 },
    /// Signer identifier is malformed.
    #[error("hedging signer identifier is malformed")]
    InvalidSignerId,
    /// Policy identifier is inert.
    #[error("hedging trust policy id must not be zero")]
    InvalidPolicyId,
    /// Policy digest is inert.
    #[error("hedging policy digest must not be zero")]
    InvalidPolicyDigest,
    /// Policy validity interval is malformed.
    #[error("hedging trust policy validity interval is invalid")]
    InvalidPolicyValidity,
    /// Maximum sample age is outside the V1 bound.
    #[error("hedging maximum sample age {found} exceeds 1..={max}")]
    InvalidMaximumSampleAge { found: u64, max: u64 },
    /// Maximum future skew is outside the V1 bound.
    #[error("hedging maximum future skew {found} exceeds {max}")]
    InvalidMaximumFutureSkew { found: u64, max: u64 },
    /// No trusted signers are configured.
    #[error("hedging trust policy has no trusted signers")]
    EmptyTrustedSignerSet,
    /// All trusted signers are revoked.
    #[error("hedging trust policy has no active trusted signers")]
    NoActiveTrustedSigners,
    /// One trusted signer has no authorized feeds.
    #[error("hedging signer `{signer_id}` has no feed authorizations")]
    EmptyFeedAuthorizations { signer_id: String },
    /// Duplicate authorization pair.
    #[error("duplicate hedging feed authorization `{feed_id}` / `{feed_source}`")]
    DuplicateFeedAuthorization {
        feed_id: String,
        feed_source: String,
    },
    /// Duplicate signer identity.
    #[error("duplicate hedging trusted signer `{signer_id}`")]
    DuplicateTrustedSigner { signer_id: String },
    /// Public key is reused by multiple signer ids.
    #[error("hedging trust policy reuses a trusted public key")]
    DuplicateTrustedPublicKey,
    /// Revocation repeats a signer id.
    #[error("duplicate hedging signer revocation `{signer_id}`")]
    DuplicateRevocation { signer_id: String },
    /// Revocation names an unknown signer.
    #[error("hedging policy revokes unknown signer `{signer_id}`")]
    UnknownRevokedSigner { signer_id: String },
    /// Canonical sequence order is invalid.
    #[error("{field} must be in canonical order")]
    NonCanonicalOrder { field: &'static str },
    /// A bounded collection exceeds its schema limit.
    #[error("{field} count {count} exceeds maximum {max}")]
    ResourceLimitExceeded {
        field: &'static str,
        count: usize,
        max: usize,
    },
    /// Strong public-key validation failed.
    #[error("invalid hedging public key for `{signer_id}`: {reason}")]
    InvalidPublicKey { signer_id: String, reason: String },
    /// Signature structure is malformed.
    #[error("invalid hedging signature for `{signer_id}`: {reason}")]
    InvalidSignature { signer_id: String, reason: String },
    /// Envelope policy digest differs from external policy.
    #[error("signed hedging feed policy digest mismatch")]
    PolicyDigestMismatch,
    /// Durable feed ledger is bound to a different external policy.
    #[error("signed hedging feed ledger policy digest mismatch")]
    FeedLedgerPolicyDigestMismatch,
    /// Signer is absent from external policy.
    #[error("untrusted hedging signer `{signer_id}`")]
    UntrustedSigner { signer_id: String },
    /// Signer is explicitly revoked.
    #[error("revoked hedging signer `{signer_id}`")]
    RevokedSigner { signer_id: String },
    /// Signer is not authorized for the asserted feed/source pair.
    #[error("signer `{signer_id}` is not authorized for `{feed_id}` / `{feed_source}`")]
    UnauthorizedFeedBinding {
        signer_id: String,
        feed_id: String,
        feed_source: String,
    },
    /// Cryptographic signature verification failed.
    #[error("hedging signature verification failed for `{signer_id}`: {reason}")]
    SignatureVerification { signer_id: String, reason: String },
    /// Admission time is the zero sentinel.
    #[error("hedging admission time must not be zero")]
    InvalidAdmissionTime,
    /// Durable feed-ledger clock is absent or does not match its latest entry.
    #[error("signed hedging feed ledger clock is inconsistent with retained high-water marks")]
    InvalidFeedLedgerClock,
    /// Durable feed-ledger admission clock moved backwards.
    #[error("hedging feed admission time moved backwards from {previous} to {found}")]
    FeedAdmissionTimeRollback { previous: u64, found: u64 },
    /// A retained feed admission is later than the durable global clock.
    #[error("retained hedging feed admission {admitted} exceeds ledger clock {ledger_clock}")]
    FeedAdmissionAfterLedgerClock { admitted: u64, ledger_clock: u64 },
    /// Exact signed envelope was submitted more than once.
    #[error("signed hedging feed `{feed_id}` was replayed")]
    FeedReplay { feed_id: String },
    /// A feed id reused an observation timestamp with different signed contents.
    #[error("signed hedging feed `{feed_id}` equivocated at Unix second {observed_at_unix}")]
    FeedObservationEquivocation {
        feed_id: String,
        observed_at_unix: u64,
    },
    /// A feed id attempted to roll its observation clock backwards.
    #[error(
        "signed hedging feed `{feed_id}` observation moved backwards from {previous} to {found}"
    )]
    FeedObservationRollback {
        feed_id: String,
        previous: u64,
        found: u64,
    },
    /// An evidence digest was reused by another admitted feed sample.
    #[error("signed hedging feed evidence digest was replayed")]
    FeedEvidenceReplay,
    /// Policy is inactive at admission.
    #[error("hedging policy is inactive at admission")]
    PolicyInactiveAtAdmission,
    /// Policy is inactive at feed observation.
    #[error("hedging policy is inactive at feed observation")]
    PolicyInactiveAtObservation,
    /// Policy is inactive at decision effective time.
    #[error("hedging policy is inactive at decision effective time")]
    PolicyInactiveAtDecision,
    /// Feed sample is too far in the future.
    #[error("hedging feed sample is too far in the future")]
    FeedFromFuture,
    /// Feed sample exceeds policy freshness.
    #[error("hedging feed sample is too old")]
    FeedTooOld,
    /// Governed decision is too far in the future at admission.
    #[error("governed hedging decision is too far in the future")]
    DecisionFromFuture,
    /// Time arithmetic overflowed.
    #[error("hedging time arithmetic overflow")]
    TimeOverflow,
    /// Decision permits older samples than external policy.
    #[error("decision max age {decision_max_age}s exceeds policy max {policy_max_age}s")]
    DecisionAgePolicyExceeded {
        decision_max_age: u64,
        policy_max_age: u64,
    },
    /// Signed-feed inventory count differs from intrinsic feeds.
    #[error("signed feed count does not match decision feed count")]
    SignedFeedCountMismatch,
    /// Signed feed does not equal the intrinsic decision feed at an index.
    #[error("signed feed does not match decision feed at index {index}")]
    DecisionFeedMismatch { index: usize },
    /// Signed-feed evidence digest differs.
    #[error("signed feed evidence digest mismatch")]
    SignedFeedsDigestMismatch,
    /// Billing statement embeds a different intrinsic decision.
    #[error("billing statement reference price differs from governed decision")]
    StatementReferencePriceMismatch,
    /// Governed billing binding digest differs.
    #[error("governed billing binding digest mismatch")]
    GovernedBillingDigestMismatch,
    /// Exact encoded length is unavailable for safe preflight.
    #[error("{payload} does not expose an exact canonical encoded length")]
    EncodedLengthUnavailable { payload: &'static str },
    /// Encoded payload exceeds its pre-decode or pre-encode cap.
    #[error("{payload} encoded length {length} exceeds maximum {max}")]
    EncodedPayloadTooLarge {
        payload: &'static str,
        length: usize,
        max: usize,
    },
    /// Canonical encoded length does not fit the digest preimage.
    #[error("signed hedging payload length overflow")]
    LengthOverflow,
    /// Bounded allocation failed.
    #[error("signed hedging allocation failed for {context}")]
    AllocationFailed { context: &'static str },
    /// Bounded Norito decoding failed.
    #[error("failed to decode {payload}: {reason}")]
    Decoding {
        payload: &'static str,
        reason: String,
    },
    /// Input used a valid but noncanonical Norito encoding.
    #[error("{payload} is not canonical Norito")]
    NonCanonicalEncoding { payload: &'static str },
    /// Canonical Norito encoding failed.
    #[error("failed to encode signed hedging payload: {0}")]
    Encoding(String),
    /// Intrinsic hedging/billing validation failed.
    #[error(transparent)]
    Hedging(#[from] HedgingValidationError),
}
#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::numeric::Quantity;
    use super::*;
    use crate::XorQuantity;
    use crate::hedging::{
        BillingLineDirectionV1, BillingLineItemKindV1, HEDGING_PRICE_FEED_VERSION_V1,
        HedgingFeedStatusV1, build_billing_line_item_v1, build_billing_statement_v1,
    };
    const EFFECTIVE_AT: u64 = 1_800_000_000;
    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    fn usd(value: &str) -> Quantity {
        value.parse().expect("canonical USD quantity")
    }
    fn keys() -> [SigningKey; 2] {
        [
            SigningKey::from_bytes(&[11; 32]),
            SigningKey::from_bytes(&[22; 32]),
        ]
    }
    fn binding(feed_id: &str, source: &str) -> HedgingFeedBindingV1 {
        HedgingFeedBindingV1 {
            version: HEDGING_FEED_BINDING_VERSION_V1,
            feed_id: feed_id.into(),
            source: source.into(),
        }
    }
    fn policy() -> HedgingFeedTrustPolicyV1 {
        let keys = keys();
        HedgingFeedTrustPolicyV1 {
            version: HEDGING_FEED_TRUST_POLICY_VERSION_V1,
            policy_id: [0xA5; 32],
            valid_from_unix: EFFECTIVE_AT - 1_000,
            valid_until_unix: EFFECTIVE_AT + 1_000,
            max_sample_age_secs: 300,
            max_future_skew_secs: 30,
            signers: vec![
                HedgingTrustedSignerV1 {
                    version: HEDGING_TRUSTED_SIGNER_VERSION_V1,
                    signer_id: "collector-1".into(),
                    public_key: keys[0].verifying_key().to_bytes(),
                    authorized_feeds: vec![binding("primary", "primary-source")],
                },
                HedgingTrustedSignerV1 {
                    version: HEDGING_TRUSTED_SIGNER_VERSION_V1,
                    signer_id: "collector-2".into(),
                    public_key: keys[1].verifying_key().to_bytes(),
                    authorized_feeds: vec![binding("secondary", "secondary-source")],
                },
            ],
            revoked_signer_ids: Vec::new(),
        }
    }
    fn feed(
        feed_id: &str,
        source: &str,
        observed_at_unix: u64,
        weight_bps: u16,
    ) -> HedgingPriceFeedV1 {
        HedgingPriceFeedV1 {
            version: HEDGING_PRICE_FEED_VERSION_V1,
            feed_id: feed_id.into(),
            source: source.into(),
            observed_at_unix,
            xor_usd_price: if feed_id == "primary" {
                usd("2")
            } else {
                usd("2.02")
            },
            weight_bps,
            evidence_digest: *blake3::hash(feed_id.as_bytes()).as_bytes(),
            status: HedgingFeedStatusV1::Ok,
        }
    }
    fn sign_feed(
        policy: &HedgingFeedTrustPolicyV1,
        signer_index: usize,
        feed: HedgingPriceFeedV1,
    ) -> SignedHedgingPriceFeedV1 {
        let keys = keys();
        let mut envelope = SignedHedgingPriceFeedV1 {
            version: SIGNED_HEDGING_PRICE_FEED_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("policy digest"),
            feed,
            signer_id: format!("collector-{}", signer_index + 1),
            signature: [0; ed25519_dalek::SIGNATURE_LENGTH],
        };
        let digest = envelope.signing_digest().expect("feed signing digest");
        envelope.signature = keys[signer_index].sign(&digest).to_bytes();
        envelope
    }
    fn signed_feeds(policy: &HedgingFeedTrustPolicyV1) -> Vec<SignedHedgingPriceFeedV1> {
        vec![
            sign_feed(
                policy,
                0,
                feed("primary", "primary-source", EFFECTIVE_AT - 10, 5_000),
            ),
            sign_feed(
                policy,
                1,
                feed("secondary", "secondary-source", EFFECTIVE_AT - 20, 5_000),
            ),
        ]
    }
    fn governed_decision() -> (
        HedgingFeedTrustPolicyV1,
        GovernedHedgingReferencePriceDecisionV1,
    ) {
        let policy = policy();
        let decision = derive_governed_reference_price_decision_v1(
            &policy,
            signed_feeds(&policy),
            EFFECTIVE_AT,
            EFFECTIVE_AT,
            120,
            500,
        )
        .expect("governed decision");
        (policy, decision)
    }
    #[test]
    fn signed_feed_verifies_external_identity_binding_and_freshness() {
        let policy = policy();
        let envelope = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 10, 10_000),
        );
        envelope
            .verify(&policy, EFFECTIVE_AT)
            .expect("authorized fresh feed");
        let mut wrong_signature = envelope.clone();
        wrong_signature.feed.xor_usd_price = wrong_signature
            .feed
            .xor_usd_price
            .checked_add(&usd("0.000001"))
            .expect("tampered price remains representable");
        assert!(matches!(
            wrong_signature.verify(&policy, EFFECTIVE_AT),
            Err(SignedHedgingError::SignatureVerification { .. })
        ));
        let unauthorized = sign_feed(
            &policy,
            0,
            feed("primary", "other-source", EFFECTIVE_AT - 10, 10_000),
        );
        assert!(matches!(
            unauthorized.verify(&policy, EFFECTIVE_AT),
            Err(SignedHedgingError::UnauthorizedFeedBinding { .. })
        ));
        let stale = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 301, 10_000),
        );
        assert!(matches!(
            stale.verify(&policy, EFFECTIVE_AT),
            Err(SignedHedgingError::FeedTooOld)
        ));
        let future = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT + 31, 10_000),
        );
        assert!(matches!(
            future.verify(&policy, EFFECTIVE_AT),
            Err(SignedHedgingError::FeedFromFuture)
        ));
    }
    #[test]
    fn policy_substitution_revocation_weak_keys_and_order_fail_closed() {
        let policy = policy();
        let envelope = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 10, 10_000),
        );
        let mut substituted = policy.clone();
        substituted.policy_id[0] ^= 1;
        assert!(matches!(
            envelope.verify(&substituted, EFFECTIVE_AT),
            Err(SignedHedgingError::PolicyDigestMismatch)
        ));
        let mut revoked_policy = policy.clone();
        revoked_policy.revoked_signer_ids = vec!["collector-1".into()];
        let revoked = sign_feed(
            &revoked_policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 10, 10_000),
        );
        assert!(matches!(
            revoked.verify(&revoked_policy, EFFECTIVE_AT),
            Err(SignedHedgingError::RevokedSigner { .. })
        ));
        let mut weak = policy.clone();
        weak.signers[0].public_key = [0; ed25519_dalek::PUBLIC_KEY_LENGTH];
        assert!(matches!(
            weak.validate(),
            Err(SignedHedgingError::InvalidPublicKey { .. })
        ));
        let mut unsorted = policy;
        unsorted.signers.swap(0, 1);
        assert!(matches!(
            unsorted.validate(),
            Err(SignedHedgingError::NonCanonicalOrder { field: "signers" })
        ));
    }
    #[test]
    fn policy_rejects_duplicate_bindings_keys_revocations_and_empty_active_set() {
        let mut duplicate_binding = policy();
        let repeated_binding = duplicate_binding.signers[0].authorized_feeds[0].clone();
        duplicate_binding.signers[0]
            .authorized_feeds
            .push(repeated_binding);
        assert!(matches!(
            duplicate_binding.validate(),
            Err(SignedHedgingError::DuplicateFeedAuthorization { .. })
        ));
        let mut duplicate_key = policy();
        duplicate_key.signers[1].public_key = duplicate_key.signers[0].public_key;
        assert!(matches!(
            duplicate_key.validate(),
            Err(SignedHedgingError::DuplicateTrustedPublicKey)
        ));
        let mut unknown_revocation = policy();
        unknown_revocation.revoked_signer_ids = vec!["collector-9".into()];
        assert!(matches!(
            unknown_revocation.validate(),
            Err(SignedHedgingError::UnknownRevokedSigner { .. })
        ));
        let mut all_revoked = policy();
        all_revoked.revoked_signer_ids = vec!["collector-1".into(), "collector-2".into()];
        assert!(matches!(
            all_revoked.validate(),
            Err(SignedHedgingError::NoActiveTrustedSigners)
        ));
    }
    #[test]
    fn signed_feed_rejects_unknown_signer_and_malformed_signature() {
        let policy = policy();
        let envelope = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 10, 10_000),
        );
        let mut unknown = envelope.clone();
        unknown.signer_id = "collector-9".into();
        assert!(matches!(
            unknown.verify(&policy, EFFECTIVE_AT),
            Err(SignedHedgingError::UntrustedSigner { .. })
        ));
        let mut malformed = envelope;
        malformed.signature = [0; ed25519_dalek::SIGNATURE_LENGTH];
        assert!(matches!(
            malformed.validate_structure(),
            Err(SignedHedgingError::InvalidSignature { .. })
        ));
    }
    #[test]
    fn governed_decision_cannot_weaken_policy_age_or_run_ahead_of_admission() {
        let policy = policy();
        assert!(matches!(
            derive_governed_reference_price_decision_v1(
                &policy,
                signed_feeds(&policy),
                EFFECTIVE_AT,
                EFFECTIVE_AT,
                policy.max_sample_age_secs + 1,
                500,
            ),
            Err(SignedHedgingError::DecisionAgePolicyExceeded { .. })
        ));
        assert!(matches!(
            derive_governed_reference_price_decision_v1(
                &policy,
                signed_feeds(&policy),
                EFFECTIVE_AT + policy.max_future_skew_secs + 1,
                EFFECTIVE_AT,
                120,
                500,
            ),
            Err(SignedHedgingError::DecisionFromFuture)
        ));
    }
    #[test]
    fn governed_decision_retains_and_replays_all_signed_inputs() {
        let (policy, governed) = governed_decision();
        governed
            .verify(&policy, EFFECTIVE_AT)
            .expect("governed decision verifies");
        assert_eq!(governed.decision.feeds.len(), 2);
        assert_eq!(governed.signed_feeds.len(), 2);
        let mut digest_tampered = governed.clone();
        digest_tampered.signed_feeds_digest[0] ^= 1;
        assert!(matches!(
            digest_tampered.validate_structure(),
            Err(SignedHedgingError::SignedFeedsDigestMismatch)
        ));
        let mut feed_tampered = governed;
        feed_tampered.signed_feeds[0].feed.xor_usd_price = feed_tampered.signed_feeds[0]
            .feed
            .xor_usd_price
            .checked_add(&usd("0.000001"))
            .expect("tampered price remains representable");
        assert!(matches!(
            feed_tampered.validate_structure(),
            Err(SignedHedgingError::DecisionFeedMismatch { index: 0 })
        ));
    }
    #[test]
    fn governed_billing_statement_cannot_drop_authenticated_reference_evidence() {
        let (policy, governed_reference) = governed_decision();
        let line = build_billing_line_item_v1(
            BillingLineItemKindV1::Adjustment,
            BillingLineDirectionV1::Debit,
            "governed-adjustment",
            xor("0.00001"),
            &governed_reference.decision.xor_usd_price,
            0,
            None,
        )
        .expect("line");
        let statement = build_billing_statement_v1(
            b"buyer-account".to_vec(),
            EFFECTIVE_AT - 100,
            EFFECTIVE_AT,
            EFFECTIVE_AT + 100,
            governed_reference.decision.clone(),
            vec![line],
            None,
        )
        .expect("statement");
        let governed = bind_governed_billing_statement_v1(statement, governed_reference)
            .expect("bind governed statement");
        governed
            .verify(&policy, EFFECTIVE_AT)
            .expect("governed statement verifies");
        let encoded = governed
            .canonical_bytes()
            .expect("governed statement bytes");
        assert_eq!(
            decode_governed_billing_statement(&encoded).expect("decode governed statement"),
            governed
        );
        let mut tampered = governed;
        tampered.binding_digest[0] ^= 1;
        assert!(matches!(
            tampered.validate_structure(),
            Err(SignedHedgingError::GovernedBillingDigestMismatch)
        ));
    }
    #[test]
    fn signed_feed_ledger_replays_latest_samples_and_restores_checkpoint() {
        let policy = policy();
        let initial = signed_feeds(&policy);
        let mut ledger = SignedHedgingFeedLedgerV1::new(&policy).expect("empty feed ledger");
        for envelope in initial {
            ledger
                .admit(&policy, envelope, EFFECTIVE_AT)
                .expect("initial feed admission");
        }
        let mut updated_feed = feed("primary", "primary-source", EFFECTIVE_AT + 10, 5_000);
        updated_feed.xor_usd_price = usd("2.01");
        updated_feed.evidence_digest = *blake3::hash(b"primary-update").as_bytes();
        let updated = sign_feed(&policy, 0, updated_feed);
        ledger
            .admit(&policy, updated.clone(), EFFECTIVE_AT + 10)
            .expect("updated feed admission");
        let latest = ledger
            .latest_signed_feeds(&policy)
            .expect("latest signed feeds");
        assert_eq!(latest.len(), 2);
        assert_eq!(latest[0], updated);
        let governed = ledger
            .derive_latest_reference_price(&policy, EFFECTIVE_AT + 10, EFFECTIVE_AT + 10, 120, 500)
            .expect("derive latest governed decision");
        governed
            .verify(&policy, EFFECTIVE_AT + 10)
            .expect("verify latest governed decision");
        assert_eq!(governed.signed_feeds, latest);
        let checkpoint = ledger.canonical_bytes(&policy).expect("ledger checkpoint");
        assert_eq!(
            decode_signed_hedging_feed_ledger(&checkpoint, &policy).expect("restore feed ledger"),
            ledger
        );
    }
    #[test]
    fn signed_feed_ledger_rejects_replay_rollback_equivocation_and_evidence_reuse_atomically() {
        let policy = policy();
        let original = sign_feed(
            &policy,
            0,
            feed("primary", "primary-source", EFFECTIVE_AT - 10, 10_000),
        );
        let mut ledger = SignedHedgingFeedLedgerV1::new(&policy).expect("empty feed ledger");
        ledger
            .admit(&policy, original.clone(), EFFECTIVE_AT)
            .expect("initial feed admission");
        let baseline = ledger.clone();
        assert!(matches!(
            ledger.admit(&policy, original.clone(), EFFECTIVE_AT + 1),
            Err(SignedHedgingError::FeedReplay { .. })
        ));
        assert_eq!(ledger, baseline, "replay rejection must be atomic");
        let mut rollback_feed = feed("primary", "primary-source", EFFECTIVE_AT - 20, 10_000);
        rollback_feed.evidence_digest = *blake3::hash(b"rollback-evidence").as_bytes();
        let rollback = sign_feed(&policy, 0, rollback_feed);
        assert!(matches!(
            ledger.admit(&policy, rollback, EFFECTIVE_AT + 1),
            Err(SignedHedgingError::FeedObservationRollback { .. })
        ));
        assert_eq!(ledger, baseline, "observation rollback must be atomic");
        let mut equivocation_feed = original.feed.clone();
        equivocation_feed.xor_usd_price = equivocation_feed
            .xor_usd_price
            .checked_add(&usd("0.000001"))
            .expect("equivocated price remains representable");
        equivocation_feed.evidence_digest = *blake3::hash(b"equivocation-evidence").as_bytes();
        let equivocation = sign_feed(&policy, 0, equivocation_feed);
        assert!(matches!(
            ledger.admit(&policy, equivocation, EFFECTIVE_AT + 1),
            Err(SignedHedgingError::FeedObservationEquivocation { .. })
        ));
        assert_eq!(ledger, baseline, "equivocation rejection must be atomic");
        let mut reused_evidence_feed =
            feed("secondary", "secondary-source", EFFECTIVE_AT - 5, 10_000);
        reused_evidence_feed.evidence_digest = original.feed.evidence_digest;
        let reused_evidence = sign_feed(&policy, 1, reused_evidence_feed);
        assert!(matches!(
            ledger.admit(&policy, reused_evidence, EFFECTIVE_AT + 1),
            Err(SignedHedgingError::FeedEvidenceReplay)
        ));
        assert_eq!(ledger, baseline, "evidence replay must be atomic");
        let mut newer_feed = feed("primary", "primary-source", EFFECTIVE_AT + 1, 10_000);
        newer_feed.evidence_digest = *blake3::hash(b"newer-evidence").as_bytes();
        let newer = sign_feed(&policy, 0, newer_feed);
        assert!(matches!(
            ledger.admit(&policy, newer, EFFECTIVE_AT - 1),
            Err(SignedHedgingError::FeedAdmissionTimeRollback { .. })
        ));
        assert_eq!(ledger, baseline, "admission rollback must be atomic");
        let mut substituted_policy = policy.clone();
        substituted_policy.policy_id[0] ^= 1;
        assert!(matches!(
            ledger.validate(&substituted_policy),
            Err(SignedHedgingError::FeedLedgerPolicyDigestMismatch)
        ));
    }
    #[test]
    fn signed_feed_ledger_checkpoint_rejects_reordered_and_tampered_high_water_marks() {
        let policy = policy();
        let mut ledger = SignedHedgingFeedLedgerV1::new(&policy).expect("empty feed ledger");
        for envelope in signed_feeds(&policy) {
            ledger
                .admit(&policy, envelope, EFFECTIVE_AT)
                .expect("feed admission");
        }
        let mut reordered = ledger.clone();
        reordered.admissions.swap(0, 1);
        assert!(matches!(
            reordered.validate(&policy),
            Err(SignedHedgingError::NonCanonicalOrder {
                field: "feed_ledger_admissions"
            })
        ));
        let mut clock_tampered = ledger.clone();
        clock_tampered.last_admitted_at_unix += 1;
        assert!(matches!(
            clock_tampered.validate(&policy),
            Err(SignedHedgingError::InvalidFeedLedgerClock)
        ));
        let mut tampered = ledger;
        tampered.admissions[1].envelope.feed.xor_usd_price = tampered.admissions[1]
            .envelope
            .feed
            .xor_usd_price
            .checked_add(&usd("0.000001"))
            .expect("tampered price remains representable");
        assert!(matches!(
            tampered.validate(&policy),
            Err(SignedHedgingError::SignatureVerification { .. })
        ));
    }
    #[test]
    fn canonical_decoders_reject_trailing_compressed_and_oversized_payloads() {
        let (policy, governed) = governed_decision();
        let policy_bytes = policy.canonical_bytes().expect("policy bytes");
        assert_eq!(
            decode_hedging_feed_trust_policy(&policy_bytes).expect("decode policy"),
            policy
        );
        let envelope_bytes = governed.signed_feeds[0]
            .canonical_bytes()
            .expect("feed bytes");
        assert_eq!(
            decode_signed_hedging_price_feed(&envelope_bytes).expect("decode feed"),
            governed.signed_feeds[0]
        );
        let governed_bytes = governed.canonical_bytes().expect("decision bytes");
        assert_eq!(
            decode_governed_reference_price_decision(&governed_bytes)
                .expect("decode governed decision"),
            governed
        );
        let mut trailing = policy_bytes;
        trailing.push(0);
        assert!(matches!(
            decode_hedging_feed_trust_policy(&trailing),
            Err(SignedHedgingError::Decoding { .. })
        ));
        let compressed =
            norito::to_compressed_bytes(&policy, Some(norito::CompressionConfig::default()))
                .expect("compress policy");
        assert!(matches!(
            decode_hedging_feed_trust_policy(&compressed),
            Err(SignedHedgingError::NonCanonicalEncoding { .. })
        ));
        let oversized = vec![0; MAX_HEDGING_TRUST_POLICY_BYTES + 1];
        assert!(matches!(
            decode_hedging_feed_trust_policy(&oversized),
            Err(SignedHedgingError::EncodedPayloadTooLarge { .. })
        ));
    }
}
