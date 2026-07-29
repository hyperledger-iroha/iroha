//! Chain-authoritative SoraFS reputation source journal.
//!
//! The first-release reputation projector consumes this payload-free journal
//! for the three source families that are not represented by the existing
//! proof, repair, orderbook, and reserve ledgers: terminal `PoR` outcomes,
//! provider-dispute transitions, and stream-token validation outcomes.
//!
//! Source identifiers are domain separated from their native identifiers.
//! Event identifiers are the BLAKE3 digest of the complete canonical entry
//! with its `event_id` field cleared. A ledger implementation must retain one
//! global event-id index and one exact source-head index. Exact event-id
//! replays are idempotent; a different event at an occupied source revision is
//! equivocation.

use std::collections::{BTreeMap, BTreeSet};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    account::AccountId,
    sorafs::capacity::{CapacityDisputeId, CapacityDisputeOutcome, ProviderId},
};

/// First-release reputation-journal entry version.
pub const REPUTATION_JOURNAL_ENTRY_VERSION_V1: u16 = 1;
/// First-release governed recorder-policy version.
pub const REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1: u16 = 1;
/// Hard item ceiling for one finalized reputation-journal event page.
pub const REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1: usize = 128;
/// Hard encoded-byte ceiling for one finalized reputation-journal event page.
pub const REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 128 * 1024;
/// Hard encoded-byte ceiling for one committed journal entry.
pub const REPUTATION_JOURNAL_MAX_ENTRY_BYTES_V1: usize = 16 * 1024;
/// Hard UTF-8 byte ceiling for a governance resolution rationale.
pub const REPUTATION_JOURNAL_MAX_TEXT_BYTES_V1: usize = 2_048;
/// Hard governed ceiling for the age of a submitted source observation.
pub const REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1: u64 = 30 * 24 * 60 * 60 * 1_000;

/// Domain separator for governed recorder-policy digests.
pub const REPUTATION_JOURNAL_AUTHORITY_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.reputation.journal.authority-policy.v1";
/// Domain separator for globally unique `PoR` source identifiers.
pub const REPUTATION_JOURNAL_POR_SOURCE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.reputation.journal.source.por.v1";
/// Domain separator for globally unique provider-dispute source identifiers.
pub const REPUTATION_JOURNAL_DISPUTE_SOURCE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.reputation.journal.source.provider-dispute.v1";
/// Domain separator for globally unique stream-token source identifiers.
pub const REPUTATION_JOURNAL_TOKEN_SOURCE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.reputation.journal.source.stream-token.v1";
/// Domain separator for content-derived journal event identifiers.
pub const REPUTATION_JOURNAL_EVENT_ID_DOMAIN_V1: &[u8] = b"sorafs.reputation.journal.event-id.v1";

/// Globally namespaced identity of one native reputation source.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(transparent),
    norito(with = "crate::json_helpers::fixed_bytes")
)]
pub struct ReputationJournalSourceIdV1(pub [u8; 32]);

impl ReputationJournalSourceIdV1 {
    /// Reserved inert identity.
    pub const ZERO: Self = Self([0; 32]);

    /// Derive the globally namespaced source for a native `PoR` challenge.
    #[must_use]
    pub fn for_por_challenge(challenge_id: [u8; 32]) -> Self {
        source_id(REPUTATION_JOURNAL_POR_SOURCE_ID_DOMAIN_V1, challenge_id)
    }

    /// Derive the globally namespaced source for a native capacity dispute.
    #[must_use]
    pub fn for_provider_dispute(dispute_id: CapacityDisputeId) -> Self {
        source_id(
            REPUTATION_JOURNAL_DISPUTE_SOURCE_ID_DOMAIN_V1,
            *dispute_id.as_bytes(),
        )
    }

    /// Derive the globally namespaced source for a native stream-token validation.
    #[must_use]
    pub fn for_stream_token_validation(validation_id: [u8; 32]) -> Self {
        source_id(REPUTATION_JOURNAL_TOKEN_SOURCE_ID_DOMAIN_V1, validation_id)
    }

    /// Access the raw digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Globally unique content-derived identity of one journal event.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(transparent),
    norito(with = "crate::json_helpers::fixed_bytes")
)]
pub struct ReputationJournalEventIdV1(pub [u8; 32]);

impl ReputationJournalEventIdV1 {
    /// Reserved inert identity used only while deriving the canonical id.
    pub const ZERO: Self = Self([0; 32]);

    /// Access the raw digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable journal source family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "source", content = "detail", rename_all = "snake_case")
)]
pub enum ReputationJournalSourceKindV1 {
    /// Terminal proof-of-retrievability outcomes.
    Por,
    /// Provider capacity-dispute lifecycle.
    ProviderDispute,
    /// Gateway stream-token validation outcomes.
    StreamToken,
}

/// Governed accounts allowed to append each first-release journal source.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalAuthorityPolicyV1 {
    /// Schema version.
    pub version: u16,
    /// Monotonic policy revision beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding canonical policy.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Exact governed authority allowed to record `PoR` terminals.
    pub por_recorder_authority: AccountId,
    /// Exact governed authority allowed to record dispute transitions.
    pub dispute_recorder_authority: AccountId,
    /// Exact governed regional-gateway authority allowed to record token outcomes.
    pub token_recorder_authority: AccountId,
    /// Maximum age of an authenticated source decision or observation at commit.
    pub max_source_age_ms: u64,
}

impl ReputationJournalAuthorityPolicyV1 {
    /// Validate the policy version and predecessor chain.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for an unsupported version, zero
    /// revision, non-canonical predecessor link, or unsafe freshness bound.
    pub fn validate(&self) -> Result<(), ReputationJournalValidationError> {
        if self.version != REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1 {
            return Err(
                ReputationJournalValidationError::UnsupportedAuthorityPolicyVersion {
                    found: self.version,
                },
            );
        }
        if self.revision == 0 {
            return Err(ReputationJournalValidationError::ZeroAuthorityPolicyRevision);
        }
        if self.max_source_age_ms == 0
            || self.max_source_age_ms > REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1
        {
            return Err(ReputationJournalValidationError::InvalidSourceAgeLimit {
                found: self.max_source_age_ms,
                maximum: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
            });
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => Ok(()),
            (1, Some(_)) => Err(ReputationJournalValidationError::UnexpectedPolicyPredecessor),
            (_, Some(digest)) if digest != [0; 32] => Ok(()),
            _ => Err(ReputationJournalValidationError::MissingPolicyPredecessor),
        }
    }

    /// Compute the canonical domain-separated policy digest.
    ///
    /// # Errors
    ///
    /// Returns a policy validation or canonical encoding error.
    pub fn canonical_digest(&self) -> Result<[u8; 32], ReputationJournalValidationError> {
        self.validate()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| ReputationJournalValidationError::CanonicalEncoding)?;
        Ok(domain_digest(
            REPUTATION_JOURNAL_AUTHORITY_POLICY_DIGEST_DOMAIN_V1,
            &bytes,
        ))
    }

    /// Return the exact recorder authorized for `source`.
    #[must_use]
    pub const fn recorder_authority(&self, source: ReputationJournalSourceKindV1) -> &AccountId {
        match source {
            ReputationJournalSourceKindV1::Por => &self.por_recorder_authority,
            ReputationJournalSourceKindV1::ProviderDispute => &self.dispute_recorder_authority,
            ReputationJournalSourceKindV1::StreamToken => &self.token_recorder_authority,
        }
    }
}

/// Auditable activation record for one governed recorder-policy revision.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalAuthorityPolicyRecordV1 {
    /// Canonical governed policy.
    pub policy: ReputationJournalAuthorityPolicyV1,
    /// Canonical domain-separated digest of `policy`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Governance authority that activated the revision.
    pub activated_by: AccountId,
    /// Exact committing block timestamp in milliseconds since Unix epoch.
    pub activated_at_unix_ms: u64,
}

impl ReputationJournalAuthorityPolicyRecordV1 {
    /// Construct and validate an auditable policy activation record.
    ///
    /// # Errors
    ///
    /// Returns a policy, digest, or timestamp validation error.
    pub fn try_new(
        policy: ReputationJournalAuthorityPolicyV1,
        activated_by: AccountId,
        activated_at_unix_ms: u64,
    ) -> Result<Self, ReputationJournalValidationError> {
        let policy_digest = policy.canonical_digest()?;
        ensure_digest(policy_digest, "policy_digest")?;
        ensure_timestamp(activated_at_unix_ms, "activated_at_unix_ms")?;
        Ok(Self {
            policy,
            policy_digest,
            activated_by,
            activated_at_unix_ms,
        })
    }

    /// Validate the canonical policy digest and activation timestamp.
    ///
    /// # Errors
    ///
    /// Returns a policy, digest, or timestamp validation error.
    pub fn validate(&self) -> Result<(), ReputationJournalValidationError> {
        ensure_digest(self.policy_digest, "policy_digest")?;
        ensure_timestamp(self.activated_at_unix_ms, "activated_at_unix_ms")?;
        if self.policy_digest != self.policy.canonical_digest()? {
            return Err(ReputationJournalValidationError::AuthorityPolicyDigestMismatch);
        }
        Ok(())
    }
}

/// Stable provider-attributable `PoR` failure classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum PorTerminalFailureKindV1 {
    /// No proof arrived before the governed deadline.
    DeadlineExpired,
    /// A proof arrived after the governed deadline.
    SubmissionLate,
    /// An authenticated proof failed binding, coverage, or Merkle verification.
    InvalidProof,
    /// Retained storage was unavailable for safe proof generation.
    StorageUnavailable,
}

impl PorTerminalFailureKindV1 {
    const fn carries_proof(self) -> bool {
        matches!(self, Self::SubmissionLate | Self::InvalidProof)
    }
}

/// `PoR` terminal that cannot safely affect provider reputation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum PorTerminalExcludedKindV1 {
    /// Governance revoked provider admission while the challenge was active.
    AdmissionRevoked,
    /// The active admission did not authorize the provider or proof key.
    AdmissionInactive,
}

/// Final, immutable `PoR` classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "detail", rename_all = "snake_case")
)]
pub enum PorTerminalStatusV1 {
    /// The original provider proof verified before the deadline.
    Verified,
    /// The challenge ended with a provider-attributable failure.
    Failed(PorTerminalFailureKindV1),
    /// The challenge ended without a safely attributable provider observation.
    Excluded(PorTerminalExcludedKindV1),
}

impl PorTerminalStatusV1 {
    /// Whether this event increments the provider `PoR` observation counter.
    #[must_use]
    pub const fn counts_for_provider(self) -> bool {
        !matches!(self, Self::Excluded(_))
    }

