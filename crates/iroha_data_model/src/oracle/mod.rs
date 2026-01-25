//! Oracle layer data model and scheduling helpers.
//!
//! This module defines the schemas and deterministic helpers required by the
//! validator-operated oracle layer (see `docs/source/soracles.md`). The data
//! model covers feed configuration, signed observations, aggregated reports,
//! and replay protection/gossip keys used to distribute oracle messages across
//! validators.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::{NonZeroU64, NonZeroUsize},
    str::FromStr,
};

use iroha_crypto::{Hash, HashOf, SignatureOf};
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, error::ParseError, name::Name, nexus::UniversalAccountId};

/// Oracle identifier (validator-bound oracle signing key).
pub type OracleId = AccountId;
/// Slot index used by oracle feeds.
pub type FeedSlot = u64;

/// Identifier for an oracle feed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct FeedId(pub Name);

impl FeedId {
    /// Borrow the feed identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for FeedId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for FeedId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Name::from_str(s)?))
    }
}

/// Version number for a feed configuration (monotonic per feed).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct FeedConfigVersion(pub u32);

impl From<u32> for FeedConfigVersion {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// Risk classification attached to a feed; drives quorum/dispute policy.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "risk_class", content = "value"))]
pub enum RiskClass {
    /// Low-risk feeds (reduced quorum, lighter dispute windows).
    Low,
    /// Medium-risk feeds (standard quorum, dispute windows).
    #[default]
    Medium,
    /// High-risk feeds (elevated quorum, extended dispute windows).
    High,
}

/// Change classification for oracle governance proposals.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "change_class", content = "value"))]
pub enum OracleChangeClass {
    /// Low-impact change (routine rotations, cosmetic manifest updates).
    Low,
    /// Medium-impact change (provider set updates, cadence adjustments).
    #[default]
    Medium,
    /// High-impact change (risk policy shifts, schema/signature changes).
    High,
}

impl OracleChangeClass {
    /// Minimum approvals required for the class using the provided thresholds.
    #[must_use]
    pub fn required_votes(
        self,
        low: NonZeroUsize,
        medium: NonZeroUsize,
        high: NonZeroUsize,
    ) -> NonZeroUsize {
        match self {
            Self::Low => low,
            Self::Medium => medium,
            Self::High => high,
        }
    }
}

/// Pipeline stage for oracle change governance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "oracle_change_stage", content = "value")
)]
pub enum OracleChangeStage {
    /// Intake stage used to admit the proposal.
    Intake,
    /// Rules Committee review and normalization.
    RulesCommittee,
    /// COP review with elevated quorum for high-risk changes.
    CopReview,
    /// Technical audit of connector/manifest and risk deltas.
    TechnicalAudit,
    /// Policy Jury approval prior to enactment.
    PolicyJury,
    /// Enactment stage that applies the change.
    Enactment,
}

impl OracleChangeStage {
    /// Deterministic ordering index.
    #[must_use]
    pub const fn order(self) -> usize {
        match self {
            Self::Intake => 0,
            Self::RulesCommittee => 1,
            Self::CopReview => 2,
            Self::TechnicalAudit => 3,
            Self::PolicyJury => 4,
            Self::Enactment => 5,
        }
    }

    /// Next stage in the pipeline, or `None` when this is the terminal stage.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Intake => Some(Self::RulesCommittee),
            Self::RulesCommittee => Some(Self::CopReview),
            Self::CopReview => Some(Self::TechnicalAudit),
            Self::TechnicalAudit => Some(Self::PolicyJury),
            Self::PolicyJury => Some(Self::Enactment),
            Self::Enactment => None,
        }
    }
}

/// Failure modes for a stage.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "oracle_change_failure", content = "value")
)]
pub enum OracleChangeStageFailure {
    /// The stage missed its deadline.
    DeadlineMissed,
    /// Stage votes explicitly rejected the proposal.
    Rejected,
    /// Evidence requirements were not met for the stage.
    EvidenceMissing,
    /// Explicit rollback with a human-readable reason.
    Rollback(String),
}

/// Failure details captured when a proposal cannot complete.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeFailure {
    /// Stage that triggered the failure.
    pub stage: OracleChangeStage,
    /// Reason for the failure.
    pub reason: OracleChangeStageFailure,
    /// Block height when the failure was recorded.
    pub at: u64,
}

/// Lifecycle status for an oracle change proposal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "oracle_change_status", content = "value")
)]
pub enum OracleChangeStatus {
    /// Proposal is still progressing through the pipeline.
    Pending,
    /// Proposal completed enactment.
    Enacted(u64),
    /// Proposal rolled back or failed at a specific stage.
    Failed(OracleChangeFailure),
}

impl OracleChangeStatus {
    /// Returns `true` when the change reached a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Pending)
    }
}

/// Identifier for oracle change proposals (hash).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeId(pub Hash);

impl OracleChangeId {
    /// Borrow the underlying hash bytes.
    #[must_use]
    pub fn as_hash(&self) -> &Hash {
        &self.0
    }
}

impl From<Hash> for OracleChangeId {
    fn from(value: Hash) -> Self {
        Self(value)
    }
}

/// Evidence pointer attached to a pipeline stage (e.g., `SoraFS` bundle hash).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeEvidence {
    /// Stage the evidence applies to.
    pub stage: OracleChangeStage,
    /// Hash of the evidence packet or `SoraFS` artefact.
    pub evidence_hash: Hash,
    /// Optional free-form note describing the artefact contents.
    #[norito(default)]
    pub note: Option<String>,
}

/// Per-stage record for oracle change governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeStageRecord {
    /// Stage identifier.
    pub stage: OracleChangeStage,
    /// Accounts that approved this stage.
    #[norito(default)]
    pub approvals: BTreeSet<AccountId>,
    /// Accounts that rejected this stage.
    #[norito(default)]
    pub rejections: BTreeSet<AccountId>,
    /// Evidence hashes recorded for this stage.
    #[norito(default)]
    pub evidence: Vec<Hash>,
    /// Block height when the stage started (if any).
    pub started_at: Option<u64>,
    /// Deadline height after which the stage should be rolled back.
    pub deadline: Option<u64>,
    /// Block height when the stage completed successfully.
    pub completed_at: Option<u64>,
    /// Failure reason when the stage rolled back or was rejected.
    pub failure: Option<OracleChangeStageFailure>,
}

impl OracleChangeStageRecord {
    /// Returns `true` when the stage has not completed or failed.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.completed_at.is_none() && self.failure.is_none()
    }
}

/// Governance proposal for an oracle feed change.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeProposal {
    /// Unique identifier for the change (hash).
    pub id: OracleChangeId,
    /// Feed configuration that will be enacted after approval.
    pub feed: FeedConfig,
    /// Risk classification for the proposal pipeline.
    pub class: OracleChangeClass,
    /// Hash of the change manifest or `SoraFS` artefact.
    pub payload_hash: Hash,
    /// Account that submitted the change.
    pub proposer: AccountId,
    /// Block height when the change was proposed.
    pub created_at: u64,
    /// Evidence packets attached to the proposal.
    #[norito(default)]
    pub evidence: Vec<OracleChangeEvidence>,
    /// Pipeline stages and approvals.
    #[norito(default)]
    pub stages: Vec<OracleChangeStageRecord>,
    /// Current lifecycle status for the proposal.
    pub status: OracleChangeStatus,
}

impl OracleChangeProposal {
    /// Returns the current active stage (pending or failed) if present.
    #[must_use]
    pub fn current_stage(&self) -> Option<&OracleChangeStageRecord> {
        self.stages
            .iter()
            .find(|stage| stage.is_pending() || stage.failure.is_some())
    }

    /// Returns the mutable current active stage (pending or failed) if present.
    pub fn current_stage_mut(&mut self) -> Option<&mut OracleChangeStageRecord> {
        self.stages
            .iter_mut()
            .find(|stage| stage.is_pending() || stage.failure.is_some())
    }
}

/// Deterministic aggregation rule used to combine observations into a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "aggregation_rule", content = "value"))]
pub enum AggregationRule {
    /// Median with a k-MAD (scaled by 100 for integer math) outlier threshold.
    MedianMad(u16),
    /// Percentile aggregation (basis points: `10_000` = 100%).
    Percentile(u16),
}

impl Default for AggregationRule {
    fn default() -> Self {
        Self::MedianMad(300)
    }
}

/// Outlier detection policy applied to observations before aggregation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[allow(variant_size_differences)]
#[cfg_attr(feature = "json", norito(tag = "outlier_policy", content = "value"))]
pub enum OutlierPolicy {
    /// k-MAD outlier detection (scaled by 100 for integer math).
    Mad(u16),
    /// Absolute deviation measured against the median (scaled fixed-point value).
    Absolute(AbsoluteOutlier),
}

/// Absolute delta bound used by outlier detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AbsoluteOutlier {
    /// Maximum absolute deviation allowed (mantissa aligned with feed value scale).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::i128_string"))]
    pub max_delta: i128,
}

impl Default for OutlierPolicy {
    fn default() -> Self {
        Self::Mad(350)
    }
}

/// Feed configuration registered on-chain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FeedConfig {
    /// Unique feed identifier.
    pub feed_id: FeedId,
    /// Monotonic configuration version for this feed.
    pub feed_config_version: FeedConfigVersion,
    /// Oracle providers eligible for committee draws.
    pub providers: Vec<OracleId>,
    /// Connector identifier used by fetchers (off-chain, versioned).
    pub connector_id: String,
    /// Connector version number pinned for this feed.
    pub connector_version: u32,
    /// Slot cadence; feed is active when `slot % cadence_slots == 0`.
    pub cadence_slots: NonZeroU64,
    /// Aggregation rule applied to observations.
    pub aggregation: AggregationRule,
    /// Outlier policy enforced before aggregation.
    pub outlier_policy: OutlierPolicy,
    /// Minimum signatures required on a report (must exceed byzantine bound).
    pub min_signers: u16,
    /// Committee size for the feed.
    pub committee_size: u16,
    /// Risk classification (drives governance/dispute policy).
    pub risk_class: RiskClass,
    /// Maximum observations accepted per slot.
    pub max_observers: u16,
    /// Maximum encoded value length accepted (bytes).
    pub max_value_len: u16,
    /// Maximum tolerated error rate (basis points) before slashing.
    pub max_error_rate_bps: u16,
    /// Dispute window length expressed in slots.
    pub dispute_window_slots: NonZeroU64,
    /// Replay window length expressed in slots.
    pub replay_window_slots: NonZeroU64,
}

impl FeedConfig {
    /// Returns `true` when the feed is active for the given slot (`slot % cadence == 0`).
    #[must_use]
    pub fn is_active_slot(&self, slot: FeedSlot) -> bool {
        slot.is_multiple_of(self.cadence_slots.get())
    }

    /// Enforce the cadence guard, returning an error when the slot is inactive.
    ///
    /// # Errors
    /// Returns [`OracleModelError::InactiveSlot`] when the slot does not align with the feed cadence.
    pub fn ensure_active_slot(&self, slot: FeedSlot) -> Result<(), OracleModelError> {
        if self.is_active_slot(slot) {
            Ok(())
        } else {
            Err(OracleModelError::InactiveSlot {
                slot,
                cadence: self.cadence_slots.get(),
            })
        }
    }

    /// Validate that an observation references the pinned connector id/version.
    ///
    /// # Errors
    /// Returns [`OracleModelError::ConnectorMismatch`] when the connector id or version differs from the feed config.
    pub fn validate_connector_pin(
        &self,
        observation: &ObservationBody,
    ) -> Result<(), OracleModelError> {
        if observation.connector_id != self.connector_id
            || observation.connector_version != self.connector_version
        {
            return Err(OracleModelError::ConnectorMismatch {
                expected_id: self.connector_id.clone(),
                expected_version: self.connector_version,
                provided_id: observation.connector_id.clone(),
                provided_version: observation.connector_version,
            });
        }

        Ok(())
    }

    /// Validate feed id/version, cadence, and connector pinning for an observation.
    ///
    /// # Errors
    /// Returns a [`OracleModelError`] when feed identifiers, versions, cadence, or connector pins do not match.
    pub fn validate_observation_meta(
        &self,
        observation: &ObservationBody,
    ) -> Result<(), OracleModelError> {
        if observation.feed_id != self.feed_id {
            return Err(OracleModelError::FeedMismatch {
                expected: self.feed_id.clone(),
                provided: observation.feed_id.clone(),
            });
        }

        if observation.feed_config_version != self.feed_config_version {
            return Err(OracleModelError::FeedVersionMismatch {
                expected: self.feed_config_version,
                provided: observation.feed_config_version,
            });
        }

        self.ensure_active_slot(observation.slot)?;
        self.validate_connector_pin(observation)
    }
}