    /// Whether this terminal is a successful original `PoR` observation.
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Verified)
    }

    /// Whether this terminal increments the provider `PoR` failure counter.
    #[must_use]
    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// Payload-free terminal `PoR` projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PorTerminalOutcomeV1 {
    /// Native BLAKE3-256 challenge identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub challenge_id: [u8; 32],
    /// Manifest named by the challenge.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_digest: [u8; 32],
    /// Challenge epoch.
    pub epoch_id: u64,
    /// drand round mixed into challenge selection.
    pub drand_round: u64,
    /// Whether the coordinator forced sampling because provider VRF was absent.
    pub forced: bool,
    /// Number of samples requested.
    pub sample_count: u16,
    /// Number of samples classified as failed.
    pub failed_samples: u16,
    /// Challenge issue time in milliseconds since Unix epoch.
    pub issued_at_unix_ms: u64,
    /// Governed response deadline in milliseconds since Unix epoch.
    pub deadline_at_unix_ms: u64,
    /// Proof receipt time, present exactly for proof-bearing terminals.
    pub responded_at_unix_ms: Option<u64>,
    /// Authenticated terminal decision time in milliseconds since Unix epoch.
    pub decided_at_unix_ms: u64,
    /// Canonical provider-signed proof digest for proof-bearing terminals.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub proof_digest: Option<[u8; 32]>,
    /// Authoritative repair task for a failed challenge.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub repair_task_id: Option<[u8; 32]>,
    /// Deterministic verifier latency in integer milliseconds.
    pub verifier_latency_ms: Option<u32>,
    /// Stable terminal classification.
    pub status: PorTerminalStatusV1,
}

impl PorTerminalOutcomeV1 {
    fn validate(&self, source_time_unix_ms: u64) -> Result<(), ReputationJournalValidationError> {
        ensure_digest(self.challenge_id, "challenge_id")?;
        ensure_digest(self.manifest_digest, "manifest_digest")?;
        if self.epoch_id == 0 || self.drand_round == 0 {
            return Err(ReputationJournalValidationError::InvalidPorRandomnessIdentity);
        }
        ensure_timestamp(self.issued_at_unix_ms, "issued_at_unix_ms")?;
        ensure_timestamp(self.deadline_at_unix_ms, "deadline_at_unix_ms")?;
        ensure_timestamp(self.decided_at_unix_ms, "decided_at_unix_ms")?;
        if self.deadline_at_unix_ms <= self.issued_at_unix_ms {
            return Err(ReputationJournalValidationError::InvalidTimestampOrder {
                earlier: "issued_at_unix_ms",
                later: "deadline_at_unix_ms",
            });
        }
        if self.decided_at_unix_ms < self.issued_at_unix_ms {
            return Err(ReputationJournalValidationError::InvalidTimestampOrder {
                earlier: "issued_at_unix_ms",
                later: "decided_at_unix_ms",
            });
        }
        if self.decided_at_unix_ms != source_time_unix_ms {
            return Err(ReputationJournalValidationError::SourceTimestampMismatch);
        }
        if let Some(responded_at) = self.responded_at_unix_ms {
            ensure_timestamp(responded_at, "responded_at_unix_ms")?;
            if responded_at < self.issued_at_unix_ms || responded_at > self.decided_at_unix_ms {
                return Err(ReputationJournalValidationError::InvalidPorResponseTimestamp);
            }
        }
        if self.sample_count == 0
            || self.failed_samples > self.sample_count
            || self.proof_digest.is_some_and(|digest| digest == [0; 32])
            || self
                .repair_task_id
                .is_some_and(|task_id| task_id == [0; 32])
        {
            return Err(ReputationJournalValidationError::InvalidPorCounters);
        }

        match self.status {
            PorTerminalStatusV1::Verified => {
                if self.failed_samples != 0
                    || self.responded_at_unix_ms.is_none()
                    || self.proof_digest.is_none()
                    || self.repair_task_id.is_some()
                    || self.verifier_latency_ms.is_none_or(|latency| latency == 0)
                    || self
                        .responded_at_unix_ms
                        .is_some_and(|responded_at| responded_at > self.deadline_at_unix_ms)
                {
                    return Err(ReputationJournalValidationError::ImpossiblePorStatus);
                }
            }
            PorTerminalStatusV1::Failed(failure) => {
                if self.failed_samples == 0 || self.repair_task_id.is_none() {
                    return Err(ReputationJournalValidationError::ImpossiblePorStatus);
                }
                self.validate_failure_material(failure)?;
                if failure == PorTerminalFailureKindV1::DeadlineExpired
                    && self.decided_at_unix_ms < self.deadline_at_unix_ms
                {
                    return Err(ReputationJournalValidationError::ImpossiblePorStatus);
                }
            }
            PorTerminalStatusV1::Excluded(_) => {
                if self.failed_samples != 0
                    || self.responded_at_unix_ms.is_some()
                    || self.proof_digest.is_some()
                    || self.repair_task_id.is_some()
                    || self.verifier_latency_ms.is_some()
                {
                    return Err(ReputationJournalValidationError::ImpossiblePorStatus);
                }
            }
        }
        Ok(())
    }

    fn validate_failure_material(
        &self,
        failure: PorTerminalFailureKindV1,
    ) -> Result<(), ReputationJournalValidationError> {
        let proof_present = self.proof_digest.is_some();
        let response_present = self.responded_at_unix_ms.is_some();
        let latency_present = self.verifier_latency_ms.is_some_and(|latency| latency > 0);
        if failure.carries_proof() != proof_present
            || failure.carries_proof() != response_present
            || failure.carries_proof() != latency_present
        {
            return Err(ReputationJournalValidationError::ImpossiblePorStatus);
        }
        if failure == PorTerminalFailureKindV1::SubmissionLate
            && self
                .responded_at_unix_ms
                .is_none_or(|responded_at| responded_at <= self.deadline_at_unix_ms)
        {
            return Err(ReputationJournalValidationError::ImpossiblePorStatus);
        }
        Ok(())
    }
}

/// Stable provider-dispute category aligned with capacity governance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum ProviderDisputeKindV1 {
    /// Provider failed replication-order requirements.
    ReplicationShortfall,
    /// Provider uptime fell below the governed SLA.
    UptimeBreach,
    /// Provider failed proof sampling.
    ProofFailure,
    /// Disagreement over fee accrual or billing.
    FeeDispute,
    /// Governed category not covered above.
    Other,
}

/// Resolution material for a terminal provider-dispute transition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProviderDisputeResolutionV1 {
    /// Existing capacity-governance outcome vocabulary.
    pub outcome: CapacityDisputeOutcome,
    /// Authenticated governance decision time.
    pub resolved_at_unix_ms: u64,
    /// Digest of the canonical decision evidence/envelope.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub decision_digest: [u8; 32],
    /// Optional bounded, canonical governance rationale.
    pub rationale: Option<String>,
}

/// Chain-authoritative provider-dispute lifecycle transition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "detail", rename_all = "snake_case")
)]
pub enum ProviderDisputeStatusV1 {
    /// Dispute became active at `submitted_at_unix_ms`.
    Opened,
    /// Governance committed one terminal outcome.
    Resolved(
        /// Exact terminal decision material.
        ProviderDisputeResolutionV1,
    ),
}

/// Payload-free provider-dispute journal projection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProviderDisputeEventV1 {
    /// Existing capacity-dispute identity.
    pub dispute_id: CapacityDisputeId,
    /// Existing capacity-dispute category.
    pub kind: ProviderDisputeKindV1,
    /// Digest of the canonical evidence bundle.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub evidence_digest: [u8; 32],
    /// Authenticated source submission time.
    pub submitted_at_unix_ms: u64,
    /// Lifecycle transition.
    pub status: ProviderDisputeStatusV1,
}

impl ProviderDisputeEventV1 {
    fn validate(&self, source_time_unix_ms: u64) -> Result<(), ReputationJournalValidationError> {
        ensure_digest(*self.dispute_id.as_bytes(), "dispute_id")?;
        ensure_digest(self.evidence_digest, "evidence_digest")?;
        ensure_timestamp(self.submitted_at_unix_ms, "submitted_at_unix_ms")?;
        match &self.status {
            ProviderDisputeStatusV1::Opened => {
                if source_time_unix_ms != self.submitted_at_unix_ms {
                    return Err(ReputationJournalValidationError::SourceTimestampMismatch);
                }
            }
            ProviderDisputeStatusV1::Resolved(resolution) => {
                ensure_timestamp(resolution.resolved_at_unix_ms, "resolved_at_unix_ms")?;
                ensure_digest(resolution.decision_digest, "decision_digest")?;
                if resolution.resolved_at_unix_ms <= self.submitted_at_unix_ms {
                    return Err(ReputationJournalValidationError::InvalidTimestampOrder {
                        earlier: "submitted_at_unix_ms",
                        later: "resolved_at_unix_ms",
                    });
                }
                if source_time_unix_ms != resolution.resolved_at_unix_ms {
                    return Err(ReputationJournalValidationError::SourceTimestampMismatch);
                }
                if let Some(rationale) = &resolution.rationale {
                    validate_text(rationale)?;
                }
            }
        }
        Ok(())
    }
}

/// Provider-attributable stream-token policy violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum StreamTokenViolationKindV1 {
    /// An authenticated token was used after its expiry.
    Expired,
    /// An authenticated token claimed an issuance time beyond allowed skew.
    FutureIssuedAt,
    /// The token did not authorize the requested manifest.
    ManifestMismatch,
    /// The token named a different chunking profile.
    ProfileMismatch,
    /// The token named a different admitted provider.
    ProviderMismatch,
    /// The token exceeded its signed concurrency ceiling.
    ConcurrencyLimitExceeded,
    /// The token exceeded its signed request quota.
    RequestQuotaExceeded,
    /// The token exceeded its signed byte-rate ceiling.
    ByteRateLimitExceeded,
    /// The token identifier collided with a different signed policy.
    IdentifierPolicyConflict,
}

/// Stream-token rejection that cannot safely affect provider reputation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", rename_all = "snake_case")
)]
pub enum StreamTokenExcludedKindV1 {
    /// No stream token was supplied.
    MissingToken,
    /// The transport or canonical token body was malformed.
    MalformedEncoding,
    /// A decoded token carried an invalid signature.
    InvalidSignature,
    /// A decoded token named an unsupported signing-key version.
    UnsupportedKeyVersion,
}

impl StreamTokenExcludedKindV1 {
    const fn carries_decoded_body(self) -> bool {
        matches!(self, Self::InvalidSignature | Self::UnsupportedKeyVersion)
    }
}