/// HTTP-style method used by connector requests.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "method", content = "value"))]
pub enum ConnectorRequestMethod {
    /// HTTP GET (no request body).
    #[default]
    Get,
    /// HTTP POST (request body present).
    Post,
    /// HTTP PUT (request body present).
    Put,
    /// HTTP DELETE.
    Delete,
}

/// Redacted header value; secrets are hashed to keep them off-chain.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum RedactedHeaderValue {
    /// Plain header value (safe to expose).
    Plain(String),
    /// Hashed header value (secret redacted).
    Hashed(Hash),
}

impl RedactedHeaderValue {
    /// Create a hashed header value from a sensitive secret without exposing it.
    #[must_use]
    pub fn from_secret(secret: impl AsRef<[u8]>) -> Self {
        Self::Hashed(Hash::new(secret))
    }
}

/// Canonical connector request; hashed to derive `request_hash`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ConnectorRequest {
    /// Target feed identifier.
    pub feed_id: FeedId,
    /// Configuration version bound to this request.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index for the request.
    pub slot: FeedSlot,
    /// Connector identifier.
    pub connector_id: String,
    /// Connector version.
    pub connector_version: u32,
    /// HTTP-like method.
    #[cfg_attr(feature = "json", norito(default))]
    pub method: ConnectorRequestMethod,
    /// Endpoint path used by the connector (e.g., `/price`).
    pub endpoint: String,
    /// Sorted query parameters (deterministic hashing).
    #[cfg_attr(feature = "json", norito(default))]
    pub query: BTreeMap<String, String>,
    /// Redacted headers (sensitive values should be hashed).
    #[cfg_attr(feature = "json", norito(default))]
    pub headers: BTreeMap<String, RedactedHeaderValue>,
    /// Hash of the request body (keeps secrets off-chain).
    pub body_hash: Hash,
}

impl ConnectorRequest {
    /// Compute the canonical hash for this connector request.
    #[must_use]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }

    /// Convenience helper that returns the untyped request hash.
    #[must_use]
    pub fn request_hash(&self) -> Hash {
        self.hash().into()
    }

    /// Validate that sensitive headers have been redacted (hashed).
    ///
    /// # Errors
    /// Returns [`OracleModelError::UnredactedSensitiveHeader`] if a sensitive header value remains plaintext.
    pub fn validate_redaction(&self) -> Result<(), OracleModelError> {
        for (name, value) in &self.headers {
            let lower = name.to_ascii_lowercase();
            if is_sensitive_header(&lower) && matches!(value, RedactedHeaderValue::Plain(_)) {
                return Err(OracleModelError::UnredactedSensitiveHeader {
                    header: name.clone(),
                });
            }
        }
        Ok(())
    }
}

fn is_sensitive_header(name: &str) -> bool {
    const SENSITIVE_PREFIXES: &[&str] = &["authorization", "proxy-authorization", "x-api", "api-"];
    const SENSITIVE_EXACT: &[&str] = &[
        "cookie",
        "set-cookie",
        "x-auth-token",
        "x-access-token",
        "bearer",
        "token",
        "secret",
    ];

    SENSITIVE_EXACT.contains(&name)
        || SENSITIVE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Connector response envelope with payload hash for audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ConnectorResponse {
    /// HTTP-style status code returned by the connector.
    pub status: u16,
    /// Hash of the payload (body) returned by the connector.
    pub payload_hash: Hash,
    /// Optional connector error classification.
    #[norito(default)]
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub error_code: Option<ObservationErrorCode>,
}

impl ConnectorResponse {
    /// Canonical hash of the connector response.
    #[must_use]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }
}

/// Keyed hash/HMAC used for PII-safe identifiers (e.g., social IDs).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct KeyedHash {
    /// Identifier for the pepper/secret used to compute the hash.
    pub pepper_id: String,
    /// Hashed identifier (HMAC-like).
    pub digest: Hash,
}

impl KeyedHash {
    /// Compute a keyed hash using the provided pepper and payload.
    #[must_use]
    pub fn new(
        pepper_id: impl Into<String>,
        pepper: impl AsRef<[u8]>,
        payload: impl AsRef<[u8]>,
    ) -> Self {
        let mut data = Vec::with_capacity(pepper.as_ref().len() + payload.as_ref().len());
        data.extend_from_slice(pepper.as_ref());
        data.extend_from_slice(payload.as_ref());
        Self {
            pepper_id: pepper_id.into(),
            digest: Hash::new(data),
        }
    }

    /// Verify that `candidate` matches this keyed hash under the supplied pepper.
    #[must_use]
    pub fn verify(&self, pepper: impl AsRef<[u8]>, candidate: impl AsRef<[u8]>) -> bool {
        let expected = Self::new(self.pepper_id.clone(), pepper, candidate);
        expected.digest == self.digest
    }
}

/// Identifier of the canonical Twitter follow binding feed.
pub const TWITTER_FOLLOW_FEED_ID: &str = "twitter_follow_binding";

/// Status recorded for a Twitter follow binding attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum TwitterBindingStatus {
    /// Connector verified a follow for the target handle.
    Following,
    /// Connector could not verify the challenge tweet or follow.
    ChallengeMissing,
    /// Connector explicitly denied or rejected the follow binding.
    Denied,
}

/// Attestation produced by the twitter follow oracle feed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TwitterBindingAttestation {
    /// Pseudonymous keyed hash derived from the `twitter_user_id` and pepper.
    pub binding_hash: KeyedHash,
    /// UAID bound to the twitter user.
    pub uaid: UniversalAccountId,
    /// Reported follow status for the attestation.
    pub status: TwitterBindingStatus,
    /// Optional tweet identifier used for the follow challenge.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub tweet_id: Option<String>,
    /// Hash of the challenge payload (optional).
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub challenge_hash: Option<Hash>,
    /// Absolute expiry timestamp in milliseconds since Unix epoch.
    pub expires_at_ms: u64,
    /// Timestamp (milliseconds) when the connector observed the follow state.
    pub observed_at_ms: u64,
    /// Canonical connector request hash for the attestation.
    pub request_hash: Hash,
    /// Slot the attestation corresponds to.
    pub slot: FeedSlot,
    /// Feed configuration version used by the committee.
    pub feed_config_version: FeedConfigVersion,
}

impl TwitterBindingAttestation {
    /// Observation value derived from the keyed hash (used to match aggregated feed outputs).
    #[must_use]
    pub fn observation_value(&self) -> ObservationValue {
        ObservationValue::from_keyed_hash(&self.binding_hash)
    }

    /// Whether the attestation has expired at the given timestamp.
    #[must_use]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

/// Persisted record for a twitter follow binding attestation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TwitterBindingRecord {
    /// Feed identifier that produced the attestation.
    pub feed_id: FeedId,
    /// Oracle provider that submitted the attestation.
    pub provider: OracleId,
    /// Signed attestation payload.
    pub attestation: TwitterBindingAttestation,
    /// Timestamp (milliseconds) when the record was persisted.
    pub recorded_at_ms: u64,
}

impl TwitterBindingRecord {
    /// Digest used as the binding map key.
    #[must_use]
    pub fn binding_digest(&self) -> Hash {
        self.attestation.binding_hash.digest
    }
}

/// Signed observation outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[allow(variant_size_differences)]
pub enum ObservationOutcome {
    /// Successful value (fixed-point).
    Value(ObservationValue),
    /// Error reported by the connector.
    Error(ObservationErrorCode),
}

impl ObservationOutcome {
    /// Returns `true` when this outcome represents a successful value.
    #[must_use]
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(_))
    }
}

/// Fixed-point observation value (mantissa + scale).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ObservationValue {
    /// Signed mantissa.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::i128_string"))]
    pub mantissa: i128,
    /// Decimal scale (number of fractional digits).
    pub scale: u32,
}

impl ObservationValue {
    /// Construct a new fixed-point observation value.
    #[must_use]
    pub fn new(mantissa: i128, scale: u32) -> Self {
        Self { mantissa, scale }
    }

    /// Derive a deterministic fixed-point value from a hash digest (scale = 0).
    ///
    /// This helper is intended for hashed identifiers such as UAIDs or keyed
    /// social IDs so oracle feeds can agree on an encoded value without leaking
    /// the underlying PII. The top bit is cleared to keep the mantissa
    /// non-negative.
    #[must_use]
    pub fn from_hash(hash: &Hash) -> Self {
        let mut mantissa_bytes = [0_u8; 16];
        mantissa_bytes.copy_from_slice(&hash.as_ref()[..16]);
        mantissa_bytes[15] &= 0x7F;
        Self::new(i128::from_le_bytes(mantissa_bytes), 0)
    }

    /// Derive a deterministic fixed-point value from a keyed hash digest.
    #[must_use]
    pub fn from_keyed_hash(keyed_hash: &KeyedHash) -> Self {
        Self::from_hash(&keyed_hash.digest)
    }

    /// Derive a deterministic fixed-point value from a UAID hash.
    #[must_use]
    pub fn from_uaid(uaid: &UniversalAccountId) -> Self {
        Self::from_hash(uaid.as_hash())
    }
}

/// Connector error codes surfaced by observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "error_code", content = "value"))]
pub enum ObservationErrorCode {
    /// The upstream resource could not be fetched.
    ResourceUnavailable,
    /// Authentication or credentials failed.
    AuthFailed,
    /// Request exceeded the allowed time budget.
    Timeout,
    /// Connector returned no payload or could not parse one.
    Missing,
    /// Custom error code (allows vendor-specific mapping).
    Other(u16),
}

/// Classification of connector errors for slashing/reward semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "fault_class", content = "value"))]
pub enum ObservationErrorClass {
    /// Honest or transient failures (network, timeout).
    Honest,
    /// Misconfiguration or invalid credentials.
    Misconfigured,
    /// Connector returned no data.
    Missing,
}

impl ObservationErrorCode {
    /// Map error codes into fault classes for slashing/reward policy.
    #[must_use]
    pub fn classification(&self) -> ObservationErrorClass {
        match self {
            Self::ResourceUnavailable | Self::Timeout | Self::Other(_) => {
                ObservationErrorClass::Honest
            }
            Self::AuthFailed => ObservationErrorClass::Misconfigured,
            Self::Missing => ObservationErrorClass::Missing,
        }
    }
}

/// Observation payload signed by an oracle.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ObservationBody {
    /// Target feed identifier.
    pub feed_id: FeedId,
    /// Configuration version expected by the submitter.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index for the observation.
    pub slot: FeedSlot,
    /// Oracle identifier (provider signing key).
    pub provider_id: OracleId,
    /// Connector identifier used to fetch this observation.
    pub connector_id: String,
    /// Connector version used to fetch this observation.
    pub connector_version: u32,
    /// Canonical hash of the upstream request.
    pub request_hash: Hash,
    /// Reported outcome.
    pub outcome: ObservationOutcome,
    /// Optional timestamp (audit-only; not used in aggregation).
    #[norito(default)]
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub timestamp_ms: Option<u64>,
}

impl ObservationBody {
    /// Compute the canonical digest of this observation body.
    #[must_use]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }
}

/// Signed observation wrapper.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Observation {
    /// Observation payload.
    pub body: ObservationBody,
    /// Oracle signature over the payload.
    pub signature: SignatureOf<ObservationBody>,
}

impl Observation {
    /// Typed digest of the observation payload.
    #[must_use]
    pub fn hash(&self) -> HashOf<ObservationBody> {
        self.body.hash()
    }

    /// Gossip key scoped to `(feed_id, feed_config_version, slot)`.
    #[must_use]
    pub fn gossip_key(&self) -> GossipKey {
        GossipKey::new(
            self.body.feed_id.clone(),
            self.body.feed_config_version,
            self.body.slot,
        )
    }

    /// Replay key scoped to `(feed_id, feed_config_version, slot, request_hash)`.
    #[must_use]
    pub fn replay_key(&self) -> ReplayKey {
        ReplayKey::new(
            self.body.feed_id.clone(),
            self.body.feed_config_version,
            self.body.slot,
            self.body.request_hash,
        )
    }
}