/// Typed terminal result of one stream-token validation attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "detail", rename_all = "snake_case")
)]
pub enum StreamTokenValidationStatusV1 {
    /// The token was admitted for the requested provider-bound stream.
    Accepted,
    /// A validated provider-bound token violated signed policy.
    ProviderViolation(StreamTokenViolationKindV1),
    /// Validation failed before safe provider attribution.
    Excluded(StreamTokenExcludedKindV1),
}

impl StreamTokenValidationStatusV1 {
    /// Whether this event increments the provider token-observation counter.
    #[must_use]
    pub const fn counts_for_provider(self) -> bool {
        !matches!(self, Self::Excluded(_))
    }

    /// Whether this event increments the provider token-violation counter.
    #[must_use]
    pub const fn is_violation(self) -> bool {
        matches!(self, Self::ProviderViolation(_))
    }

    const fn carries_decoded_body(self) -> bool {
        match self {
            Self::Accepted | Self::ProviderViolation(_) => true,
            Self::Excluded(reason) => reason.carries_decoded_body(),
        }
    }
}

/// Payload-free result of one provider-bound stream-token validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StreamTokenValidationOutcomeV1 {
    /// Native request-attempt identity, unique across regional gateways.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub validation_id: [u8; 32],
    /// Payload-free digest of the exact range request and serving context.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Canonical signed token-body digest, present exactly after successful decode.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub token_body_digest: Option<[u8; 32]>,
    /// Signing-key version from the decoded token body.
    pub token_key_version: Option<u32>,
    /// Authenticated gateway observation time in milliseconds since Unix epoch.
    pub validated_at_unix_ms: u64,
    /// Stable validation result.
    pub status: StreamTokenValidationStatusV1,
}

impl StreamTokenValidationOutcomeV1 {
    fn validate(&self, source_time_unix_ms: u64) -> Result<(), ReputationJournalValidationError> {
        ensure_digest(self.validation_id, "validation_id")?;
        ensure_digest(self.request_digest, "request_digest")?;
        ensure_timestamp(self.validated_at_unix_ms, "validated_at_unix_ms")?;
        if self.validated_at_unix_ms != source_time_unix_ms {
            return Err(ReputationJournalValidationError::SourceTimestampMismatch);
        }
        let carries_decoded_body = self.status.carries_decoded_body();
        if carries_decoded_body != self.token_body_digest.is_some()
            || carries_decoded_body != self.token_key_version.is_some()
            || self
                .token_body_digest
                .is_some_and(|digest| digest == [0; 32])
            || self.token_key_version == Some(0)
        {
            return Err(ReputationJournalValidationError::TokenMaterialMismatch);
        }
        Ok(())
    }
}

/// One typed first-release journal payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "payload", rename_all = "snake_case")
)]
pub enum ReputationJournalPayloadV1 {
    /// Terminal proof-of-retrievability projection.
    PorTerminal(PorTerminalOutcomeV1),
    /// Provider-dispute lifecycle transition.
    ProviderDispute(ProviderDisputeEventV1),
    /// Stream-token validation result.
    StreamTokenValidation(StreamTokenValidationOutcomeV1),
}

impl ReputationJournalPayloadV1 {
    /// Return the stable journal source family.
    #[must_use]
    pub const fn source_kind(&self) -> ReputationJournalSourceKindV1 {
        match self {
            Self::PorTerminal(_) => ReputationJournalSourceKindV1::Por,
            Self::ProviderDispute(_) => ReputationJournalSourceKindV1::ProviderDispute,
            Self::StreamTokenValidation(_) => ReputationJournalSourceKindV1::StreamToken,
        }
    }

    /// Return the domain-separated native source identity.
    #[must_use]
    pub fn source_id(&self) -> ReputationJournalSourceIdV1 {
        match self {
            Self::PorTerminal(outcome) => {
                ReputationJournalSourceIdV1::for_por_challenge(outcome.challenge_id)
            }
            Self::ProviderDispute(event) => {
                ReputationJournalSourceIdV1::for_provider_dispute(event.dispute_id)
            }
            Self::StreamTokenValidation(outcome) => {
                ReputationJournalSourceIdV1::for_stream_token_validation(outcome.validation_id)
            }
        }
    }

    const fn expected_source_revision(&self) -> u32 {
        match self {
            Self::PorTerminal(_) | Self::StreamTokenValidation(_) => 1,
            Self::ProviderDispute(event) => match &event.status {
                ProviderDisputeStatusV1::Opened => 1,
                ProviderDisputeStatusV1::Resolved(_) => 2,
            },
        }
    }

    fn validate(&self, source_time_unix_ms: u64) -> Result<(), ReputationJournalValidationError> {
        match self {
            Self::PorTerminal(outcome) => outcome.validate(source_time_unix_ms),
            Self::ProviderDispute(event) => event.validate(source_time_unix_ms),
            Self::StreamTokenValidation(outcome) => outcome.validate(source_time_unix_ms),
        }
    }
}

/// One chain-authoritative reputation journal entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalEntryV1 {
    /// Entry schema version.
    pub version: u16,
    /// Content-derived globally unique journal event id.
    pub event_id: ReputationJournalEventIdV1,
    /// Domain-separated native source id.
    pub source_id: ReputationJournalSourceIdV1,
    /// Monotonic source revision; V1 permits only dispute revision two.
    pub source_revision: u32,
    /// Exact predecessor event for source revisions after one.
    pub predecessor_event_id: Option<ReputationJournalEventIdV1>,
    /// Provider whose counters may be affected.
    pub provider_id: ProviderId,
    /// Digest of the active recorder-authority policy.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub authority_policy_digest: [u8; 32],
    /// Exact governed transaction authority that committed the entry.
    pub recorded_by: AccountId,
    /// Authenticated source decision or observation time in Unix milliseconds.
    pub source_time_unix_ms: u64,
    /// Typed, payload-free source projection.
    pub payload: ReputationJournalPayloadV1,
}

impl ReputationJournalEntryV1 {
    /// Construct and validate an entry with a canonical content-derived id.
    ///
    /// `predecessor_event_id` is required exactly for a resolved dispute and
    /// forbidden for every revision-one entry. `source_time_unix_ms` must equal
    /// the typed payload decision/observation time; consensus supplies the
    /// separate authoritative recorded time when the instruction executes.
    ///
    /// # Errors
    ///
    /// Returns a typed structural, semantic, or encoded-size validation error.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: ProviderId,
        authority_policy_digest: [u8; 32],
        recorded_by: AccountId,
        source_time_unix_ms: u64,
        predecessor_event_id: Option<ReputationJournalEventIdV1>,
        payload: ReputationJournalPayloadV1,
    ) -> Result<Self, ReputationJournalValidationError> {
        let source_id = payload.source_id();
        let source_revision = payload.expected_source_revision();
        let mut entry = Self {
            version: REPUTATION_JOURNAL_ENTRY_VERSION_V1,
            event_id: ReputationJournalEventIdV1::ZERO,
            source_id,
            source_revision,
            predecessor_event_id,
            provider_id,
            authority_policy_digest,
            recorded_by,
            source_time_unix_ms,
            payload,
        };
        entry.event_id = entry.expected_event_id()?;
        entry.validate()?;
        Ok(entry)
    }

    /// Return the stable source family.
    #[must_use]
    pub const fn source_kind(&self) -> ReputationJournalSourceKindV1 {
        self.payload.source_kind()
    }

    /// Validate the complete canonical entry.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid ids, provider binding, timestamps,
    /// source revision linkage, payload semantics, event-id mismatch, or size.
    pub fn validate(&self) -> Result<(), ReputationJournalValidationError> {
        if self.version != REPUTATION_JOURNAL_ENTRY_VERSION_V1 {
            return Err(ReputationJournalValidationError::UnsupportedEntryVersion {
                found: self.version,
            });
        }
        if self.event_id == ReputationJournalEventIdV1::ZERO {
            return Err(ReputationJournalValidationError::ZeroEventId);
        }
        if self.source_id == ReputationJournalSourceIdV1::ZERO {
            return Err(ReputationJournalValidationError::ZeroSourceId);
        }
        if self.event_id.as_bytes() == self.source_id.as_bytes() {
            return Err(ReputationJournalValidationError::SourceEventIdCollision);
        }
        if self.provider_id.as_bytes() == &[0; 32] {
            return Err(ReputationJournalValidationError::ZeroProviderId);
        }
        ensure_digest(self.authority_policy_digest, "authority_policy_digest")?;
        ensure_timestamp(self.source_time_unix_ms, "source_time_unix_ms")?;
        self.payload.validate(self.source_time_unix_ms)?;
        if self.source_id != self.payload.source_id() {
            return Err(ReputationJournalValidationError::SourceIdMismatch);
        }
        if self.source_revision != self.payload.expected_source_revision() {
            return Err(ReputationJournalValidationError::InvalidSourceRevision);
        }
        match (self.source_revision, self.predecessor_event_id) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(ReputationJournalValidationError::UnexpectedEventPredecessor);
            }
            (2, Some(predecessor))
                if predecessor != ReputationJournalEventIdV1::ZERO
                    && predecessor != self.event_id
                    && predecessor.as_bytes() != self.source_id.as_bytes() => {}
            (2, Some(_)) => {
                return Err(ReputationJournalValidationError::InvalidEventPredecessor);
            }
            (2, None) => {
                return Err(ReputationJournalValidationError::MissingEventPredecessor);
            }
            _ => return Err(ReputationJournalValidationError::InvalidSourceRevision),
        }
        if self.event_id != self.expected_event_id()? {
            return Err(ReputationJournalValidationError::EventIdMismatch);
        }
        let encoded = norito::encode_canonical(self)
            .map_err(|_| ReputationJournalValidationError::CanonicalEncoding)?;
        if encoded.len() > REPUTATION_JOURNAL_MAX_ENTRY_BYTES_V1 {
            return Err(ReputationJournalValidationError::EntryTooLarge {
                found: encoded.len(),
                maximum: REPUTATION_JOURNAL_MAX_ENTRY_BYTES_V1,
            });
        }
        Ok(())
    }

    /// Validate this entry against the exact active governed authority policy.
    ///
    /// # Errors
    ///
    /// Returns a structural entry/policy error, digest mismatch, or recorder
    /// authority mismatch. Core execution must additionally require the
    /// transaction authority to equal `recorded_by`.
    pub fn validate_against_policy(
        &self,
        policy: &ReputationJournalAuthorityPolicyV1,
    ) -> Result<(), ReputationJournalValidationError> {
        self.validate()?;
        if self.authority_policy_digest != policy.canonical_digest()? {
            return Err(ReputationJournalValidationError::AuthorityPolicyDigestMismatch);
        }
        if &self.recorded_by != policy.recorder_authority(self.source_kind()) {
            return Err(ReputationJournalValidationError::RecorderAuthorityMismatch);
        }
        Ok(())
    }

    /// Validate policy binding and source freshness at authoritative commit time.
    ///
    /// V1 accepts no future source skew: an authenticated source decision or
    /// observation must not be later than the executing block. Its age must
    /// also remain within the active governed policy bound.
    ///
    /// # Errors
    ///
    /// Returns a structural/policy error, a future source-time error, or a
    /// governed freshness violation.
    pub fn validate_at_commit(
        &self,
        policy: &ReputationJournalAuthorityPolicyV1,
        recorded_at_unix_ms: u64,
    ) -> Result<(), ReputationJournalValidationError> {
        self.validate_against_policy(policy)?;
        ensure_timestamp(recorded_at_unix_ms, "recorded_at_unix_ms")?;
        let source_age_ms = recorded_at_unix_ms
            .checked_sub(self.source_time_unix_ms)
            .ok_or(ReputationJournalValidationError::SourceTimeAfterCommit)?;
        if source_age_ms > policy.max_source_age_ms {
            return Err(ReputationJournalValidationError::SourceObservationStale {
                age_ms: source_age_ms,
                maximum_ms: policy.max_source_age_ms,
            });
        }
        Ok(())
    }

    fn expected_event_id(
        &self,
    ) -> Result<ReputationJournalEventIdV1, ReputationJournalValidationError> {
        let mut material = self.clone();
        material.event_id = ReputationJournalEventIdV1::ZERO;
        let bytes = norito::encode_canonical(&material)
            .map_err(|_| ReputationJournalValidationError::CanonicalEncoding)?;
        Ok(ReputationJournalEventIdV1(domain_digest(
            REPUTATION_JOURNAL_EVENT_ID_DOMAIN_V1,
            &bytes,
        )))
    }
}