/// Aggregated entry inside a report.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReportEntry {
    /// Oracle identifier (committee member).
    pub oracle_id: OracleId,
    /// Digest of the signed observation payload.
    pub observation_hash: HashOf<ObservationBody>,
    /// Value used for aggregation (fixed-point).
    pub value: ObservationValue,
    /// Whether the observation was classified as an outlier.
    pub outlier: bool,
}

/// Report payload signed by the submitter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReportBody {
    /// Target feed identifier.
    pub feed_id: FeedId,
    /// Configuration version used for aggregation.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index covered by this report.
    pub slot: FeedSlot,
    /// Canonical request hash shared across the committee.
    pub request_hash: Hash,
    /// Sorted aggregated entries (by `oracle_id`).
    pub entries: Vec<ReportEntry>,
    /// Submitter (committee member) that built the report.
    pub submitter: OracleId,
}

impl ReportBody {
    /// Compute the canonical digest of this report body.
    #[must_use]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }
}

/// Signed oracle report.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Report {
    /// Report payload.
    pub body: ReportBody,
    /// Signature over the report payload by the submitter.
    pub signature: SignatureOf<ReportBody>,
}

impl Report {
    /// Typed digest of the report payload.
    #[must_use]
    pub fn hash(&self) -> HashOf<ReportBody> {
        self.body.hash()
    }

    /// Gossip key scoped to `(feed_id, feed_config_version, slot)`.
    #[must_use]
    pub fn gossip_key(&self) -> GossipKey {
        GossipKey::new(
            self.body.feed_id.clone(),
            self.body.feed_config_version,
            self.body.slot,
        )
    }

    /// Replay key scoped to `(feed_id, feed_config_version, slot, request_hash)`.
    #[must_use]
    pub fn replay_key(&self) -> ReplayKey {
        ReplayKey::new(
            self.body.feed_id.clone(),
            self.body.feed_config_version,
            self.body.slot,
            self.body.request_hash,
        )
    }
}

/// Aggregation success payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FeedSuccess {
    /// Aggregated value.
    pub value: ObservationValue,
    /// Entries that contributed to the final value.
    pub entries: Vec<ReportEntry>,
}

/// Aggregation error payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FeedError {
    /// Error code shared by observations.
    pub code: ObservationErrorCode,
}

/// Outcome persisted in the ledger after aggregation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "result", content = "detail"))]
pub enum FeedEventOutcome {
    /// Aggregation succeeded and produced a consensus value.
    Success(FeedSuccess),
    /// All observations failed with the same error code.
    Error(FeedError),
    /// No valid observations were received within the slot window.
    Missing,
}

/// Event emitted after processing a feed slot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FeedEvent {
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Configuration version used for the event.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index.
    pub slot: FeedSlot,
    /// Aggregation outcome.
    pub outcome: FeedEventOutcome,
}

/// Classification for oracle penalties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "detail"))]
pub enum OraclePenaltyKind {
    /// Observation was marked as an outlier during aggregation.
    Outlier,
    /// Observation reported an explicit error outcome.
    Error,
    /// Provider failed to submit an observation for the slot.
    NoShow,
    /// Observation failed admission (e.g., signature/format errors).
    BadSignature,
    /// Penalty applied as part of a dispute resolution.
    Dispute,
}

/// Penalty record emitted when a provider is slashed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OraclePenalty {
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Configuration version used for the event.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index.
    pub slot: FeedSlot,
    /// Canonical request hash for the slot.
    pub request_hash: Hash,
    /// Provider that incurred the penalty.
    pub oracle_id: OracleId,
    /// Kind of penalty that was applied.
    pub kind: OraclePenaltyKind,
    /// Amount slashed from the provider.
    pub amount: Numeric,
}

/// Reward record emitted when a provider is paid for an inlier observation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleReward {
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Configuration version used for the event.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index.
    pub slot: FeedSlot,
    /// Canonical request hash for the slot.
    pub request_hash: Hash,
    /// Provider that earned the reward.
    pub oracle_id: OracleId,
    /// Amount paid to the provider.
    pub amount: Numeric,
}

/// Key identifying per-provider aggregation statistics for a feed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleProviderKey {
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Provider account identifier.
    pub provider_id: AccountId,
}

impl OracleProviderKey {
    /// Construct a new provider key.
    #[must_use]
    pub fn new(feed_id: FeedId, provider_id: AccountId) -> Self {
        Self {
            feed_id,
            provider_id,
        }
    }
}

/// Aggregate counters for a provider across feed slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleProviderStats {
    /// Number of inlier observations.
    pub inliers: u64,
    /// Number of outlier observations.
    pub outliers: u64,
    /// Number of error observations.
    pub errors: u64,
    /// Number of missed submissions.
    pub no_shows: u64,
    /// Number of rewards applied.
    pub rewards: u64,
    /// Number of slashes applied.
    pub slashes: u64,
}

impl OracleProviderStats {
    /// Increment the reward counter.
    pub fn record_reward(&mut self) {
        self.rewards = self.rewards.saturating_add(1);
    }

    /// Increment the slash counter.
    pub fn record_slash(&mut self) {
        self.slashes = self.slashes.saturating_add(1);
    }
}

/// Identifier for an oracle dispute.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleDisputeId(pub u64);

/// Resolution status for a dispute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "outcome", content = "detail"))]
pub enum OracleDisputeOutcome {
    /// Dispute upheld; full penalty applied.
    Upheld,
    /// Dispute partially upheld; reduced penalty applied.
    Reduced,
    /// Dispute deemed frivolous; challenger is penalised.
    Frivolous,
}

/// Persisted record for a dispute.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleDispute {
    /// Unique dispute identifier.
    pub id: OracleDisputeId,
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Slot index.
    pub slot: FeedSlot,
    /// Canonical request hash for the slot.
    pub request_hash: Hash,
    /// Account that opened the dispute.
    pub challenger: AccountId,
    /// Provider being challenged.
    pub target: OracleId,
    /// Bond amount staked by the challenger.
    pub bond: Numeric,
    /// Evidence hashes supplied by the challenger.
    #[cfg_attr(feature = "json", norito(default))]
    pub evidence: Vec<Hash>,
    /// Optional human-readable reason.
    pub reason: String,
    /// Current outcome for the dispute.
    pub status: OracleDisputeStatus,
}

/// Lifecycle status for an oracle dispute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "detail"))]
pub enum OracleDisputeStatus {
    /// Dispute is open and awaiting resolution.
    Open,
    /// Dispute was resolved with an outcome.
    Resolved(OracleDisputeOutcome),
    /// Dispute was dismissed without penalties.
    Dismissed,
}

/// Gossip key identifying observation/report messages for replay protection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct GossipKey {
    /// Feed identifier.
    pub feed_id: FeedId,
    /// Configuration version.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index.
    pub slot: FeedSlot,
}

impl GossipKey {
    /// Construct a new gossip key.
    #[must_use]
    pub fn new(feed_id: FeedId, feed_config_version: FeedConfigVersion, slot: FeedSlot) -> Self {
        Self {
            feed_id,
            feed_config_version,
            slot,
        }
    }
}

/// Replay key extending [`GossipKey`] with the canonical request hash.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReplayKey {
    /// Gossip key components.
    pub gossip: GossipKey,
    /// Canonical request hash.
    pub request_hash: Hash,
}

impl ReplayKey {
    /// Build a replay key from its components.
    #[must_use]
    pub fn new(
        feed_id: FeedId,
        feed_config_version: FeedConfigVersion,
        slot: FeedSlot,
        request_hash: Hash,
    ) -> Self {
        Self {
            gossip: GossipKey {
                feed_id,
                feed_config_version,
                slot,
            },
            request_hash,
        }
    }
}

/// Replay outcomes emitted by [`ReplayProtection`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayStatus {
    /// First time seeing this replay key within the window.
    Fresh,
    /// Replay detected (same key within the window).
    Duplicate,
    /// Replay key is too old for the configured window.
    Expired,
}

/// Sliding-window replay protection keyed by `(feed, version, slot, request_hash)`.
#[derive(Debug)]
pub struct ReplayProtection {
    window_slots: NonZeroU64,
    seen: BTreeMap<ReplayKey, FeedSlot>,
}

impl ReplayProtection {
    /// Create a new replay guard with the given slot window.
    #[must_use]
    pub fn new(window_slots: NonZeroU64) -> Self {
        Self {
            window_slots,
            seen: BTreeMap::new(),
        }
    }

    /// Record a replay key and return whether it is new, duplicate, or expired.
    #[must_use]
    pub fn record(&mut self, key: ReplayKey, current_slot: FeedSlot) -> ReplayStatus {
        let earliest_slot = current_slot.saturating_sub(self.window_slots.get());
        if key.gossip.slot < earliest_slot {
            return ReplayStatus::Expired;
        }

        self.seen.retain(|_, slot| *slot >= earliest_slot);

        if let Some(last_seen) = self.seen.get(&key)
            && *last_seen >= earliest_slot
        {
            return ReplayStatus::Duplicate;
        }

        self.seen.insert(key, current_slot);
        ReplayStatus::Fresh
    }
}

/// Deterministic committee draw for a feed and epoch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CommitteeDraw {
    /// Seed used to derive ordering and leader selection.
    pub seed: Hash,
    /// Selected committee members in deterministic order.
    pub members: Vec<OracleId>,
}

impl CommitteeDraw {
    /// Deterministically select the slot leader from the committee.
    #[must_use]
    pub fn leader_for_slot(&self, slot: FeedSlot) -> Option<OracleId> {
        if self.members.is_empty() {
            return None;
        }

        let mut seed_bytes = Vec::with_capacity(Hash::LENGTH + std::mem::size_of::<FeedSlot>());
        seed_bytes.extend_from_slice(self.seed.as_ref());
        seed_bytes.extend_from_slice(&slot.to_le_bytes());
        let leader_hash = Hash::new(seed_bytes);
        let leader_seed = u64::from_le_bytes(
            leader_hash.as_ref()[..8]
                .try_into()
                .expect("8 bytes available"),
        );
        let len_u64 = u64::try_from(self.members.len()).unwrap_or(u64::MAX);
        let idx = usize::try_from(leader_seed % len_u64).unwrap_or(0);
        self.members.get(idx).cloned()
    }
}

/// Compute the deterministic committee draw for a feed/epoch using the validator set root.
#[must_use]
pub fn derive_committee(
    feed_id: &FeedId,
    feed_config_version: FeedConfigVersion,
    epoch: u64,
    validator_set_root: &Hash,
    providers: &[OracleId],
    committee_size: NonZeroUsize,
) -> CommitteeDraw {
    let mut seed_bytes = Vec::with_capacity(Hash::LENGTH + 24);
    seed_bytes.extend_from_slice(validator_set_root.as_ref());
    seed_bytes.extend_from_slice(&feed_id.encode());
    seed_bytes.extend_from_slice(&feed_config_version.encode());
    seed_bytes.extend_from_slice(&epoch.to_le_bytes());
    let seed = Hash::new(seed_bytes);

    let mut scored: Vec<(OracleId, [u8; Hash::LENGTH])> = providers
        .iter()
        .cloned()
        .map(|oracle_id| {
            let mut bytes = Vec::with_capacity(Hash::LENGTH + 32);
            bytes.extend_from_slice(seed.as_ref());
            bytes.extend_from_slice(&oracle_id.encode());
            let digest = Hash::new(bytes);
            (oracle_id, *digest.as_ref())
        })
        .collect();

    // Deduplicate providers while preserving the lowest hash score.
    scored.sort_by(|left, right| match left.0.cmp(&right.0) {
        Ordering::Equal => left.1.cmp(&right.1),
        other => other,
    });
    scored.dedup_by(|left, right| left.0 == right.0);

    // Re-sort by score to pick the top committee_size providers.
    scored.sort_by(|left, right| left.1.cmp(&right.1));

    let members: Vec<OracleId> = scored
        .into_iter()
        .map(|(oracle_id, _)| oracle_id)
        .take(committee_size.get())
        .collect();

    CommitteeDraw { seed, members }
}