/// Authoritative head of one native reputation source lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalSourceHeadV1 {
    /// Source family permanently bound to the source id.
    pub source_kind: ReputationJournalSourceKindV1,
    /// Latest committed source revision.
    pub source_revision: u32,
    /// Content-derived id of the latest committed source event.
    pub event_id: ReputationJournalEventIdV1,
    /// Global journal sequence containing the latest source event.
    pub sequence: u64,
}

impl ReputationJournalSourceHeadV1 {
    /// Validate a non-inert source head.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an inert revision, event id, or sequence.
    pub fn validate(self) -> Result<(), ReputationJournalValidationError> {
        let valid_revision = match self.source_kind {
            ReputationJournalSourceKindV1::ProviderDispute => {
                (1..=2).contains(&self.source_revision)
            }
            ReputationJournalSourceKindV1::Por | ReputationJournalSourceKindV1::StreamToken => {
                self.source_revision == 1
            }
        };
        if !valid_revision {
            return Err(ReputationJournalValidationError::InvalidSourceRevision);
        }
        if self.event_id == ReputationJournalEventIdV1::ZERO {
            return Err(ReputationJournalValidationError::ZeroEventId);
        }
        if self.sequence == 0 {
            return Err(ReputationJournalValidationError::InvalidJournalSequence);
        }
        Ok(())
    }
}

/// Pre-finality journal record persisted by consensus execution.
///
/// The committing block hash is deliberately absent while the block executes.
/// Fixed-view queries resolve `target_block_height` against the finalized block
/// hash journal and return [`ReputationJournalFinalizedEventV1`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalCommittedEventRecordV1 {
    /// One-based global journal sequence.
    pub sequence: u64,
    /// Executing block height that must later resolve through finality.
    pub target_block_height: u64,
    /// Reputation-journal event index within the executing block.
    pub event_index: u32,
    /// Authoritative executing-block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Complete canonical journal entry.
    pub entry: ReputationJournalEntryV1,
}

impl ReputationJournalCommittedEventRecordV1 {
    /// Validate the persisted position and canonical journal entry.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for an inert sequence/height or entry.
    pub fn validate(&self) -> Result<(), ReputationJournalValidationError> {
        if self.sequence == 0 {
            return Err(ReputationJournalValidationError::InvalidJournalSequence);
        }
        if self.target_block_height == 0 {
            return Err(ReputationJournalValidationError::InvalidTargetBlockHeight);
        }
        ensure_timestamp(self.recorded_at_unix_ms, "recorded_at_unix_ms")?;
        self.entry.validate()?;
        if self.entry.source_time_unix_ms > self.recorded_at_unix_ms {
            return Err(ReputationJournalValidationError::SourceTimeAfterCommit);
        }
        Ok(())
    }
}

/// Finalized block anchor for one coherent journal query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Exact finalized block hash from that same view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Exact finalized block timestamp in milliseconds since Unix epoch.
    pub finalized_at_unix_ms: u64,
}

impl ReputationJournalFinalizedCursorV1 {
    /// Validate a non-inert finalized identity.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the height, block hash, or finalized
    /// timestamp is inert or otherwise reserved.
    pub fn validate(self) -> Result<(), ReputationJournalValidationError> {
        if self.height == 0
            || self.block_hash == [0; 32]
            || self.finalized_at_unix_ms == 0
            || self.finalized_at_unix_ms == u64::MAX
        {
            return Err(ReputationJournalValidationError::InvalidFinalizedCursor);
        }
        Ok(())
    }
}

/// Exclusive cursor for one committed reputation-journal event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalFinalizedEventCursorV1 {
    /// Monotonic journal sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Exact committing block hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Journal-event index within the committing block.
    pub event_index: u32,
}

impl ReputationJournalFinalizedEventCursorV1 {
    /// Validate a non-inert event cursor.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the sequence, block height, or block hash is
    /// inert.
    pub fn validate(self) -> Result<(), ReputationJournalValidationError> {
        if self.sequence == 0 || self.block_height == 0 || self.block_hash == [0; 32] {
            return Err(ReputationJournalValidationError::InvalidEventCursor);
        }
        Ok(())
    }
}

/// One typed journal entry with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalFinalizedEventV1 {
    /// Monotonic journal sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Journal-event index within the committing block.
    pub event_index: u32,
    /// Authoritative committing-block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Chain-authoritative journal entry.
    pub entry: ReputationJournalEntryV1,
}

impl ReputationJournalFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> ReputationJournalFinalizedEventCursorV1 {
        ReputationJournalFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }

    fn validate(
        &self,
        finalized_cursor: ReputationJournalFinalizedCursorV1,
    ) -> Result<(), ReputationJournalValidationError> {
        self.cursor().validate()?;
        self.entry.validate()?;
        if self.block_height > finalized_cursor.height {
            return Err(ReputationJournalValidationError::EventBeyondFinality);
        }
        let is_at_finalized_height = self.block_height == finalized_cursor.height;
        let matches_finalized_hash = self.block_hash == finalized_cursor.block_hash;
        if is_at_finalized_height && !matches_finalized_hash {
            return Err(ReputationJournalValidationError::FinalizedBlockHashMismatch);
        }
        ensure_timestamp(self.recorded_at_unix_ms, "recorded_at_unix_ms")?;
        if self.entry.source_time_unix_ms > self.recorded_at_unix_ms {
            return Err(ReputationJournalValidationError::SourceTimeAfterCommit);
        }
        if self.recorded_at_unix_ms > finalized_cursor.finalized_at_unix_ms {
            return Err(ReputationJournalValidationError::EventAfterFinalizedTimestamp);
        }
        Ok(())
    }
}

/// Cursor-bounded page of typed committed reputation-journal events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReputationJournalFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: ReputationJournalFinalizedCursorV1,
    /// Events in strictly contiguous sequence and block/index order.
    pub events: Vec<ReputationJournalFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present exactly when `has_more` is true.
    pub next_after: Option<ReputationJournalFinalizedEventCursorV1>,
}

impl ReputationJournalFinalizedEventPageV1 {
    /// Validate page bounds, internal ordering, identities, and continuation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed finality, invalid entries, duplicate
    /// or reordered events, source equivocation, cursor mismatch, or size.
    pub fn validate(&self) -> Result<(), ReputationJournalValidationError> {
        self.validate_after(None)
    }

    /// Validate a page against the exclusive cursor used by the query.
    ///
    /// Passing the request cursor detects stale, overlapping, or skipped
    /// continuations rather than trusting only the response's `next_after`.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed finality, event ordering or
    /// identity, source equivocation, cursor discontinuity, or page bounds.
    pub fn validate_after(
        &self,
        requested_after: Option<ReputationJournalFinalizedEventCursorV1>,
    ) -> Result<(), ReputationJournalValidationError> {
        self.finalized_cursor.validate()?;
        if self.events.len() > REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1 {
            return Err(ReputationJournalValidationError::TooManyPageItems {
                found: self.events.len(),
                maximum: REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1,
            });
        }
        if self.has_more && self.events.is_empty() {
            return Err(ReputationJournalValidationError::EmptyContinuationPage);
        }
        match (self.has_more, self.next_after, self.events.last()) {
            (true, Some(next_after), Some(last)) if next_after == last.cursor() => {}
            (true, Some(_), Some(_)) => {
                return Err(ReputationJournalValidationError::ContinuationCursorMismatch);
            }
            (true, Some(_), None) => {
                return Err(ReputationJournalValidationError::EmptyContinuationPage);
            }
            (true, None, _) => {
                return Err(ReputationJournalValidationError::MissingContinuationCursor);
            }
            (false, None, _) => {}
            (false, Some(_), _) => {
                return Err(ReputationJournalValidationError::UnexpectedContinuationCursor);
            }
        }

        if let Some(requested_after) = requested_after {
            requested_after.validate()?;
            if requested_after.block_height > self.finalized_cursor.height
                || (requested_after.block_height == self.finalized_cursor.height
                    && requested_after.block_hash != self.finalized_cursor.block_hash)
            {
                return Err(ReputationJournalValidationError::RequestedCursorMismatch);
            }
            if let Some(first) = self.events.first() {
                let expected_sequence = requested_after
                    .sequence
                    .checked_add(1)
                    .ok_or(ReputationJournalValidationError::EventSequenceOverflow)?;
                if first.sequence != expected_sequence
                    || !cursor_precedes_event(requested_after, first)
                {
                    return Err(ReputationJournalValidationError::RequestedCursorMismatch);
                }
            }
        }

        let mut event_ids = BTreeSet::new();
        let mut source_revisions = BTreeMap::new();
        let mut previous: Option<&ReputationJournalFinalizedEventV1> = None;
        for event in &self.events {
            event.validate(self.finalized_cursor)?;
            if !event_ids.insert(event.entry.event_id) {
                return Err(ReputationJournalValidationError::DuplicateEventId);
            }
            if let Some(previous) = previous {
                let expected_sequence = previous
                    .sequence
                    .checked_add(1)
                    .ok_or(ReputationJournalValidationError::EventSequenceOverflow)?;
                if event.sequence <= previous.sequence {
                    return Err(ReputationJournalValidationError::EventSequenceReordered);
                }
                if event.sequence != expected_sequence {
                    return Err(ReputationJournalValidationError::EventSequenceGap);
                }
                validate_block_order(previous, event)?;
                if event.recorded_at_unix_ms < previous.recorded_at_unix_ms {
                    return Err(ReputationJournalValidationError::EventTimestampReordered);
                }
            }
            validate_source_revision(&mut source_revisions, &event.entry)?;
            previous = Some(event);
        }

        let encoded = norito::encode_canonical(self)
            .map_err(|_| ReputationJournalValidationError::CanonicalEncoding)?;
        if encoded.len() > REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
            return Err(ReputationJournalValidationError::EncodedPageTooLarge {
                found: encoded.len(),
                maximum: REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            });
        }
        Ok(())
    }
}

/// Structural and semantic validation failures for the V1 journal.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReputationJournalValidationError {
    /// Recorder policy version is unsupported.
    #[error("unsupported reputation journal authority-policy version {found}")]
    UnsupportedAuthorityPolicyVersion {
        /// Version found in the rejected policy.
        found: u16,
    },
    /// Recorder policy revision zero is reserved.
    #[error("reputation journal authority-policy revision must be non-zero")]
    ZeroAuthorityPolicyRevision,
    /// Revision one must not name a predecessor.
    #[error("reputation journal authority-policy revision one must not name a predecessor")]
    UnexpectedPolicyPredecessor,
    /// Later policy revisions require a non-zero predecessor.
    #[error("reputation journal authority-policy predecessor is missing or zero")]
    MissingPolicyPredecessor,
    /// Governed source-age bound is zero or exceeds the V1 ceiling.
    #[error("reputation journal source-age limit {found}ms is outside 1..={maximum}ms")]
    InvalidSourceAgeLimit {
        /// Rejected governed source-age limit.
        found: u64,
        /// Hard first-release maximum.
        maximum: u64,
    },
    /// Entry version is unsupported.
    #[error("unsupported reputation journal entry version {found}")]
    UnsupportedEntryVersion {
        /// Version found in the rejected entry.
        found: u16,
    },
    /// Event id zero is reserved.
    #[error("reputation journal event id must be non-zero")]
    ZeroEventId,
    /// Source id zero is reserved.
    #[error("reputation journal source id must be non-zero")]
    ZeroSourceId,
    /// Source and event identities must remain in separate namespaces.
    #[error("reputation journal source and event ids must be distinct")]
    SourceEventIdCollision,
    /// Provider id zero is inert.
    #[error("reputation journal provider id must be non-zero")]
    ZeroProviderId,
    /// A required digest is inert.
    #[error("reputation journal digest `{field}` must be non-zero")]
    ZeroDigest {
        /// Canonical field name containing the zero digest.
        field: &'static str,
    },
    /// A required timestamp is zero or the reserved maximum sentinel.
    #[error("reputation journal timestamp `{field}` must be finite and non-zero")]
    InvalidTimestamp {
        /// Canonical field name containing the invalid timestamp.
        field: &'static str,
    },
    /// Two timestamps are not in canonical order.
    #[error("reputation journal timestamp `{later}` must follow `{earlier}`")]
    InvalidTimestampOrder {
        /// Timestamp field that must occur first.
        earlier: &'static str,
        /// Timestamp field that must occur later.
        later: &'static str,
    },
    /// Common source time differs from the typed payload decision/observation.
    #[error("reputation journal source_time_unix_ms differs from the typed payload time")]
    SourceTimestampMismatch,
    /// A source decision or observation lies after the authoritative block time.
    #[error("reputation journal source time lies after authoritative commit time")]
    SourceTimeAfterCommit,
    /// Source material exceeded the active governed freshness limit.
    #[error("reputation journal source observation age {age_ms}ms exceeds {maximum_ms}ms")]
    SourceObservationStale {
        /// Observed source age at authoritative commit.
        age_ms: u64,
        /// Active governed maximum source age.
        maximum_ms: u64,
    },
    /// PoR epoch or drand identity is inert.
    #[error("PoR epoch and drand round must be non-zero")]
    InvalidPorRandomnessIdentity,
    /// `PoR` response time lies outside issue/decision bounds.
    #[error("PoR response timestamp lies outside the issue/decision interval")]
    InvalidPorResponseTimestamp,
    /// `PoR` sample counters or optional digests are impossible.
    #[error("PoR sample counters or optional identifiers are inconsistent")]
    InvalidPorCounters,
    /// `PoR` status does not match its proof, timing, latency, or repair fields.
    #[error("PoR terminal status is inconsistent with its evidence fields")]
    ImpossiblePorStatus,
    /// Stream-token status does not match decoded token material.
    #[error("stream-token validation status is inconsistent with decoded token material")]
    TokenMaterialMismatch,
    /// Governance text is empty, non-canonical, control-bearing, or too large.
    #[error("governance rationale must be trimmed control-free UTF-8 within the V1 bound")]
    InvalidText,
    /// Entry source id does not match its domain-separated native id.
    #[error("reputation journal source id does not match the typed payload")]
    SourceIdMismatch,
    /// Source revision does not match the typed lifecycle.
    #[error("reputation journal source revision is invalid for the typed payload")]
    InvalidSourceRevision,
    /// A revision-one event unexpectedly names a predecessor.
    #[error("reputation journal revision-one event must not name a predecessor")]
    UnexpectedEventPredecessor,
    /// A later source revision is missing its predecessor.
    #[error("reputation journal source revision requires a predecessor event")]
    MissingEventPredecessor,
    /// A predecessor id is zero or aliases this source/current event.
    #[error("reputation journal predecessor event id is invalid")]
    InvalidEventPredecessor,
    /// Event id does not match canonical content.
    #[error("reputation journal event id does not match canonical entry content")]
    EventIdMismatch,
    /// Entry exceeds the hard encoded-byte bound.
    #[error("reputation journal entry has {found} bytes; maximum is {maximum}")]
    EntryTooLarge {
        /// Encoded byte count found.
        found: usize,
        /// Maximum admitted encoded bytes.
        maximum: usize,
    },
    /// The active policy digest differs from the entry binding.
    #[error("reputation journal authority-policy digest mismatch")]
    AuthorityPolicyDigestMismatch,
    /// The committing account is not the governed recorder for this source.
    #[error("reputation journal recorder authority mismatch")]
    RecorderAuthorityMismatch,
    /// Canonical Norito encoding failed.
    #[error("reputation journal canonical encoding failed")]
    CanonicalEncoding,
    /// Finalized block identity is inert.
    #[error("reputation journal finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Event cursor identity is inert.
    #[error("reputation journal event cursor is invalid")]
    InvalidEventCursor,
    /// Event lies beyond the page finality anchor.
    #[error("reputation journal event lies beyond finalized height")]
    EventBeyondFinality,
    /// An event at the final height names another block hash.
    #[error("reputation journal event hash differs from finalized block hash")]
    FinalizedBlockHashMismatch,
    /// Event timestamp lies after the finalized block timestamp.
    #[error("reputation journal event timestamp lies after finalized time")]
    EventAfterFinalizedTimestamp,
    /// Page item count exceeds the hard bound.
    #[error("reputation journal page has {found} items; maximum is {maximum}")]
    TooManyPageItems {
        /// Item count found.
        found: usize,
        /// Maximum admitted item count.
        maximum: usize,
    },
    /// Encoded page exceeds the hard byte bound.
    #[error("reputation journal page has {found} bytes; maximum is {maximum}")]
    EncodedPageTooLarge {
        /// Encoded byte count found.
        found: usize,
        /// Maximum admitted encoded bytes.
        maximum: usize,
    },
    /// A page claims continuation without advancing.
    #[error("reputation journal continuation page must contain an event")]
    EmptyContinuationPage,
    /// A continuing page omitted its cursor.
    #[error("reputation journal continuing page is missing next_after")]
    MissingContinuationCursor,
    /// A terminal page unexpectedly retained a cursor.
    #[error("reputation journal terminal page must not contain next_after")]
    UnexpectedContinuationCursor,
    /// Continuation cursor is not the last returned event.
    #[error("reputation journal next_after does not identify the last event")]
    ContinuationCursorMismatch,
    /// Query cursor and returned first event are not contiguous.
    #[error("reputation journal page does not continue its requested cursor")]
    RequestedCursorMismatch,
    /// An event id appears more than once in one page.
    #[error("reputation journal page contains a duplicate event id")]
    DuplicateEventId,
    /// Journal sequence moved backwards or repeated.
    #[error("reputation journal event sequence is reordered")]
    EventSequenceReordered,
    /// Journal sequence skipped an event.
    #[error("reputation journal event sequence contains a gap")]
    EventSequenceGap,
    /// Journal sequence overflowed.
    #[error("reputation journal event sequence overflow")]
    EventSequenceOverflow,
    /// Persisted journal sequence zero is reserved.
    #[error("reputation journal persisted sequence must be non-zero")]
    InvalidJournalSequence,
    /// Persisted target block height zero is reserved.
    #[error("reputation journal target block height must be non-zero")]
    InvalidTargetBlockHeight,
    /// Committing block/event position moved backwards or repeated.
    #[error("reputation journal block/event position is reordered")]
    BlockEventReordered,
    /// Events from one block disagree on its hash.
    #[error("reputation journal events at one height disagree on block hash")]
    BlockHashMismatch,
    /// Committing timestamps moved backwards.
    #[error("reputation journal event timestamps are reordered")]
    EventTimestampReordered,
    /// One source revision has conflicting semantic content.
    #[error("reputation journal source revision equivocation")]
    SourceEquivocation,
    /// Same-source lifecycle revisions are reordered or skip their predecessor.
    #[error("reputation journal source lifecycle is not predecessor-contiguous")]
    SourceRevisionDiscontinuity,
}

fn ensure_digest(
    digest: [u8; 32],
    field: &'static str,
) -> Result<(), ReputationJournalValidationError> {
    if digest == [0; 32] {
        return Err(ReputationJournalValidationError::ZeroDigest { field });
    }
    Ok(())
}