/// Oracle ABI manifest used to pin the schema for on-chain validation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleAbiManifest {
    /// ABI version identifier.
    pub version: u32,
    /// Type name for the feed configuration.
    pub feed_config: String,
    /// Type name for observations.
    pub observation: String,
    /// Type name for reports.
    pub report: String,
    /// Type name for feed events.
    pub feed_event: String,
    /// Gossip key type name.
    pub gossip_key: String,
    /// Replay key type name.
    pub replay_key: String,
}

impl OracleAbiManifest {
    /// Current ABI version for the oracle schema.
    pub const VERSION: u32 = 1;

    /// Build the canonical manifest for the current version.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            version: Self::VERSION,
            feed_config: FeedConfig::type_name(),
            observation: Observation::type_name(),
            report: Report::type_name(),
            feed_event: FeedEvent::type_name(),
            gossip_key: GossipKey::type_name(),
            replay_key: ReplayKey::type_name(),
        }
    }
}

/// Canonical ABI hash for the oracle message schema (blake2b-256 over the manifest).
#[must_use]
pub fn oracle_abi_hash() -> HashOf<OracleAbiManifest> {
    HashOf::new(&OracleAbiManifest::v1())
}

/// Backoff policy used by connectors to retry within the active slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchDiscipline {
    /// Maximum number of attempts (including the initial fetch at `slot`).
    pub max_attempts: NonZeroUsize,
    /// Base backoff expressed in slots between attempts.
    pub base_backoff_slots: NonZeroU64,
    /// Maximum jitter applied to each retry (0 = no jitter).
    pub jitter_max_slots: u64,
}

impl Default for FetchDiscipline {
    fn default() -> Self {
        Self {
            max_attempts: NonZeroUsize::new(2).expect("non-zero attempts"),
            base_backoff_slots: NonZeroU64::new(1).expect("non-zero backoff"),
            jitter_max_slots: 0,
        }
    }
}

/// Deterministic fetch schedule for a provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchPlan {
    /// Whether the provider is allowed to fetch (must be in committee).
    pub allowed: bool,
    /// Slots when the provider should attempt fetches (sorted, unique).
    pub attempts: Vec<FeedSlot>,
}

impl FetchPlan {
    /// Returns `true` when fetches are allowed for this plan.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }
}

/// Build a deterministic fetch plan for a provider using the committee draw.
#[must_use]
pub fn plan_committee_fetches(
    committee: &CommitteeDraw,
    provider_id: &OracleId,
    slot: FeedSlot,
    policy: FetchDiscipline,
    request_hash: &Hash,
) -> FetchPlan {
    if !committee.members.contains(provider_id) {
        return FetchPlan {
            allowed: false,
            attempts: Vec::new(),
        };
    }

    let mut attempts = Vec::with_capacity(policy.max_attempts.get());
    attempts.push(slot);

    for attempt_idx in 1..policy.max_attempts.get() {
        let jitter = if policy.jitter_max_slots == 0 {
            0
        } else {
            let mut seed_bytes = Vec::with_capacity(Hash::LENGTH + 16);
            seed_bytes.extend_from_slice(provider_id.encode().as_ref());
            seed_bytes.extend_from_slice(request_hash.as_ref());
            seed_bytes.extend_from_slice(&(attempt_idx as u64).to_le_bytes());
            let jitter_hash = Hash::new(seed_bytes);
            u64::from_le_bytes(
                jitter_hash.as_ref()[..8]
                    .try_into()
                    .expect("8 bytes available for jitter"),
            ) % (policy.jitter_max_slots + 1)
        };

        let next = slot
            .saturating_add(policy.base_backoff_slots.get() * attempt_idx as u64)
            .saturating_add(jitter);
        attempts.push(next);
    }

    attempts.sort_unstable();
    attempts.dedup();

    FetchPlan {
        allowed: true,
        attempts,
    }
}

/// Aggregated report and outcome for a feed slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregationOutput {
    /// Built report body (entries include outlier flags).
    pub report: ReportBody,
    /// Aggregation outcome persisted on-chain.
    pub outcome: FeedEventOutcome,
}

impl AggregationOutput {
    /// Convert aggregation output into a feed event.
    #[must_use]
    pub fn into_feed_event(self) -> FeedEvent {
        FeedEvent {
            feed_id: self.report.feed_id.clone(),
            feed_config_version: self.report.feed_config_version,
            slot: self.report.slot,
            outcome: self.outcome,
        }
    }
}

/// Errors surfaced while aggregating observations into a report.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum OracleAggregationError {
    /// Generic model validation failure (feed/config/connector/cadence).
    #[error(transparent)]
    Model(#[from] OracleModelError),
    /// Observation was recorded for a different slot than expected.
    #[error("observation slot mismatch: expected {expected}, got {provided}")]
    SlotMismatch {
        /// Slot expected by the caller.
        expected: FeedSlot,
        /// Slot provided by the observation.
        provided: FeedSlot,
    },
    /// Observation used a different request hash than the aggregation window.
    #[error("request hash mismatch: expected {expected}, got {provided}")]
    RequestHashMismatch {
        /// Expected request hash.
        expected: Hash,
        /// Provided request hash.
        provided: Hash,
    },
    /// Observation came from an oracle not present in the feed provider list.
    #[error("observation from unknown provider {oracle_id}")]
    UnknownProvider {
        /// Oracle identifier that is not part of the feed.
        oracle_id: OracleId,
    },
    /// Duplicate observation detected for the same provider within the slot.
    #[error("duplicate observation from {oracle_id}")]
    DuplicateObservation {
        /// Oracle identifier that sent multiple observations.
        oracle_id: OracleId,
    },
    /// Observation count exceeds the feed cap.
    #[error("too many observations: provided {provided}, max {max}")]
    TooManyObservations {
        /// Number of observations provided.
        provided: usize,
        /// Maximum allowed observations.
        max: u16,
    },
    /// Observation values use mismatched decimal scales.
    #[error("mismatched scales: expected {expected}, got {provided}")]
    MismatchedScale {
        /// Expected scale (from the first observation).
        expected: u32,
        /// Provided scale that differed.
        provided: u32,
    },
    /// Observation value encoding exceeds the feed cap.
    #[error("observation value too long: encoded {len} > max {max}")]
    ValueTooLong {
        /// Encoded value length in bytes.
        len: usize,
        /// Maximum allowed length.
        max: u16,
    },
    /// Outlier filtering rejected all values.
    #[error("no inlier values after outlier filtering")]
    NoInliers,
}

/// Stable rejection codes for oracle aggregation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OracleRejectionCode {
    /// Report contained more entries than the feed permits.
    ModelTooManyEntries,
    /// Duplicate oracle identifier detected in a report.
    ModelDuplicateOracle,
    /// Slot is inactive for the given cadence.
    ModelInactiveSlot,
    /// Feed identifier did not match the expected configuration.
    ModelFeedMismatch,
    /// Feed configuration version mismatch.
    ModelFeedVersionMismatch,
    /// Observation referenced a different connector id/version from the feed.
    ModelConnectorMismatch,
    /// Sensitive headers must be redacted (hashed) before being stored.
    ModelUnredactedSensitiveHeader,
    /// Observation was recorded for a different slot than expected.
    AggregationSlotMismatch,
    /// Observation used a different request hash than the aggregation window.
    AggregationRequestHashMismatch,
    /// Observation came from an oracle not present in the feed provider list.
    AggregationUnknownProvider,
    /// Duplicate observation detected for the same provider within the slot.
    AggregationDuplicateObservation,
    /// Observation count exceeds the feed cap.
    AggregationTooManyObservations,
    /// Observation values use mismatched decimal scales.
    AggregationMismatchedScale,
    /// Observation value encoding exceeds the feed cap.
    AggregationValueTooLong,
    /// Outlier filtering rejected all values.
    AggregationNoInliers,
}

impl OracleRejectionCode {
    /// Stable string identifier for the rejection code.
    #[must_use]
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::ModelTooManyEntries => "oracle_model_too_many_entries",
            Self::ModelDuplicateOracle => "oracle_model_duplicate_oracle",
            Self::ModelInactiveSlot => "oracle_model_inactive_slot",
            Self::ModelFeedMismatch => "oracle_model_feed_mismatch",
            Self::ModelFeedVersionMismatch => "oracle_model_feed_version_mismatch",
            Self::ModelConnectorMismatch => "oracle_model_connector_mismatch",
            Self::ModelUnredactedSensitiveHeader => "oracle_model_unredacted_sensitive_header",
            Self::AggregationSlotMismatch => "oracle_agg_slot_mismatch",
            Self::AggregationRequestHashMismatch => "oracle_agg_request_hash_mismatch",
            Self::AggregationUnknownProvider => "oracle_agg_unknown_provider",
            Self::AggregationDuplicateObservation => "oracle_agg_duplicate_observation",
            Self::AggregationTooManyObservations => "oracle_agg_too_many_observations",
            Self::AggregationMismatchedScale => "oracle_agg_mismatched_scale",
            Self::AggregationValueTooLong => "oracle_agg_value_too_long",
            Self::AggregationNoInliers => "oracle_agg_no_inliers",
        }
    }

    /// Human-readable description of the rejection.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::ModelTooManyEntries => {
                "report contained more entries than the feed configuration allows"
            }
            Self::ModelDuplicateOracle => "duplicate oracle entry detected in the report",
            Self::ModelInactiveSlot => "observation slot is not active for the configured cadence",
            Self::ModelFeedMismatch => "feed identifier differs from the registered feed",
            Self::ModelFeedVersionMismatch => {
                "feed configuration version differs from the active version"
            }
            Self::ModelConnectorMismatch => {
                "connector id/version does not match the feed configuration"
            }
            Self::ModelUnredactedSensitiveHeader => {
                "sensitive connector header must be hashed before submission"
            }
            Self::AggregationSlotMismatch => "observation slot differs from the aggregation window",
            Self::AggregationRequestHashMismatch => {
                "observation request hash differs from the aggregation window"
            }
            Self::AggregationUnknownProvider => {
                "observation provider is not part of the configured feed committee"
            }
            Self::AggregationDuplicateObservation => {
                "duplicate observation received for the same provider and slot"
            }
            Self::AggregationTooManyObservations => {
                "observation count exceeds the feed configuration cap"
            }
            Self::AggregationMismatchedScale => {
                "observation values use mixed decimal scales within the slot"
            }
            Self::AggregationValueTooLong => "observation value encoding exceeds configured length",
            Self::AggregationNoInliers => {
                "outlier filtering rejected all observations for the slot"
            }
        }
    }

    /// Category of the rejection (model vs aggregation).
    #[must_use]
    pub const fn category(self) -> &'static str {
        match self {
            Self::ModelTooManyEntries
            | Self::ModelDuplicateOracle
            | Self::ModelInactiveSlot
            | Self::ModelFeedMismatch
            | Self::ModelFeedVersionMismatch
            | Self::ModelConnectorMismatch
            | Self::ModelUnredactedSensitiveHeader => "model",
            Self::AggregationSlotMismatch
            | Self::AggregationRequestHashMismatch
            | Self::AggregationUnknownProvider
            | Self::AggregationDuplicateObservation
            | Self::AggregationTooManyObservations
            | Self::AggregationMismatchedScale
            | Self::AggregationValueTooLong
            | Self::AggregationNoInliers => "aggregation",
        }
    }

    /// Ordered list of all rejection codes.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::ModelTooManyEntries,
            Self::ModelDuplicateOracle,
            Self::ModelInactiveSlot,
            Self::ModelFeedMismatch,
            Self::ModelFeedVersionMismatch,
            Self::ModelConnectorMismatch,
            Self::ModelUnredactedSensitiveHeader,
            Self::AggregationSlotMismatch,
            Self::AggregationRequestHashMismatch,
            Self::AggregationUnknownProvider,
            Self::AggregationDuplicateObservation,
            Self::AggregationTooManyObservations,
            Self::AggregationMismatchedScale,
            Self::AggregationValueTooLong,
            Self::AggregationNoInliers,
        ]
    }
}

impl From<&OracleAggregationError> for OracleRejectionCode {
    fn from(err: &OracleAggregationError) -> Self {
        match err {
            OracleAggregationError::Model(model) => model.into(),
            OracleAggregationError::SlotMismatch { .. } => Self::AggregationSlotMismatch,
            OracleAggregationError::RequestHashMismatch { .. } => {
                Self::AggregationRequestHashMismatch
            }
            OracleAggregationError::UnknownProvider { .. } => Self::AggregationUnknownProvider,
            OracleAggregationError::DuplicateObservation { .. } => {
                Self::AggregationDuplicateObservation
            }
            OracleAggregationError::TooManyObservations { .. } => {
                Self::AggregationTooManyObservations
            }
            OracleAggregationError::MismatchedScale { .. } => Self::AggregationMismatchedScale,
            OracleAggregationError::ValueTooLong { .. } => Self::AggregationValueTooLong,
            OracleAggregationError::NoInliers => Self::AggregationNoInliers,
        }
    }
}

impl From<&OracleModelError> for OracleRejectionCode {
    fn from(err: &OracleModelError) -> Self {
        match err {
            OracleModelError::TooManyEntries { .. } => Self::ModelTooManyEntries,
            OracleModelError::DuplicateOracle { .. } => Self::ModelDuplicateOracle,
            OracleModelError::InactiveSlot { .. } => Self::ModelInactiveSlot,
            OracleModelError::FeedMismatch { .. } => Self::ModelFeedMismatch,
            OracleModelError::FeedVersionMismatch { .. } => Self::ModelFeedVersionMismatch,
            OracleModelError::ConnectorMismatch { .. } => Self::ModelConnectorMismatch,
            OracleModelError::UnredactedSensitiveHeader { .. } => {
                Self::ModelUnredactedSensitiveHeader
            }
        }
    }
}

fn abs_i128(value: i128) -> i128 {
    if value >= 0 {
        value
    } else {
        // SAFETY: two's complement i128::MIN.abs() would overflow; saturate.
        value.saturating_mul(-1)
    }
}

fn median(sorted: &[i128]) -> i128 {
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        sorted[mid - 1]
            .saturating_add(sorted[mid])
            .checked_div(2)
            .unwrap_or(sorted[mid - 1])
    } else {
        sorted[mid]
    }
}

fn percentile(sorted: &[i128], percentile_bps: u16) -> i128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as u128 - 1) * u128::from(percentile_bps)) / 10_000;
    let idx = usize::try_from(idx).unwrap_or_else(|_| sorted.len().saturating_sub(1));
    sorted[idx]
}

/// Aggregate observations into a report and feed event outcome.
///
/// # Errors
/// Propagates [`OracleAggregationError`] when validation fails or aggregation cannot be computed.
#[allow(clippy::too_many_lines)]
pub fn aggregate_observations(
    config: &FeedConfig,
    slot: FeedSlot,
    request_hash: Hash,
    submitter: OracleId,
    observations: &[Observation],
) -> Result<AggregationOutput, OracleAggregationError> {
    config.ensure_active_slot(slot)?;

    if observations.len() > usize::from(config.max_observers) {
        return Err(OracleAggregationError::TooManyObservations {
            provided: observations.len(),
            max: config.max_observers,
        });
    }

    let provider_set: BTreeSet<_> = config.providers.iter().cloned().collect();
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    let mut errors = Vec::new();

    for obs in observations {
        config.validate_observation_meta(&obs.body)?;
        if obs.body.slot != slot {
            return Err(OracleAggregationError::SlotMismatch {
                expected: slot,
                provided: obs.body.slot,
            });
        }
        if obs.body.request_hash != request_hash {
            return Err(OracleAggregationError::RequestHashMismatch {
                expected: request_hash,
                provided: obs.body.request_hash,
            });
        }
        if !provider_set.contains(&obs.body.provider_id) {
            return Err(OracleAggregationError::UnknownProvider {
                oracle_id: obs.body.provider_id.clone(),
            });
        }
        if !seen.insert(obs.body.provider_id.clone()) {
            return Err(OracleAggregationError::DuplicateObservation {
                oracle_id: obs.body.provider_id.clone(),
            });
        }

        match obs.body.outcome {
            ObservationOutcome::Value(value) => {
                let len = value.encode().len();
                if len > usize::from(config.max_value_len) {
                    return Err(OracleAggregationError::ValueTooLong {
                        len,
                        max: config.max_value_len,
                    });
                }
                values.push((obs.body.provider_id.clone(), value, obs.body.hash()));
            }
            ObservationOutcome::Error(code) => errors.push(code),
        }
    }

    if values.is_empty() {
        if let Some(code) = errors.first().copied()
            && errors.iter().all(|candidate| *candidate == code)
        {
            let empty_report = ReportBody {
                feed_id: config.feed_id.clone(),
                feed_config_version: config.feed_config_version,
                slot,
                request_hash,
                entries: Vec::new(),
                submitter,
            };
            return Ok(AggregationOutput {
                report: empty_report,
                outcome: FeedEventOutcome::Error(FeedError { code }),
            });
        }

        let empty_report = ReportBody {
            feed_id: config.feed_id.clone(),
            feed_config_version: config.feed_config_version,
            slot,
            request_hash,
            entries: Vec::new(),
            submitter,
        };
        return Ok(AggregationOutput {
            report: empty_report,
            outcome: FeedEventOutcome::Missing,
        });
    }

    let expected_scale = values[0].1.scale;
    let mut numeric: Vec<(OracleId, i128, HashOf<ObservationBody>)> = Vec::new();
    for (oracle_id, value, observation_hash) in &values {
        if value.scale != expected_scale {
            return Err(OracleAggregationError::MismatchedScale {
                expected: expected_scale,
                provided: value.scale,
            });
        }
        numeric.push((oracle_id.clone(), value.mantissa, *observation_hash));
    }

    let mut sorted_mantissas: Vec<i128> = numeric.iter().map(|(_, value, _)| *value).collect();
    sorted_mantissas.sort_unstable();
    let median_value = median(&sorted_mantissas);

    let inlier_mask: Vec<bool> = match config.outlier_policy {
        OutlierPolicy::Mad(k_times_1e2) => {
            let mut deviations: Vec<i128> = sorted_mantissas
                .iter()
                .map(|value| abs_i128(*value - median_value))
                .collect();
            deviations.sort_unstable();
            let mad = median(&deviations);
            let threshold = mad.saturating_mul(i128::from(k_times_1e2)) / 100;
            numeric
                .iter()
                .map(|(_, value, _)| abs_i128(*value - median_value) > threshold)
                .map(|is_outlier| !is_outlier)
                .collect()
        }
        OutlierPolicy::Absolute(policy) => numeric
            .iter()
            .map(|(_, value, _)| abs_i128(*value - median_value) <= policy.max_delta)
            .collect(),
    };

    let mut entries: Vec<ReportEntry> = numeric
        .iter()
        .zip(inlier_mask.iter())
        .map(
            |((oracle_id, mantissa, observation_hash), inlier)| ReportEntry {
                oracle_id: oracle_id.clone(),
                observation_hash: *observation_hash,
                value: ObservationValue::new(*mantissa, expected_scale),
                outlier: !inlier,
            },
        )
        .collect();

    let mut inlier_values: Vec<i128> = numeric
        .iter()
        .zip(inlier_mask.iter())
        .filter_map(|((_, mantissa, _), inlier)| inlier.then_some(*mantissa))
        .collect();

    if inlier_values.is_empty() {
        return Err(OracleAggregationError::NoInliers);
    }

    inlier_values.sort_unstable();
    let aggregated_mantissa = match config.aggregation {
        AggregationRule::MedianMad(_) => median(&inlier_values),
        AggregationRule::Percentile(percentile_bps) => percentile(&inlier_values, percentile_bps),
    };

    entries.sort_by(|left, right| left.oracle_id.cmp(&right.oracle_id));
    let report = ReportBody {
        feed_id: config.feed_id.clone(),
        feed_config_version: config.feed_config_version,
        slot,
        request_hash,
        entries,
        submitter,
    };
    validate_report_caps(config, &report)?;

    Ok(AggregationOutput {
        report: report.clone(),
        outcome: FeedEventOutcome::Success(FeedSuccess {
            value: ObservationValue::new(aggregated_mantissa, expected_scale),
            entries: report.entries,
        }),
    })
}

/// Validate that a report respects the feed configuration caps.
///
/// # Errors
/// Returns [`OracleModelError`] when entry counts exceed caps, entries are duplicated, or namespaces mismatch.
pub fn validate_report_caps(
    config: &FeedConfig,
    report: &ReportBody,
) -> Result<(), OracleModelError> {
    if report.entries.len() > usize::from(config.max_observers) {
        return Err(OracleModelError::TooManyEntries {
            provided: report.entries.len(),
            max: config.max_observers,
        });
    }

    let mut seen = BTreeSet::new();
    for entry in &report.entries {
        if !seen.insert(&entry.oracle_id) {
            return Err(OracleModelError::DuplicateOracle {
                oracle_id: entry.oracle_id.clone(),
            });
        }
    }

    Ok(())
}

/// Validation errors for oracle model helpers.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum OracleModelError {
    /// Report contained more entries than the feed permits.
    #[error("too many report entries: provided {provided}, max {max}")]
    TooManyEntries {
        /// Number of entries observed.
        provided: usize,
        /// Maximum allowed entries.
        max: u16,
    },
    /// Duplicate oracle identifier detected in a report.
    #[error("duplicate oracle entry for {oracle_id}")]
    DuplicateOracle {
        /// Duplicate oracle identifier.
        oracle_id: OracleId,
    },
    /// Slot is inactive for the given cadence.
    #[error("slot {slot} is not active for cadence {cadence}")]
    InactiveSlot {
        /// Slot index that was rejected.
        slot: FeedSlot,
        /// Cadence (slots) required for activation.
        cadence: u64,
    },
    /// Feed identifier did not match the expected configuration.
    #[error("feed mismatch: expected {expected:?}, got {provided:?}")]
    FeedMismatch {
        /// Expected feed identifier.
        expected: FeedId,
        /// Provided feed identifier.
        provided: FeedId,
    },
    /// Feed configuration version mismatch.
    #[error("feed config version mismatch: expected {expected:?}, got {provided:?}")]
    FeedVersionMismatch {
        /// Expected version.
        expected: FeedConfigVersion,
        /// Provided version.
        provided: FeedConfigVersion,
    },
    /// Observation referenced a different connector id/version from the feed.
    #[error(
        "connector mismatch: expected {expected_id}@{expected_version}, got {provided_id}@{provided_version}"
    )]
    ConnectorMismatch {
        /// Expected connector identifier.
        expected_id: String,
        /// Expected connector version.
        expected_version: u32,
        /// Provided connector identifier.
        provided_id: String,
        /// Provided connector version.
        provided_version: u32,
    },
    /// Sensitive headers must be redacted (hashed) before being stored.
    #[error("header `{header}` must be hashed before sending")]
    UnredactedSensitiveHeader {
        /// Header name that was rejected.
        header: String,
    },
}

/// Reference oracle kits used by SDK/CLI examples and fixtures.
pub mod kits {
    use norito::json;

    use super::*;