fn ensure_timestamp(
    timestamp: u64,
    field: &'static str,
) -> Result<(), ReputationJournalValidationError> {
    if timestamp == 0 || timestamp == u64::MAX {
        return Err(ReputationJournalValidationError::InvalidTimestamp { field });
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<(), ReputationJournalValidationError> {
    if text.is_empty()
        || text.len() > REPUTATION_JOURNAL_MAX_TEXT_BYTES_V1
        || text.trim() != text
        || text.chars().any(char::is_control)
    {
        return Err(ReputationJournalValidationError::InvalidText);
    }
    Ok(())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn source_id(domain: &[u8], native_id: [u8; 32]) -> ReputationJournalSourceIdV1 {
    ReputationJournalSourceIdV1(domain_digest(domain, &native_id))
}

fn cursor_precedes_event(
    cursor: ReputationJournalFinalizedEventCursorV1,
    event: &ReputationJournalFinalizedEventV1,
) -> bool {
    if cursor.block_height > event.block_height {
        return false;
    }
    if cursor.block_height == event.block_height {
        return cursor.block_hash == event.block_hash
            && cursor.event_index.checked_add(1) == Some(event.event_index);
    }
    event.event_index == 0
}

fn validate_block_order(
    previous: &ReputationJournalFinalizedEventV1,
    event: &ReputationJournalFinalizedEventV1,
) -> Result<(), ReputationJournalValidationError> {
    if event.block_height < previous.block_height {
        return Err(ReputationJournalValidationError::BlockEventReordered);
    }
    if event.block_height == previous.block_height {
        if event.block_hash != previous.block_hash {
            return Err(ReputationJournalValidationError::BlockHashMismatch);
        }
        if previous.event_index.checked_add(1) != Some(event.event_index) {
            return Err(ReputationJournalValidationError::BlockEventReordered);
        }
    } else if event.event_index != 0 {
        return Err(ReputationJournalValidationError::BlockEventReordered);
    }
    Ok(())
}

fn validate_source_revision(
    revisions: &mut BTreeMap<
        ReputationJournalSourceIdV1,
        (
            ReputationJournalSourceKindV1,
            u32,
            ReputationJournalEventIdV1,
        ),
    >,
    entry: &ReputationJournalEntryV1,
) -> Result<(), ReputationJournalValidationError> {
    let Some((kind, revision, event_id)) = revisions.get(&entry.source_id).copied() else {
        revisions.insert(
            entry.source_id,
            (entry.source_kind(), entry.source_revision, entry.event_id),
        );
        return Ok(());
    };
    if kind != entry.source_kind() || revision == entry.source_revision {
        return Err(ReputationJournalValidationError::SourceEquivocation);
    }
    if revision.checked_add(1) != Some(entry.source_revision)
        || entry.predecessor_event_id != Some(event_id)
    {
        return Err(ReputationJournalValidationError::SourceRevisionDiscontinuity);
    }
    revisions.insert(
        entry.source_id,
        (entry.source_kind(), entry.source_revision, entry.event_id),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    const BLOCK_HASH: [u8; 32] = [0xB1; 32];
    const FINAL_HASH: [u8; 32] = [0xF1; 32];
    const SOURCE_TIME: u64 = 1_700_000_001_700;
    const RECORDED_AT: u64 = SOURCE_TIME + 250;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("nonzero deterministic Ed25519 seed");
        AccountId::new(keypair.public_key().clone())
    }

    fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(value).expect("encode alternate-layout reputation value")
    }

    fn policy() -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: account(1),
            dispute_recorder_authority: account(2),
            token_recorder_authority: account(3),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }

    #[test]
    fn authority_policy_digest_ignores_ambient_norito_layout() {
        let policy = policy();
        let canonical_digest = policy.canonical_digest().expect("valid policy digest");
        let canonical_bytes =
            norito::encode_canonical(&policy).expect("encode canonical authority policy");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&policy).expect("encode alternate-layout authority policy"),
            canonical_bytes,
            "fixture must exercise a distinct ambient Norito layout"
        );
        assert_eq!(
            policy
                .canonical_digest()
                .expect("digest policy under alternate ambient layout"),
            canonical_digest
        );
    }

    fn digest_from_u32(value: u32) -> [u8; 32] {
        let mut digest = [0xA5; 32];
        digest[..4].copy_from_slice(&value.to_be_bytes());
        digest
    }

    fn por_payload(seed: u8) -> ReputationJournalPayloadV1 {
        ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
            challenge_id: [seed.max(1); 32],
            manifest_digest: [0x44; 32],
            epoch_id: 9,
            drand_round: 11,
            forced: false,
            sample_count: 8,
            failed_samples: 0,
            issued_at_unix_ms: 1_700_000_000_000,
            deadline_at_unix_ms: 1_700_000_001_500,
            responded_at_unix_ms: Some(1_700_000_001_400),
            decided_at_unix_ms: SOURCE_TIME,
            proof_digest: Some([0x55; 32]),
            repair_task_id: None,
            verifier_latency_ms: Some(7),
            status: PorTerminalStatusV1::Verified,
        })
    }

    fn por_entry(seed: u8) -> ReputationJournalEntryV1 {
        let policy = policy();
        ReputationJournalEntryV1::try_new(
            ProviderId::new([0x22; 32]),
            policy.canonical_digest().expect("policy digest"),
            policy.por_recorder_authority.clone(),
            SOURCE_TIME,
            None,
            por_payload(seed),
        )
        .expect("valid PoR entry")
    }

    fn finalized_event(
        sequence: u64,
        event_index: u32,
        seed: u8,
    ) -> ReputationJournalFinalizedEventV1 {
        ReputationJournalFinalizedEventV1 {
            sequence,
            block_height: 5,
            block_hash: BLOCK_HASH,
            event_index,
            recorded_at_unix_ms: RECORDED_AT,
            entry: por_entry(seed),
        }
    }

    fn finalized_cursor() -> ReputationJournalFinalizedCursorV1 {
        ReputationJournalFinalizedCursorV1 {
            height: 10,
            block_hash: FINAL_HASH,
            finalized_at_unix_ms: SOURCE_TIME + 1_000,
        }
    }

    fn page(
        events: Vec<ReputationJournalFinalizedEventV1>,
    ) -> ReputationJournalFinalizedEventPageV1 {
        ReputationJournalFinalizedEventPageV1 {
            finalized_cursor: finalized_cursor(),
            events,
            has_more: false,
            next_after: None,
        }
    }

    fn resolved_dispute_entry(index: u32, rationale: String) -> ReputationJournalEntryV1 {
        let policy = policy();
        ReputationJournalEntryV1::try_new(
            ProviderId::new([0x33; 32]),
            policy.canonical_digest().expect("policy digest"),
            policy.dispute_recorder_authority.clone(),
            SOURCE_TIME,
            Some(ReputationJournalEventIdV1(digest_from_u32(
                index.saturating_add(10_000),
            ))),
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: CapacityDisputeId::new(digest_from_u32(index.saturating_add(1))),
                kind: ProviderDisputeKindV1::ProofFailure,
                evidence_digest: digest_from_u32(index.saturating_add(20_000)),
                submitted_at_unix_ms: SOURCE_TIME - 1_000,
                status: ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
                    outcome: CapacityDisputeOutcome::Upheld,
                    resolved_at_unix_ms: SOURCE_TIME,
                    decision_digest: digest_from_u32(index.saturating_add(30_000)),
                    rationale: Some(rationale),
                }),
            }),
        )
        .expect("valid resolved dispute")
    }

    #[test]
    fn canonical_ids_and_policy_authority_validate() {
        let entry = por_entry(1);
        entry.validate().expect("entry validates");
        entry
            .validate_against_policy(&policy())
            .expect("governed authority validates");
        assert_ne!(entry.source_id.as_bytes(), entry.event_id.as_bytes());

        let mut wrong_authority = entry;
        wrong_authority.recorded_by = account(9);
        wrong_authority.event_id = wrong_authority.expected_event_id().expect("event id");
        assert_eq!(
            wrong_authority.validate_against_policy(&policy()),
            Err(ReputationJournalValidationError::RecorderAuthorityMismatch)
        );
    }

    #[test]
    fn source_age_policy_is_strictly_bounded() {
        let mut invalid = policy();
        invalid.max_source_age_ms = 0;
        assert_eq!(
            invalid.validate(),
            Err(ReputationJournalValidationError::InvalidSourceAgeLimit {
                found: 0,
                maximum: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
            })
        );

        invalid.max_source_age_ms = REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1 + 1;
        assert_eq!(
            invalid.validate(),
            Err(ReputationJournalValidationError::InvalidSourceAgeLimit {
                found: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1 + 1,
                maximum: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
            })
        );

        invalid.max_source_age_ms = REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1;
        invalid
            .validate()
            .expect("the hard maximum source-age policy remains valid");
    }

    #[test]
    fn instruction_payloads_have_total_structural_order() {
        fn assert_total_order<T: Ord>() {}

        assert_total_order::<ReputationJournalAuthorityPolicyV1>();
        assert_total_order::<ReputationJournalEntryV1>();

        let first = por_entry(1);
        let second = por_entry(2);
        let ordering = first.cmp(&second);
        assert_ne!(ordering, std::cmp::Ordering::Equal);
        assert_eq!(ordering, second.cmp(&first).reverse());
    }

    #[test]
    fn policy_activation_and_persisted_index_records_fail_closed() {
        let policy = policy();
        let activation = ReputationJournalAuthorityPolicyRecordV1::try_new(
            policy.clone(),
            account(9),
            SOURCE_TIME,
        )
        .expect("canonical policy activation");
        activation.validate().expect("activation validates");

        let entry = por_entry(4);
        assert_eq!(
            entry.source_id,
            ReputationJournalSourceIdV1::for_por_challenge([4; 32])
        );
        let source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::Por,
            source_revision: 1,
            event_id: entry.event_id,
            sequence: 1,
        };
        source_head.validate().expect("source head validates");
        let committed = ReputationJournalCommittedEventRecordV1 {
            sequence: 1,
            target_block_height: 7,
            event_index: 0,
            recorded_at_unix_ms: RECORDED_AT,
            entry,
        };
        committed.validate().expect("committed event validates");

        let mut bad_activation = activation;
        bad_activation.policy_digest[0] ^= 0x01;
        assert_eq!(
            bad_activation.validate(),
            Err(ReputationJournalValidationError::AuthorityPolicyDigestMismatch)
        );
        let mut bad_committed = committed.clone();
        bad_committed.sequence = 0;
        assert_eq!(
            bad_committed.validate(),
            Err(ReputationJournalValidationError::InvalidJournalSequence)
        );

        let mut future_source = committed;
        future_source.recorded_at_unix_ms = SOURCE_TIME - 1;
        assert_eq!(
            future_source.validate(),
            Err(ReputationJournalValidationError::SourceTimeAfterCommit)
        );
    }

    #[test]
    fn zero_and_cross_bound_id_attacks_fail_closed() {
        let base = por_entry(1);
        for expected in [
            ReputationJournalValidationError::ZeroEventId,
            ReputationJournalValidationError::ZeroSourceId,
            ReputationJournalValidationError::ZeroProviderId,
        ] {
            let mut entry = base.clone();
            match expected {
                ReputationJournalValidationError::ZeroEventId => {
                    entry.event_id = ReputationJournalEventIdV1::ZERO;
                }
                ReputationJournalValidationError::ZeroSourceId => {
                    entry.source_id = ReputationJournalSourceIdV1::ZERO;
                }
                ReputationJournalValidationError::ZeroProviderId => {
                    entry.provider_id = ProviderId::new([0; 32]);
                }
                _ => unreachable!(),
            }
            assert_eq!(entry.validate(), Err(expected));
        }

        let mut wrong_source = base;
        wrong_source.source_id = source_id(REPUTATION_JOURNAL_TOKEN_SOURCE_ID_DOMAIN_V1, [1; 32]);
        assert_eq!(
            wrong_source.validate(),
            Err(ReputationJournalValidationError::SourceIdMismatch)
        );
    }

    #[test]
    fn bad_source_commit_and_freshness_timestamps_are_rejected() {
        let mut payload = por_payload(1);
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &mut payload else {
            unreachable!()
        };
        outcome.deadline_at_unix_ms = outcome.issued_at_unix_ms;
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x22; 32]),
                policy().canonical_digest().expect("policy digest"),
                policy().por_recorder_authority,
                SOURCE_TIME,
                None,
                payload,
            ),
            Err(ReputationJournalValidationError::InvalidTimestampOrder {
                earlier: "issued_at_unix_ms",
                later: "deadline_at_unix_ms",
            })
        );

        let mut entry = por_entry(2);
        entry.source_time_unix_ms += 1;
        assert_eq!(
            entry.validate(),
            Err(ReputationJournalValidationError::SourceTimestampMismatch)
        );

        let mut max_timestamp = por_entry(4);
        max_timestamp.source_time_unix_ms = u64::MAX;
        assert_eq!(
            max_timestamp.validate(),
            Err(ReputationJournalValidationError::InvalidTimestamp {
                field: "source_time_unix_ms",
            })
        );

        let entry = por_entry(5);
        let policy = policy();
        entry
            .validate_at_commit(&policy, RECORDED_AT)
            .expect("asynchronous source time remains fresh");
        assert_eq!(
            entry.validate_at_commit(&policy, SOURCE_TIME - 1),
            Err(ReputationJournalValidationError::SourceTimeAfterCommit)
        );
        assert_eq!(
            entry.validate_at_commit(&policy, SOURCE_TIME + policy.max_source_age_ms + 1,),
            Err(ReputationJournalValidationError::SourceObservationStale {
                age_ms: policy.max_source_age_ms + 1,
                maximum_ms: policy.max_source_age_ms,
            })
        );

        let mut before_source = finalized_event(1, 0, 3);
        before_source.recorded_at_unix_ms = SOURCE_TIME - 1;
        assert_eq!(
            page(vec![before_source]).validate(),
            Err(ReputationJournalValidationError::SourceTimeAfterCommit)
        );

        let event = finalized_event(1, 0, 3);
        let mut page = page(vec![event]);
        page.finalized_cursor.finalized_at_unix_ms = RECORDED_AT - 1;
        assert_eq!(
            page.validate(),
            Err(ReputationJournalValidationError::EventAfterFinalizedTimestamp)
        );
        page.finalized_cursor.finalized_at_unix_ms = u64::MAX;
        assert_eq!(
            page.validate(),
            Err(ReputationJournalValidationError::InvalidFinalizedCursor)
        );
    }

    #[test]
    fn impossible_por_counters_and_statuses_are_rejected() {
        let mut payload = por_payload(1);
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &mut payload else {
            unreachable!()
        };
        outcome.failed_samples = 1;
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x22; 32]),
                policy().canonical_digest().expect("policy digest"),
                policy().por_recorder_authority,
                SOURCE_TIME,
                None,
                payload,
            ),
            Err(ReputationJournalValidationError::ImpossiblePorStatus)
        );

        let mut payload = por_payload(2);
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &mut payload else {
            unreachable!()
        };
        outcome.status = PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::DeadlineExpired);
        outcome.failed_samples = outcome.sample_count + 1;
        outcome.responded_at_unix_ms = None;
        outcome.proof_digest = None;
        outcome.repair_task_id = Some([0x77; 32]);
        outcome.verifier_latency_ms = None;
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x22; 32]),
                policy().canonical_digest().expect("policy digest"),
                policy().por_recorder_authority,
                SOURCE_TIME,
                None,
                payload,
            ),
            Err(ReputationJournalValidationError::InvalidPorCounters)
        );
    }

    #[test]
    fn por_attribution_and_repair_feed_separation_are_explicit() {
        let policy = policy();
        let mut failed_payload = por_payload(3);
        let ReputationJournalPayloadV1::PorTerminal(failed) = &mut failed_payload else {
            unreachable!()
        };
        failed.status = PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::DeadlineExpired);
        failed.failed_samples = failed.sample_count;
        failed.responded_at_unix_ms = None;
        failed.proof_digest = None;
        failed.repair_task_id = Some([0x77; 32]);
        failed.verifier_latency_ms = None;
        let failed_entry = ReputationJournalEntryV1::try_new(
            ProviderId::new([0x22; 32]),
            policy.canonical_digest().expect("policy digest"),
            policy.por_recorder_authority.clone(),
            SOURCE_TIME,
            None,
            failed_payload,
        )
        .expect("original failed PoR terminal");
        let ReputationJournalPayloadV1::PorTerminal(failed) = &failed_entry.payload else {
            unreachable!()
        };
        assert!(failed.status.counts_for_provider());
        assert!(failed.status.is_failure());
        assert!(!failed.status.is_success());
        assert_eq!(failed_entry.source_revision, 1);

        let mut forbidden_recovery_revision = failed_entry;
        forbidden_recovery_revision.source_revision = 2;
        forbidden_recovery_revision.predecessor_event_id =
            Some(forbidden_recovery_revision.event_id);
        assert_eq!(
            forbidden_recovery_revision.validate(),
            Err(ReputationJournalValidationError::InvalidSourceRevision),
            "PoR recovery must arrive through the native repair feed, not replace the terminal"
        );

        let mut excluded_payload = por_payload(4);
        let ReputationJournalPayloadV1::PorTerminal(excluded) = &mut excluded_payload else {
            unreachable!()
        };
        excluded.status =
            PorTerminalStatusV1::Excluded(PorTerminalExcludedKindV1::AdmissionRevoked);
        excluded.responded_at_unix_ms = None;
        excluded.proof_digest = None;
        excluded.repair_task_id = None;
        excluded.verifier_latency_ms = None;
        let excluded_entry = ReputationJournalEntryV1::try_new(
            ProviderId::new([0x22; 32]),
            policy.canonical_digest().expect("policy digest"),
            policy.por_recorder_authority.clone(),
            SOURCE_TIME,
            None,
            excluded_payload.clone(),
        )
        .expect("non-attributable terminal");
        let ReputationJournalPayloadV1::PorTerminal(excluded) = &excluded_entry.payload else {
            unreachable!()
        };
        assert!(!excluded.status.counts_for_provider());
        assert!(!excluded.status.is_failure());
        assert!(!excluded.status.is_success());

        let ReputationJournalPayloadV1::PorTerminal(excluded) = &mut excluded_payload else {
            unreachable!()
        };
        excluded.proof_digest = Some([0x99; 32]);
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x22; 32]),
                policy.canonical_digest().expect("policy digest"),
                policy.por_recorder_authority,
                SOURCE_TIME,
                None,
                excluded_payload,
            ),
            Err(ReputationJournalValidationError::ImpossiblePorStatus)
        );
    }

    #[test]
    fn token_status_material_is_exact_and_counter_effect_is_typed() {
        let policy = policy();
        let accepted = StreamTokenValidationOutcomeV1 {
            validation_id: [0x11; 32],
            request_digest: [0x12; 32],
            token_body_digest: Some([0x13; 32]),
            token_key_version: Some(1),
            validated_at_unix_ms: SOURCE_TIME,
            status: StreamTokenValidationStatusV1::Accepted,
        };
        let entry = ReputationJournalEntryV1::try_new(
            ProviderId::new([0x22; 32]),
            policy.canonical_digest().expect("policy digest"),
            policy.token_recorder_authority.clone(),
            SOURCE_TIME,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(accepted),
        )
        .expect("accepted token entry");
        entry
            .validate_against_policy(&policy)
            .expect("token recorder authority");
        assert!(accepted.status.counts_for_provider());
        assert!(!accepted.status.is_violation());
        assert!(
            StreamTokenValidationStatusV1::ProviderViolation(
                StreamTokenViolationKindV1::RequestQuotaExceeded
            )
            .is_violation()
        );
        assert!(
            !StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::InvalidSignature)
                .counts_for_provider()
        );

        let mut malformed = accepted;
        malformed.status =
            StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::MalformedEncoding);
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x22; 32]),
                policy.canonical_digest().expect("policy digest"),
                policy.token_recorder_authority,
                SOURCE_TIME,
                None,
                ReputationJournalPayloadV1::StreamTokenValidation(malformed),
            ),
            Err(ReputationJournalValidationError::TokenMaterialMismatch)
        );
    }

    #[test]
    fn text_and_page_resource_bounds_are_enforced() {
        assert_eq!(
            ReputationJournalEntryV1::try_new(
                ProviderId::new([0x33; 32]),
                policy().canonical_digest().expect("policy digest"),
                policy().dispute_recorder_authority,
                SOURCE_TIME,
                Some(ReputationJournalEventIdV1([0x88; 32])),
                ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                    dispute_id: CapacityDisputeId::new([0x21; 32]),
                    kind: ProviderDisputeKindV1::Other,
                    evidence_digest: [0x22; 32],
                    submitted_at_unix_ms: SOURCE_TIME - 1,
                    status: ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
                        outcome: CapacityDisputeOutcome::Dismissed,
                        resolved_at_unix_ms: SOURCE_TIME,
                        decision_digest: [0x23; 32],
                        rationale: Some("x".repeat(REPUTATION_JOURNAL_MAX_TEXT_BYTES_V1 + 1)),
                    }),
                }),
            ),
            Err(ReputationJournalValidationError::InvalidText)
        );

        let too_many = (0..=REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
            .map(|index| {
                finalized_event(
                    u64::try_from(index + 1).expect("bounded sequence"),
                    u32::try_from(index).expect("bounded event index"),
                    u8::try_from(index + 1).unwrap_or(u8::MAX),
                )
            })
            .collect();
        assert!(matches!(
            page(too_many).validate(),
            Err(ReputationJournalValidationError::TooManyPageItems { .. })
        ));

        let large_events = (0..REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
            .map(|index| ReputationJournalFinalizedEventV1 {
                sequence: u64::try_from(index + 1).expect("bounded sequence"),
                block_height: 5,
                block_hash: BLOCK_HASH,
                event_index: u32::try_from(index).expect("bounded event index"),
                recorded_at_unix_ms: RECORDED_AT,
                entry: resolved_dispute_entry(
                    u32::try_from(index).expect("bounded dispute index"),
                    "x".repeat(REPUTATION_JOURNAL_MAX_TEXT_BYTES_V1),
                ),
            })
            .collect();
        assert!(matches!(
            page(large_events).validate(),
            Err(ReputationJournalValidationError::EncodedPageTooLarge { .. })
        ));
    }

    #[test]
    fn duplicate_reordered_and_gapped_page_events_are_rejected() {
        let first = finalized_event(1, 0, 1);
        let mut duplicate = first.clone();
        duplicate.sequence = 2;
        duplicate.event_index = 1;
        assert_eq!(
            page(vec![first.clone(), duplicate]).validate(),
            Err(ReputationJournalValidationError::DuplicateEventId)
        );

        let mut reordered = finalized_event(1, 1, 2);
        reordered.sequence = 1;
        assert_eq!(
            page(vec![first.clone(), reordered]).validate(),
            Err(ReputationJournalValidationError::EventSequenceReordered)
        );

        let gap = finalized_event(3, 1, 3);
        assert_eq!(
            page(vec![first.clone(), gap]).validate(),
            Err(ReputationJournalValidationError::EventSequenceGap)
        );

        let mut timestamp_reordered = finalized_event(2, 1, 3);
        timestamp_reordered.recorded_at_unix_ms = RECORDED_AT - 1;
        assert_eq!(
            page(vec![first.clone(), timestamp_reordered]).validate(),
            Err(ReputationJournalValidationError::EventTimestampReordered)
        );

        let index_gap = finalized_event(2, 2, 4);
        assert_eq!(
            page(vec![first.clone(), index_gap]).validate(),
            Err(ReputationJournalValidationError::BlockEventReordered)
        );

        let mut next_block_without_reset = finalized_event(2, 1, 5);
        next_block_without_reset.block_height = 6;
        next_block_without_reset.block_hash = [0xC1; 32];
        assert_eq!(
            page(vec![first, next_block_without_reset]).validate(),
            Err(ReputationJournalValidationError::BlockEventReordered)
        );
    }

    #[test]
    fn continuation_and_requested_cursor_mismatches_are_rejected() {
        let event = finalized_event(10, 4, 1);
        let mut continuing = page(vec![event.clone()]);
        continuing.has_more = true;
        continuing.next_after = Some(ReputationJournalFinalizedEventCursorV1 {
            sequence: 9,
            ..event.cursor()
        });
        assert_eq!(
            continuing.validate(),
            Err(ReputationJournalValidationError::ContinuationCursorMismatch)
        );

        let terminal = page(vec![event]);
        assert_eq!(
            terminal.validate_after(Some(ReputationJournalFinalizedEventCursorV1 {
                sequence: 7,
                block_height: 5,
                block_hash: BLOCK_HASH,
                event_index: 3,
            })),
            Err(ReputationJournalValidationError::RequestedCursorMismatch)
        );
    }

    #[test]
    fn semantic_mutation_and_same_source_revision_are_equivocation() {
        let mut mutated = por_entry(1);
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &mut mutated.payload else {
            unreachable!()
        };
        outcome.verifier_latency_ms = Some(8);
        assert_eq!(
            mutated.validate(),
            Err(ReputationJournalValidationError::EventIdMismatch)
        );

        let first = por_entry(2);
        let mut payload = first.payload.clone();
        let ReputationJournalPayloadV1::PorTerminal(outcome) = &mut payload else {
            unreachable!()
        };
        outcome.verifier_latency_ms = Some(9);
        let second_entry = ReputationJournalEntryV1::try_new(
            first.provider_id,
            first.authority_policy_digest,
            first.recorded_by.clone(),
            first.source_time_unix_ms,
            None,
            payload,
        )
        .expect("individually valid conflicting entry");
        let events = vec![
            ReputationJournalFinalizedEventV1 {
                sequence: 1,
                block_height: 5,
                block_hash: BLOCK_HASH,
                event_index: 0,
                recorded_at_unix_ms: RECORDED_AT,
                entry: first,
            },
            ReputationJournalFinalizedEventV1 {
                sequence: 2,
                block_height: 5,
                block_hash: BLOCK_HASH,
                event_index: 1,
                recorded_at_unix_ms: RECORDED_AT,
                entry: second_entry,
            },
        ];
        assert_eq!(
            page(events).validate(),
            Err(ReputationJournalValidationError::SourceEquivocation)
        );
    }

    #[test]
    fn dispute_revision_requires_exact_predecessor() {
        let opened_policy = policy();
        let opened = ReputationJournalEntryV1::try_new(
            ProviderId::new([0x33; 32]),
            opened_policy.canonical_digest().expect("policy digest"),
            opened_policy.dispute_recorder_authority.clone(),
            SOURCE_TIME - 1_000,
            None,
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: CapacityDisputeId::new([0x61; 32]),
                kind: ProviderDisputeKindV1::ProofFailure,
                evidence_digest: [0x62; 32],
                submitted_at_unix_ms: SOURCE_TIME - 1_000,
                status: ProviderDisputeStatusV1::Opened,
            }),
        )
        .expect("open dispute");
        let resolved = ReputationJournalEntryV1::try_new(
            opened.provider_id,
            opened.authority_policy_digest,
            opened.recorded_by.clone(),
            SOURCE_TIME,
            Some(opened.event_id),
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: CapacityDisputeId::new([0x61; 32]),
                kind: ProviderDisputeKindV1::ProofFailure,
                evidence_digest: [0x62; 32],
                submitted_at_unix_ms: SOURCE_TIME - 1_000,
                status: ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
                    outcome: CapacityDisputeOutcome::Upheld,
                    resolved_at_unix_ms: SOURCE_TIME,
                    decision_digest: [0x63; 32],
                    rationale: Some("proof failure upheld".to_owned()),
                }),
            }),
        )
        .expect("resolve dispute");
        let events = vec![
            ReputationJournalFinalizedEventV1 {
                sequence: 1,
                block_height: 4,
                block_hash: [0x40; 32],
                event_index: 9,
                recorded_at_unix_ms: SOURCE_TIME - 500,
                entry: opened,
            },
            ReputationJournalFinalizedEventV1 {
                sequence: 2,
                block_height: 5,
                block_hash: BLOCK_HASH,
                event_index: 0,
                recorded_at_unix_ms: RECORDED_AT,
                entry: resolved,
            },
        ];
        page(events)
            .validate()
            .expect("predecessor-contiguous dispute lifecycle");
    }

    #[test]
    fn canonical_norito_and_json_round_trip() {
        let value = page(vec![finalized_event(1, 0, 1)]);
        let bytes = norito::encode_canonical(&value).expect("encode canonical journal page");
        #[cfg(feature = "json")]
        let canonical_json = norito::json::to_string(&value).expect("encode journal JSON");
        let expected_event_id = value.events[0].entry.event_id;
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            value
                .validate()
                .expect("page size gate ignores ambient layout");
            assert_eq!(
                value.events[0]
                    .entry
                    .expected_event_id()
                    .expect("derive event id under alternate ambient layout"),
                expected_event_id
            );
            assert_eq!(
                norito::encode_canonical(&value)
                    .expect("encode page under alternate ambient layout"),
                bytes
            );
            #[cfg(feature = "json")]
            assert_eq!(
                norito::json::to_string(&value).expect("encode ambient-independent journal JSON"),
                canonical_json
            );
        }
        let decoded: ReputationJournalFinalizedEventPageV1 =
            norito::decode_canonical(&bytes).expect("decode canonical journal page");
        assert_eq!(decoded, value);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode canonical journal page"),
            bytes
        );
        let alternate = encode_with_alternate_norito_layout(&value);
        let alternate_decoded: ReputationJournalFinalizedEventPageV1 =
            norito::decode_from_bytes(&alternate)
                .expect("alternate-layout journal page remains structurally decodable");
        assert_eq!(alternate_decoded, value);
        assert!(matches!(
            norito::decode_canonical::<ReputationJournalFinalizedEventPageV1>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));

        #[cfg(feature = "json")]
        {
            let decoded: ReputationJournalFinalizedEventPageV1 =
                norito::json::from_slice(canonical_json.as_bytes()).expect("decode journal JSON");
            assert_eq!(decoded, value);
        }

        let persisted = ReputationJournalCommittedEventRecordV1 {
            sequence: 1,
            target_block_height: 5,
            event_index: 0,
            recorded_at_unix_ms: RECORDED_AT,
            entry: value.events[0].entry.clone(),
        };
        persisted
            .validate()
            .expect("valid persisted journal record");
        let persisted_bytes =
            norito::encode_canonical(&persisted).expect("encode persisted journal record");
        let persisted_decoded: ReputationJournalCommittedEventRecordV1 =
            norito::decode_canonical(&persisted_bytes).expect("decode persisted journal record");
        assert_eq!(persisted_decoded, persisted);
        let persisted_alternate = encode_with_alternate_norito_layout(&persisted);
        assert!(matches!(
            norito::decode_canonical::<ReputationJournalCommittedEventRecordV1>(
                &persisted_alternate
            ),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn provider_dispute_resolution_norito_and_json_round_trip() {
        let value = ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
            outcome: CapacityDisputeOutcome::Upheld,
            resolved_at_unix_ms: SOURCE_TIME,
            decision_digest: [0xD4; 32],
            rationale: Some("governance upheld the dispute".to_owned()),
        });
        let bytes = norito::encode_canonical(&value).expect("encode provider-dispute resolution");
        let decoded: ProviderDisputeStatusV1 =
            norito::decode_canonical(&bytes).expect("decode provider-dispute resolution");
        assert_eq!(decoded, value);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode provider-dispute resolution"),
            bytes
        );
        let alternate = encode_with_alternate_norito_layout(&value);
        let alternate_decoded: ProviderDisputeStatusV1 = norito::decode_from_bytes(&alternate)
            .expect("alternate-layout dispute resolution remains structurally decodable");
        assert_eq!(alternate_decoded, value);
        assert!(matches!(
            norito::decode_canonical::<ProviderDisputeStatusV1>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));

        #[cfg(feature = "json")]
        {
            let json =
                norito::json::to_string(&value).expect("encode provider-dispute resolution JSON");
            let decoded: ProviderDisputeStatusV1 = norito::json::from_slice(json.as_bytes())
                .expect("decode provider-dispute resolution JSON");
            assert_eq!(decoded, value);
        }
    }
}