    /// Bundled reference assets for an oracle feed.
    #[derive(Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OracleKit {
        /// Canonical feed configuration.
        pub feed_config: FeedConfig,
        /// Canonical connector request.
        pub connector_request: ConnectorRequest,
        /// Sample observations used to seed reports/events.
        pub observations: Vec<Observation>,
        /// Sample report produced from the observations.
        pub report: Report,
        /// Sample feed event outcome.
        pub feed_event: FeedEvent,
    }

    /// Canonical XOR/USD price feed kit (Median + MAD).
    #[must_use]
    pub fn price_xor_usd() -> OracleKit {
        let feed_config = price_feed_config();
        let connector_request = price_connector_request();
        let observations = price_observations();
        let report = price_report();
        let feed_event = price_feed_event();
        OracleKit {
            feed_config,
            connector_request,
            observations,
            report,
            feed_event,
        }
    }

    /// Canonical twitter follow binding feed kit (`twitter_user_id` → UAID).
    #[must_use]
    pub fn twitter_follow_binding() -> OracleKit {
        let feed_config = social_feed_config();
        let connector_request = social_connector_request();
        let observations = social_observations();
        let report = social_report();
        let feed_event = social_feed_event();
        OracleKit {
            feed_config,
            connector_request,
            observations,
            report,
            feed_event,
        }
    }

    /// Hash used to derive the reference social UAID.
    pub const SOCIAL_UAID_HASH_HEX: &str =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn price_feed_config() -> FeedConfig {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_config_price_xor_usd.json"
        ))
        .expect("price feed config fixture")
    }

    fn social_feed_config() -> FeedConfig {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_config_social_follow.json"
        ))
        .expect("social feed config fixture")
    }

    fn price_connector_request() -> ConnectorRequest {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/connector_request_price_xor_usd.json"
        ))
        .expect("price connector request fixture")
    }

    fn social_connector_request() -> ConnectorRequest {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/connector_request_social_follow.json"
        ))
        .expect("social connector request fixture")
    }

    fn price_observation() -> Observation {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/observation_price_xor_usd.json"
        ))
        .expect("price observation fixture")
    }

    fn social_observation() -> Observation {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/observation_social_follow.json"
        ))
        .expect("social observation fixture")
    }

    fn price_observations() -> Vec<Observation> {
        vec![price_observation()]
    }

    fn social_observations() -> Vec<Observation> {
        vec![social_observation()]
    }

    fn price_report() -> Report {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/report_price_xor_usd.json"
        ))
        .expect("price report fixture")
    }

    fn social_report() -> Report {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/report_social_follow.json"
        ))
        .expect("social report fixture")
    }

    fn price_feed_event() -> FeedEvent {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_event_price_xor_usd.json"
        ))
        .expect("price feed event fixture")
    }

    fn social_feed_event() -> FeedEvent {
        json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_event_social_follow.json"
        ))
        .expect("social feed event fixture")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use norito::json;

    use super::*;
    use crate::domain::DomainId;

    fn feed_id(name: &str) -> FeedId {
        FeedId(Name::from_str(name).expect("feed name"))
    }

    fn oracle(name: &str, domain: &str) -> OracleId {
        let domain_id = DomainId::from_str(domain).expect("domain id");
        let seed = format!("{name}:{domain}");
        let keypair = KeyPair::from_seed(seed.into_bytes(), Algorithm::Ed25519);
        AccountId::new(domain_id, keypair.public_key().clone())
    }

    fn observation_value(mantissa: i128, scale: u32) -> ObservationValue {
        ObservationValue::new(mantissa, scale)
    }

    fn sample_social_uaid() -> UniversalAccountId {
        let uaid_hash = Hash::from_str(kits::SOCIAL_UAID_HASH_HEX).expect("uaid hash");
        UniversalAccountId::from_hash(uaid_hash)
    }

    fn observation_with_value(provider: &OracleId, mantissa: i128, scale: u32) -> Observation {
        let mut obs = sample_observation();
        obs.body.provider_id = provider.clone();
        obs.body.outcome = ObservationOutcome::Value(observation_value(mantissa, scale));
        obs
    }

    fn observation_with_error(provider: &OracleId, code: ObservationErrorCode) -> Observation {
        let mut obs = sample_observation();
        obs.body.provider_id = provider.clone();
        obs.body.outcome = ObservationOutcome::Error(code);
        obs
    }

    #[cfg(feature = "json")]
    fn pretty_json<T: json::JsonSerialize>(value: &T) -> String {
        let value = json::to_value(value).expect("serialize to JSON value");
        json::to_string_pretty(&value).expect("render JSON")
    }

    fn sample_connector_request() -> ConnectorRequest {
        kits::price_xor_usd().connector_request
    }

    fn sample_social_connector_request() -> ConnectorRequest {
        kits::twitter_follow_binding().connector_request
    }

    fn sample_providers() -> Vec<OracleId> {
        kits::price_xor_usd().feed_config.providers.clone()
    }

    fn sample_request_hash() -> Hash {
        sample_connector_request().request_hash()
    }

    fn sample_social_request_hash() -> Hash {
        sample_social_connector_request().request_hash()
    }

    fn sample_feed_config() -> FeedConfig {
        kits::price_xor_usd().feed_config
    }

    fn sample_social_feed_config() -> FeedConfig {
        kits::twitter_follow_binding().feed_config
    }

    fn sample_observation() -> Observation {
        kits::price_xor_usd()
            .observations
            .first()
            .expect("at least one observation")
            .clone()
    }

    fn sample_social_observation() -> Observation {
        kits::twitter_follow_binding()
            .observations
            .first()
            .expect("at least one observation")
            .clone()
    }

    fn sample_report() -> Report {
        kits::price_xor_usd().report
    }

    fn sample_social_report() -> Report {
        kits::twitter_follow_binding().report
    }

    fn sample_feed_event() -> FeedEvent {
        kits::price_xor_usd().feed_event
    }

    fn sample_social_feed_event() -> FeedEvent {
        kits::twitter_follow_binding().feed_event
    }

    #[test]
    fn committee_draw_is_deterministic_and_stable() {
        let feed_id = feed_id("price_xor_usd");
        let providers = vec![
            oracle("alice", "validators"),
            oracle("bob", "validators"),
            oracle("charlie", "validators"),
            oracle("diana", "validators"),
        ];
        let validator_root = Hash::new(b"validator-set-root");

        let draw_one = derive_committee(
            &feed_id,
            FeedConfigVersion(1),
            7,
            &validator_root,
            &providers,
            NonZeroUsize::new(3).unwrap(),
        );

        // Shuffling providers should not affect membership ordering.
        let mut reversed = providers.clone();
        reversed.reverse();
        let draw_two = derive_committee(
            &feed_id,
            FeedConfigVersion(1),
            7,
            &validator_root,
            &reversed,
            NonZeroUsize::new(3).unwrap(),
        );

        assert_eq!(draw_one.seed, draw_two.seed);
        assert_eq!(draw_one.members, draw_two.members);
        assert_eq!(draw_one.members.len(), 3);
    }

    #[test]
    fn leader_changes_per_slot() {
        let feed_id = feed_id("price_xor_usd");
        let providers = vec![
            oracle("alice", "validators"),
            oracle("bob", "validators"),
            oracle("charlie", "validators"),
        ];
        let validator_root = Hash::new(b"validator-set-root");
        let draw = derive_committee(
            &feed_id,
            FeedConfigVersion(2),
            9,
            &validator_root,
            &providers,
            NonZeroUsize::new(3).unwrap(),
        );

        let leader_slot_10 = draw.leader_for_slot(10).expect("leader");
        let leader_slot_11 = draw.leader_for_slot(11).expect("leader");

        // Leaders should be deterministic but not constant across slots.
        assert_ne!(leader_slot_10, leader_slot_11);
        assert!(providers.contains(&leader_slot_10));
        assert!(providers.contains(&leader_slot_11));
    }

    #[test]
    fn replay_guard_detects_duplicates_and_expires() {
        let mut guard = ReplayProtection::new(NonZeroU64::new(3).unwrap());
        let key = ReplayKey::new(
            feed_id("price_xor_usd"),
            FeedConfigVersion(1),
            10,
            Hash::new(b"request"),
        );

        assert_eq!(ReplayStatus::Fresh, guard.record(key.clone(), 10));
        assert_eq!(ReplayStatus::Duplicate, guard.record(key.clone(), 11));

        // Once outside the replay window, stale keys are rejected.
        assert_eq!(ReplayStatus::Expired, guard.record(key, 15));
    }

    #[test]
    fn report_caps_enforce_entry_limits_and_duplicates() {
        let config = FeedConfig {
            feed_id: feed_id("price_xor_usd"),
            feed_config_version: FeedConfigVersion(1),
            providers: vec![oracle("alice", "validators"), oracle("bob", "validators")],
            connector_id: "conn".to_string(),
            connector_version: 1,
            cadence_slots: NonZeroU64::new(5).unwrap(),
            aggregation: AggregationRule::default(),
            outlier_policy: OutlierPolicy::default(),
            min_signers: 1,
            committee_size: 2,
            risk_class: RiskClass::Low,
            max_observers: 2,
            max_value_len: 64,
            max_error_rate_bps: 250,
            dispute_window_slots: NonZeroU64::new(10).unwrap(),
            replay_window_slots: NonZeroU64::new(4).unwrap(),
        };

        let observation_hash = ObservationBody {
            feed_id: config.feed_id.clone(),
            feed_config_version: config.feed_config_version,
            slot: 10,
            provider_id: oracle("alice", "validators"),
            connector_id: "conn".to_string(),
            connector_version: 1,
            request_hash: Hash::new(b"req"),
            outcome: ObservationOutcome::Value(observation_value(100, 2)),
            timestamp_ms: None,
        }
        .hash();

        let ok_report = ReportBody {
            feed_id: config.feed_id.clone(),
            feed_config_version: config.feed_config_version,
            slot: 10,
            request_hash: Hash::new(b"req"),
            entries: vec![ReportEntry {
                oracle_id: oracle("alice", "validators"),
                observation_hash,
                value: observation_value(100, 2),
                outlier: false,
            }],
            submitter: oracle("alice", "validators"),
        };

        assert_eq!(Ok(()), validate_report_caps(&config, &ok_report));

        let too_many_entries = ReportBody {
            entries: vec![
                ok_report.entries[0].clone(),
                ReportEntry {
                    oracle_id: oracle("bob", "validators"),
                    observation_hash,
                    value: observation_value(101, 2),
                    outlier: false,
                },
                ReportEntry {
                    oracle_id: oracle("charlie", "validators"),
                    observation_hash,
                    value: observation_value(102, 2),
                    outlier: false,
                },
            ],
            ..ok_report.clone()
        };

        assert!(matches!(
            validate_report_caps(&config, &too_many_entries),
            Err(OracleModelError::TooManyEntries { .. })
        ));

        let duplicate = ReportBody {
            entries: vec![
                ok_report.entries[0].clone(),
                ReportEntry {
                    oracle_id: ok_report.entries[0].oracle_id.clone(),
                    observation_hash,
                    value: observation_value(101, 2),
                    outlier: false,
                },
            ],
            ..ok_report
        };

        assert!(matches!(
            validate_report_caps(&config, &duplicate),
            Err(OracleModelError::DuplicateOracle { .. })
        ));
    }

    #[test]
    fn observation_fixture_decodes() {
        let expected = sample_observation();
        let observation: Observation = json::from_str(include_str!(
            "../../../../fixtures/oracle/observation_price_xor_usd.json"
        ))
        .expect("decode observation");

        assert_eq!(expected, observation);
    }

    #[test]
    fn report_fixture_decodes_and_validates() {
        let expected = sample_report();
        let report: Report = json::from_str(include_str!(
            "../../../../fixtures/oracle/report_price_xor_usd.json"
        ))
        .expect("decode report");
        let feed = sample_feed_config();

        assert_eq!(expected, report);
        assert_eq!(Ok(()), validate_report_caps(&feed, &report.body));
    }

    #[test]
    fn feed_event_fixture_decodes() {
        let expected = sample_feed_event();
        let event: FeedEvent = json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_event_price_xor_usd.json"
        ))
        .expect("decode feed event");

        assert_eq!(expected, event);
    }

    #[test]
    fn feed_config_activity_matches_cadence() {
        let config = FeedConfig {
            feed_id: feed_id("cadenced"),
            feed_config_version: FeedConfigVersion(1),
            providers: vec![oracle("alice", "validators")],
            connector_id: "conn".to_string(),
            connector_version: 1,
            cadence_slots: NonZeroU64::new(5).unwrap(),
            aggregation: AggregationRule::default(),
            outlier_policy: OutlierPolicy::default(),
            min_signers: 1,
            committee_size: 1,
            risk_class: RiskClass::Medium,
            max_observers: 1,
            max_value_len: 64,
            max_error_rate_bps: 0,
            dispute_window_slots: NonZeroU64::new(5).unwrap(),
            replay_window_slots: NonZeroU64::new(5).unwrap(),
        };

        assert!(config.is_active_slot(10));
        assert!(!config.is_active_slot(12));
    }

    #[test]
    fn gossip_and_replay_keys_match_helpers() {
        let request_hash = Hash::new(b"request");
        let signature = Signature::from_hex("00").expect("signature hex");
        let observation = Observation {
            body: ObservationBody {
                feed_id: feed_id("fx_xor_usd"),
                feed_config_version: FeedConfigVersion(3),
                slot: 12,
                provider_id: oracle("alice", "validators"),
                connector_id: "conn".to_string(),
                connector_version: 2,
                request_hash,
                outcome: ObservationOutcome::Value(observation_value(10, 2)),
                timestamp_ms: Some(42),
            },
            signature: SignatureOf::from_signature(signature.clone()),
        };

        let report = Report {
            body: ReportBody {
                feed_id: observation.body.feed_id.clone(),
                feed_config_version: observation.body.feed_config_version,
                slot: observation.body.slot,
                request_hash,
                entries: vec![],
                submitter: oracle("bob", "validators"),
            },
            signature: SignatureOf::from_signature(signature),
        };

        let expected_gossip =
            GossipKey::new(observation.body.feed_id.clone(), FeedConfigVersion(3), 12);
        let expected_replay = ReplayKey::new(
            observation.body.feed_id.clone(),
            FeedConfigVersion(3),
            12,
            request_hash,
        );

        assert_eq!(expected_gossip, observation.gossip_key());
        assert_eq!(expected_gossip, report.gossip_key());
        assert_eq!(expected_replay, observation.replay_key());
        assert_eq!(expected_replay, report.replay_key());

        let mut guard = ReplayProtection::new(NonZeroU64::new(4).unwrap());
        assert_eq!(
            ReplayStatus::Fresh,
            guard.record(report.replay_key(), observation.body.slot)
        );
        assert_eq!(
            ReplayStatus::Duplicate,
            guard.record(report.replay_key(), observation.body.slot + 1)
        );
    }

    #[test]
    fn connector_request_fixture_round_trip() {
        let expected = sample_connector_request();
        let request: ConnectorRequest = json::from_str(include_str!(
            "../../../../fixtures/oracle/connector_request_price_xor_usd.json"
        ))
        .expect("decode connector request");

        assert_eq!(expected, request);
        let hash_literal: HashOf<ConnectorRequest> =
            json::from_value(json::to_value(&request.hash()).expect("hash json"))
                .expect("hash literal parses");
        assert_eq!(hash_literal, request.hash());
    }

    #[test]
    #[ignore = "fixture printer for manual inspection"]
    fn print_connector_request_fixture() {
        let request = sample_connector_request();
        let json_value = json::to_value(&request).expect("serialize request");
        let json_string = json::to_string_pretty(&json_value).expect("stringify request");
        println!("{json_string}");
        println!("request_hash_hex={}", request.request_hash());
        let hash_literal = json::to_string(&json::to_value(&request.hash()).expect("hash value"))
            .expect("stringify hash");
        println!("request_hash_literal={hash_literal}");

        let observation_json =
            json::to_string_pretty(&json::to_value(&sample_observation()).expect("obs json"))
                .expect("stringify observation");
        println!("observation_json={observation_json}");

        let report_json =
            json::to_string_pretty(&json::to_value(&sample_report()).expect("report json"))
                .expect("stringify report");
        println!("report_json={report_json}");

        let event_json =
            json::to_string_pretty(&json::to_value(&sample_feed_event()).expect("event json"))
                .expect("stringify event");
        println!("event_json={event_json}");
    }

    #[test]
    fn connector_request_hash_is_stable() {
        let request = sample_connector_request();
        let hash = request.hash();
        assert_eq!(
            hash.to_string(),
            "59cfea4268fb255e2fdb550b37cb83df836d62744f835f121e5731ab62679bdb",
            "update the connector request fixture if this hash intentionally changes"
        );
    }

    #[test]
    fn feed_config_meta_validation_detects_mismatches() {
        let config = sample_feed_config();
        let mut observation = sample_observation().body;

        // Slot not aligned with cadence.
        observation.slot = 11;
        assert!(matches!(
            config.validate_observation_meta(&observation),
            Err(OracleModelError::InactiveSlot { .. })
        ));

        // Connector mismatch.
        observation.slot = 10;
        observation.connector_id = "other".to_string();
        assert!(matches!(
            config.validate_observation_meta(&observation),
            Err(OracleModelError::ConnectorMismatch { .. })
        ));

        // Feed mismatch.
        observation.connector_id = config.connector_id.clone();
        observation.feed_id = feed_id("other");
        assert!(matches!(
            config.validate_observation_meta(&observation),
            Err(OracleModelError::FeedMismatch { .. })
        ));

        // Version mismatch.
        observation.feed_id = config.feed_id.clone();
        observation.feed_config_version = FeedConfigVersion(99);
        assert!(matches!(
            config.validate_observation_meta(&observation),
            Err(OracleModelError::FeedVersionMismatch { .. })
        ));

        // Happy path.
        observation.feed_config_version = config.feed_config_version;
        observation.connector_id = config.connector_id.clone();
        assert_eq!(Ok(()), config.validate_observation_meta(&observation));
    }

    #[test]
    fn observation_error_codes_classify() {
        assert_eq!(
            ObservationErrorClass::Honest,
            ObservationErrorCode::ResourceUnavailable.classification()
        );
        assert_eq!(
            ObservationErrorClass::Misconfigured,
            ObservationErrorCode::AuthFailed.classification()
        );
        assert_eq!(
            ObservationErrorClass::Missing,
            ObservationErrorCode::Missing.classification()
        );
        assert_eq!(
            ObservationErrorClass::Honest,
            ObservationErrorCode::Other(7).classification()
        );
    }

    #[test]
    fn committee_fetch_plan_respects_membership_and_backoff() {
        let feed_id = feed_id("price_xor_usd");
        let validator_root = Hash::new(b"validator-set-root");
        let providers = vec![
            oracle("alice", "validators"),
            oracle("bob", "validators"),
            oracle("charlie", "validators"),
        ];
        let committee = derive_committee(
            &feed_id,
            FeedConfigVersion(1),
            4,
            &validator_root,
            &providers,
            NonZeroUsize::new(2).unwrap(),
        );
        let request_hash = sample_request_hash();
        let policy = FetchDiscipline {
            max_attempts: NonZeroUsize::new(3).unwrap(),
            base_backoff_slots: NonZeroU64::new(2).unwrap(),
            jitter_max_slots: 1,
        };

        // Member plan has deterministic attempts.
        let member = committee.members.first().expect("committee member");
        let member_plan = plan_committee_fetches(&committee, member, 10, policy, &request_hash);
        assert!(member_plan.is_allowed());
        assert_eq!(vec![10, 13, 15], member_plan.attempts);

        // Non-member gets a denied plan.
        let dave = oracle("dave", "validators");
        let dave_plan = plan_committee_fetches(&committee, &dave, 10, policy, &request_hash);
        assert!(!dave_plan.is_allowed());
        assert!(dave_plan.attempts.is_empty());
    }

    #[test]
    fn aggregation_produces_success_and_outlier_flags() {
        let config = sample_feed_config();
        let request_hash = sample_request_hash();
        let providers = sample_providers();
        let observations = vec![
            observation_with_value(&providers[0], 1_002_500, 5),
            observation_with_value(&providers[1], 1_001_500, 5),
        ];

        let output = aggregate_observations(
            &config,
            10,
            request_hash,
            providers[1].clone(),
            &observations,
        )
        .expect("aggregation succeeds");

        assert_eq!(config.feed_id, output.report.feed_id);
        assert_eq!(
            FeedEventOutcome::Success(FeedSuccess {
                value: observation_value(1_002_000, 5),
                entries: output.report.entries.clone(),
            }),
            output.outcome
        );
        assert!(output.report.entries.iter().all(|entry| !entry.outlier));

        let event = output.clone().into_feed_event();
        assert_eq!(event.feed_id, config.feed_id);
        assert_eq!(event.slot, 10);
        assert!(matches!(event.outcome, FeedEventOutcome::Success(_)));
    }

    #[test]
    fn aggregation_marks_outliers_and_rejects_when_all_filtered() {
        let mut config = sample_feed_config();
        config.outlier_policy = OutlierPolicy::Mad(350);
        let carol = oracle("carol", "validators");
        config.providers.push(carol.clone());
        let request_hash = sample_request_hash();
        let providers = sample_providers();
        let observations = vec![
            observation_with_value(&providers[0], 1_000_000, 5),
            observation_with_value(&providers[1], 1_000_100, 5),
            observation_with_value(&carol, 1_200_000, 5),
        ];

        let output = aggregate_observations(
            &config,
            10,
            request_hash,
            providers[0].clone(),
            &observations,
        )
        .expect("aggregation succeeds");

        assert!(output.report.entries.iter().any(|entry| entry.outlier));

        let mut strict = config.clone();
        strict.outlier_policy = OutlierPolicy::Absolute(AbsoluteOutlier { max_delta: 0 });
        let all_outliers = aggregate_observations(
            &strict,
            10,
            request_hash,
            providers[0].clone(),
            &[
                observation_with_value(&providers[0], 2_000_000, 5),
                observation_with_value(&providers[1], 2_100_000, 5),
            ],
        );
        assert!(matches!(
            all_outliers,
            Err(OracleAggregationError::NoInliers)
        ));
    }

    #[test]
    fn aggregation_handles_errors_and_missing() {
        let config = sample_feed_config();
        let request_hash = sample_request_hash();
        let providers = sample_providers();
        let errors_only = aggregate_observations(
            &config,
            10,
            request_hash,
            providers[0].clone(),
            &[
                observation_with_error(&providers[0], ObservationErrorCode::Timeout),
                observation_with_error(&providers[1], ObservationErrorCode::Timeout),
            ],
        )
        .expect("errors aggregate");

        assert_eq!(
            FeedEventOutcome::Error(FeedError {
                code: ObservationErrorCode::Timeout
            }),
            errors_only.outcome
        );
        assert!(errors_only.report.entries.is_empty());

        let missing = aggregate_observations(&config, 10, request_hash, providers[0].clone(), &[])
            .expect("empty observations produce missing");
        assert!(matches!(missing.outcome, FeedEventOutcome::Missing));
    }

    #[test]
    fn social_follow_feed_requires_matching_hashed_values() {
        let strict_config = sample_social_feed_config();
        let mut tolerant_config = strict_config.clone();
        tolerant_config.outlier_policy = OutlierPolicy::Absolute(AbsoluteOutlier {
            max_delta: i128::MAX,
        });
        let uaid_value = ObservationValue::from_uaid(&sample_social_uaid());

        let mut first = sample_social_observation();
        first.body.outcome = ObservationOutcome::Value(uaid_value);
        first.body.request_hash = sample_social_request_hash();

        let mut second = first.clone();
        second.body.provider_id = strict_config
            .providers
            .get(1)
            .cloned()
            .expect("config must list a second provider");
        second.body.timestamp_ms = Some(1_700_000_111_001);

        let output = aggregate_observations(
            &tolerant_config,
            15,
            sample_social_request_hash(),
            second.body.provider_id.clone(),
            &[first.clone(), second.clone()],
        )
        .expect("matching UAID hashes aggregate");

        assert_eq!(
            FeedEventOutcome::Success(FeedSuccess {
                value: ObservationValue::from_uaid(&sample_social_uaid()),
                entries: output.report.entries.clone(),
            }),
            output.outcome
        );
        assert!(output.report.entries.iter().all(|entry| !entry.outlier));

        let submitter = second.body.provider_id.clone();
        let mut mismatched = second;
        let alt_uaid = UniversalAccountId::from_hash(Hash::new(b"other-uaid"));
        mismatched.body.outcome = ObservationOutcome::Value(ObservationValue::from_uaid(&alt_uaid));

        let err = aggregate_observations(
            &strict_config,
            15,
            sample_social_request_hash(),
            submitter,
            &[first, mismatched],
        )
        .expect_err("mixed UAID hashes should be rejected");
        assert!(matches!(err, OracleAggregationError::NoInliers));
    }

    #[test]
    fn aggregation_rejects_mismatched_request_hash_and_scale() {
        let config = sample_feed_config();
        let mut observation = sample_observation();
        observation.body.request_hash = Hash::new(b"other");
        let err = aggregate_observations(
            &config,
            10,
            sample_request_hash(),
            observation.body.provider_id.clone(),
            &[observation.clone()],
        )
        .expect_err("hash mismatch");
        assert!(matches!(
            err,
            OracleAggregationError::RequestHashMismatch { .. }
        ));

        observation.body.request_hash = sample_request_hash();
        observation.body.outcome = ObservationOutcome::Value(ObservationValue::new(1_000, 4));
        let second_provider = config
            .providers
            .get(1)
            .cloned()
            .expect("fixture provides at least two providers");
        let second = observation_with_value(&second_provider, 1_000, 5);
        let err = aggregate_observations(
            &config,
            10,
            sample_request_hash(),
            observation.body.provider_id.clone(),
            &[observation, second],
        )
        .expect_err("scale mismatch");
        assert!(matches!(
            err,
            OracleAggregationError::MismatchedScale { .. }
        ));
    }

    #[test]
    fn aggregation_rejects_value_length_overflow() {
        let mut config = sample_feed_config();
        config.max_value_len = 4;
        let provider = sample_providers()[0].clone();
        let observation = observation_with_value(&provider, i128::MAX, 5);
        let err =
            aggregate_observations(&config, 10, sample_request_hash(), provider, &[observation])
                .expect_err("value too long");
        assert!(matches!(err, OracleAggregationError::ValueTooLong { .. }));
    }

    #[test]
    fn connector_request_rejects_unredacted_sensitive_headers() {
        let mut request = sample_connector_request();
        request.headers.insert(
            "Authorization".to_string(),
            RedactedHeaderValue::Plain("Bearer abc".to_string()),
        );
        assert!(matches!(
            request.validate_redaction(),
            Err(OracleModelError::UnredactedSensitiveHeader { .. })
        ));

        request.headers.insert(
            "Authorization".to_string(),
            RedactedHeaderValue::Hashed(Hash::new(b"Bearer abc")),
        );
        assert_eq!(Ok(()), request.validate_redaction());
    }

    #[test]
    fn keyed_hash_verifies_payloads() {
        let keyed = KeyedHash::new("pepper-1", b"pepper", b"twitter_user_123");
        assert!(keyed.verify(b"pepper", b"twitter_user_123"));
        assert!(!keyed.verify(b"pepper", b"other"));
        assert!(!keyed.verify(b"wrong-pepper", b"twitter_user_123"));
    }

    #[test]
    fn hashed_identifiers_map_to_stable_values() {
        let uaid_value = ObservationValue::from_uaid(&sample_social_uaid());
        assert_eq!(uaid_value.scale, 0);
        assert_eq!(
            uaid_value.mantissa,
            170_052_220_750_163_104_028_821_061_988_450_963_712_i128
        );

        let keyed = KeyedHash::new("pepper-1", b"pepper", b"twitter_user_123");
        let keyed_value = ObservationValue::from_keyed_hash(&keyed);
        assert_eq!(keyed_value.scale, 0);
        assert!(keyed_value.mantissa >= 0);
    }

    #[test]
    #[cfg(feature = "json")]
    #[allow(clippy::too_many_lines)]
    fn oracle_fixtures_round_trip() {
        let expected_feed_config = sample_feed_config();
        let expected_social_feed_config = sample_social_feed_config();
        let expected_connector_request = sample_connector_request();
        let expected_social_connector_request = sample_social_connector_request();
        let expected_observation = sample_observation();
        let expected_social_observation = sample_social_observation();
        let expected_report = sample_report();
        let expected_social_report = sample_social_report();
        let expected_event = sample_feed_event();
        let expected_social_event = sample_social_feed_event();

        let feed_config: FeedConfig = json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_config_price_xor_usd.json"
        ))
        .expect("feed config fixture");
        let social_feed_config: FeedConfig = json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_config_social_follow.json"
        ))
        .expect("social feed config fixture");
        let connector_request: ConnectorRequest = json::from_str(include_str!(
            "../../../../fixtures/oracle/connector_request_price_xor_usd.json"
        ))
        .expect("connector request fixture");
        let social_connector_request: ConnectorRequest = json::from_str(include_str!(
            "../../../../fixtures/oracle/connector_request_social_follow.json"
        ))
        .expect("social connector request fixture");
        let observation: Observation = json::from_str(include_str!(
            "../../../../fixtures/oracle/observation_price_xor_usd.json"
        ))
        .expect("observation fixture");
        let social_observation: Observation = json::from_str(include_str!(
            "../../../../fixtures/oracle/observation_social_follow.json"
        ))
        .expect("social observation fixture");
        let report: Report = json::from_str(include_str!(
            "../../../../fixtures/oracle/report_price_xor_usd.json"
        ))
        .expect("report fixture");
        let social_report: Report = json::from_str(include_str!(
            "../../../../fixtures/oracle/report_social_follow.json"
        ))
        .expect("social report fixture");
        let event: FeedEvent = json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_event_price_xor_usd.json"
        ))
        .expect("feed event fixture");
        let social_event: FeedEvent = json::from_str(include_str!(
            "../../../../fixtures/oracle/feed_event_social_follow.json"
        ))
        .expect("social feed event fixture");

        if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {
            println!("price_feed_config={}", pretty_json(&expected_feed_config));
            println!(
                "price_connector_request={}",
                pretty_json(&expected_connector_request)
            );
            println!("price_observation={}", pretty_json(&expected_observation));
            println!("price_report={}", pretty_json(&expected_report));
            println!("price_event={}", pretty_json(&expected_event));
            println!(
                "social_feed_config={}",
                pretty_json(&expected_social_feed_config)
            );
            println!(
                "social_connector_request={}",
                pretty_json(&expected_social_connector_request)
            );
            println!(
                "social_observation={}",
                pretty_json(&expected_social_observation)
            );
            println!("social_report={}", pretty_json(&expected_social_report));
            println!("social_event={}", pretty_json(&expected_social_event));
        }

        assert_eq!(expected_feed_config, feed_config);
        assert_eq!(expected_social_feed_config, social_feed_config);
        assert_eq!(expected_connector_request, connector_request);
        assert_eq!(expected_social_connector_request, social_connector_request);
        assert_eq!(expected_observation, observation);
        assert_eq!(expected_social_observation, social_observation);
        assert_eq!(expected_report, report);
        assert_eq!(expected_social_report, social_report);
        assert_eq!(expected_event, event);
        assert_eq!(expected_social_event, social_event);

        let encoded_feed_config =
            json::to_value(&feed_config).expect("feed config encodes to JSON");
        let decoded_feed_config: FeedConfig =
            json::from_value(encoded_feed_config.clone()).expect("feed config decodes from JSON");
        assert_eq!(decoded_feed_config, feed_config);

        let encoded_observation =
            json::to_value(&observation).expect("observation encodes to JSON");
        let decoded_observation: Observation =
            json::from_value(encoded_observation.clone()).expect("observation decodes from JSON");
        assert_eq!(decoded_observation, observation);

        let encoded_report = json::to_value(&report).expect("report encodes to JSON");
        let decoded_report: Report =
            json::from_value(encoded_report.clone()).expect("report decodes from JSON");
        assert_eq!(decoded_report, report);

        let encoded_event = json::to_value(&event).expect("event encodes to JSON");
        let decoded_event: FeedEvent =
            json::from_value(encoded_event.clone()).expect("event decodes from JSON");
        assert_eq!(decoded_event, event);

        let encoded_social_feed_config =
            json::to_value(&social_feed_config).expect("social feed config encodes to JSON");
        let decoded_social_feed_config: FeedConfig =
            json::from_value(encoded_social_feed_config.clone())
                .expect("social feed config decodes from JSON");
        assert_eq!(decoded_social_feed_config, social_feed_config);

        let encoded_social_observation =
            json::to_value(&social_observation).expect("social observation encodes to JSON");
        let decoded_social_observation: Observation =
            json::from_value(encoded_social_observation.clone())
                .expect("social observation decodes from JSON");
        assert_eq!(decoded_social_observation, social_observation);

        let encoded_social_report =
            json::to_value(&social_report).expect("social report encodes to JSON");
        let decoded_social_report: Report =
            json::from_value(encoded_social_report.clone()).expect("social report decodes");
        assert_eq!(decoded_social_report, social_report);

        let encoded_social_event =
            json::to_value(&social_event).expect("social event encodes to JSON");
        let decoded_social_event: FeedEvent =
            json::from_value(encoded_social_event.clone()).expect("social event decodes");
        assert_eq!(decoded_social_event, social_event);

        let encoded_social_connector =
            json::to_value(&social_connector_request).expect("social connector encodes to JSON");
        let decoded_social_connector: ConnectorRequest =
            json::from_value(encoded_social_connector.clone())
                .expect("social connector decodes from JSON");
        assert_eq!(decoded_social_connector, social_connector_request);
    }

    #[test]
    fn rejection_code_catalog_is_unique_and_descriptive() {
        use std::collections::BTreeSet;

        let mut seen = BTreeSet::new();
        for code in OracleRejectionCode::all() {
            assert!(
                seen.insert(code.as_code()),
                "duplicate rejection code {}",
                code.as_code()
            );
            assert!(
                matches!(code.category(), "model" | "aggregation"),
                "unexpected category for {}",
                code.as_code()
            );
            assert!(
                !code.description().is_empty(),
                "missing description for {}",
                code.as_code()
            );
        }

        let model_err = OracleModelError::FeedVersionMismatch {
            expected: FeedConfigVersion(1),
            provided: FeedConfigVersion(2),
        };
        let agg_err = OracleAggregationError::SlotMismatch {
            expected: 10,
            provided: 11,
        };

        assert_eq!(
            OracleRejectionCode::from(&model_err).as_code(),
            "oracle_model_feed_version_mismatch"
        );
        assert_eq!(
            OracleRejectionCode::from(&agg_err).as_code(),
            "oracle_agg_slot_mismatch"
        );
    }

    #[test]
    fn oracle_abi_hash_is_stable() {
        // The hash guards schema changes for the oracle message surface.
        let digest = oracle_abi_hash();
        assert_eq!(
            digest.to_string(),
            "25d675ad1e61609f0b5951743ecac11ae06ba3df9e4aaecdf10db61c2dd9507b",
            "update the golden value if the schema intentionally changes"
        );
    }

    #[test]
    fn oracle_change_stage_order_and_next_are_stable() {
        let stages = [
            OracleChangeStage::Intake,
            OracleChangeStage::RulesCommittee,
            OracleChangeStage::CopReview,
            OracleChangeStage::TechnicalAudit,
            OracleChangeStage::PolicyJury,
            OracleChangeStage::Enactment,
        ];
        let orders: Vec<_> = stages.iter().map(|stage| stage.order()).collect();
        assert_eq!(orders, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(
            OracleChangeStage::Intake.next(),
            Some(OracleChangeStage::RulesCommittee)
        );
        assert_eq!(
            OracleChangeStage::RulesCommittee.next(),
            Some(OracleChangeStage::CopReview)
        );
        assert!(OracleChangeStage::Enactment.next().is_none());
    }

    #[test]
    fn oracle_change_required_votes_follow_class() {
        use nonzero_ext::nonzero;

        let low = nonzero!(1_usize);
        let medium = nonzero!(2_usize);
        let high = nonzero!(3_usize);

        assert_eq!(
            OracleChangeClass::Low.required_votes(low, medium, high),
            low
        );
        assert_eq!(
            OracleChangeClass::Medium.required_votes(low, medium, high),
            medium
        );
        assert_eq!(
            OracleChangeClass::High.required_votes(low, medium, high),
            high
        );
    }

    #[test]
    fn oracle_change_current_stage_tracks_progression() {
        let mut proposal = OracleChangeProposal {
            id: OracleChangeId(Hash::new(b"proposal")),
            feed: sample_feed_config(),
            class: OracleChangeClass::Medium,
            payload_hash: Hash::new(b"payload"),
            proposer: oracle("alice", "validators"),
            created_at: 5,
            evidence: Vec::new(),
            stages: vec![
                OracleChangeStageRecord {
                    stage: OracleChangeStage::Intake,
                    approvals: BTreeSet::new(),
                    rejections: BTreeSet::new(),
                    evidence: Vec::new(),
                    started_at: Some(5),
                    deadline: Some(7),
                    completed_at: None,
                    failure: None,
                },
                OracleChangeStageRecord {
                    stage: OracleChangeStage::RulesCommittee,
                    approvals: BTreeSet::new(),
                    rejections: BTreeSet::new(),
                    evidence: Vec::new(),
                    started_at: None,
                    deadline: None,
                    completed_at: None,
                    failure: None,
                },
            ],
            status: OracleChangeStatus::Pending,
        };

        assert_eq!(
            proposal.current_stage().map(|stage| stage.stage),
            Some(OracleChangeStage::Intake)
        );

        proposal.stages[0].completed_at = Some(6);
        proposal.stages[1].started_at = Some(6);
        proposal.stages[1].deadline = Some(9);

        assert_eq!(
            proposal.current_stage().map(|stage| stage.stage),
            Some(OracleChangeStage::RulesCommittee)
        );

        proposal.stages[1].failure = Some(OracleChangeStageFailure::Rejected);

        assert_eq!(
            proposal.current_stage().map(|stage| stage.stage),
            Some(OracleChangeStage::RulesCommittee)
        );
    }
}
