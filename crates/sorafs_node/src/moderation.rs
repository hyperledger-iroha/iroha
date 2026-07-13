//! Local SoraFS moderation ballot lifecycle runtime.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

use iroha_data_model::sorafs::moderation::{
    AdversarialCorpusManifestV1, ModerationReproManifestV1, SoraFsModerationBallotCommitV1,
    SoraFsModerationBallotContextV1, SoraFsModerationBallotError, SoraFsModerationBallotRevealV1,
    SoraFsModerationVoteChoice,
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
    SoraFsModerationBallotGovernanceChallengeDecisionV1,
    SoraFsModerationBallotGovernanceChallengeKindV1, SoraFsModerationBallotGovernanceChallengeV1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
    SoraFsModerationVoteCountsV1, XorQuantity,
};
use thiserror::Error;

const MODERATION_ROSTER_HASH_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.panel-roster-hash.v1";
const MODERATION_BALLOT_NO_SHOW_PENALTY_PLAN_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.ballot-no-show-penalty-plan.v1";
const MODERATION_SCREENING_RECORD_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.screening-record.v1";
const MODERATION_QUARANTINE_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-record.v1";
const MODERATION_QUARANTINE_OBJECT_ID_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object-id.v1";
const MODERATION_QUARANTINE_OBJECT_NONCE_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object-nonce.v1";
const MODERATION_QUARANTINE_OBJECT_KEY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object-key-id.v1";
const MODERATION_QUARANTINE_OBJECT_ENCRYPTION_KEY_DOMAIN_V1: &str =
    "sorafs.moderation.local.quarantine-object.encryption-key.v1";
const MODERATION_QUARANTINE_OBJECT_AUTH_KEY_DOMAIN_V1: &str =
    "sorafs.moderation.local.quarantine-object.auth-key.v1";
const MODERATION_QUARANTINE_OBJECT_KEYSTREAM_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object.keystream.v1";
const MODERATION_QUARANTINE_OBJECT_AUTH_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object.auth.v1";
const MODERATION_EVIDENCE_VIEWER_SESSION_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.evidence-viewer-session.v1";
const MODERATION_EVIDENCE_VIEWER_ACCESS_EVENT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.evidence-viewer-access-event.v1";
const MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.evidence-viewer-audit-report.v1";
const MODERATION_EVIDENCE_VIEWER_AUDIT_DIGEST_SET_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.evidence-viewer-audit-digest-set.v1";
const MODERATION_EVIDENCE_VIEWER_MAX_SESSION_TTL_MS: u64 = 15 * 60 * 1_000;
const MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_MAX_WINDOW_SECS: u64 = 24 * 60 * 60;
pub(crate) const MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_VERSION_V1: u16 = 1;
pub(crate) const MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1: u16 = 1;
pub(crate) const MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1: &str = "blake3-xof-local-seal-v1";
pub(crate) const MODERATION_QUARANTINE_OBJECTS_DIR: &str = "objects";
pub(crate) const MODERATION_QUARANTINE_OBJECT_EXT: &str = "qobj";

/// Derive the local moderation panel roster hash for an ordered juror roster.
///
/// The local runtime includes the quorum and ordered juror identifiers so
/// replayed announcements cannot silently swap failover order or quorum policy.
#[must_use]
pub fn local_moderation_panel_roster_hash(juror_ids: &[String], quorum: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_ROSTER_HASH_DOMAIN_V1);
    hasher.update(&quorum.to_le_bytes());
    hasher.update(&(juror_ids.len() as u64).to_le_bytes());
    for juror_id in juror_ids {
        hasher.update(&(juror_id.len() as u64).to_le_bytes());
        hasher.update(juror_id.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Local moderation ballot announcement accepted by the lifecycle runtime.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotAnnouncement {
    /// Immutable case context bound by every commit and reveal.
    pub context: SoraFsModerationBallotContextV1,
    /// Confirmed appeal deposit asset-lock id that admitted this ballot.
    pub appeal_deposit_escrow_id_hex: Option<String>,
    /// Confirmed appeal deposit metadata used to publish tally finance reports.
    pub appeal_deposit: Option<ModerationAppealDeposit>,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Ordered juror identifiers eligible to participate in this ballot.
    pub juror_ids: Vec<String>,
    /// Minimum number of valid reveals required before a tally can finalize.
    pub quorum: u16,
    /// UTC timestamp (milliseconds) when the ballot was announced.
    pub announced_at_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which commits are accepted.
    pub commit_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) for the post-commit challenge buffer.
    pub challenge_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which reveals are accepted.
    pub reveal_deadline_unix_ms: u64,
}

/// Confirmed appeal deposit metadata captured at moderation intake.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationAppealDeposit {
    /// Confirmed native asset-lock escrow id.
    pub escrow_id_hex: String,
    /// Canonical account that funded the deposit lock.
    pub payer_account: String,
    /// Canonical account receiving non-refunded deposit drawdowns.
    pub destination_account: String,
    /// Optional canonical account required to approve drawdowns.
    pub release_authority_account: Option<String>,
    /// Canonical asset definition held by the lock.
    pub asset_definition_id: String,
    /// Deterministic native asset-lock custody account.
    pub custody_account: String,
    /// Exact non-negative deposited XOR decimal amount.
    pub deposit_xor: XorQuantity,
    /// Optional lock expiry timestamp in Unix milliseconds.
    pub expires_at_ms: Option<u64>,
    /// Client-supplied idempotency key used to derive the escrow id.
    pub idempotency_key: String,
    /// Canonical lowercase evidence hashes used to derive the escrow id.
    pub evidence_hashes_hex: Vec<String>,
}

/// Local moderation ballot challenge type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationBallotChallengeKind {
    /// The announced panel roster or roster hash is disputed.
    RosterMismatch,
    /// A duplicate commitment attempt or duplicate-commit evidence is disputed.
    DuplicateCommit,
    /// A commitment or reveal is alleged to be bound to the wrong payload.
    PayloadMismatch,
    /// A juror eligibility or authority assertion is disputed.
    JurorEligibility,
    /// The evidence bundle or policy binding is disputed.
    EvidenceMismatch,
    /// Operator-reviewed challenge category outside the fixed local labels.
    Other,
}

impl ModerationBallotChallengeKind {
    fn requires_target_juror(self) -> bool {
        matches!(
            self,
            Self::DuplicateCommit | Self::PayloadMismatch | Self::JurorEligibility
        )
    }
}

/// Local moderation ballot challenge decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationBallotChallengeDecision {
    /// The challenge was rejected and the ballot may continue.
    Rejected,
    /// The challenge was accepted and higher-level dispute handling must resolve the ballot.
    Accepted,
}

/// Input used to raise a local moderation ballot challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotChallengeInput {
    /// Operator- or juror-supplied challenge id unique within the ballot.
    pub challenge_id: String,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Canonical account or service identifier raising the challenge.
    pub challenger_id: String,
    /// Challenge category.
    pub kind: ModerationBallotChallengeKind,
    /// Juror targeted by the challenge, when the category requires one.
    pub target_juror_id: Option<String>,
    /// Digest of the payload-free challenge evidence packet.
    pub evidence_digest: [u8; 32],
    /// Payload-free operator-readable reason label.
    pub reason: String,
}

/// Input used to resolve a local moderation ballot challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotChallengeResolution {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Challenge id being resolved.
    pub challenge_id: String,
    /// Canonical account or service identifier resolving the challenge.
    pub resolved_by: String,
    /// Resolution decision.
    pub decision: ModerationBallotChallengeDecision,
    /// Optional payload-free resolution note.
    pub note: Option<String>,
}

/// Durable local moderation ballot challenge record.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotChallengeRecord {
    /// Challenge id unique within the ballot.
    pub challenge_id: String,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Canonical account or service identifier raising the challenge.
    pub challenger_id: String,
    /// Challenge category.
    pub kind: ModerationBallotChallengeKind,
    /// Juror targeted by the challenge, when any.
    pub target_juror_id: Option<String>,
    /// Digest of the payload-free challenge evidence packet.
    pub evidence_digest: [u8; 32],
    /// Payload-free operator-readable reason label.
    pub reason: String,
    /// UTC timestamp (milliseconds) when the challenge was raised.
    pub raised_at_unix_ms: u64,
    /// Resolution decision, when reviewed.
    pub decision: Option<ModerationBallotChallengeDecision>,
    /// Canonical account or service identifier that resolved the challenge.
    pub resolved_by: Option<String>,
    /// UTC timestamp (milliseconds) when the challenge was resolved.
    pub resolved_at_unix_ms: Option<u64>,
    /// Optional payload-free resolution note.
    pub resolution_note: Option<String>,
}

impl From<ModerationBallotChallengeKind> for SoraFsModerationBallotGovernanceChallengeKindV1 {
    fn from(kind: ModerationBallotChallengeKind) -> Self {
        match kind {
            ModerationBallotChallengeKind::RosterMismatch => Self::RosterMismatch,
            ModerationBallotChallengeKind::DuplicateCommit => Self::DuplicateCommit,
            ModerationBallotChallengeKind::PayloadMismatch => Self::PayloadMismatch,
            ModerationBallotChallengeKind::JurorEligibility => Self::JurorEligibility,
            ModerationBallotChallengeKind::EvidenceMismatch => Self::EvidenceMismatch,
            ModerationBallotChallengeKind::Other => Self::Other,
        }
    }
}

impl From<ModerationBallotChallengeDecision>
    for SoraFsModerationBallotGovernanceChallengeDecisionV1
{
    fn from(decision: ModerationBallotChallengeDecision) -> Self {
        match decision {
            ModerationBallotChallengeDecision::Rejected => Self::Rejected,
            ModerationBallotChallengeDecision::Accepted => Self::Accepted,
        }
    }
}

impl From<&ModerationBallotChallengeRecord> for SoraFsModerationBallotGovernanceChallengeV1 {
    fn from(record: &ModerationBallotChallengeRecord) -> Self {
        Self {
            challenge_id: record.challenge_id.clone(),
            case_id: record.case_id.clone(),
            round_id: record.round_id.clone(),
            challenger_id: record.challenger_id.clone(),
            kind: record.kind.into(),
            target_juror_id: record.target_juror_id.clone(),
            evidence_digest: record.evidence_digest,
            reason: record.reason.clone(),
            raised_at_unix_ms: record.raised_at_unix_ms,
            decision: record.decision.map(Into::into),
            resolved_by: record.resolved_by.clone(),
            resolved_at_unix_ms: record.resolved_at_unix_ms,
            resolution_note: record.resolution_note.clone(),
        }
    }
}

/// Local registry record for an admitted moderation reproducibility manifest.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationReproRegistryRecord {
    /// Referenced moderation committee manifest id.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the manifest payload recorded in the manifest body.
    pub manifest_digest: [u8; 32],
    /// BLAKE3 digest of the compiled deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Runner version string recorded in the manifest body.
    pub runtime_version: String,
    /// Unix timestamp (seconds) when the manifest was issued.
    pub issued_at_unix: u64,
    /// Number of model fingerprint entries covered by the manifest.
    pub model_count: u32,
    /// Number of validated signer entries on the manifest.
    pub signer_count: u32,
}

/// Local registry record for an admitted adversarial corpus manifest.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationCorpusRegistryRecord {
    /// BLAKE3 digest of the canonical Norito corpus manifest bytes.
    pub corpus_digest: [u8; 32],
    /// Unix timestamp (seconds) when the corpus manifest was assembled.
    pub issued_at_unix: u64,
    /// Optional cohort label for the calibration window.
    pub cohort_label: Option<String>,
    /// Number of perceptual families in the corpus.
    pub family_count: u32,
    /// Number of variants across all corpus families.
    pub variant_count: u32,
}

/// Snapshot of the local SoraFS moderation model registry.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationModelRegistrySnapshot {
    /// Admitted reproducibility manifests sorted by manifest id.
    pub reproducibility_manifests: Vec<ModerationReproRegistryRecord>,
    /// Admitted adversarial corpus manifests sorted by corpus digest.
    pub adversarial_corpora: Vec<ModerationCorpusRegistryRecord>,
}

/// Local SFM-4a screening verdict.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
#[repr(u8)]
pub enum ModerationScreeningVerdict {
    /// Screening passed without additional moderation action.
    Pass = 1,
    /// Screening passed with a warning or low-severity policy hit.
    Warn = 2,
    /// Content must be quarantined for operator review.
    Quarantine = 3,
    /// Content must be escalated to a moderation panel or appeal workflow.
    Escalate = 4,
    /// Content must be blocked outright.
    Block = 5,
}

impl ModerationScreeningVerdict {
    /// Stable lower-case label used in JSON and digest material.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Quarantine => "quarantine",
            Self::Escalate => "escalate",
            Self::Block => "block",
        }
    }

    fn requires_quarantine_record(self) -> bool {
        matches!(self, Self::Quarantine | Self::Escalate)
    }
}

/// Candidate local screening result to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationScreeningInput {
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the screened payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Governance-approved reproducibility manifest id used for screening.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Combined moderation score in basis points.
    pub combined_score_bps: u16,
    /// Screening verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when screening completed.
    pub screened_at_unix: u64,
    /// Optional digest of evidence material retained outside this local index.
    pub evidence_digest: Option<[u8; 32]>,
    /// Optional digest of the policy/configuration used for the run.
    pub policy_digest: Option<[u8; 32]>,
    /// Optional operator note.
    pub notes: Option<String>,
}

/// Persisted local SFM-4a screening result.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningRecord {
    /// Stable record id derived from the deterministic record digest.
    pub record_id: [u8; 16],
    /// BLAKE3 digest over canonical screening record fields.
    pub record_digest: [u8; 32],
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the screened payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Governance-approved reproducibility manifest id used for screening.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Combined moderation score in basis points.
    pub combined_score_bps: u16,
    /// Screening verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when screening completed.
    pub screened_at_unix: u64,
    /// Optional digest of evidence material retained outside this local index.
    pub evidence_digest: Option<[u8; 32]>,
    /// Optional digest of the policy/configuration used for the run.
    pub policy_digest: Option<[u8; 32]>,
    /// Optional operator note.
    pub notes: Option<String>,
}

/// Local state of a quarantined screening record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(u8)]
pub enum ModerationQuarantineState {
    /// Awaiting operator or panel review.
    PendingReview = 1,
    /// Reviewed by an operator and ready for a release decision.
    Reviewed = 2,
    /// Released by an authorized operator after review.
    Released = 3,
}

impl ModerationQuarantineState {
    /// Stable lower-case label used in JSON.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingReview => "pending_review",
            Self::Reviewed => "reviewed",
            Self::Released => "released",
        }
    }
}

/// Local review action for a quarantined screening record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineReviewInput {
    /// Stable quarantine id to review.
    pub quarantine_id: [u8; 16],
    /// Canonical operator or panel member that reviewed the record.
    pub reviewed_by: String,
    /// Unix timestamp (seconds) when review completed.
    pub reviewed_at_unix: u64,
    /// Optional review note.
    pub notes: Option<String>,
}

/// Local release action for a reviewed quarantine record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineReleaseInput {
    /// Stable quarantine id to release.
    pub quarantine_id: [u8; 16],
    /// Canonical operator or release authority approving release.
    pub release_authority: String,
    /// Unix timestamp (seconds) when release completed.
    pub released_at_unix: u64,
    /// Optional release note.
    pub notes: Option<String>,
}

/// Persisted local quarantine queue record derived from a screening result.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationQuarantineRecord {
    /// Stable quarantine id derived from screening id and subject digest.
    pub quarantine_id: [u8; 16],
    /// Screening record that produced this queue entry.
    pub screening_record_id: [u8; 16],
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the quarantined payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Triggering verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when the record entered local quarantine.
    pub queued_at_unix: u64,
    /// Local review state.
    pub state: ModerationQuarantineState,
    /// Unix timestamp (seconds) when local review completed.
    pub reviewed_at_unix: Option<u64>,
    /// Canonical operator or panel member that reviewed the record.
    pub reviewed_by: Option<String>,
    /// Optional local review note.
    pub review_notes: Option<String>,
    /// Unix timestamp (seconds) when the local record was released.
    pub released_at_unix: Option<u64>,
    /// Canonical operator or authority that approved release.
    pub release_authority: Option<String>,
    /// Optional local release note.
    pub release_notes: Option<String>,
}

/// Candidate quarantined payload bytes to store in the local encrypted object store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineObjectInput {
    /// Stable quarantine id that owns this payload object.
    pub quarantine_id: [u8; 16],
    /// Plaintext payload bytes to seal locally.
    pub payload: Vec<u8>,
    /// Unix timestamp (seconds) when the payload was captured.
    pub captured_at_unix: u64,
    /// Optional media/content type label for operator review.
    pub content_type: Option<String>,
    /// Optional object-store note.
    pub notes: Option<String>,
}

/// Persisted index record for one encrypted local quarantine payload object.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationQuarantineObjectRecord {
    /// Stable quarantine id that owns this payload object.
    pub quarantine_id: [u8; 16],
    /// Stable object id derived from the sealed payload metadata and ciphertext digest.
    pub object_id: [u8; 16],
    /// BLAKE3 digest of the plaintext payload.
    pub payload_digest: [u8; 32],
    /// BLAKE3 digest of the ciphertext payload bytes.
    pub ciphertext_digest: [u8; 32],
    /// Plaintext payload length in bytes.
    pub payload_len: u64,
    /// Unix timestamp (seconds) when the payload was captured.
    pub captured_at_unix: u64,
    /// Optional media/content type label for operator review.
    pub content_type: Option<String>,
    /// Optional object-store note.
    pub notes: Option<String>,
    /// Local at-rest encryption algorithm label.
    pub encryption_algorithm: String,
    /// Stable key identifier derived from the node-local sealing key.
    pub key_id: [u8; 16],
    /// Relative path of the Norito object envelope under the object-store root.
    pub envelope_path: String,
}

/// Decrypted local quarantine object payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineObjectPayload {
    /// Persisted object index record.
    pub record: ModerationQuarantineObjectRecord,
    /// Decrypted payload bytes.
    pub payload: Vec<u8>,
}

/// Snapshot of local encrypted quarantine object index records.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationQuarantineObjectSnapshot {
    /// Object records sorted by quarantine id.
    pub objects: Vec<ModerationQuarantineObjectRecord>,
}

/// Payload-free local evidence viewer session admission request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvidenceViewerSessionInput {
    /// Stable quarantine id whose sealed object will be reviewed.
    pub quarantine_id: [u8; 16],
    /// Canonical operator or service account that requested the viewer session.
    pub requested_by: String,
    /// Canonical juror, auditor, or legal reviewer account represented by the session.
    pub viewer_account: String,
    /// Role label scoped to this evidence review session.
    pub viewer_role: String,
    /// Human-readable purpose bound to the audit manifest.
    pub purpose: String,
    /// Digest of the device/user attestation transcript.
    pub attestation_digest: [u8; 32],
    /// Digest of the per-session watermark metadata.
    pub watermark_metadata_digest: [u8; 32],
    /// Digest of the session nonce or runtime-only token preimage.
    pub session_nonce_digest: [u8; 32],
    /// Unix timestamp (milliseconds) when the session starts.
    pub issued_at_unix_ms: u64,
    /// Unix timestamp (milliseconds) when the session expires.
    pub expires_at_unix_ms: u64,
    /// Optional legal-hold or retention receipt identifier.
    pub legal_hold_id: Option<String>,
    /// Optional operator note.
    pub notes: Option<String>,
    /// Whether raw evidence bytes were included in the request.
    pub raw_evidence_included: bool,
    /// Whether a signed URL was included in the request.
    pub signed_url_included: bool,
    /// Whether a runtime session token was included in the request.
    pub session_token_included: bool,
    /// Whether watermark secret material was included in the request.
    pub watermark_secret_included: bool,
}

/// Append-only local evidence viewer access event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationEvidenceViewerAccessKind {
    /// The viewer opened or resumed evidence playback.
    Viewed,
    /// The viewer sought to another playback position or page.
    Seeked,
    /// The viewer paused playback.
    Paused,
    /// The viewer attempted a screenshot or screen capture.
    ScreenshotAttempted,
    /// The viewer attempted to download evidence.
    DownloadAttempted,
    /// The viewer created or updated an annotation.
    Annotated,
    /// The viewer service observed access after session expiry.
    SessionExpired,
    /// The viewer service rejected access because attestation failed.
    AttestationFailed,
}

impl ModerationEvidenceViewerAccessKind {
    /// Stable lower-case JSON label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewed => "viewed",
            Self::Seeked => "seeked",
            Self::Paused => "paused",
            Self::ScreenshotAttempted => "screenshot_attempted",
            Self::DownloadAttempted => "download_attempted",
            Self::Annotated => "annotated",
            Self::SessionExpired => "session_expired",
            Self::AttestationFailed => "attestation_failed",
        }
    }

    fn is_expiry_event(self) -> bool {
        matches!(self, Self::SessionExpired)
    }
}

/// Payload-free local evidence viewer access-log append request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvidenceViewerAccessInput {
    /// Session id that owns the access event.
    pub session_id: [u8; 16],
    /// Access event kind.
    pub kind: ModerationEvidenceViewerAccessKind,
    /// Canonical viewer account represented by the event.
    pub actor_account: String,
    /// Unix timestamp (milliseconds) when the event occurred.
    pub event_at_unix_ms: u64,
    /// Digest of the request metadata or interaction envelope.
    pub request_digest: [u8; 32],
    /// Optional digest of event-specific metadata such as seek range or annotation text.
    pub event_metadata_digest: Option<[u8; 32]>,
    /// Optional event note.
    pub notes: Option<String>,
    /// Whether raw evidence bytes were included in the event.
    pub raw_evidence_included: bool,
    /// Whether a signed URL was included in the event.
    pub signed_url_included: bool,
    /// Whether a runtime session token was included in the event.
    pub session_token_included: bool,
    /// Whether a response body or raw access-log payload was included in the event.
    pub response_body_included: bool,
}

/// Persisted payload-free local evidence viewer session record.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationEvidenceViewerSessionRecord {
    /// Stable quarantine id whose sealed object is reviewed.
    pub quarantine_id: [u8; 16],
    /// Stable encrypted object id for the reviewed payload.
    pub object_id: [u8; 16],
    /// Stable session id derived from payload-free session metadata.
    pub session_id: [u8; 16],
    /// Digest of the quarantined evidence payload.
    pub evidence_digest: [u8; 32],
    /// Digest of the device/user attestation transcript.
    pub attestation_digest: [u8; 32],
    /// Digest of the per-session watermark metadata.
    pub watermark_metadata_digest: [u8; 32],
    /// Digest of the session nonce or runtime-only token preimage.
    pub session_nonce_digest: [u8; 32],
    /// Canonical operator or service account that requested the viewer session.
    pub requested_by: String,
    /// Canonical juror, auditor, or legal reviewer account represented by the session.
    pub viewer_account: String,
    /// Role label scoped to this evidence review session.
    pub viewer_role: String,
    /// Human-readable purpose bound to the audit manifest.
    pub purpose: String,
    /// Unix timestamp (milliseconds) when the session starts.
    pub issued_at_unix_ms: u64,
    /// Unix timestamp (milliseconds) when the session expires.
    pub expires_at_unix_ms: u64,
    /// Optional legal-hold or retention receipt identifier.
    pub legal_hold_id: Option<String>,
    /// Optional operator note.
    pub notes: Option<String>,
    /// Digest of the payload-free session manifest.
    pub session_manifest_digest: [u8; 32],
}

/// Persisted payload-free local evidence viewer access-log event.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationEvidenceViewerAccessEventRecord {
    /// Monotonic append-only event sequence.
    pub sequence: u64,
    /// Stable event id derived from payload-free event metadata.
    pub event_id: [u8; 16],
    /// Session id that owns the event.
    pub session_id: [u8; 16],
    /// Stable quarantine id for the reviewed evidence.
    pub quarantine_id: [u8; 16],
    /// Stable encrypted object id for the reviewed payload.
    pub object_id: [u8; 16],
    /// Digest of the quarantined evidence payload.
    pub evidence_digest: [u8; 32],
    /// Access event kind.
    pub kind: ModerationEvidenceViewerAccessKind,
    /// Canonical viewer account represented by the event.
    pub actor_account: String,
    /// Unix timestamp (milliseconds) when the event occurred.
    pub event_at_unix_ms: u64,
    /// Digest of the request metadata or interaction envelope.
    pub request_digest: [u8; 32],
    /// Optional digest of event-specific metadata such as seek range or annotation text.
    pub event_metadata_digest: Option<[u8; 32]>,
    /// Optional event note.
    pub notes: Option<String>,
    /// Digest of the payload-free access event.
    pub event_digest: [u8; 32],
}

/// Snapshot of local payload-free evidence viewer session and access-log state.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationEvidenceViewerSnapshot {
    /// Session records sorted by session id.
    pub sessions: Vec<ModerationEvidenceViewerSessionRecord>,
    /// Append-only access events sorted by sequence.
    pub access_events: Vec<ModerationEvidenceViewerAccessEventRecord>,
}

/// Request for a payload-free local evidence-viewer audit report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvidenceViewerAuditReportInput {
    /// Public report scope label, for example `local-daily`.
    pub report_scope: String,
    /// Inclusive Unix timestamp (seconds) at the start of the report window.
    pub window_start_unix: u64,
    /// Exclusive Unix timestamp (seconds) at the end of the report window.
    pub window_end_unix: u64,
    /// Unix timestamp (seconds) when the report was generated.
    pub generated_at_unix: u64,
    /// Optional policy/configuration digest for this report.
    pub policy_digest: Option<[u8; 32]>,
    /// Whether raw evidence bytes were included in the report request.
    pub raw_evidence_included: bool,
    /// Whether raw access-log payloads were included in the report request.
    pub raw_access_logs_included: bool,
    /// Whether raw viewer account identifiers were included in the report request.
    pub viewer_accounts_included: bool,
    /// Whether signed URLs were included in the report request.
    pub signed_urls_included: bool,
    /// Whether runtime session tokens were included in the report request.
    pub session_tokens_included: bool,
    /// Whether response bodies were included in the report request.
    pub response_bodies_included: bool,
}

/// One access-kind count in a payload-free evidence-viewer audit report.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationEvidenceViewerAuditKindCount {
    /// Stable access-kind label.
    pub kind: String,
    /// Number of matching access events in the report window.
    pub count: u64,
}

/// Payload-free local evidence-viewer access report for transparency export.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationEvidenceViewerAuditReport {
    /// Schema version; currently [`MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_VERSION_V1`].
    pub version: u16,
    /// Stable report id derived from the payload-free report body.
    pub report_id: [u8; 16],
    /// Public report scope label, for example `local-daily`.
    pub report_scope: String,
    /// Inclusive Unix timestamp (seconds) at the start of the report window.
    pub window_start_unix: u64,
    /// Exclusive Unix timestamp (seconds) at the end of the report window.
    pub window_end_unix: u64,
    /// Unix timestamp (seconds) when the report was generated.
    pub generated_at_unix: u64,
    /// Number of sessions active during the report window.
    pub session_count: u64,
    /// Number of active sessions with at least one logged access event in the report window.
    pub logged_session_count: u64,
    /// Number of access events in the report window.
    pub access_event_count: u64,
    /// Number of distinct viewer role labels represented by active sessions.
    pub unique_viewer_role_count: u64,
    /// Number of active sessions with non-zero attestation digests.
    pub attested_session_count: u64,
    /// Number of active sessions with non-zero watermark metadata digests.
    pub watermarked_session_count: u64,
    /// Number of active sessions bound to legal-hold metadata.
    pub legal_hold_bound_session_count: u64,
    /// First access-event timestamp in the report window, if any.
    pub first_event_at_unix_ms: Option<u64>,
    /// Last access-event timestamp in the report window, if any.
    pub last_event_at_unix_ms: Option<u64>,
    /// Access-event counts sorted by stable access-kind label.
    pub access_kind_counts: Vec<ModerationEvidenceViewerAuditKindCount>,
    /// Digest of the sorted unique evidence digests represented by active sessions.
    pub evidence_digest_set_digest: [u8; 32],
    /// Digest of the sorted unique session manifest digests represented by active sessions.
    pub session_manifest_digest_set_digest: [u8; 32],
    /// Digest of the sorted unique access-event digests represented by logged events.
    pub access_event_digest_set_digest: [u8; 32],
    /// Digest of the sorted unique request digests represented by logged events.
    pub request_digest_set_digest: [u8; 32],
    /// Digest of the sorted unique attestation digests represented by active sessions.
    pub attestation_digest_set_digest: [u8; 32],
    /// Digest of the sorted unique watermark metadata digests represented by active sessions.
    pub watermark_metadata_digest_set_digest: [u8; 32],
    /// Optional policy/configuration digest for this report.
    pub policy_digest: Option<[u8; 32]>,
    /// Digest of the full payload-free report body.
    pub report_digest: [u8; 32],
}

impl ModerationEvidenceViewerAuditReport {
    /// Validate the report's payload-free invariants and derived digests.
    ///
    /// # Errors
    ///
    /// Returns a human-readable validation message when report metadata is
    /// malformed, counts are inconsistent, or derived digests do not match.
    pub fn validate(&self) -> Result<(), String> {
        validate_evidence_viewer_audit_report(self)
    }
}

/// Outcome of recording one local screening result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationScreeningOutcome {
    /// Persisted screening record.
    pub record: ModerationScreeningRecord,
    /// Quarantine queue record when the verdict requires review.
    pub quarantine: Option<ModerationQuarantineRecord>,
}

/// Snapshot of local SFM-4a screening and quarantine state.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningSnapshot {
    /// Screening records sorted by record id.
    pub screening_records: Vec<ModerationScreeningRecord>,
    /// Quarantine records sorted by quarantine id.
    pub quarantine_records: Vec<ModerationQuarantineRecord>,
}

/// Error raised by the local screening/quarantine runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationScreeningError {
    /// A configured authoritative-state ceiling was reached.
    #[error("moderation screening resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// The durable screening/quarantine checkpoint could not be committed.
    #[error("moderation screening checkpoint failed: {message}")]
    Checkpoint {
        /// Persistence failure detail.
        message: String,
    },
    /// Screening input is invalid.
    #[error("invalid moderation screening input: {message}")]
    InvalidInput {
        /// Validation detail.
        message: String,
    },
    /// A screening record id collided with different content.
    #[error("moderation screening record `{record_id_hex}` conflicts with local state")]
    ConflictingRecord {
        /// Conflicting screening record id as lowercase hex.
        record_id_hex: String,
    },
    /// A requested quarantine queue record does not exist.
    #[error("moderation quarantine record `{quarantine_id_hex}` is unknown")]
    UnknownQuarantine {
        /// Unknown quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// A quarantine state transition was invalid.
    #[error("moderation quarantine record `{quarantine_id_hex}` transition is invalid: {message}")]
    InvalidTransition {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
        /// Transition validation detail.
        message: String,
    },
    /// A restored snapshot is internally inconsistent.
    #[error("moderation screening snapshot is invalid: {message}")]
    InvalidSnapshot {
        /// Validation detail.
        message: String,
    },
    /// The local screening runtime lock was poisoned.
    #[error("moderation screening state lock poisoned")]
    StateLockPoisoned,
}

/// Error raised by the local encrypted quarantine object store.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationQuarantineObjectError {
    /// A configured authoritative-state ceiling was reached.
    #[error("moderation quarantine object resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// The local SoraFS storage backend is disabled.
    #[error("SoraFS moderation quarantine object store is disabled")]
    StorageDisabled,
    /// Object input or checkpoint data is invalid.
    #[error("invalid moderation quarantine object input: {message}")]
    InvalidInput {
        /// Validation detail.
        message: String,
    },
    /// The referenced quarantine queue record does not exist.
    #[error("moderation quarantine record `{quarantine_id_hex}` is unknown")]
    UnknownQuarantine {
        /// Unknown quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// A requested encrypted object does not exist locally.
    #[error("moderation quarantine object for `{quarantine_id_hex}` is missing")]
    MissingObject {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// Plaintext bytes do not match the quarantine record subject digest.
    #[error(
        "moderation quarantine object digest mismatch for `{quarantine_id_hex}`: expected {expected_digest_hex}, got {actual_digest_hex}"
    )]
    DigestMismatch {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
        /// Expected lowercase BLAKE3 digest.
        expected_digest_hex: String,
        /// Actual lowercase BLAKE3 digest.
        actual_digest_hex: String,
    },
    /// The object index already contains a different record for this quarantine id.
    #[error("moderation quarantine object for `{quarantine_id_hex}` conflicts with local index")]
    ConflictingObject {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// The encrypted envelope failed authentication.
    #[error("moderation quarantine object `{quarantine_id_hex}` failed authentication")]
    AuthenticationFailed {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// The object index checkpoint is internally inconsistent.
    #[error("moderation quarantine object snapshot is invalid: {message}")]
    InvalidSnapshot {
        /// Validation detail.
        message: String,
    },
    /// Filesystem operation failed.
    #[error("moderation quarantine object I/O failed at `{path}`: {message}")]
    Io {
        /// Path involved in the failed operation.
        path: String,
        /// Underlying error message.
        message: String,
    },
    /// Norito encoding or decoding failed.
    #[error("moderation quarantine object codec failed: {message}")]
    Codec {
        /// Underlying codec error message.
        message: String,
    },
    /// The local object-store lock was poisoned.
    #[error("moderation quarantine object state lock poisoned")]
    StateLockPoisoned,
}

/// Error raised by the payload-free local evidence viewer audit runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationEvidenceViewerError {
    /// A configured authoritative-state ceiling was reached.
    #[error("moderation evidence viewer resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Session or access event input is invalid.
    #[error("invalid moderation evidence viewer input: {message}")]
    InvalidInput {
        /// Validation detail.
        message: String,
    },
    /// Input attempted to include payloads, tokens, signed URLs, bodies, or secrets.
    #[error("moderation evidence viewer payload-safety violation: {message}")]
    PayloadSafetyViolation {
        /// Validation detail.
        message: String,
    },
    /// The referenced quarantine queue record does not exist.
    #[error("moderation quarantine record `{quarantine_id_hex}` is unknown")]
    UnknownQuarantine {
        /// Unknown quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// A requested encrypted quarantine object does not exist locally.
    #[error("moderation quarantine object for `{quarantine_id_hex}` is missing")]
    MissingObject {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// The session id does not exist in the local evidence viewer state.
    #[error("moderation evidence viewer session `{session_id_hex}` is unknown")]
    UnknownSession {
        /// Unknown session id as lowercase hex.
        session_id_hex: String,
    },
    /// A session id collided with different payload-free metadata.
    #[error("moderation evidence viewer session `{session_id_hex}` conflicts with local state")]
    ConflictingSession {
        /// Conflicting session id as lowercase hex.
        session_id_hex: String,
    },
    /// A non-expiry access event arrived outside the active session window.
    #[error("moderation evidence viewer session `{session_id_hex}` is not active at event time")]
    ExpiredSession {
        /// Expired or inactive session id as lowercase hex.
        session_id_hex: String,
    },
    /// A restored snapshot is internally inconsistent.
    #[error("moderation evidence viewer snapshot is invalid: {message}")]
    InvalidSnapshot {
        /// Validation detail.
        message: String,
    },
    /// The payload-free audit report could not be exported into transparency source state.
    #[error("moderation evidence viewer transparency export failed: {message}")]
    TransparencyExport {
        /// Validation or transparency-ingest detail.
        message: String,
    },
    /// Filesystem operation failed.
    #[error("moderation evidence viewer I/O failed at `{path}`: {message}")]
    Io {
        /// Path involved in the failed operation.
        path: String,
        /// Underlying error message.
        message: String,
    },
    /// Norito encoding or decoding failed.
    #[error("moderation evidence viewer codec failed: {message}")]
    Codec {
        /// Underlying codec error message.
        message: String,
    },
    /// The local evidence-viewer state lock was poisoned.
    #[error("moderation evidence viewer state lock poisoned")]
    StateLockPoisoned,
}

/// Error raised while admitting moderation model registry material.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationModelRegistryError {
    /// A configured authoritative-state ceiling was reached.
    #[error("moderation model registry resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// The durable model-registry checkpoint could not be committed.
    #[error("moderation model registry checkpoint failed: {message}")]
    Checkpoint {
        /// Persistence failure detail.
        message: String,
    },
    /// A reproducibility manifest failed canonical validation.
    #[error("moderation reproducibility manifest validation failed: {message}")]
    InvalidReproManifest {
        /// Validation detail.
        message: String,
    },
    /// An adversarial corpus manifest failed canonical validation.
    #[error("moderation adversarial corpus manifest validation failed: {message}")]
    InvalidCorpusManifest {
        /// Validation detail.
        message: String,
    },
    /// A reproducibility manifest conflicts with a previously admitted manifest id.
    #[error("moderation reproducibility manifest `{manifest_id_hex}` conflicts with registry")]
    ConflictingReproManifest {
        /// Conflicting manifest id as lowercase hex.
        manifest_id_hex: String,
    },
    /// A persisted registry snapshot is not internally consistent.
    #[error("moderation model registry snapshot is invalid: {message}")]
    InvalidRegistrySnapshot {
        /// Validation detail.
        message: String,
    },
    /// Corpus manifest canonical encoding failed.
    #[error("failed to encode moderation adversarial corpus manifest: {message}")]
    CorpusEncoding {
        /// Encoding detail.
        message: String,
    },
    /// The local moderation model registry lock was poisoned.
    #[error("moderation model registry state lock poisoned")]
    StateLockPoisoned,
}

/// Local in-memory registry for moderation model release artifacts.
#[derive(Debug)]
pub(crate) struct ModerationModelRegistry {
    repro_manifests: BTreeMap<[u8; 16], ModerationReproRegistryRecord>,
    corpora: BTreeMap<[u8; 32], ModerationCorpusRegistryRecord>,
    entry_limit: usize,
}

impl Default for ModerationModelRegistry {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl ModerationModelRegistry {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            repro_manifests: BTreeMap::new(),
            corpora: BTreeMap::new(),
            entry_limit: entry_limit.max(1),
        }
    }

    pub(crate) fn admit_repro_manifest(
        &mut self,
        manifest: ModerationReproManifestV1,
    ) -> Result<ModerationReproRegistryRecord, ModerationModelRegistryError> {
        let summary = manifest.validate().map_err(|err| {
            ModerationModelRegistryError::InvalidReproManifest {
                message: err.to_string(),
            }
        })?;
        let record = ModerationReproRegistryRecord {
            manifest_id: summary.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            runtime_version: manifest.body.runtime_version,
            issued_at_unix: summary.issued_at_unix,
            model_count: summary.model_count,
            signer_count: summary.signer_count,
        };
        match self.repro_manifests.get(&record.manifest_id) {
            Some(existing) if existing != &record => {
                Err(ModerationModelRegistryError::ConflictingReproManifest {
                    manifest_id_hex: hex::encode(record.manifest_id),
                })
            }
            Some(existing) => Ok(existing.clone()),
            None => {
                if self.repro_manifests.len() >= self.entry_limit {
                    return Err(ModerationModelRegistryError::ResourceExhausted {
                        resource: "reproducibility_manifests",
                        limit: self.entry_limit,
                    });
                }
                self.repro_manifests
                    .insert(record.manifest_id, record.clone());
                Ok(record)
            }
        }
    }

    pub(crate) fn admit_corpus_manifest(
        &mut self,
        manifest: AdversarialCorpusManifestV1,
    ) -> Result<ModerationCorpusRegistryRecord, ModerationModelRegistryError> {
        manifest
            .validate()
            .map_err(|err| ModerationModelRegistryError::InvalidCorpusManifest {
                message: err.to_string(),
            })?;
        let encoded = norito::to_bytes(&manifest).map_err(|err| {
            ModerationModelRegistryError::CorpusEncoding {
                message: err.to_string(),
            }
        })?;
        let corpus_digest = *blake3::hash(&encoded).as_bytes();
        let family_count = manifest.families.len().min(u32::MAX as usize) as u32;
        let variant_count = manifest
            .families
            .iter()
            .map(|family| family.variants.len())
            .sum::<usize>()
            .min(u32::MAX as usize) as u32;
        let record = ModerationCorpusRegistryRecord {
            corpus_digest,
            issued_at_unix: manifest.issued_at_unix,
            cohort_label: manifest.cohort_label,
            family_count,
            variant_count,
        };
        if !self.corpora.contains_key(&record.corpus_digest)
            && self.corpora.len() >= self.entry_limit
        {
            return Err(ModerationModelRegistryError::ResourceExhausted {
                resource: "adversarial_corpora",
                limit: self.entry_limit,
            });
        }
        Ok(self
            .corpora
            .entry(record.corpus_digest)
            .or_insert_with(|| record.clone())
            .clone())
    }

    pub(crate) fn snapshot(&self) -> ModerationModelRegistrySnapshot {
        ModerationModelRegistrySnapshot {
            reproducibility_manifests: self.repro_manifests.values().cloned().collect(),
            adversarial_corpora: self.corpora.values().cloned().collect(),
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationModelRegistrySnapshot,
    ) -> Result<(), ModerationModelRegistryError> {
        for (resource, count) in [
            (
                "reproducibility_manifests",
                snapshot.reproducibility_manifests.len(),
            ),
            ("adversarial_corpora", snapshot.adversarial_corpora.len()),
        ] {
            if count > self.entry_limit {
                return Err(ModerationModelRegistryError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }
        let mut repro_manifests = BTreeMap::new();
        for record in snapshot.reproducibility_manifests {
            if record.runtime_version.trim().is_empty() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has an empty runtime version",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if record.model_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has no model fingerprints",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if record.signer_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has no governance signers",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if repro_manifests.insert(record.manifest_id, record).is_some() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: "duplicate reproducibility manifest id".to_string(),
                });
            }
        }

        let mut corpora = BTreeMap::new();
        for record in snapshot.adversarial_corpora {
            if record.family_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "adversarial corpus `{}` has no perceptual families",
                        hex::encode(record.corpus_digest)
                    ),
                });
            }
            if record.variant_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "adversarial corpus `{}` has no variants",
                        hex::encode(record.corpus_digest)
                    ),
                });
            }
            if corpora.insert(record.corpus_digest, record).is_some() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: "duplicate adversarial corpus digest".to_string(),
                });
            }
        }

        self.repro_manifests = repro_manifests;
        self.corpora = corpora;
        Ok(())
    }
}

/// Local in-memory index for encrypted moderation quarantine payload objects.
#[derive(Debug)]
pub(crate) struct ModerationQuarantineObjectRuntime {
    objects: BTreeMap<[u8; 16], ModerationQuarantineObjectRecord>,
    entry_limit: usize,
}

impl Default for ModerationQuarantineObjectRuntime {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl ModerationQuarantineObjectRuntime {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            objects: BTreeMap::new(),
            entry_limit: entry_limit.max(1),
        }
    }

    pub(crate) fn ensure_insert_capacity(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<(), ModerationQuarantineObjectError> {
        if !self.objects.contains_key(quarantine_id) && self.objects.len() >= self.entry_limit {
            return Err(ModerationQuarantineObjectError::ResourceExhausted {
                resource: "quarantine_objects",
                limit: self.entry_limit,
            });
        }
        Ok(())
    }

    pub(crate) fn ensure_snapshot_capacity(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        if snapshot.objects.len() > self.entry_limit {
            return Err(ModerationQuarantineObjectError::ResourceExhausted {
                resource: "quarantine_objects",
                limit: self.entry_limit,
            });
        }
        Ok(())
    }

    pub(crate) fn get(&self, quarantine_id: &[u8; 16]) -> Option<ModerationQuarantineObjectRecord> {
        self.objects.get(quarantine_id).cloned()
    }

    pub(crate) fn insert(
        &mut self,
        record: ModerationQuarantineObjectRecord,
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        validate_quarantine_object_record(&record)
            .map_err(|message| ModerationQuarantineObjectError::InvalidInput { message })?;
        match self.objects.get(&record.quarantine_id) {
            Some(existing) if existing != &record => {
                Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                })
            }
            Some(existing) => Ok(existing.clone()),
            None => {
                self.ensure_insert_capacity(&record.quarantine_id)?;
                self.objects.insert(record.quarantine_id, record.clone());
                Ok(record)
            }
        }
    }

    pub(crate) fn snapshot(&self) -> ModerationQuarantineObjectSnapshot {
        ModerationQuarantineObjectSnapshot {
            objects: self.objects.values().cloned().collect(),
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        self.ensure_snapshot_capacity(&snapshot)?;
        let mut objects = BTreeMap::new();
        for record in snapshot.objects {
            validate_quarantine_object_record(&record)
                .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
            if objects.insert(record.quarantine_id, record).is_some() {
                return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                    message: "duplicate moderation quarantine object id".to_string(),
                });
            }
        }
        self.objects = objects;
        Ok(())
    }
}

/// Local in-memory payload-free evidence viewer session and access-log runtime.
#[derive(Debug)]
pub(crate) struct ModerationEvidenceViewerRuntime {
    sessions: BTreeMap<[u8; 16], ModerationEvidenceViewerSessionRecord>,
    access_events: Vec<ModerationEvidenceViewerAccessEventRecord>,
    entry_limit: usize,
}

impl Default for ModerationEvidenceViewerRuntime {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl ModerationEvidenceViewerRuntime {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            sessions: BTreeMap::new(),
            access_events: Vec::new(),
            entry_limit: entry_limit.max(1),
        }
    }

    pub(crate) fn create_session(
        &mut self,
        input: ModerationEvidenceViewerSessionInput,
        object: &ModerationQuarantineObjectRecord,
    ) -> Result<ModerationEvidenceViewerSessionRecord, ModerationEvidenceViewerError> {
        let record = evidence_viewer_session_record_from_input(input, object)?;
        match self.sessions.get(&record.session_id) {
            Some(existing) if existing != &record => {
                Err(ModerationEvidenceViewerError::ConflictingSession {
                    session_id_hex: hex::encode(record.session_id),
                })
            }
            Some(existing) => Ok(existing.clone()),
            None => {
                if self.sessions.len() >= self.entry_limit {
                    return Err(ModerationEvidenceViewerError::ResourceExhausted {
                        resource: "evidence_viewer_sessions",
                        limit: self.entry_limit,
                    });
                }
                self.sessions.insert(record.session_id, record.clone());
                Ok(record)
            }
        }
    }

    pub(crate) fn record_access(
        &mut self,
        input: ModerationEvidenceViewerAccessInput,
    ) -> Result<ModerationEvidenceViewerAccessEventRecord, ModerationEvidenceViewerError> {
        let session = self
            .sessions
            .get(&input.session_id)
            .cloned()
            .ok_or_else(|| ModerationEvidenceViewerError::UnknownSession {
                session_id_hex: hex::encode(input.session_id),
            })?;
        if self.access_events.len() >= self.entry_limit {
            return Err(ModerationEvidenceViewerError::ResourceExhausted {
                resource: "evidence_viewer_access_events",
                limit: self.entry_limit,
            });
        }
        let sequence = self
            .access_events
            .last()
            .map_or(Some(1), |event| event.sequence.checked_add(1))
            .ok_or_else(|| ModerationEvidenceViewerError::InvalidInput {
                message: "evidence viewer access event sequence exhausted".to_owned(),
            })?;
        let record = evidence_viewer_access_event_record_from_input(sequence, input, &session)?;
        self.access_events.push(record.clone());
        Ok(record)
    }

    pub(crate) fn snapshot(&self) -> ModerationEvidenceViewerSnapshot {
        ModerationEvidenceViewerSnapshot {
            sessions: self.sessions.values().cloned().collect(),
            access_events: self.access_events.clone(),
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationEvidenceViewerSnapshot,
    ) -> Result<(), ModerationEvidenceViewerError> {
        for (resource, count) in [
            ("evidence_viewer_sessions", snapshot.sessions.len()),
            (
                "evidence_viewer_access_events",
                snapshot.access_events.len(),
            ),
        ] {
            if count > self.entry_limit {
                return Err(ModerationEvidenceViewerError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }
        let mut sessions = BTreeMap::new();
        for record in snapshot.sessions {
            validate_evidence_viewer_session_record(&record)
                .map_err(|message| ModerationEvidenceViewerError::InvalidSnapshot { message })?;
            if sessions.insert(record.session_id, record).is_some() {
                return Err(ModerationEvidenceViewerError::InvalidSnapshot {
                    message: "duplicate evidence viewer session id".to_string(),
                });
            }
        }

        let mut expected_sequence = 1_u64;
        let mut events = Vec::with_capacity(snapshot.access_events.len());
        let mut event_ids = BTreeSet::new();
        for record in snapshot.access_events {
            if record.sequence != expected_sequence {
                return Err(ModerationEvidenceViewerError::InvalidSnapshot {
                    message: format!(
                        "evidence viewer access event `{}` has non-contiguous sequence",
                        hex::encode(record.event_id)
                    ),
                });
            }
            let session = sessions.get(&record.session_id).ok_or_else(|| {
                ModerationEvidenceViewerError::InvalidSnapshot {
                    message: format!(
                        "evidence viewer access event `{}` references unknown session `{}`",
                        hex::encode(record.event_id),
                        hex::encode(record.session_id)
                    ),
                }
            })?;
            validate_evidence_viewer_access_event_record(&record, session)
                .map_err(|message| ModerationEvidenceViewerError::InvalidSnapshot { message })?;
            if !event_ids.insert(record.event_id) {
                return Err(ModerationEvidenceViewerError::InvalidSnapshot {
                    message: "duplicate evidence viewer access event id".to_owned(),
                });
            }
            events.push(record);
            expected_sequence = expected_sequence.checked_add(1).ok_or_else(|| {
                ModerationEvidenceViewerError::InvalidSnapshot {
                    message: "evidence viewer access event sequence exhausted".to_owned(),
                }
            })?;
        }

        self.sessions = sessions;
        self.access_events = events;
        Ok(())
    }
}

pub(crate) fn moderation_evidence_viewer_audit_report_from_snapshot(
    input: ModerationEvidenceViewerAuditReportInput,
    snapshot: &ModerationEvidenceViewerSnapshot,
) -> Result<ModerationEvidenceViewerAuditReport, ModerationEvidenceViewerError> {
    if input.raw_evidence_included
        || input.raw_access_logs_included
        || input.viewer_accounts_included
        || input.signed_urls_included
        || input.session_tokens_included
        || input.response_bodies_included
    {
        return Err(ModerationEvidenceViewerError::PayloadSafetyViolation {
            message: "evidence viewer audit reports must include only aggregate counts and digest-set hashes, never raw evidence, raw access logs, viewer accounts, signed URLs, session tokens, or response bodies".to_string(),
        });
    }
    if input.window_start_unix == 0 {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "window_start_unix must be non-zero".to_string(),
        });
    }
    if input.window_end_unix <= input.window_start_unix {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "window_end_unix must be later than window_start_unix".to_string(),
        });
    }
    if input
        .window_end_unix
        .saturating_sub(input.window_start_unix)
        > MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_MAX_WINDOW_SECS
    {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "evidence viewer audit reports must cover no more than one day".to_string(),
        });
    }
    if input.generated_at_unix < input.window_end_unix {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "generated_at_unix must be at or after window_end_unix".to_string(),
        });
    }
    if input.policy_digest.is_some_and(digest_is_zero) {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "policy_digest must not be all zeroes when present".to_string(),
        });
    }

    let report_scope = clean_evidence_viewer_text(input.report_scope, "report_scope")?;
    let window_start_ms = u128::from(input.window_start_unix) * 1_000;
    let window_end_ms = u128::from(input.window_end_unix) * 1_000;
    let sessions = snapshot
        .sessions
        .iter()
        .filter(|session| {
            u128::from(session.issued_at_unix_ms) < window_end_ms
                && u128::from(session.expires_at_unix_ms) > window_start_ms
        })
        .collect::<Vec<_>>();
    let session_ids = sessions
        .iter()
        .map(|session| session.session_id)
        .collect::<BTreeSet<_>>();
    let access_events = snapshot
        .access_events
        .iter()
        .filter(|event| {
            session_ids.contains(&event.session_id)
                && u128::from(event.event_at_unix_ms) >= window_start_ms
                && u128::from(event.event_at_unix_ms) < window_end_ms
        })
        .collect::<Vec<_>>();

    let mut logged_sessions = BTreeSet::new();
    let mut viewer_roles = BTreeSet::new();
    let mut evidence_digests = BTreeSet::new();
    let mut session_manifest_digests = BTreeSet::new();
    let mut attestation_digests = BTreeSet::new();
    let mut watermark_metadata_digests = BTreeSet::new();
    let mut legal_hold_bound_session_count = 0_u64;
    for session in &sessions {
        viewer_roles.insert(session.viewer_role.clone());
        evidence_digests.insert(session.evidence_digest);
        session_manifest_digests.insert(session.session_manifest_digest);
        attestation_digests.insert(session.attestation_digest);
        watermark_metadata_digests.insert(session.watermark_metadata_digest);
        if session.legal_hold_id.is_some() {
            legal_hold_bound_session_count = legal_hold_bound_session_count.saturating_add(1);
        }
    }

    let mut access_kind_counts = BTreeMap::new();
    let mut access_event_digests = BTreeSet::new();
    let mut request_digests = BTreeSet::new();
    let mut first_event_at_unix_ms: Option<u64> = None;
    let mut last_event_at_unix_ms: Option<u64> = None;
    for event in &access_events {
        logged_sessions.insert(event.session_id);
        *access_kind_counts
            .entry(event.kind.as_str().to_string())
            .or_insert(0_u64) += 1;
        access_event_digests.insert(event.event_digest);
        request_digests.insert(event.request_digest);
        first_event_at_unix_ms = Some(
            first_event_at_unix_ms.map_or(event.event_at_unix_ms, |current| {
                current.min(event.event_at_unix_ms)
            }),
        );
        last_event_at_unix_ms = Some(
            last_event_at_unix_ms.map_or(event.event_at_unix_ms, |current| {
                current.max(event.event_at_unix_ms)
            }),
        );
    }
    let access_kind_counts = access_kind_counts
        .into_iter()
        .map(|(kind, count)| ModerationEvidenceViewerAuditKindCount { kind, count })
        .collect::<Vec<_>>();

    let mut report = ModerationEvidenceViewerAuditReport {
        version: MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_VERSION_V1,
        report_id: [0; 16],
        report_scope,
        window_start_unix: input.window_start_unix,
        window_end_unix: input.window_end_unix,
        generated_at_unix: input.generated_at_unix,
        session_count: len_to_u64(sessions.len()),
        logged_session_count: len_to_u64(logged_sessions.len()),
        access_event_count: len_to_u64(access_events.len()),
        unique_viewer_role_count: len_to_u64(viewer_roles.len()),
        attested_session_count: len_to_u64(sessions.len()),
        watermarked_session_count: len_to_u64(sessions.len()),
        legal_hold_bound_session_count,
        first_event_at_unix_ms,
        last_event_at_unix_ms,
        access_kind_counts,
        evidence_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "evidence_digest",
            evidence_digests,
        ),
        session_manifest_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "session_manifest_digest",
            session_manifest_digests,
        ),
        access_event_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "access_event_digest",
            access_event_digests,
        ),
        request_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "request_digest",
            request_digests,
        ),
        attestation_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "attestation_digest",
            attestation_digests,
        ),
        watermark_metadata_digest_set_digest: evidence_viewer_audit_digest_set_digest(
            "watermark_metadata_digest",
            watermark_metadata_digests,
        ),
        policy_digest: input.policy_digest,
        report_digest: [0; 32],
    };
    let digest = evidence_viewer_audit_report_digest(&report);
    report.report_id = digest_id16(digest);
    report.report_digest = digest;
    validate_evidence_viewer_audit_report(&report)
        .map_err(|message| ModerationEvidenceViewerError::InvalidInput { message })?;
    Ok(report)
}

/// Local in-memory runtime for SFM-4a screening and quarantine evidence.
#[derive(Debug)]
pub(crate) struct ModerationScreeningRuntime {
    screening_records: BTreeMap<[u8; 16], ModerationScreeningRecord>,
    quarantine_records: BTreeMap<[u8; 16], ModerationQuarantineRecord>,
    entry_limit: usize,
}

impl Default for ModerationScreeningRuntime {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl ModerationScreeningRuntime {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            screening_records: BTreeMap::new(),
            quarantine_records: BTreeMap::new(),
            entry_limit: entry_limit.max(1),
        }
    }

    pub(crate) fn record_screening(
        &mut self,
        input: ModerationScreeningInput,
    ) -> Result<ModerationScreeningOutcome, ModerationScreeningError> {
        let record = screening_record_from_input(input)?;
        let quarantine = record
            .verdict
            .requires_quarantine_record()
            .then(|| quarantine_record_from_screening(&record));

        match self.screening_records.get(&record.record_id) {
            Some(existing) if existing != &record => {
                return Err(ModerationScreeningError::ConflictingRecord {
                    record_id_hex: hex::encode(record.record_id),
                });
            }
            Some(existing) => {
                let quarantine = quarantine
                    .as_ref()
                    .and_then(|record| self.quarantine_records.get(&record.quarantine_id))
                    .cloned();
                return Ok(ModerationScreeningOutcome {
                    record: existing.clone(),
                    quarantine,
                });
            }
            None => {
                if self.screening_records.len() >= self.entry_limit {
                    return Err(ModerationScreeningError::ResourceExhausted {
                        resource: "screening_records",
                        limit: self.entry_limit,
                    });
                }
            }
        }

        if let Some(quarantine) = quarantine.as_ref() {
            match self.quarantine_records.get(&quarantine.quarantine_id) {
                Some(existing) if existing != quarantine => {
                    return Err(ModerationScreeningError::ConflictingRecord {
                        record_id_hex: hex::encode(record.record_id),
                    });
                }
                Some(_) => {}
                None if self.quarantine_records.len() >= self.entry_limit => {
                    return Err(ModerationScreeningError::ResourceExhausted {
                        resource: "quarantine_records",
                        limit: self.entry_limit,
                    });
                }
                None => {}
            }
        }

        self.screening_records
            .insert(record.record_id, record.clone());
        if let Some(quarantine) = quarantine.clone() {
            self.quarantine_records
                .entry(quarantine.quarantine_id)
                .or_insert(quarantine);
        }

        Ok(ModerationScreeningOutcome { record, quarantine })
    }

    pub(crate) fn snapshot(&self) -> ModerationScreeningSnapshot {
        ModerationScreeningSnapshot {
            screening_records: self.screening_records.values().cloned().collect(),
            quarantine_records: self.quarantine_records.values().cloned().collect(),
        }
    }

    pub(crate) fn quarantine_record(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Option<ModerationQuarantineRecord> {
        self.quarantine_records.get(quarantine_id).cloned()
    }

    pub(crate) fn review_quarantine(
        &mut self,
        input: ModerationQuarantineReviewInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let reviewed_by =
            clean_required_text(input.reviewed_by, "reviewed_by", input.quarantine_id, true)?;
        if input.reviewed_at_unix == 0 {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                message: "reviewed_at_unix must be non-zero".to_string(),
            });
        }
        let review_notes = clean_optional_text(input.notes);
        let record = self
            .quarantine_records
            .get_mut(&input.quarantine_id)
            .ok_or(ModerationScreeningError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(input.quarantine_id),
            })?;

        match record.state {
            ModerationQuarantineState::PendingReview => {
                record.state = ModerationQuarantineState::Reviewed;
                record.reviewed_at_unix = Some(input.reviewed_at_unix);
                record.reviewed_by = Some(reviewed_by);
                record.review_notes = review_notes;
                Ok(record.clone())
            }
            ModerationQuarantineState::Reviewed
                if record.reviewed_at_unix == Some(input.reviewed_at_unix)
                    && record.reviewed_by.as_deref() == Some(reviewed_by.as_str())
                    && record.review_notes == review_notes =>
            {
                Ok(record.clone())
            }
            ModerationQuarantineState::Reviewed => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record is already reviewed with different metadata".to_string(),
                })
            }
            ModerationQuarantineState::Released => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "released records cannot be reviewed again".to_string(),
                })
            }
        }
    }

    pub(crate) fn release_quarantine(
        &mut self,
        input: ModerationQuarantineReleaseInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let release_authority = clean_required_text(
            input.release_authority,
            "release_authority",
            input.quarantine_id,
            true,
        )?;
        if input.released_at_unix == 0 {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                message: "released_at_unix must be non-zero".to_string(),
            });
        }
        let release_notes = clean_optional_text(input.notes);
        let record = self
            .quarantine_records
            .get_mut(&input.quarantine_id)
            .ok_or(ModerationScreeningError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(input.quarantine_id),
            })?;

        match record.state {
            ModerationQuarantineState::PendingReview => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record must be reviewed before release".to_string(),
                })
            }
            ModerationQuarantineState::Reviewed => {
                if record
                    .reviewed_at_unix
                    .is_some_and(|reviewed_at| input.released_at_unix < reviewed_at)
                {
                    return Err(ModerationScreeningError::InvalidTransition {
                        quarantine_id_hex: hex::encode(input.quarantine_id),
                        message: "released_at_unix must be >= reviewed_at_unix".to_string(),
                    });
                }
                record.state = ModerationQuarantineState::Released;
                record.released_at_unix = Some(input.released_at_unix);
                record.release_authority = Some(release_authority);
                record.release_notes = release_notes;
                Ok(record.clone())
            }
            ModerationQuarantineState::Released
                if record.released_at_unix == Some(input.released_at_unix)
                    && record.release_authority.as_deref() == Some(release_authority.as_str())
                    && record.release_notes == release_notes =>
            {
                Ok(record.clone())
            }
            ModerationQuarantineState::Released => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record is already released with different metadata".to_string(),
                })
            }
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        for (resource, count) in [
            ("screening_records", snapshot.screening_records.len()),
            ("quarantine_records", snapshot.quarantine_records.len()),
        ] {
            if count > self.entry_limit {
                return Err(ModerationScreeningError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }
        let mut screening_records = BTreeMap::new();
        for record in snapshot.screening_records {
            validate_screening_record(&record)?;
            if screening_records.insert(record.record_id, record).is_some() {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate moderation screening record id".to_string(),
                });
            }
        }

        let mut quarantine_records = BTreeMap::new();
        for quarantine in snapshot.quarantine_records {
            validate_quarantine_record(&quarantine)?;
            let Some(screening_record) = screening_records.get(&quarantine.screening_record_id)
            else {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "quarantine record `{}` references unknown screening record `{}`",
                        hex::encode(quarantine.quarantine_id),
                        hex::encode(quarantine.screening_record_id)
                    ),
                });
            };
            validate_quarantine_record_matches_screening(&quarantine, screening_record)?;
            if quarantine_records
                .insert(quarantine.quarantine_id, quarantine)
                .is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate moderation quarantine record id".to_string(),
                });
            }
        }

        self.screening_records = screening_records;
        self.quarantine_records = quarantine_records;
        Ok(())
    }
}

fn screening_record_from_input(
    input: ModerationScreeningInput,
) -> Result<ModerationScreeningRecord, ModerationScreeningError> {
    if input.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidInput {
            message: "subject must not be blank".to_string(),
        });
    }
    if input.combined_score_bps > 10_000 {
        return Err(ModerationScreeningError::InvalidInput {
            message: "combined_score_bps must be <= 10000".to_string(),
        });
    }
    if input.screened_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidInput {
            message: "screened_at_unix must be non-zero".to_string(),
        });
    }

    let record_digest = screening_record_digest(
        &input.subject,
        input.subject_digest,
        input.manifest_id,
        input.runner_hash,
        input.combined_score_bps,
        input.verdict,
        input.screened_at_unix,
        input.evidence_digest,
        input.policy_digest,
        input.notes.as_deref(),
    );
    let mut record_id = [0u8; 16];
    record_id.copy_from_slice(&record_digest[..16]);
    Ok(ModerationScreeningRecord {
        record_id,
        record_digest,
        subject: input.subject,
        subject_digest: input.subject_digest,
        manifest_id: input.manifest_id,
        runner_hash: input.runner_hash,
        combined_score_bps: input.combined_score_bps,
        verdict: input.verdict,
        screened_at_unix: input.screened_at_unix,
        evidence_digest: input.evidence_digest,
        policy_digest: input.policy_digest,
        notes: input.notes,
    })
}

fn validate_screening_record(
    record: &ModerationScreeningRecord,
) -> Result<(), ModerationScreeningError> {
    if record.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: "screening record subject must not be blank".to_string(),
        });
    }
    if record.combined_score_bps > 10_000 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` has combined_score_bps > 10000",
                hex::encode(record.record_id)
            ),
        });
    }
    if record.screened_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` has zero screened_at_unix",
                hex::encode(record.record_id)
            ),
        });
    }
    let expected_digest = screening_record_digest(
        &record.subject,
        record.subject_digest,
        record.manifest_id,
        record.runner_hash,
        record.combined_score_bps,
        record.verdict,
        record.screened_at_unix,
        record.evidence_digest,
        record.policy_digest,
        record.notes.as_deref(),
    );
    let mut expected_id = [0u8; 16];
    expected_id.copy_from_slice(&expected_digest[..16]);
    if record.record_digest != expected_digest || record.record_id != expected_id {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` digest/id mismatch",
                hex::encode(record.record_id)
            ),
        });
    }
    Ok(())
}

fn quarantine_record_from_screening(
    record: &ModerationScreeningRecord,
) -> ModerationQuarantineRecord {
    let quarantine_digest = quarantine_record_digest(
        record.record_id,
        record.subject_digest,
        record.verdict,
        record.screened_at_unix,
    );
    let mut quarantine_id = [0u8; 16];
    quarantine_id.copy_from_slice(&quarantine_digest[..16]);
    ModerationQuarantineRecord {
        quarantine_id,
        screening_record_id: record.record_id,
        subject: record.subject.clone(),
        subject_digest: record.subject_digest,
        verdict: record.verdict,
        queued_at_unix: record.screened_at_unix,
        state: ModerationQuarantineState::PendingReview,
        reviewed_at_unix: None,
        reviewed_by: None,
        review_notes: None,
        released_at_unix: None,
        release_authority: None,
        release_notes: None,
    }
}

fn validate_quarantine_record(
    record: &ModerationQuarantineRecord,
) -> Result<(), ModerationScreeningError> {
    if !record.verdict.requires_quarantine_record() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has non-quarantine verdict `{}`",
                hex::encode(record.quarantine_id),
                record.verdict.as_str()
            ),
        });
    }
    if record.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has blank subject",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    if record.queued_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has zero queued_at_unix",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    validate_quarantine_state_fields(record)?;
    Ok(())
}

fn validate_quarantine_record_matches_screening(
    record: &ModerationQuarantineRecord,
    screening_record: &ModerationScreeningRecord,
) -> Result<(), ModerationScreeningError> {
    let expected = quarantine_record_from_screening(screening_record);
    if record.quarantine_id != expected.quarantine_id
        || record.screening_record_id != expected.screening_record_id
        || record.subject != expected.subject
        || record.subject_digest != expected.subject_digest
        || record.verdict != expected.verdict
        || record.queued_at_unix != expected.queued_at_unix
    {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` does not match its screening record",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    Ok(())
}

fn validate_quarantine_state_fields(
    record: &ModerationQuarantineRecord,
) -> Result<(), ModerationScreeningError> {
    match record.state {
        ModerationQuarantineState::PendingReview => {
            if record.reviewed_at_unix.is_some()
                || record.reviewed_by.is_some()
                || record.review_notes.is_some()
                || record.released_at_unix.is_some()
                || record.release_authority.is_some()
                || record.release_notes.is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "pending quarantine record `{}` must not carry review/release metadata",
                        hex::encode(record.quarantine_id)
                    ),
                });
            }
        }
        ModerationQuarantineState::Reviewed => {
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "reviewed_at_unix",
                record.reviewed_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "reviewed_by",
                &record.reviewed_by,
            )?;
            if record.released_at_unix.is_some()
                || record.release_authority.is_some()
                || record.release_notes.is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "reviewed quarantine record `{}` must not carry release metadata",
                        hex::encode(record.quarantine_id)
                    ),
                });
            }
        }
        ModerationQuarantineState::Released => {
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "reviewed_at_unix",
                record.reviewed_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "reviewed_by",
                &record.reviewed_by,
            )?;
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "released_at_unix",
                record.released_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "release_authority",
                &record.release_authority,
            )?;
            if let (Some(reviewed_at), Some(released_at)) =
                (record.reviewed_at_unix, record.released_at_unix)
            {
                if released_at < reviewed_at {
                    return Err(ModerationScreeningError::InvalidSnapshot {
                        message: format!(
                            "released quarantine record `{}` has released_at_unix < reviewed_at_unix",
                            hex::encode(record.quarantine_id)
                        ),
                    });
                }
            }
        }
    }
    validate_optional_note(record.quarantine_id, "review_notes", &record.review_notes)?;
    validate_optional_note(record.quarantine_id, "release_notes", &record.release_notes)
}

fn validate_nonzero_optional_timestamp(
    quarantine_id: [u8; 16],
    field: &str,
    value: Option<u64>,
) -> Result<(), ModerationScreeningError> {
    match value {
        Some(0) | None => Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` requires non-zero {field}",
                hex::encode(quarantine_id)
            ),
        }),
        Some(_) => Ok(()),
    }
}

fn validate_nonblank_optional_text(
    quarantine_id: [u8; 16],
    field: &str,
    value: &Option<String>,
) -> Result<(), ModerationScreeningError> {
    match value.as_deref() {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` requires non-blank {field}",
                hex::encode(quarantine_id)
            ),
        }),
    }
}

fn validate_optional_note(
    quarantine_id: [u8; 16],
    field: &str,
    value: &Option<String>,
) -> Result<(), ModerationScreeningError> {
    if value
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has blank {field}",
                hex::encode(quarantine_id)
            ),
        });
    }
    Ok(())
}

fn clean_required_text(
    value: String,
    field: &str,
    quarantine_id: [u8; 16],
    transition_error: bool,
) -> Result<String, ModerationScreeningError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        let message = format!("{field} must not be blank");
        if transition_error {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(quarantine_id),
                message,
            });
        }
        return Err(ModerationScreeningError::InvalidInput { message });
    }
    Ok(value)
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct ModerationQuarantineObjectEnvelopeV1 {
    pub version: u16,
    pub algorithm: String,
    pub quarantine_id: [u8; 16],
    pub object_id: [u8; 16],
    pub payload_digest: [u8; 32],
    pub ciphertext_digest: [u8; 32],
    pub payload_len: u64,
    pub captured_at_unix: u64,
    pub content_type: Option<String>,
    pub notes: Option<String>,
    pub key_id: [u8; 16],
    pub nonce: [u8; 16],
    pub ciphertext: Vec<u8>,
    pub auth_tag: [u8; 32],
}

pub(crate) fn seal_moderation_quarantine_object(
    input: ModerationQuarantineObjectInput,
    local_key: [u8; 32],
) -> Result<(ModerationQuarantineObjectRecord, Vec<u8>), ModerationQuarantineObjectError> {
    let cleaned = clean_quarantine_object_input(input)?;
    let payload_digest = *blake3::hash(&cleaned.payload).as_bytes();
    let payload_len = len_to_u64(cleaned.payload.len());
    let key_id = moderation_quarantine_object_key_id(local_key);
    let nonce = moderation_quarantine_object_nonce(
        cleaned.quarantine_id,
        payload_digest,
        cleaned.captured_at_unix,
        cleaned.content_type.as_deref(),
        cleaned.notes.as_deref(),
    );
    let encryption_key = blake3::derive_key(
        MODERATION_QUARANTINE_OBJECT_ENCRYPTION_KEY_DOMAIN_V1,
        &local_key,
    );
    let ciphertext = xor_quarantine_object_keystream(&cleaned.payload, encryption_key, nonce);
    let ciphertext_digest = *blake3::hash(&ciphertext).as_bytes();
    let object_id = moderation_quarantine_object_id(ModerationQuarantineObjectIdInput {
        quarantine_id: cleaned.quarantine_id,
        payload_digest,
        ciphertext_digest,
        payload_len,
        captured_at_unix: cleaned.captured_at_unix,
        content_type: cleaned.content_type.as_deref(),
        notes: cleaned.notes.as_deref(),
        key_id,
    });
    let envelope_path =
        moderation_quarantine_object_relative_path(cleaned.quarantine_id, object_id);
    let mut envelope = ModerationQuarantineObjectEnvelopeV1 {
        version: MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1,
        algorithm: MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1.to_string(),
        quarantine_id: cleaned.quarantine_id,
        object_id,
        payload_digest,
        ciphertext_digest,
        payload_len,
        captured_at_unix: cleaned.captured_at_unix,
        content_type: cleaned.content_type,
        notes: cleaned.notes,
        key_id,
        nonce,
        ciphertext,
        auth_tag: [0; 32],
    };
    let auth_key = blake3::derive_key(MODERATION_QUARANTINE_OBJECT_AUTH_KEY_DOMAIN_V1, &local_key);
    envelope.auth_tag = moderation_quarantine_object_auth_tag(&envelope, auth_key);
    let record = moderation_quarantine_object_record_from_envelope(&envelope, envelope_path)?;
    let bytes =
        norito::to_bytes(&envelope).map_err(|err| ModerationQuarantineObjectError::Codec {
            message: err.to_string(),
        })?;
    Ok((record, bytes))
}

pub(crate) fn open_moderation_quarantine_object(
    envelope: ModerationQuarantineObjectEnvelopeV1,
    record: &ModerationQuarantineObjectRecord,
    local_key: [u8; 32],
) -> Result<Vec<u8>, ModerationQuarantineObjectError> {
    validate_quarantine_object_envelope(&envelope)?;
    let rebuilt = moderation_quarantine_object_record_from_envelope(
        &envelope,
        moderation_quarantine_object_relative_path(envelope.quarantine_id, envelope.object_id),
    )?;
    if &rebuilt != record {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` does not match local index",
                hex::encode(envelope.object_id)
            ),
        });
    }
    let auth_key = blake3::derive_key(MODERATION_QUARANTINE_OBJECT_AUTH_KEY_DOMAIN_V1, &local_key);
    let expected_tag = moderation_quarantine_object_auth_tag(&envelope, auth_key);
    if !constant_time_eq(&expected_tag, &envelope.auth_tag) {
        return Err(ModerationQuarantineObjectError::AuthenticationFailed {
            quarantine_id_hex: hex::encode(envelope.quarantine_id),
        });
    }
    let encryption_key = blake3::derive_key(
        MODERATION_QUARANTINE_OBJECT_ENCRYPTION_KEY_DOMAIN_V1,
        &local_key,
    );
    let payload =
        xor_quarantine_object_keystream(&envelope.ciphertext, encryption_key, envelope.nonce);
    if len_to_u64(payload.len()) != envelope.payload_len {
        return Err(ModerationQuarantineObjectError::AuthenticationFailed {
            quarantine_id_hex: hex::encode(envelope.quarantine_id),
        });
    }
    if *blake3::hash(&payload).as_bytes() != envelope.payload_digest {
        return Err(ModerationQuarantineObjectError::AuthenticationFailed {
            quarantine_id_hex: hex::encode(envelope.quarantine_id),
        });
    }
    Ok(payload)
}

fn clean_quarantine_object_input(
    input: ModerationQuarantineObjectInput,
) -> Result<ModerationQuarantineObjectInput, ModerationQuarantineObjectError> {
    if input.captured_at_unix == 0 {
        return Err(ModerationQuarantineObjectError::InvalidInput {
            message: "captured_at_unix must be non-zero".to_string(),
        });
    }
    Ok(ModerationQuarantineObjectInput {
        quarantine_id: input.quarantine_id,
        payload: input.payload,
        captured_at_unix: input.captured_at_unix,
        content_type: clean_optional_object_text(input.content_type, "content_type")?,
        notes: clean_optional_object_text(input.notes, "notes")?,
    })
}

fn clean_optional_object_text(
    value: Option<String>,
    field: &str,
) -> Result<Option<String>, ModerationQuarantineObjectError> {
    value
        .map(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Err(ModerationQuarantineObjectError::InvalidInput {
                    message: format!("{field} must not be blank when present"),
                });
            }
            Ok(trimmed)
        })
        .transpose()
}

fn validate_quarantine_object_envelope(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
) -> Result<(), ModerationQuarantineObjectError> {
    if envelope.version != MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1 {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!("unsupported object envelope version {}", envelope.version),
        });
    }
    if envelope.algorithm != MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1 {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "unsupported object envelope algorithm `{}`",
                envelope.algorithm
            ),
        });
    }
    if len_to_u64(envelope.ciphertext.len()) != envelope.payload_len {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` ciphertext length does not match payload_len",
                hex::encode(envelope.object_id)
            ),
        });
    }
    if *blake3::hash(&envelope.ciphertext).as_bytes() != envelope.ciphertext_digest {
        return Err(ModerationQuarantineObjectError::AuthenticationFailed {
            quarantine_id_hex: hex::encode(envelope.quarantine_id),
        });
    }
    Ok(())
}

fn moderation_quarantine_object_record_from_envelope(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    envelope_path: String,
) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
    let record = ModerationQuarantineObjectRecord {
        quarantine_id: envelope.quarantine_id,
        object_id: envelope.object_id,
        payload_digest: envelope.payload_digest,
        ciphertext_digest: envelope.ciphertext_digest,
        payload_len: envelope.payload_len,
        captured_at_unix: envelope.captured_at_unix,
        content_type: envelope.content_type.clone(),
        notes: envelope.notes.clone(),
        encryption_algorithm: envelope.algorithm.clone(),
        key_id: envelope.key_id,
        envelope_path,
    };
    validate_quarantine_object_record(&record)
        .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
    Ok(record)
}

fn validate_quarantine_object_record(
    record: &ModerationQuarantineObjectRecord,
) -> Result<(), String> {
    if record.captured_at_unix == 0 {
        return Err(format!(
            "quarantine object `{}` has zero captured_at_unix",
            hex::encode(record.object_id)
        ));
    }
    if record.encryption_algorithm != MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1 {
        return Err(format!(
            "quarantine object `{}` uses unsupported algorithm `{}`",
            hex::encode(record.object_id),
            record.encryption_algorithm
        ));
    }
    validate_optional_object_record_text(record.object_id, "content_type", &record.content_type)?;
    validate_optional_object_record_text(record.object_id, "notes", &record.notes)?;
    let expected_id = moderation_quarantine_object_id(ModerationQuarantineObjectIdInput {
        quarantine_id: record.quarantine_id,
        payload_digest: record.payload_digest,
        ciphertext_digest: record.ciphertext_digest,
        payload_len: record.payload_len,
        captured_at_unix: record.captured_at_unix,
        content_type: record.content_type.as_deref(),
        notes: record.notes.as_deref(),
        key_id: record.key_id,
    });
    if record.object_id != expected_id {
        return Err(format!(
            "quarantine object `{}` id does not match metadata",
            hex::encode(record.object_id)
        ));
    }
    let expected_path =
        moderation_quarantine_object_relative_path(record.quarantine_id, record.object_id);
    if record.envelope_path != expected_path {
        return Err(format!(
            "quarantine object `{}` has unexpected envelope path `{}`",
            hex::encode(record.object_id),
            record.envelope_path
        ));
    }
    validate_relative_object_path(&record.envelope_path)?;
    Ok(())
}

fn validate_optional_object_record_text(
    object_id: [u8; 16],
    field: &str,
    value: &Option<String>,
) -> Result<(), String> {
    if value
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(format!(
            "quarantine object `{}` has blank {field}",
            hex::encode(object_id)
        ));
    }
    Ok(())
}

pub(crate) fn validate_relative_object_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("object envelope path must be relative".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("object envelope path must not contain traversal components".to_string());
    }
    Ok(())
}

pub(crate) fn moderation_quarantine_object_relative_path(
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
) -> String {
    format!(
        "{}/{}/{}.{}",
        MODERATION_QUARANTINE_OBJECTS_DIR,
        hex::encode(quarantine_id),
        hex::encode(object_id),
        MODERATION_QUARANTINE_OBJECT_EXT
    )
}

fn moderation_quarantine_object_key_id(local_key: [u8; 32]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_KEY_ID_DOMAIN_V1);
    hasher.update(&local_key);
    let digest = hasher.finalize();
    let mut key_id = [0u8; 16];
    key_id.copy_from_slice(&digest.as_bytes()[..16]);
    key_id
}

fn moderation_quarantine_object_nonce(
    quarantine_id: [u8; 16],
    payload_digest: [u8; 32],
    captured_at_unix: u64,
    content_type: Option<&str>,
    notes: Option<&str>,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_NONCE_DOMAIN_V1);
    hasher.update(&quarantine_id);
    hasher.update(&payload_digest);
    hasher.update(&captured_at_unix.to_le_bytes());
    update_optional_string(&mut hasher, content_type);
    update_optional_string(&mut hasher, notes);
    let digest = hasher.finalize();
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&digest.as_bytes()[..16]);
    nonce
}

struct ModerationQuarantineObjectIdInput<'a> {
    quarantine_id: [u8; 16],
    payload_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    payload_len: u64,
    captured_at_unix: u64,
    content_type: Option<&'a str>,
    notes: Option<&'a str>,
    key_id: [u8; 16],
}

fn moderation_quarantine_object_id(input: ModerationQuarantineObjectIdInput<'_>) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_ID_DOMAIN_V1);
    hasher.update(&input.quarantine_id);
    hasher.update(&input.payload_digest);
    hasher.update(&input.ciphertext_digest);
    hasher.update(&input.payload_len.to_le_bytes());
    hasher.update(&input.captured_at_unix.to_le_bytes());
    update_optional_string(&mut hasher, input.content_type);
    update_optional_string(&mut hasher, input.notes);
    hasher.update(&input.key_id);
    let digest = hasher.finalize();
    let mut object_id = [0u8; 16];
    object_id.copy_from_slice(&digest.as_bytes()[..16]);
    object_id
}

fn xor_quarantine_object_keystream(
    input: &[u8],
    encryption_key: [u8; 32],
    nonce: [u8; 16],
) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    for (counter, chunk) in input.chunks(32).enumerate() {
        let mut hasher = blake3::Hasher::new_keyed(&encryption_key);
        hasher.update(MODERATION_QUARANTINE_OBJECT_KEYSTREAM_DOMAIN_V1);
        hasher.update(&nonce);
        hasher.update(&(counter as u64).to_le_bytes());
        let block = hasher.finalize();
        output.extend(
            chunk
                .iter()
                .zip(block.as_bytes().iter())
                .map(|(byte, mask)| *byte ^ *mask),
        );
    }
    output
}

fn evidence_viewer_session_record_from_input(
    input: ModerationEvidenceViewerSessionInput,
    object: &ModerationQuarantineObjectRecord,
) -> Result<ModerationEvidenceViewerSessionRecord, ModerationEvidenceViewerError> {
    if input.raw_evidence_included
        || input.signed_url_included
        || input.session_token_included
        || input.watermark_secret_included
    {
        return Err(ModerationEvidenceViewerError::PayloadSafetyViolation {
            message: "viewer sessions must include only digest metadata, never raw evidence, signed URLs, session tokens, or watermark secrets".to_string(),
        });
    }
    let requested_by = clean_evidence_viewer_text(input.requested_by, "requested_by")?;
    let viewer_account = clean_evidence_viewer_text(input.viewer_account, "viewer_account")?;
    let viewer_role = clean_evidence_viewer_text(input.viewer_role, "viewer_role")?;
    let purpose = clean_evidence_viewer_text(input.purpose, "purpose")?;
    let legal_hold_id = clean_optional_evidence_viewer_text(input.legal_hold_id, "legal_hold_id")?;
    let notes = clean_optional_evidence_viewer_text(input.notes, "notes")?;
    if input.issued_at_unix_ms == 0 {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "issued_at_unix_ms must be non-zero".to_string(),
        });
    }
    if input.expires_at_unix_ms <= input.issued_at_unix_ms {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "expires_at_unix_ms must be later than issued_at_unix_ms".to_string(),
        });
    }
    if input
        .expires_at_unix_ms
        .saturating_sub(input.issued_at_unix_ms)
        > MODERATION_EVIDENCE_VIEWER_MAX_SESSION_TTL_MS
    {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "evidence viewer sessions must expire within 15 minutes".to_string(),
        });
    }
    for (field, digest) in [
        ("attestation_digest", input.attestation_digest),
        ("watermark_metadata_digest", input.watermark_metadata_digest),
        ("session_nonce_digest", input.session_nonce_digest),
    ] {
        if digest_is_zero(digest) {
            return Err(ModerationEvidenceViewerError::InvalidInput {
                message: format!("{field} must not be all zeroes"),
            });
        }
    }

    let mut record = ModerationEvidenceViewerSessionRecord {
        quarantine_id: input.quarantine_id,
        object_id: object.object_id,
        session_id: [0; 16],
        evidence_digest: object.payload_digest,
        attestation_digest: input.attestation_digest,
        watermark_metadata_digest: input.watermark_metadata_digest,
        session_nonce_digest: input.session_nonce_digest,
        requested_by,
        viewer_account,
        viewer_role,
        purpose,
        issued_at_unix_ms: input.issued_at_unix_ms,
        expires_at_unix_ms: input.expires_at_unix_ms,
        legal_hold_id,
        notes,
        session_manifest_digest: [0; 32],
    };
    let digest = evidence_viewer_session_digest(&record);
    record.session_id.copy_from_slice(&digest[..16]);
    record.session_manifest_digest = digest;
    validate_evidence_viewer_session_record(&record)
        .map_err(|message| ModerationEvidenceViewerError::InvalidInput { message })?;
    Ok(record)
}

fn evidence_viewer_access_event_record_from_input(
    sequence: u64,
    input: ModerationEvidenceViewerAccessInput,
    session: &ModerationEvidenceViewerSessionRecord,
) -> Result<ModerationEvidenceViewerAccessEventRecord, ModerationEvidenceViewerError> {
    if input.raw_evidence_included
        || input.signed_url_included
        || input.session_token_included
        || input.response_body_included
    {
        return Err(ModerationEvidenceViewerError::PayloadSafetyViolation {
            message: "viewer access events must include only digest metadata, never raw evidence, signed URLs, session tokens, response bodies, or raw access logs".to_string(),
        });
    }
    let actor_account = clean_evidence_viewer_text(input.actor_account, "actor_account")?;
    if actor_account != session.viewer_account {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "actor_account must match the session viewer_account".to_string(),
        });
    }
    if sequence == 0 {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "access event sequence must be non-zero".to_string(),
        });
    }
    if input.event_at_unix_ms == 0 {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "event_at_unix_ms must be non-zero".to_string(),
        });
    }
    if input.event_at_unix_ms < session.issued_at_unix_ms {
        return Err(ModerationEvidenceViewerError::ExpiredSession {
            session_id_hex: hex::encode(input.session_id),
        });
    }
    if input.kind.is_expiry_event() {
        if input.event_at_unix_ms < session.expires_at_unix_ms {
            return Err(ModerationEvidenceViewerError::InvalidInput {
                message: "session_expired events must be at or after expires_at_unix_ms"
                    .to_string(),
            });
        }
    } else if input.event_at_unix_ms >= session.expires_at_unix_ms {
        return Err(ModerationEvidenceViewerError::ExpiredSession {
            session_id_hex: hex::encode(input.session_id),
        });
    }
    if digest_is_zero(input.request_digest) {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "request_digest must not be all zeroes".to_string(),
        });
    }
    if input.event_metadata_digest.is_some_and(digest_is_zero) {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: "event_metadata_digest must not be all zeroes when present".to_string(),
        });
    }
    let notes = clean_optional_evidence_viewer_text(input.notes, "notes")?;
    let mut record = ModerationEvidenceViewerAccessEventRecord {
        sequence,
        event_id: [0; 16],
        session_id: input.session_id,
        quarantine_id: session.quarantine_id,
        object_id: session.object_id,
        evidence_digest: session.evidence_digest,
        kind: input.kind,
        actor_account,
        event_at_unix_ms: input.event_at_unix_ms,
        request_digest: input.request_digest,
        event_metadata_digest: input.event_metadata_digest,
        notes,
        event_digest: [0; 32],
    };
    let digest = evidence_viewer_access_event_digest(&record);
    record.event_id.copy_from_slice(&digest[..16]);
    record.event_digest = digest;
    validate_evidence_viewer_access_event_record(&record, session)
        .map_err(|message| ModerationEvidenceViewerError::InvalidInput { message })?;
    Ok(record)
}

fn validate_evidence_viewer_session_record(
    record: &ModerationEvidenceViewerSessionRecord,
) -> Result<(), String> {
    for (field, value) in [
        ("requested_by", record.requested_by.as_str()),
        ("viewer_account", record.viewer_account.as_str()),
        ("viewer_role", record.viewer_role.as_str()),
        ("purpose", record.purpose.as_str()),
    ] {
        validate_evidence_viewer_record_text(field, value)?;
    }
    validate_optional_evidence_viewer_record_text("legal_hold_id", &record.legal_hold_id)?;
    validate_optional_evidence_viewer_record_text("notes", &record.notes)?;
    if record.issued_at_unix_ms == 0 {
        return Err(format!(
            "evidence viewer session `{}` has zero issued_at_unix_ms",
            hex::encode(record.session_id)
        ));
    }
    if record.expires_at_unix_ms <= record.issued_at_unix_ms {
        return Err(format!(
            "evidence viewer session `{}` expires before it starts",
            hex::encode(record.session_id)
        ));
    }
    if record
        .expires_at_unix_ms
        .saturating_sub(record.issued_at_unix_ms)
        > MODERATION_EVIDENCE_VIEWER_MAX_SESSION_TTL_MS
    {
        return Err(format!(
            "evidence viewer session `{}` exceeds the maximum TTL",
            hex::encode(record.session_id)
        ));
    }
    for (field, digest) in [
        ("evidence_digest", record.evidence_digest),
        ("attestation_digest", record.attestation_digest),
        (
            "watermark_metadata_digest",
            record.watermark_metadata_digest,
        ),
        ("session_nonce_digest", record.session_nonce_digest),
    ] {
        if digest_is_zero(digest) {
            return Err(format!(
                "evidence viewer session `{}` has all-zero {field}",
                hex::encode(record.session_id)
            ));
        }
    }
    let digest = evidence_viewer_session_digest(record);
    if record.session_manifest_digest != digest {
        return Err(format!(
            "evidence viewer session `{}` manifest digest does not match metadata",
            hex::encode(record.session_id)
        ));
    }
    if record.session_id != digest_id16(digest) {
        return Err(format!(
            "evidence viewer session `{}` id does not match metadata",
            hex::encode(record.session_id)
        ));
    }
    Ok(())
}

fn validate_evidence_viewer_access_event_record(
    record: &ModerationEvidenceViewerAccessEventRecord,
    session: &ModerationEvidenceViewerSessionRecord,
) -> Result<(), String> {
    if record.sequence == 0 {
        return Err(format!(
            "evidence viewer access event `{}` has zero sequence",
            hex::encode(record.event_id)
        ));
    }
    if record.session_id != session.session_id
        || record.quarantine_id != session.quarantine_id
        || record.object_id != session.object_id
        || record.evidence_digest != session.evidence_digest
    {
        return Err(format!(
            "evidence viewer access event `{}` does not match its session",
            hex::encode(record.event_id)
        ));
    }
    validate_evidence_viewer_record_text("actor_account", &record.actor_account)?;
    if record.actor_account != session.viewer_account {
        return Err(format!(
            "evidence viewer access event `{}` actor does not match session viewer",
            hex::encode(record.event_id)
        ));
    }
    if record.event_at_unix_ms == 0 {
        return Err(format!(
            "evidence viewer access event `{}` has zero event_at_unix_ms",
            hex::encode(record.event_id)
        ));
    }
    if record.event_at_unix_ms < session.issued_at_unix_ms {
        return Err(format!(
            "evidence viewer access event `{}` predates its session",
            hex::encode(record.event_id)
        ));
    }
    if record.kind.is_expiry_event() {
        if record.event_at_unix_ms < session.expires_at_unix_ms {
            return Err(format!(
                "evidence viewer access event `{}` reports expiry before session expiration",
                hex::encode(record.event_id)
            ));
        }
    } else if record.event_at_unix_ms >= session.expires_at_unix_ms {
        return Err(format!(
            "evidence viewer access event `{}` occurs after session expiration",
            hex::encode(record.event_id)
        ));
    }
    if digest_is_zero(record.request_digest) {
        return Err(format!(
            "evidence viewer access event `{}` has all-zero request_digest",
            hex::encode(record.event_id)
        ));
    }
    if record.event_metadata_digest.is_some_and(digest_is_zero) {
        return Err(format!(
            "evidence viewer access event `{}` has all-zero event_metadata_digest",
            hex::encode(record.event_id)
        ));
    }
    validate_optional_evidence_viewer_record_text("notes", &record.notes)?;
    let digest = evidence_viewer_access_event_digest(record);
    if record.event_digest != digest {
        return Err(format!(
            "evidence viewer access event `{}` digest does not match metadata",
            hex::encode(record.event_id)
        ));
    }
    if record.event_id != digest_id16(digest) {
        return Err(format!(
            "evidence viewer access event `{}` id does not match metadata",
            hex::encode(record.event_id)
        ));
    }
    Ok(())
}

fn validate_evidence_viewer_audit_report(
    report: &ModerationEvidenceViewerAuditReport,
) -> Result<(), String> {
    if report.version != MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_VERSION_V1 {
        return Err(format!(
            "evidence viewer audit report `{}` has unsupported version {}",
            hex::encode(report.report_id),
            report.version
        ));
    }
    validate_evidence_viewer_record_text("report_scope", &report.report_scope)?;
    if report.window_start_unix == 0 {
        return Err(format!(
            "evidence viewer audit report `{}` has zero window_start_unix",
            hex::encode(report.report_id)
        ));
    }
    if report.window_end_unix <= report.window_start_unix {
        return Err(format!(
            "evidence viewer audit report `{}` has an invalid window",
            hex::encode(report.report_id)
        ));
    }
    if report
        .window_end_unix
        .saturating_sub(report.window_start_unix)
        > MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_MAX_WINDOW_SECS
    {
        return Err(format!(
            "evidence viewer audit report `{}` exceeds the maximum window",
            hex::encode(report.report_id)
        ));
    }
    if report.generated_at_unix < report.window_end_unix {
        return Err(format!(
            "evidence viewer audit report `{}` was generated before its window closed",
            hex::encode(report.report_id)
        ));
    }
    if report.logged_session_count > report.session_count
        || report.attested_session_count > report.session_count
        || report.watermarked_session_count > report.session_count
        || report.legal_hold_bound_session_count > report.session_count
        || report.unique_viewer_role_count > report.session_count
    {
        return Err(format!(
            "evidence viewer audit report `{}` has inconsistent session counts",
            hex::encode(report.report_id)
        ));
    }
    let mut previous_kind: Option<&str> = None;
    let mut counted_events = 0_u64;
    for kind_count in &report.access_kind_counts {
        validate_evidence_viewer_record_text("access_kind_counts.kind", &kind_count.kind)?;
        if let Some(previous) = previous_kind
            && previous >= kind_count.kind.as_str()
        {
            return Err(format!(
                "evidence viewer audit report `{}` has unsorted or duplicate access kind counts",
                hex::encode(report.report_id)
            ));
        }
        if kind_count.count == 0 {
            return Err(format!(
                "evidence viewer audit report `{}` has zero count for access kind `{}`",
                hex::encode(report.report_id),
                kind_count.kind
            ));
        }
        counted_events = counted_events.saturating_add(kind_count.count);
        previous_kind = Some(kind_count.kind.as_str());
    }
    if counted_events != report.access_event_count {
        return Err(format!(
            "evidence viewer audit report `{}` access-kind counts do not sum to access_event_count",
            hex::encode(report.report_id)
        ));
    }
    match (report.first_event_at_unix_ms, report.last_event_at_unix_ms) {
        (Some(first), Some(last)) if first <= last => {
            let window_start_ms = u128::from(report.window_start_unix) * 1_000;
            let window_end_ms = u128::from(report.window_end_unix) * 1_000;
            if u128::from(first) < window_start_ms || u128::from(last) >= window_end_ms {
                return Err(format!(
                    "evidence viewer audit report `{}` event timestamps are outside the window",
                    hex::encode(report.report_id)
                ));
            }
        }
        (None, None) if report.access_event_count == 0 => {}
        _ => {
            return Err(format!(
                "evidence viewer audit report `{}` has inconsistent event timestamp bounds",
                hex::encode(report.report_id)
            ));
        }
    }
    for (field, digest) in [
        (
            "evidence_digest_set_digest",
            report.evidence_digest_set_digest,
        ),
        (
            "session_manifest_digest_set_digest",
            report.session_manifest_digest_set_digest,
        ),
        (
            "access_event_digest_set_digest",
            report.access_event_digest_set_digest,
        ),
        (
            "request_digest_set_digest",
            report.request_digest_set_digest,
        ),
        (
            "attestation_digest_set_digest",
            report.attestation_digest_set_digest,
        ),
        (
            "watermark_metadata_digest_set_digest",
            report.watermark_metadata_digest_set_digest,
        ),
    ] {
        if digest_is_zero(digest) {
            return Err(format!(
                "evidence viewer audit report `{}` has all-zero {field}",
                hex::encode(report.report_id)
            ));
        }
    }
    if report.policy_digest.is_some_and(digest_is_zero) {
        return Err(format!(
            "evidence viewer audit report `{}` has all-zero policy_digest",
            hex::encode(report.report_id)
        ));
    }
    let digest = evidence_viewer_audit_report_digest(report);
    if report.report_digest != digest {
        return Err(format!(
            "evidence viewer audit report `{}` digest does not match metadata",
            hex::encode(report.report_id)
        ));
    }
    if report.report_id != digest_id16(digest) {
        return Err(format!(
            "evidence viewer audit report `{}` id does not match metadata",
            hex::encode(report.report_id)
        ));
    }
    Ok(())
}

fn clean_evidence_viewer_text(
    value: String,
    field: &str,
) -> Result<String, ModerationEvidenceViewerError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: format!("{field} must not be blank"),
        });
    }
    if trimmed.len() > 256 {
        return Err(ModerationEvidenceViewerError::InvalidInput {
            message: format!("{field} must be 256 bytes or shorter"),
        });
    }
    Ok(trimmed)
}

fn clean_optional_evidence_viewer_text(
    value: Option<String>,
    field: &str,
) -> Result<Option<String>, ModerationEvidenceViewerError> {
    value
        .map(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Err(ModerationEvidenceViewerError::InvalidInput {
                    message: format!("{field} must not be blank when present"),
                });
            }
            if trimmed.len() > 512 {
                return Err(ModerationEvidenceViewerError::InvalidInput {
                    message: format!("{field} must be 512 bytes or shorter"),
                });
            }
            Ok(trimmed)
        })
        .transpose()
}

fn validate_evidence_viewer_record_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be blank"));
    }
    if value != value.trim() {
        return Err(format!(
            "{field} must be normalized without surrounding whitespace"
        ));
    }
    if value.len() > 256 {
        return Err(format!("{field} must be 256 bytes or shorter"));
    }
    Ok(())
}

fn validate_optional_evidence_viewer_record_text(
    field: &str,
    value: &Option<String>,
) -> Result<(), String> {
    if let Some(value) = value.as_deref() {
        if value.trim().is_empty() {
            return Err(format!("{field} must not be blank when present"));
        }
        if value != value.trim() {
            return Err(format!(
                "{field} must be normalized without surrounding whitespace"
            ));
        }
        if value.len() > 512 {
            return Err(format!("{field} must be 512 bytes or shorter"));
        }
    }
    Ok(())
}

fn evidence_viewer_session_digest(record: &ModerationEvidenceViewerSessionRecord) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_EVIDENCE_VIEWER_SESSION_DOMAIN_V1);
    hasher.update(&record.quarantine_id);
    hasher.update(&record.object_id);
    hasher.update(&record.evidence_digest);
    hasher.update(&record.attestation_digest);
    hasher.update(&record.watermark_metadata_digest);
    hasher.update(&record.session_nonce_digest);
    update_string(&mut hasher, &record.requested_by);
    update_string(&mut hasher, &record.viewer_account);
    update_string(&mut hasher, &record.viewer_role);
    update_string(&mut hasher, &record.purpose);
    hasher.update(&record.issued_at_unix_ms.to_le_bytes());
    hasher.update(&record.expires_at_unix_ms.to_le_bytes());
    update_optional_string(&mut hasher, record.legal_hold_id.as_deref());
    update_optional_string(&mut hasher, record.notes.as_deref());
    *hasher.finalize().as_bytes()
}

fn evidence_viewer_access_event_digest(
    record: &ModerationEvidenceViewerAccessEventRecord,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_EVIDENCE_VIEWER_ACCESS_EVENT_DOMAIN_V1);
    hasher.update(&record.sequence.to_le_bytes());
    hasher.update(&record.session_id);
    hasher.update(&record.quarantine_id);
    hasher.update(&record.object_id);
    hasher.update(&record.evidence_digest);
    update_string(&mut hasher, record.kind.as_str());
    update_string(&mut hasher, &record.actor_account);
    hasher.update(&record.event_at_unix_ms.to_le_bytes());
    hasher.update(&record.request_digest);
    update_optional_digest(&mut hasher, record.event_metadata_digest);
    update_optional_string(&mut hasher, record.notes.as_deref());
    *hasher.finalize().as_bytes()
}

fn evidence_viewer_audit_report_digest(report: &ModerationEvidenceViewerAuditReport) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_DOMAIN_V1);
    hasher.update(&report.version.to_le_bytes());
    update_string(&mut hasher, &report.report_scope);
    hasher.update(&report.window_start_unix.to_le_bytes());
    hasher.update(&report.window_end_unix.to_le_bytes());
    hasher.update(&report.generated_at_unix.to_le_bytes());
    hasher.update(&report.session_count.to_le_bytes());
    hasher.update(&report.logged_session_count.to_le_bytes());
    hasher.update(&report.access_event_count.to_le_bytes());
    hasher.update(&report.unique_viewer_role_count.to_le_bytes());
    hasher.update(&report.attested_session_count.to_le_bytes());
    hasher.update(&report.watermarked_session_count.to_le_bytes());
    hasher.update(&report.legal_hold_bound_session_count.to_le_bytes());
    update_optional_u64(&mut hasher, report.first_event_at_unix_ms);
    update_optional_u64(&mut hasher, report.last_event_at_unix_ms);
    hasher.update(&(report.access_kind_counts.len() as u64).to_le_bytes());
    for kind_count in &report.access_kind_counts {
        update_string(&mut hasher, &kind_count.kind);
        hasher.update(&kind_count.count.to_le_bytes());
    }
    hasher.update(&report.evidence_digest_set_digest);
    hasher.update(&report.session_manifest_digest_set_digest);
    hasher.update(&report.access_event_digest_set_digest);
    hasher.update(&report.request_digest_set_digest);
    hasher.update(&report.attestation_digest_set_digest);
    hasher.update(&report.watermark_metadata_digest_set_digest);
    update_optional_digest(&mut hasher, report.policy_digest);
    *hasher.finalize().as_bytes()
}

fn evidence_viewer_audit_digest_set_digest(label: &str, values: BTreeSet<[u8; 32]>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_EVIDENCE_VIEWER_AUDIT_DIGEST_SET_DOMAIN_V1);
    update_string(&mut hasher, label);
    hasher.update(&(values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(&value);
    }
    *hasher.finalize().as_bytes()
}

fn digest_id16(digest: [u8; 32]) -> [u8; 16] {
    let mut id = [0; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

fn digest_is_zero(digest: [u8; 32]) -> bool {
    digest.iter().all(|byte| *byte == 0)
}

fn len_to_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

fn moderation_quarantine_object_auth_tag(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    auth_key: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(&auth_key);
    hasher.update(MODERATION_QUARANTINE_OBJECT_AUTH_DOMAIN_V1);
    hasher.update(&envelope.version.to_le_bytes());
    update_string(&mut hasher, &envelope.algorithm);
    hasher.update(&envelope.quarantine_id);
    hasher.update(&envelope.object_id);
    hasher.update(&envelope.payload_digest);
    hasher.update(&envelope.ciphertext_digest);
    hasher.update(&envelope.payload_len.to_le_bytes());
    hasher.update(&envelope.captured_at_unix.to_le_bytes());
    update_optional_string(&mut hasher, envelope.content_type.as_deref());
    update_optional_string(&mut hasher, envelope.notes.as_deref());
    hasher.update(&envelope.key_id);
    hasher.update(&envelope.nonce);
    hasher.update(&(envelope.ciphertext.len() as u64).to_le_bytes());
    hasher.update(&envelope.ciphertext);
    *hasher.finalize().as_bytes()
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

#[allow(clippy::too_many_arguments)]
fn screening_record_digest(
    subject: &str,
    subject_digest: [u8; 32],
    manifest_id: [u8; 16],
    runner_hash: [u8; 32],
    combined_score_bps: u16,
    verdict: ModerationScreeningVerdict,
    screened_at_unix: u64,
    evidence_digest: Option<[u8; 32]>,
    policy_digest: Option<[u8; 32]>,
    notes: Option<&str>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SCREENING_RECORD_DOMAIN_V1);
    update_string(&mut hasher, subject);
    hasher.update(&subject_digest);
    hasher.update(&manifest_id);
    hasher.update(&runner_hash);
    hasher.update(&combined_score_bps.to_le_bytes());
    update_string(&mut hasher, verdict.as_str());
    hasher.update(&screened_at_unix.to_le_bytes());
    update_optional_digest(&mut hasher, evidence_digest);
    update_optional_digest(&mut hasher, policy_digest);
    update_optional_string(&mut hasher, notes);
    *hasher.finalize().as_bytes()
}

fn quarantine_record_digest(
    screening_record_id: [u8; 16],
    subject_digest: [u8; 32],
    verdict: ModerationScreeningVerdict,
    queued_at_unix: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_RECORD_DOMAIN_V1);
    hasher.update(&screening_record_id);
    hasher.update(&subject_digest);
    update_string(&mut hasher, verdict.as_str());
    hasher.update(&queued_at_unix.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn update_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn update_optional_digest(hasher: &mut blake3::Hasher, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value);
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

fn update_optional_u64(hasher: &mut blake3::Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

fn update_optional_string(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            update_string(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

/// Sequenced local moderation ballot event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationBallotEventKind {
    /// A ballot was announced.
    BallotAnnounced,
    /// A juror commitment was accepted.
    CommitAccepted,
    /// A moderation ballot challenge was accepted.
    ChallengeSubmitted,
    /// A moderation ballot challenge was resolved.
    ChallengeResolved,
    /// A juror reveal was accepted.
    RevealAccepted,
    /// The ballot was tallied.
    BallotTallied,
}

impl From<ModerationBallotEventKind> for SoraFsModerationBallotGovernanceEventKindV1 {
    fn from(kind: ModerationBallotEventKind) -> Self {
        match kind {
            ModerationBallotEventKind::BallotAnnounced => Self::BallotAnnounced,
            ModerationBallotEventKind::CommitAccepted => Self::CommitAccepted,
            ModerationBallotEventKind::ChallengeSubmitted => Self::ChallengeSubmitted,
            ModerationBallotEventKind::ChallengeResolved => Self::ChallengeResolved,
            ModerationBallotEventKind::RevealAccepted => Self::RevealAccepted,
            ModerationBallotEventKind::BallotTallied => Self::BallotTallied,
        }
    }
}

/// Local moderation ballot vote counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationVoteCounts {
    /// Number of `uphold` reveals.
    pub uphold: u32,
    /// Number of `overturn` reveals.
    pub overturn: u32,
    /// Number of `modify` reveals.
    pub modify: u32,
    /// Number of `escalate` reveals.
    pub escalate: u32,
}

impl ModerationVoteCounts {
    fn increment(&mut self, choice: SoraFsModerationVoteChoice) {
        match choice {
            SoraFsModerationVoteChoice::Uphold => self.uphold = self.uphold.saturating_add(1),
            SoraFsModerationVoteChoice::Overturn => {
                self.overturn = self.overturn.saturating_add(1);
            }
            SoraFsModerationVoteChoice::Modify => self.modify = self.modify.saturating_add(1),
            SoraFsModerationVoteChoice::Escalate => {
                self.escalate = self.escalate.saturating_add(1);
            }
        }
    }

    fn winning_choice(self) -> Option<SoraFsModerationVoteChoice> {
        let choices = [
            (SoraFsModerationVoteChoice::Uphold, self.uphold),
            (SoraFsModerationVoteChoice::Overturn, self.overturn),
            (SoraFsModerationVoteChoice::Modify, self.modify),
            (SoraFsModerationVoteChoice::Escalate, self.escalate),
        ];
        let max_votes = choices.iter().map(|(_, count)| *count).max().unwrap_or(0);
        if max_votes == 0
            || choices
                .iter()
                .filter(|(_, count)| *count == max_votes)
                .count()
                != 1
        {
            return None;
        }
        choices
            .into_iter()
            .find_map(|(choice, count)| (count == max_votes).then_some(choice))
    }
}

impl From<ModerationVoteCounts> for SoraFsModerationVoteCountsV1 {
    fn from(counts: ModerationVoteCounts) -> Self {
        Self {
            uphold: counts.uphold,
            overturn: counts.overturn,
            modify: counts.modify,
            escalate: counts.escalate,
        }
    }
}

/// Final local tally for one moderation ballot.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotTally {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Vote counts by moderation choice.
    pub counts: ModerationVoteCounts,
    /// Number of valid reveals included in the tally.
    pub votes_total: u32,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Winning choice when the tally has exactly one highest vote count.
    pub winning_choice: Option<SoraFsModerationVoteChoice>,
    /// True when the vote reached quorum but no unique winner exists.
    pub contested: bool,
    /// UTC timestamp (milliseconds) when the tally was finalized locally.
    pub tallied_at_unix_ms: u64,
}

/// Payload-free local penalty plan for jurors that did not reveal.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotNoShowPlan {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// UTC timestamp (milliseconds) when the plan was generated locally.
    pub generated_at_unix_ms: u64,
    /// UTC timestamp (milliseconds) after which reveals are no longer accepted.
    pub reveal_deadline_unix_ms: u64,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Number of jurors in the announced roster.
    pub roster_size: u32,
    /// Number of accepted commitments at plan generation time.
    pub committed_count: u32,
    /// Number of accepted reveals at plan generation time.
    pub revealed_count: u32,
    /// Number of announced jurors without an accepted reveal.
    pub no_show_count: u32,
    /// True when accepted reveals satisfy quorum even with no-shows.
    pub quorum_met: bool,
    /// True when a local tally has already finalized for this ballot.
    pub tally_finalized: bool,
    /// True when the finalized local tally reached quorum but had no unique winner.
    pub contested: bool,
    /// Roster jurors that never submitted an accepted commitment.
    pub missing_commit_juror_ids: Vec<String>,
    /// Roster jurors that committed but did not submit an accepted reveal.
    pub unrevealed_committed_juror_ids: Vec<String>,
    /// Roster jurors without an accepted reveal, in roster order.
    pub no_show_juror_ids: Vec<String>,
    /// Deterministic digest binding the payload-free penalty plan material.
    pub penalty_plan_digest: [u8; 32],
}

impl From<&ModerationBallotTally> for SoraFsModerationBallotGovernanceTallyV1 {
    fn from(tally: &ModerationBallotTally) -> Self {
        Self {
            case_id: tally.case_id.clone(),
            round_id: tally.round_id.clone(),
            counts: tally.counts.into(),
            votes_total: tally.votes_total,
            quorum: tally.quorum,
            winning_choice: tally.winning_choice.map(governance_vote_choice),
            contested: tally.contested,
            tallied_at_unix_ms: tally.tallied_at_unix_ms,
        }
    }
}

/// Snapshot of one local moderation ballot lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotRecord {
    /// Ballot announcement.
    pub announcement: ModerationBallotAnnouncement,
    /// Accepted juror commitments sorted by juror id.
    pub commits: Vec<SoraFsModerationBallotCommitV1>,
    /// Accepted juror reveals sorted by juror id.
    pub reveals: Vec<SoraFsModerationBallotRevealV1>,
    /// Local challenge/dispute records sorted by challenge id.
    pub challenges: Vec<ModerationBallotChallengeRecord>,
    /// Final tally when the ballot has been finalized.
    pub tally: Option<ModerationBallotTally>,
}

/// Durable local moderation ballot lifecycle snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotSnapshot {
    /// Ballot records sorted by case id and round id.
    pub ballots: Vec<ModerationBallotRecord>,
    /// Sequenced local event backlog sorted by event sequence.
    pub events: Vec<ModerationBallotEvent>,
}

/// Result of accepting a moderation ballot commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotCommitOutcome {
    /// Accepted commitment payload.
    pub accepted_commit: SoraFsModerationBallotCommitV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Result of accepting a moderation ballot reveal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotRevealOutcome {
    /// Accepted reveal payload.
    pub accepted_reveal: SoraFsModerationBallotRevealV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Sequenced local moderation ballot event.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationBallotEvent {
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Event kind.
    pub kind: ModerationBallotEventKind,
    /// UTC timestamp (milliseconds) when the event was generated.
    pub generated_at_unix_ms: u64,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Juror associated with the event, when any.
    pub juror_id: Option<String>,
    /// Accepted commitment count after the event.
    pub committed_count: u64,
    /// Accepted reveal count after the event.
    pub revealed_count: u64,
    /// Local challenge count after the event.
    pub challenge_count: u64,
    /// Tally associated with the event, when finalized.
    pub tally: Option<ModerationBallotTally>,
    /// Challenge record associated with the event, when submitted or resolved.
    pub challenge: Option<ModerationBallotChallengeRecord>,
}

impl ModerationBallotEvent {
    pub(crate) fn to_governance_event_v1(&self) -> SoraFsModerationBallotGovernanceEventV1 {
        SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: self.sequence,
            kind: self.kind.into(),
            generated_at_unix_ms: self.generated_at_unix_ms,
            case_id: self.case_id.clone(),
            round_id: self.round_id.clone(),
            juror_id: self.juror_id.clone(),
            committed_count: self.committed_count,
            revealed_count: self.revealed_count,
            challenge_count: self.challenge_count,
            tally: self.tally.as_ref().map(Into::into),
            challenge: self.challenge.as_ref().map(Into::into),
        }
    }
}

fn governance_vote_choice(choice: SoraFsModerationVoteChoice) -> SoraFsModerationVoteChoiceV1 {
    match choice {
        SoraFsModerationVoteChoice::Uphold => SoraFsModerationVoteChoiceV1::Uphold,
        SoraFsModerationVoteChoice::Overturn => SoraFsModerationVoteChoiceV1::Overturn,
        SoraFsModerationVoteChoice::Modify => SoraFsModerationVoteChoiceV1::Modify,
        SoraFsModerationVoteChoice::Escalate => SoraFsModerationVoteChoiceV1::Escalate,
    }
}

/// Error raised by the local moderation ballot runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationBallotRuntimeError {
    /// A configured authoritative-state ceiling was reached.
    #[error("moderation ballot resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Canonical moderation ballot payload validation failed.
    #[error(transparent)]
    Validation(#[from] SoraFsModerationBallotError),
    /// Ballot already exists for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` already exists")]
    DuplicateBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot is unknown for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` is unknown")]
    UnknownBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot round id is blank.
    #[error("moderation ballot round id is required")]
    MissingRoundId,
    /// Ballot roster is empty.
    #[error("moderation ballot juror roster is required")]
    MissingJurors,
    /// Ballot roster contains a blank juror id.
    #[error("moderation ballot juror id is required")]
    BlankJurorId,
    /// Ballot roster contains a duplicate juror id.
    #[error("moderation ballot juror `{juror_id}` is duplicated")]
    DuplicateJuror {
        /// Duplicated juror identifier.
        juror_id: String,
    },
    /// Ballot quorum is zero or larger than the roster.
    #[error("moderation ballot quorum `{quorum}` is invalid for roster size `{roster_size}`")]
    InvalidQuorum {
        /// Requested quorum.
        quorum: u16,
        /// Roster length.
        roster_size: usize,
    },
    /// Announcement roster hash does not match the local canonical roster hash.
    #[error("moderation ballot roster hash does not match juror roster and quorum")]
    RosterHashMismatch,
    /// Stored appeal deposit metadata is missing its escrow id.
    #[error("moderation ballot appeal deposit escrow id is required")]
    MissingAppealDepositEscrowId,
    /// Stored appeal deposit metadata conflicts with the compatibility escrow id field.
    #[error("moderation ballot appeal deposit escrow id metadata mismatch")]
    AppealDepositEscrowIdMismatch,
    /// Ballot window timestamps are inconsistent.
    #[error("moderation ballot windows must satisfy announced < commit < challenge < reveal")]
    InvalidWindows,
    /// Commit/reveal payload is bound to a different context.
    #[error("moderation ballot payload context mismatch")]
    PayloadContextMismatch,
    /// Commit/reveal payload is bound to a different round.
    #[error("moderation ballot payload round mismatch: expected `{expected}`, found `{found}`")]
    PayloadRoundMismatch {
        /// Expected round identifier.
        expected: String,
        /// Payload round identifier.
        found: String,
    },
    /// Juror is not part of the announced roster.
    #[error(
        "juror `{juror_id}` is not eligible for moderation ballot `{case_id}` round `{round_id}`"
    )]
    IneligibleJuror {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Commit submission arrived outside the commit window.
    #[error("moderation ballot commit window is closed at `{now_unix_ms}`")]
    CommitWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// A commitment already exists for the juror.
    #[error(
        "juror `{juror_id}` already committed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Reveal submission arrived before the reveal window opened.
    #[error("moderation ballot reveal window is not open at `{now_unix_ms}`")]
    RevealWindowNotOpen {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// Reveal submission arrived after the reveal window closed.
    #[error("moderation ballot reveal window is closed at `{now_unix_ms}`")]
    RevealWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// The juror has no accepted commitment to reveal.
    #[error(
        "juror `{juror_id}` has no commitment for moderation ballot `{case_id}` round `{round_id}`"
    )]
    MissingCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// A reveal already exists for the juror.
    #[error(
        "juror `{juror_id}` already revealed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateReveal {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Ballot challenge id is blank.
    #[error("moderation ballot challenge id is required")]
    MissingChallengeId,
    /// Ballot challenger id is blank.
    #[error("moderation ballot challenger id is required")]
    MissingChallengerId,
    /// Ballot challenge target juror id is required.
    #[error("moderation ballot challenge target juror id is required for `{kind:?}`")]
    MissingChallengeTarget {
        /// Challenge category requiring a target juror.
        kind: ModerationBallotChallengeKind,
    },
    /// Ballot challenge target juror id is blank.
    #[error("moderation ballot challenge target juror id is blank")]
    BlankChallengeTarget,
    /// Ballot challenge evidence digest is all zeroes.
    #[error("moderation ballot challenge evidence digest must be non-zero")]
    MissingChallengeEvidence,
    /// Ballot challenge reason is blank.
    #[error("moderation ballot challenge reason is required")]
    MissingChallengeReason,
    /// A challenge already exists with this id.
    #[error(
        "moderation ballot challenge `{challenge_id}` already exists for `{case_id}` round `{round_id}`"
    )]
    DuplicateChallenge {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Challenge identifier.
        challenge_id: String,
    },
    /// Challenge is unknown for this ballot.
    #[error(
        "moderation ballot challenge `{challenge_id}` is unknown for `{case_id}` round `{round_id}`"
    )]
    UnknownChallenge {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Challenge identifier.
        challenge_id: String,
    },
    /// Challenge arrived before the post-commit challenge window.
    #[error("moderation ballot challenge window is not open at `{now_unix_ms}`")]
    ChallengeWindowNotOpen {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// Challenge arrived after the challenge window or after reveal/tally progress.
    #[error("moderation ballot challenge window is closed at `{now_unix_ms}`")]
    ChallengeWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// A pending challenge blocks reveal or tally progress.
    #[error(
        "moderation ballot challenge `{challenge_id}` is still pending for `{case_id}` round `{round_id}`"
    )]
    ChallengePending {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Challenge identifier.
        challenge_id: String,
    },
    /// An accepted challenge blocks local reveal or tally progress.
    #[error(
        "moderation ballot challenge `{challenge_id}` was accepted for `{case_id}` round `{round_id}`"
    )]
    ChallengeAccepted {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Challenge identifier.
        challenge_id: String,
    },
    /// Challenge has already been resolved.
    #[error(
        "moderation ballot challenge `{challenge_id}` was already resolved for `{case_id}` round `{round_id}`"
    )]
    ChallengeAlreadyResolved {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Challenge identifier.
        challenge_id: String,
    },
    /// Challenge resolver id is blank.
    #[error("moderation ballot challenge resolver id is required")]
    MissingChallengeResolver,
    /// Challenge resolution note is blank.
    #[error("moderation ballot challenge resolution note must not be blank")]
    BlankChallengeResolutionNote,
    /// Challenge resolution timestamp predates the challenge.
    #[error("moderation ballot challenge resolution timestamp is before the challenge timestamp")]
    InvalidChallengeResolutionTimestamp,
    /// Tally was requested before the reveal window closed and before all jurors revealed.
    #[error("moderation ballot reveal window is still open until `{reveal_deadline_unix_ms}`")]
    TallyWindowOpen {
        /// Reveal deadline.
        reveal_deadline_unix_ms: u64,
    },
    /// Tally was already finalized.
    #[error("moderation ballot `{case_id}` round `{round_id}` was already tallied")]
    AlreadyTallied {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Accepted reveals did not satisfy quorum.
    #[error("moderation ballot quorum `{quorum}` not met by `{reveals}` reveals")]
    QuorumNotMet {
        /// Required quorum.
        quorum: u16,
        /// Accepted reveal count.
        reveals: usize,
    },
    /// Durable snapshot is internally inconsistent.
    #[error("moderation ballot snapshot is invalid: {message}")]
    InvalidSnapshot {
        /// Validation failure.
        message: String,
    },
    /// The monotonic moderation event sequence was exhausted.
    #[error("moderation ballot event sequence exhausted")]
    EventSequenceOverflow,
    /// The durable ballot/event checkpoint could not be committed.
    #[error("moderation ballot checkpoint failed: {0}")]
    Checkpoint(String),
    /// The local moderation runtime lock was poisoned.
    #[error("moderation ballot state lock poisoned")]
    StateLockPoisoned,
}

/// Local in-memory moderation ballot lifecycle runtime.
#[derive(Debug)]
pub(crate) struct ModerationBallotRuntime {
    ballots: BTreeMap<ModerationBallotKey, ModerationBallotState>,
    juror_count: usize,
    commit_count: usize,
    reveal_count: usize,
    challenge_count: usize,
    entry_limit: usize,
}

impl Default for ModerationBallotRuntime {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl ModerationBallotRuntime {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            ballots: BTreeMap::new(),
            juror_count: 0,
            commit_count: 0,
            reveal_count: 0,
            challenge_count: 0,
            entry_limit: entry_limit.max(1),
        }
    }

    fn ensure_new_entries(
        &self,
        resource: &'static str,
        current: usize,
        additional: usize,
    ) -> Result<(), ModerationBallotRuntimeError> {
        if current
            .checked_add(additional)
            .is_none_or(|next| next > self.entry_limit)
        {
            return Err(ModerationBallotRuntimeError::ResourceExhausted {
                resource,
                limit: self.entry_limit,
            });
        }
        Ok(())
    }

    pub(crate) fn ensure_snapshot_capacity(
        &self,
        snapshot: &ModerationBallotSnapshot,
    ) -> Result<(), ModerationBallotRuntimeError> {
        if snapshot.ballots.len() > self.entry_limit {
            return Err(ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballots",
                limit: self.entry_limit,
            });
        }
        let mut jurors = 0usize;
        let mut commits = 0usize;
        let mut reveals = 0usize;
        let mut challenges = 0usize;
        for ballot in &snapshot.ballots {
            for (resource, total, additional) in [
                (
                    "ballot_jurors",
                    &mut jurors,
                    ballot.announcement.juror_ids.len(),
                ),
                ("ballot_commits", &mut commits, ballot.commits.len()),
                ("ballot_reveals", &mut reveals, ballot.reveals.len()),
                (
                    "ballot_challenges",
                    &mut challenges,
                    ballot.challenges.len(),
                ),
            ] {
                *total = total.checked_add(additional).ok_or(
                    ModerationBallotRuntimeError::ResourceExhausted {
                        resource,
                        limit: self.entry_limit,
                    },
                )?;
                if *total > self.entry_limit {
                    return Err(ModerationBallotRuntimeError::ResourceExhausted {
                        resource,
                        limit: self.entry_limit,
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn announce_ballot(
        &mut self,
        announcement: ModerationBallotAnnouncement,
    ) -> Result<ModerationBallotRecord, ModerationBallotRuntimeError> {
        validate_announcement(&announcement)?;
        let key = ModerationBallotKey::from_announcement(&announcement);
        if self.ballots.contains_key(&key) {
            return Err(ModerationBallotRuntimeError::DuplicateBallot {
                case_id: key.case_id,
                round_id: key.round_id,
            });
        }
        self.ensure_new_entries("ballots", self.ballots.len(), 1)?;
        self.ensure_new_entries(
            "ballot_jurors",
            self.juror_count,
            announcement.juror_ids.len(),
        )?;
        let juror_count = self
            .juror_count
            .checked_add(announcement.juror_ids.len())
            .expect("capacity preflight checked juror count");
        self.ballots.insert(
            key.clone(),
            ModerationBallotState {
                announcement,
                commits: BTreeMap::new(),
                reveals: BTreeMap::new(),
                challenges: BTreeMap::new(),
                tally: None,
            },
        );
        self.juror_count = juror_count;
        Ok(self
            .ballots
            .get(&key)
            .expect("inserted moderation ballot exists")
            .to_record())
    }

    pub(crate) fn submit_commit(
        &mut self,
        commit: SoraFsModerationBallotCommitV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotCommitOutcome, ModerationBallotRuntimeError> {
        commit.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&commit.context, &commit.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &commit.context, &commit.round_id)?;
        if now_unix_ms < state.announcement.announced_at_unix_ms
            || now_unix_ms > state.announcement.commit_deadline_unix_ms
        {
            return Err(ModerationBallotRuntimeError::CommitWindowClosed { now_unix_ms });
        }
        ensure_eligible_juror(state, &commit.juror_id)?;
        if state.commits.contains_key(&commit.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateCommit {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: commit.juror_id,
            });
        }
        if self.commit_count >= self.entry_limit {
            return Err(ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballot_commits",
                limit: self.entry_limit,
            });
        }
        state
            .commits
            .insert(commit.juror_id.clone(), commit.clone());
        self.commit_count += 1;
        Ok(ModerationBallotCommitOutcome {
            accepted_commit: commit,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn submit_reveal(
        &mut self,
        reveal: SoraFsModerationBallotRevealV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotRevealOutcome, ModerationBallotRuntimeError> {
        reveal.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&reveal.context, &reveal.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &reveal.context, &reveal.round_id)?;
        if now_unix_ms <= state.announcement.challenge_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowNotOpen { now_unix_ms });
        }
        if now_unix_ms > state.announcement.reveal_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowClosed { now_unix_ms });
        }
        ensure_no_blocking_challenges(state)?;
        ensure_eligible_juror(state, &reveal.juror_id)?;
        let commit = state.commits.get(&reveal.juror_id).ok_or_else(|| {
            ModerationBallotRuntimeError::MissingCommit {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
                juror_id: reveal.juror_id.clone(),
            }
        })?;
        if state.reveals.contains_key(&reveal.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateReveal {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: reveal.juror_id,
            });
        }
        commit.verify_reveal(&reveal)?;
        if self.reveal_count >= self.entry_limit {
            return Err(ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballot_reveals",
                limit: self.entry_limit,
            });
        }
        state
            .reveals
            .insert(reveal.juror_id.clone(), reveal.clone());
        self.reveal_count += 1;
        Ok(ModerationBallotRevealOutcome {
            accepted_reveal: reveal,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn submit_challenge(
        &mut self,
        input: ModerationBallotChallengeInput,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotChallengeRecord, ModerationBallotRuntimeError> {
        validate_challenge_input(&input)?;
        let key = ModerationBallotKey::new(&input.case_id, &input.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        if state.tally.is_some() || !state.reveals.is_empty() {
            return Err(ModerationBallotRuntimeError::ChallengeWindowClosed { now_unix_ms });
        }
        if now_unix_ms <= state.announcement.commit_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::ChallengeWindowNotOpen { now_unix_ms });
        }
        if now_unix_ms > state.announcement.challenge_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::ChallengeWindowClosed { now_unix_ms });
        }
        if state.challenges.contains_key(&input.challenge_id) {
            return Err(ModerationBallotRuntimeError::DuplicateChallenge {
                case_id: key.case_id,
                round_id: key.round_id,
                challenge_id: input.challenge_id,
            });
        }
        if self.challenge_count >= self.entry_limit {
            return Err(ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballot_challenges",
                limit: self.entry_limit,
            });
        }
        let record = ModerationBallotChallengeRecord {
            challenge_id: input.challenge_id,
            case_id: input.case_id,
            round_id: input.round_id,
            challenger_id: input.challenger_id,
            kind: input.kind,
            target_juror_id: input.target_juror_id,
            evidence_digest: input.evidence_digest,
            reason: input.reason,
            raised_at_unix_ms: now_unix_ms,
            decision: None,
            resolved_by: None,
            resolved_at_unix_ms: None,
            resolution_note: None,
        };
        state
            .challenges
            .insert(record.challenge_id.clone(), record.clone());
        self.challenge_count += 1;
        Ok(record)
    }

    pub(crate) fn resolve_challenge(
        &mut self,
        input: ModerationBallotChallengeResolution,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotChallengeRecord, ModerationBallotRuntimeError> {
        validate_challenge_resolution_input(&input)?;
        let key = ModerationBallotKey::new(&input.case_id, &input.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        if state.tally.is_some() || !state.reveals.is_empty() {
            return Err(ModerationBallotRuntimeError::ChallengeWindowClosed { now_unix_ms });
        }
        let challenge = state
            .challenges
            .get_mut(&input.challenge_id)
            .ok_or_else(|| ModerationBallotRuntimeError::UnknownChallenge {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
                challenge_id: input.challenge_id.clone(),
            })?;
        if challenge.decision.is_some() {
            return Err(ModerationBallotRuntimeError::ChallengeAlreadyResolved {
                case_id: key.case_id,
                round_id: key.round_id,
                challenge_id: input.challenge_id,
            });
        }
        if now_unix_ms < challenge.raised_at_unix_ms {
            return Err(ModerationBallotRuntimeError::InvalidChallengeResolutionTimestamp);
        }
        challenge.decision = Some(input.decision);
        challenge.resolved_by = Some(input.resolved_by);
        challenge.resolved_at_unix_ms = Some(now_unix_ms);
        challenge.resolution_note = input.note;
        Ok(challenge.clone())
    }

    pub(crate) fn tally_ballot(
        &mut self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotTally, ModerationBallotRuntimeError> {
        let key = ModerationBallotKey::new(case_id, round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            }
        })?;
        if state.tally.is_some() {
            return Err(ModerationBallotRuntimeError::AlreadyTallied {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            });
        }
        ensure_no_blocking_challenges(state)?;
        let all_jurors_revealed = state.reveals.len() == state.announcement.juror_ids.len();
        if now_unix_ms < state.announcement.reveal_deadline_unix_ms && !all_jurors_revealed {
            return Err(ModerationBallotRuntimeError::TallyWindowOpen {
                reveal_deadline_unix_ms: state.announcement.reveal_deadline_unix_ms,
            });
        }
        if state.reveals.len() < usize::from(state.announcement.quorum) {
            return Err(ModerationBallotRuntimeError::QuorumNotMet {
                quorum: state.announcement.quorum,
                reveals: state.reveals.len(),
            });
        }

        let mut counts = ModerationVoteCounts::default();
        for reveal in state.reveals.values() {
            counts.increment(reveal.choice);
        }
        let winning_choice = counts.winning_choice();
        let tally = ModerationBallotTally {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
            counts,
            votes_total: state.reveals.len() as u32,
            quorum: state.announcement.quorum,
            winning_choice,
            contested: winning_choice.is_none(),
            tallied_at_unix_ms: now_unix_ms,
        };
        state.tally = Some(tally.clone());
        Ok(tally)
    }

    pub(crate) fn no_show_plan(
        &self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotNoShowPlan, ModerationBallotRuntimeError> {
        let key = ModerationBallotKey::new(case_id, round_id);
        let state =
            self.ballots
                .get(&key)
                .ok_or_else(|| ModerationBallotRuntimeError::UnknownBallot {
                    case_id: case_id.to_owned(),
                    round_id: round_id.to_owned(),
                })?;
        ensure_no_blocking_challenges(state)?;
        if now_unix_ms <= state.announcement.reveal_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::TallyWindowOpen {
                reveal_deadline_unix_ms: state.announcement.reveal_deadline_unix_ms,
            });
        }

        let mut missing_commit_juror_ids = Vec::new();
        let mut unrevealed_committed_juror_ids = Vec::new();
        let mut no_show_juror_ids = Vec::new();
        for juror_id in &state.announcement.juror_ids {
            if state.reveals.contains_key(juror_id) {
                continue;
            }
            no_show_juror_ids.push(juror_id.clone());
            if state.commits.contains_key(juror_id) {
                unrevealed_committed_juror_ids.push(juror_id.clone());
            } else {
                missing_commit_juror_ids.push(juror_id.clone());
            }
        }

        let quorum_met = state.reveals.len() >= usize::from(state.announcement.quorum);
        let contested = state.tally.as_ref().is_some_and(|tally| tally.contested);
        let mut plan = ModerationBallotNoShowPlan {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
            generated_at_unix_ms: now_unix_ms,
            reveal_deadline_unix_ms: state.announcement.reveal_deadline_unix_ms,
            quorum: state.announcement.quorum,
            roster_size: state.announcement.juror_ids.len() as u32,
            committed_count: state.commits.len() as u32,
            revealed_count: state.reveals.len() as u32,
            no_show_count: no_show_juror_ids.len() as u32,
            quorum_met,
            tally_finalized: state.tally.is_some(),
            contested,
            missing_commit_juror_ids,
            unrevealed_committed_juror_ids,
            no_show_juror_ids,
            penalty_plan_digest: [0; 32],
        };
        plan.penalty_plan_digest = moderation_ballot_no_show_penalty_plan_digest(state, &plan);
        Ok(plan)
    }

    pub(crate) fn ballot(&self, case_id: &str, round_id: &str) -> Option<ModerationBallotRecord> {
        self.ballots
            .get(&ModerationBallotKey::new(case_id, round_id))
            .map(ModerationBallotState::to_record)
    }

    pub(crate) fn ballots(&self) -> Vec<ModerationBallotRecord> {
        self.ballots
            .values()
            .map(ModerationBallotState::to_record)
            .collect()
    }

    pub(crate) fn snapshot(&self) -> ModerationBallotSnapshot {
        ModerationBallotSnapshot {
            ballots: self.ballots(),
            events: Vec::new(),
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationBallotSnapshot,
    ) -> Result<(), ModerationBallotRuntimeError> {
        self.ensure_snapshot_capacity(&snapshot)?;
        let mut ballots = BTreeMap::new();
        let mut juror_count = 0usize;
        let mut commit_count = 0usize;
        let mut reveal_count = 0usize;
        let mut challenge_count = 0usize;
        for record in snapshot.ballots {
            validate_ballot_record(&record)?;
            let key = ModerationBallotKey::from_announcement(&record.announcement);
            if ballots.contains_key(&key) {
                return Err(invalid_ballot_snapshot(format!(
                    "duplicate moderation ballot `{}` round `{}`",
                    key.case_id, key.round_id
                )));
            }

            let state = ballot_state_from_record(record)?;
            for (resource, total, additional) in [
                (
                    "ballot_jurors",
                    &mut juror_count,
                    state.announcement.juror_ids.len(),
                ),
                ("ballot_commits", &mut commit_count, state.commits.len()),
                ("ballot_reveals", &mut reveal_count, state.reveals.len()),
                (
                    "ballot_challenges",
                    &mut challenge_count,
                    state.challenges.len(),
                ),
            ] {
                *total = total.checked_add(additional).ok_or(
                    ModerationBallotRuntimeError::ResourceExhausted {
                        resource,
                        limit: self.entry_limit,
                    },
                )?;
                if *total > self.entry_limit {
                    return Err(ModerationBallotRuntimeError::ResourceExhausted {
                        resource,
                        limit: self.entry_limit,
                    });
                }
            }
            ballots.insert(key, state);
        }
        self.ballots = ballots;
        self.juror_count = juror_count;
        self.commit_count = commit_count;
        self.reveal_count = reveal_count;
        self.challenge_count = challenge_count;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModerationBallotKey {
    case_id: String,
    round_id: String,
}

impl ModerationBallotKey {
    fn new(case_id: &str, round_id: &str) -> Self {
        Self {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
        }
    }

    fn from_announcement(announcement: &ModerationBallotAnnouncement) -> Self {
        Self::new(&announcement.context.case_id, &announcement.round_id)
    }

    fn from_context_and_round(context: &SoraFsModerationBallotContextV1, round_id: &str) -> Self {
        Self::new(&context.case_id, round_id)
    }
}

#[derive(Debug, Clone)]
struct ModerationBallotState {
    announcement: ModerationBallotAnnouncement,
    commits: BTreeMap<String, SoraFsModerationBallotCommitV1>,
    reveals: BTreeMap<String, SoraFsModerationBallotRevealV1>,
    challenges: BTreeMap<String, ModerationBallotChallengeRecord>,
    tally: Option<ModerationBallotTally>,
}

impl ModerationBallotState {
    fn to_record(&self) -> ModerationBallotRecord {
        ModerationBallotRecord {
            announcement: self.announcement.clone(),
            commits: self.commits.values().cloned().collect(),
            reveals: self.reveals.values().cloned().collect(),
            challenges: self.challenges.values().cloned().collect(),
            tally: self.tally.clone(),
        }
    }
}

fn validate_announcement(
    announcement: &ModerationBallotAnnouncement,
) -> Result<(), ModerationBallotRuntimeError> {
    announcement.context.validate()?;
    if announcement.round_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingRoundId);
    }
    if announcement.juror_ids.is_empty() {
        return Err(ModerationBallotRuntimeError::MissingJurors);
    }
    let mut seen = BTreeSet::new();
    for juror_id in &announcement.juror_ids {
        if juror_id.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::BlankJurorId);
        }
        if !seen.insert(juror_id.clone()) {
            return Err(ModerationBallotRuntimeError::DuplicateJuror {
                juror_id: juror_id.clone(),
            });
        }
    }
    if announcement.quorum == 0 || usize::from(announcement.quorum) > announcement.juror_ids.len() {
        return Err(ModerationBallotRuntimeError::InvalidQuorum {
            quorum: announcement.quorum,
            roster_size: announcement.juror_ids.len(),
        });
    }
    let expected_hash =
        local_moderation_panel_roster_hash(&announcement.juror_ids, announcement.quorum);
    if announcement.context.panel_roster_hash != expected_hash {
        return Err(ModerationBallotRuntimeError::RosterHashMismatch);
    }
    if let Some(deposit) = &announcement.appeal_deposit {
        if deposit.escrow_id_hex.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::MissingAppealDepositEscrowId);
        }
        if announcement
            .appeal_deposit_escrow_id_hex
            .as_deref()
            .is_some_and(|escrow_id| escrow_id != deposit.escrow_id_hex)
        {
            return Err(ModerationBallotRuntimeError::AppealDepositEscrowIdMismatch);
        }
    }
    if announcement.announced_at_unix_ms >= announcement.commit_deadline_unix_ms
        || announcement.commit_deadline_unix_ms >= announcement.challenge_deadline_unix_ms
        || announcement.challenge_deadline_unix_ms >= announcement.reveal_deadline_unix_ms
    {
        return Err(ModerationBallotRuntimeError::InvalidWindows);
    }
    Ok(())
}

fn validate_payload_scope(
    state: &ModerationBallotState,
    context: &SoraFsModerationBallotContextV1,
    round_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state.announcement.context != *context {
        return Err(ModerationBallotRuntimeError::PayloadContextMismatch);
    }
    if state.announcement.round_id != round_id {
        return Err(ModerationBallotRuntimeError::PayloadRoundMismatch {
            expected: state.announcement.round_id.clone(),
            found: round_id.to_owned(),
        });
    }
    Ok(())
}

fn ensure_eligible_juror(
    state: &ModerationBallotState,
    juror_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state
        .announcement
        .juror_ids
        .iter()
        .any(|eligible| eligible == juror_id)
    {
        return Ok(());
    }
    Err(ModerationBallotRuntimeError::IneligibleJuror {
        case_id: state.announcement.context.case_id.clone(),
        round_id: state.announcement.round_id.clone(),
        juror_id: juror_id.to_owned(),
    })
}

fn validate_challenge_input(
    input: &ModerationBallotChallengeInput,
) -> Result<(), ModerationBallotRuntimeError> {
    if input.challenge_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeId);
    }
    if input.challenger_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengerId);
    }
    validate_challenge_target(input.kind, input.target_juror_id.as_deref())?;
    if input.evidence_digest == [0; 32] {
        return Err(ModerationBallotRuntimeError::MissingChallengeEvidence);
    }
    if input.reason.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeReason);
    }
    Ok(())
}

fn validate_challenge_target(
    kind: ModerationBallotChallengeKind,
    target_juror_id: Option<&str>,
) -> Result<(), ModerationBallotRuntimeError> {
    match target_juror_id {
        Some(target_juror_id) if target_juror_id.trim().is_empty() => {
            Err(ModerationBallotRuntimeError::BlankChallengeTarget)
        }
        Some(_) => Ok(()),
        None if kind.requires_target_juror() => {
            Err(ModerationBallotRuntimeError::MissingChallengeTarget { kind })
        }
        None => Ok(()),
    }
}

fn validate_challenge_resolution_input(
    input: &ModerationBallotChallengeResolution,
) -> Result<(), ModerationBallotRuntimeError> {
    if input.case_id.trim().is_empty() || input.round_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::UnknownBallot {
            case_id: input.case_id.clone(),
            round_id: input.round_id.clone(),
        });
    }
    if input.challenge_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeId);
    }
    if input.resolved_by.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeResolver);
    }
    if input
        .note
        .as_deref()
        .is_some_and(|note| note.trim().is_empty())
    {
        return Err(ModerationBallotRuntimeError::BlankChallengeResolutionNote);
    }
    Ok(())
}

fn ensure_no_blocking_challenges(
    state: &ModerationBallotState,
) -> Result<(), ModerationBallotRuntimeError> {
    for challenge in state.challenges.values() {
        match challenge.decision {
            None => {
                return Err(ModerationBallotRuntimeError::ChallengePending {
                    case_id: state.announcement.context.case_id.clone(),
                    round_id: state.announcement.round_id.clone(),
                    challenge_id: challenge.challenge_id.clone(),
                });
            }
            Some(ModerationBallotChallengeDecision::Accepted) => {
                return Err(ModerationBallotRuntimeError::ChallengeAccepted {
                    case_id: state.announcement.context.case_id.clone(),
                    round_id: state.announcement.round_id.clone(),
                    challenge_id: challenge.challenge_id.clone(),
                });
            }
            Some(ModerationBallotChallengeDecision::Rejected) => {}
        }
    }
    Ok(())
}

fn moderation_ballot_no_show_penalty_plan_digest(
    state: &ModerationBallotState,
    plan: &ModerationBallotNoShowPlan,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_BALLOT_NO_SHOW_PENALTY_PLAN_DOMAIN_V1);
    update_string(&mut hasher, &plan.case_id);
    update_string(&mut hasher, &plan.round_id);
    hasher.update(&state.announcement.context.evidence_bundle_digest);
    update_string(
        &mut hasher,
        &state.announcement.context.appeal_finance_config_version,
    );
    hasher.update(&state.announcement.context.panel_roster_hash);
    update_string(&mut hasher, &state.announcement.context.policy_reference);
    update_optional_string(
        &mut hasher,
        state.announcement.context.evidence_uri.as_deref(),
    );
    hasher.update(&state.announcement.announced_at_unix_ms.to_le_bytes());
    hasher.update(&state.announcement.commit_deadline_unix_ms.to_le_bytes());
    hasher.update(&state.announcement.challenge_deadline_unix_ms.to_le_bytes());
    hasher.update(&plan.reveal_deadline_unix_ms.to_le_bytes());
    hasher.update(&plan.quorum.to_le_bytes());
    hasher.update(&plan.roster_size.to_le_bytes());
    hasher.update(&plan.committed_count.to_le_bytes());
    hasher.update(&plan.revealed_count.to_le_bytes());
    hasher.update(&plan.no_show_count.to_le_bytes());
    hasher.update(&[u8::from(plan.quorum_met)]);
    hasher.update(&[u8::from(plan.tally_finalized)]);
    hasher.update(&[u8::from(plan.contested)]);
    update_string_list(&mut hasher, &state.announcement.juror_ids);
    update_string_list(&mut hasher, &plan.missing_commit_juror_ids);
    update_string_list(&mut hasher, &plan.unrevealed_committed_juror_ids);
    update_string_list(&mut hasher, &plan.no_show_juror_ids);
    *hasher.finalize().as_bytes()
}

fn update_string_list(hasher: &mut blake3::Hasher, values: &[String]) {
    hasher.update(&(values.len() as u64).to_le_bytes());
    for value in values {
        update_string(hasher, value);
    }
}

fn invalid_ballot_snapshot(message: impl Into<String>) -> ModerationBallotRuntimeError {
    ModerationBallotRuntimeError::InvalidSnapshot {
        message: message.into(),
    }
}

fn validate_ballot_record(
    record: &ModerationBallotRecord,
) -> Result<(), ModerationBallotRuntimeError> {
    validate_announcement(&record.announcement)?;
    for commit in &record.commits {
        commit.validate()?;
    }
    for reveal in &record.reveals {
        reveal.validate()?;
    }
    for challenge in &record.challenges {
        validate_challenge_record_shape(challenge)?;
    }
    Ok(())
}

fn validate_challenge_record_shape(
    challenge: &ModerationBallotChallengeRecord,
) -> Result<(), ModerationBallotRuntimeError> {
    if challenge.challenge_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeId);
    }
    if challenge.case_id.trim().is_empty() || challenge.round_id.trim().is_empty() {
        return Err(invalid_ballot_snapshot(
            "challenge case id and round id are required",
        ));
    }
    if challenge.challenger_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengerId);
    }
    validate_challenge_target(challenge.kind, challenge.target_juror_id.as_deref())?;
    if challenge.evidence_digest == [0; 32] {
        return Err(ModerationBallotRuntimeError::MissingChallengeEvidence);
    }
    if challenge.reason.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingChallengeReason);
    }
    match challenge.decision {
        Some(_) => {
            if challenge
                .resolved_by
                .as_deref()
                .is_none_or(|resolved_by| resolved_by.trim().is_empty())
            {
                return Err(ModerationBallotRuntimeError::MissingChallengeResolver);
            }
            let Some(resolved_at_unix_ms) = challenge.resolved_at_unix_ms else {
                return Err(invalid_ballot_snapshot(
                    "resolved challenge is missing resolved_at_unix_ms",
                ));
            };
            if resolved_at_unix_ms < challenge.raised_at_unix_ms {
                return Err(ModerationBallotRuntimeError::InvalidChallengeResolutionTimestamp);
            }
            if challenge
                .resolution_note
                .as_deref()
                .is_some_and(|note| note.trim().is_empty())
            {
                return Err(ModerationBallotRuntimeError::BlankChallengeResolutionNote);
            }
        }
        None => {
            if challenge.resolved_by.is_some()
                || challenge.resolved_at_unix_ms.is_some()
                || challenge.resolution_note.is_some()
            {
                return Err(invalid_ballot_snapshot(
                    "pending challenge carries resolution fields",
                ));
            }
        }
    }
    Ok(())
}

fn ballot_state_from_record(
    record: ModerationBallotRecord,
) -> Result<ModerationBallotState, ModerationBallotRuntimeError> {
    let mut state = ModerationBallotState {
        announcement: record.announcement,
        commits: BTreeMap::new(),
        reveals: BTreeMap::new(),
        challenges: BTreeMap::new(),
        tally: None,
    };
    let key = ModerationBallotKey::from_announcement(&state.announcement);

    for commit in record.commits {
        validate_payload_scope(&state, &commit.context, &commit.round_id)?;
        if commit.committed_at_unix_ms < state.announcement.announced_at_unix_ms
            || commit.committed_at_unix_ms > state.announcement.commit_deadline_unix_ms
        {
            return Err(invalid_ballot_snapshot(format!(
                "commit for juror `{}` in ballot `{}` round `{}` is outside the commit window",
                commit.juror_id, key.case_id, key.round_id
            )));
        }
        ensure_eligible_juror(&state, &commit.juror_id)?;
        if state.commits.contains_key(&commit.juror_id) {
            return Err(invalid_ballot_snapshot(format!(
                "duplicate commit for juror `{}` in ballot `{}` round `{}`",
                commit.juror_id, key.case_id, key.round_id
            )));
        }
        state.commits.insert(commit.juror_id.clone(), commit);
    }

    for challenge in record.challenges {
        if challenge.case_id != key.case_id || challenge.round_id != key.round_id {
            return Err(invalid_ballot_snapshot(format!(
                "challenge `{}` in ballot `{}` round `{}` has mismatched scope",
                challenge.challenge_id, key.case_id, key.round_id
            )));
        }
        if challenge.raised_at_unix_ms <= state.announcement.commit_deadline_unix_ms
            || challenge.raised_at_unix_ms > state.announcement.challenge_deadline_unix_ms
        {
            return Err(invalid_ballot_snapshot(format!(
                "challenge `{}` in ballot `{}` round `{}` is outside the challenge window",
                challenge.challenge_id, key.case_id, key.round_id
            )));
        }
        if state
            .challenges
            .insert(challenge.challenge_id.clone(), challenge)
            .is_some()
        {
            return Err(invalid_ballot_snapshot(format!(
                "duplicate challenge in ballot `{}` round `{}`",
                key.case_id, key.round_id
            )));
        }
    }

    for reveal in record.reveals {
        ensure_no_blocking_challenges(&state).map_err(|err| {
            invalid_ballot_snapshot(format!(
                "reveal in ballot `{}` round `{}` would be blocked by restored challenge state: {err}",
                key.case_id, key.round_id
            ))
        })?;
        validate_payload_scope(&state, &reveal.context, &reveal.round_id)?;
        if reveal.revealed_at_unix_ms <= state.announcement.challenge_deadline_unix_ms
            || reveal.revealed_at_unix_ms > state.announcement.reveal_deadline_unix_ms
        {
            return Err(invalid_ballot_snapshot(format!(
                "reveal for juror `{}` in ballot `{}` round `{}` is outside the reveal window",
                reveal.juror_id, key.case_id, key.round_id
            )));
        }
        ensure_eligible_juror(&state, &reveal.juror_id)?;
        let commit = state.commits.get(&reveal.juror_id).ok_or_else(|| {
            invalid_ballot_snapshot(format!(
                "reveal for juror `{}` in ballot `{}` round `{}` has no accepted commit",
                reveal.juror_id, key.case_id, key.round_id
            ))
        })?;
        commit.verify_reveal(&reveal)?;
        if state
            .reveals
            .insert(reveal.juror_id.clone(), reveal)
            .is_some()
        {
            return Err(invalid_ballot_snapshot(format!(
                "duplicate reveal in ballot `{}` round `{}`",
                key.case_id, key.round_id
            )));
        }
    }

    if let Some(tally) = record.tally {
        ensure_no_blocking_challenges(&state).map_err(|err| {
            invalid_ballot_snapshot(format!(
                "tally in ballot `{}` round `{}` would be blocked by restored challenge state: {err}",
                key.case_id, key.round_id
            ))
        })?;
        validate_restored_tally(&state, &tally)?;
        state.tally = Some(tally);
    }
    Ok(state)
}

fn validate_restored_tally(
    state: &ModerationBallotState,
    tally: &ModerationBallotTally,
) -> Result<(), ModerationBallotRuntimeError> {
    let announcement = &state.announcement;
    if tally.case_id != announcement.context.case_id || tally.round_id != announcement.round_id {
        return Err(invalid_ballot_snapshot(
            "tally case id or round id does not match announcement",
        ));
    }
    if tally.quorum != announcement.quorum {
        return Err(invalid_ballot_snapshot(
            "tally quorum does not match announcement",
        ));
    }
    if usize::try_from(tally.votes_total).ok() != Some(state.reveals.len()) {
        return Err(invalid_ballot_snapshot(
            "tally votes_total does not match accepted reveals",
        ));
    }
    if state.reveals.len() < usize::from(announcement.quorum) {
        return Err(invalid_ballot_snapshot("tally does not satisfy quorum"));
    }
    let all_jurors_revealed = state.reveals.len() == announcement.juror_ids.len();
    if tally.tallied_at_unix_ms < announcement.reveal_deadline_unix_ms && !all_jurors_revealed {
        return Err(invalid_ballot_snapshot(
            "early tally requires every announced juror to reveal",
        ));
    }

    let mut counts = ModerationVoteCounts::default();
    for reveal in state.reveals.values() {
        counts.increment(reveal.choice);
    }
    if tally.counts != counts {
        return Err(invalid_ballot_snapshot(
            "tally vote counts do not match accepted reveals",
        ));
    }
    let winning_choice = counts.winning_choice();
    if tally.winning_choice != winning_choice {
        return Err(invalid_ballot_snapshot(
            "tally winning choice does not match vote counts",
        ));
    }
    if tally.contested != winning_choice.is_none() {
        return Err(invalid_ballot_snapshot(
            "tally contested flag does not match vote counts",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sorafs::moderation::SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1;

    use super::*;

    fn screening_input(
        subject: &str,
        verdict: ModerationScreeningVerdict,
    ) -> ModerationScreeningInput {
        ModerationScreeningInput {
            subject: subject.to_owned(),
            subject_digest: *blake3::hash(subject.as_bytes()).as_bytes(),
            manifest_id: [0x12; 16],
            runner_hash: [0x34; 32],
            combined_score_bps: if verdict.requires_quarantine_record() {
                7_000
            } else {
                1_000
            },
            verdict,
            screened_at_unix: 1_800_000_050,
            evidence_digest: Some([0xE1; 32]),
            policy_digest: Some([0xC1; 32]),
            notes: None,
        }
    }

    fn quarantine_object_record(seed: u8) -> ModerationQuarantineObjectRecord {
        seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: [seed; 16],
                payload: vec![seed; 32],
                captured_at_unix: 1_800_000_100 + u64::from(seed),
                content_type: None,
                notes: None,
            },
            [0x7B; 32],
        )
        .expect("seal quarantine object")
        .0
    }

    fn evidence_session_input(
        quarantine_id: [u8; 16],
        nonce: u8,
    ) -> ModerationEvidenceViewerSessionInput {
        ModerationEvidenceViewerSessionInput {
            quarantine_id,
            requested_by: "operator@moderation".to_owned(),
            viewer_account: "juror@moderation".to_owned(),
            viewer_role: "juror".to_owned(),
            purpose: "appeal evidence review".to_owned(),
            attestation_digest: [0xA7; 32],
            watermark_metadata_digest: [0xB7; 32],
            session_nonce_digest: [nonce; 32],
            issued_at_unix_ms: 1_800_000_100_000,
            expires_at_unix_ms: 1_800_000_200_000,
            legal_hold_id: None,
            notes: None,
            raw_evidence_included: false,
            signed_url_included: false,
            session_token_included: false,
            watermark_secret_included: false,
        }
    }

    fn evidence_access_input(session_id: [u8; 16]) -> ModerationEvidenceViewerAccessInput {
        ModerationEvidenceViewerAccessInput {
            session_id,
            kind: ModerationEvidenceViewerAccessKind::Viewed,
            actor_account: "juror@moderation".to_owned(),
            event_at_unix_ms: 1_800_000_100_001,
            request_digest: [0xD7; 32],
            event_metadata_digest: None,
            notes: None,
            raw_evidence_included: false,
            signed_url_included: false,
            session_token_included: false,
            response_body_included: false,
        }
    }

    fn ballot_announcement(case_id: &str) -> ModerationBallotAnnouncement {
        let jurors = vec!["juror-a".to_owned()];
        ModerationBallotAnnouncement {
            context: SoraFsModerationBallotContextV1 {
                version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                case_id: case_id.to_owned(),
                evidence_bundle_digest: [0xAB; 32],
                appeal_finance_config_version: "appeal-finance-v1".to_owned(),
                panel_roster_hash: local_moderation_panel_roster_hash(&jurors, 1),
                policy_reference: "policy://sorafs/moderation/v1".to_owned(),
                evidence_uri: None,
            },
            appeal_deposit_escrow_id_hex: None,
            appeal_deposit: None,
            round_id: "round-1".to_owned(),
            juror_ids: jurors,
            quorum: 1,
            announced_at_unix_ms: 1_800_000_000_000,
            commit_deadline_unix_ms: 1_800_000_010_000,
            challenge_deadline_unix_ms: 1_800_000_020_000,
            reveal_deadline_unix_ms: 1_800_000_030_000,
        }
    }

    #[test]
    fn moderation_quarantine_object_seal_open_preserves_object_id() {
        let local_key = [0x7b; 32];
        let payload = b"quarantine payload bytes".to_vec();
        let input = ModerationQuarantineObjectInput {
            quarantine_id: [0x42; 16],
            payload: payload.clone(),
            captured_at_unix: 1_700_000_001,
            content_type: Some("application/octet-stream".to_owned()),
            notes: Some("unit-test object".to_owned()),
        };

        let (record, envelope_bytes) =
            seal_moderation_quarantine_object(input, local_key).expect("seal object");
        let envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&envelope_bytes)
                .expect("decode envelope");
        let expected_object_id =
            moderation_quarantine_object_id(ModerationQuarantineObjectIdInput {
                quarantine_id: record.quarantine_id,
                payload_digest: record.payload_digest,
                ciphertext_digest: record.ciphertext_digest,
                payload_len: record.payload_len,
                captured_at_unix: record.captured_at_unix,
                content_type: record.content_type.as_deref(),
                notes: record.notes.as_deref(),
                key_id: record.key_id,
            });

        assert_eq!(record.object_id, expected_object_id);
        assert_eq!(envelope.object_id, expected_object_id);
        let opened =
            open_moderation_quarantine_object(envelope, &record, local_key).expect("open object");
        assert_eq!(opened, payload);
    }

    #[test]
    fn authoritative_moderation_collections_refuse_over_limit_without_replacement() {
        let repro = ModerationReproRegistryRecord {
            manifest_id: [1; 16],
            manifest_digest: [2; 32],
            runner_hash: [3; 32],
            runtime_version: "runner-1".to_owned(),
            issued_at_unix: 1,
            model_count: 1,
            signer_count: 1,
        };
        let mut registry = ModerationModelRegistry::with_entry_limit(1);
        registry
            .restore_snapshot(ModerationModelRegistrySnapshot {
                reproducibility_manifests: vec![repro.clone()],
                adversarial_corpora: Vec::new(),
            })
            .expect("restore registry at boundary");
        let registry_before = registry.snapshot();
        let mut second_repro = repro.clone();
        second_repro.manifest_id = [4; 16];
        assert!(matches!(
            registry
                .restore_snapshot(ModerationModelRegistrySnapshot {
                    reproducibility_manifests: vec![repro, second_repro],
                    adversarial_corpora: Vec::new(),
                })
                .expect_err("over-limit registry snapshot must fail"),
            ModerationModelRegistryError::ResourceExhausted { .. }
        ));
        assert_eq!(registry.snapshot(), registry_before);

        let first_object = quarantine_object_record(1);
        let second_object = quarantine_object_record(2);
        let mut objects = ModerationQuarantineObjectRuntime::with_entry_limit(1);
        objects
            .insert(first_object.clone())
            .expect("insert object at boundary");
        assert_eq!(
            objects
                .insert(first_object.clone())
                .expect("replay object at capacity"),
            first_object
        );
        assert!(matches!(
            objects
                .insert(second_object.clone())
                .expect_err("new object above capacity must fail"),
            ModerationQuarantineObjectError::ResourceExhausted { .. }
        ));
        let objects_before = objects.snapshot();
        assert!(matches!(
            objects
                .restore_snapshot(ModerationQuarantineObjectSnapshot {
                    objects: vec![first_object.clone(), second_object],
                })
                .expect_err("over-limit object snapshot must fail"),
            ModerationQuarantineObjectError::ResourceExhausted { .. }
        ));
        assert_eq!(objects.snapshot(), objects_before);

        let mut viewer = ModerationEvidenceViewerRuntime::with_entry_limit(1);
        let first_input = evidence_session_input(first_object.quarantine_id, 1);
        let session = viewer
            .create_session(first_input.clone(), &first_object)
            .expect("create session at boundary");
        assert_eq!(
            viewer
                .create_session(first_input, &first_object)
                .expect("replay session at capacity"),
            session
        );
        assert!(matches!(
            viewer
                .create_session(
                    evidence_session_input(first_object.quarantine_id, 2),
                    &first_object
                )
                .expect_err("new session above capacity must fail"),
            ModerationEvidenceViewerError::ResourceExhausted { .. }
        ));
        viewer
            .record_access(evidence_access_input(session.session_id))
            .expect("record access at boundary");
        assert!(matches!(
            viewer
                .record_access(evidence_access_input(session.session_id))
                .expect_err("new access above capacity must fail"),
            ModerationEvidenceViewerError::ResourceExhausted { .. }
        ));
        let viewer_before = viewer.snapshot();
        let mut extra_session = viewer_before.sessions[0].clone();
        extra_session.session_id = [9; 16];
        let mut over_limit_viewer = viewer_before.clone();
        over_limit_viewer.sessions.push(extra_session);
        assert!(matches!(
            viewer
                .restore_snapshot(over_limit_viewer)
                .expect_err("over-limit viewer snapshot must fail"),
            ModerationEvidenceViewerError::ResourceExhausted { .. }
        ));
        assert_eq!(viewer.snapshot(), viewer_before);

        let mut screening = ModerationScreeningRuntime::with_entry_limit(1);
        let input = screening_input("first", ModerationScreeningVerdict::Quarantine);
        let accepted = screening
            .record_screening(input.clone())
            .expect("record screening at boundary");
        assert_eq!(
            screening
                .record_screening(input)
                .expect("replay screening at capacity")
                .record,
            accepted.record
        );
        assert!(matches!(
            screening
                .record_screening(screening_input("second", ModerationScreeningVerdict::Pass))
                .expect_err("new screening above capacity must fail"),
            ModerationScreeningError::ResourceExhausted { .. }
        ));
        let screening_before = screening.snapshot();
        let mut extra_screening = screening_before.screening_records[0].clone();
        extra_screening.record_id = [8; 16];
        let mut over_limit_screening = screening_before.clone();
        over_limit_screening.screening_records.push(extra_screening);
        assert!(matches!(
            screening
                .restore_snapshot(over_limit_screening)
                .expect_err("over-limit screening snapshot must fail"),
            ModerationScreeningError::ResourceExhausted { .. }
        ));
        assert_eq!(screening.snapshot(), screening_before);

        let mut ballots = ModerationBallotRuntime::with_entry_limit(1);
        ballots
            .announce_ballot(ballot_announcement("first-case"))
            .expect("announce ballot at boundary");
        let ballots_before = ballots.snapshot();
        let mut extra_ballot = ballots_before.ballots[0].clone();
        extra_ballot.announcement = ballot_announcement("second-case");
        let mut over_limit_ballots = ballots_before.clone();
        over_limit_ballots.ballots.push(extra_ballot);
        assert!(matches!(
            ballots
                .restore_snapshot(over_limit_ballots)
                .expect_err("over-limit ballot snapshot must fail"),
            ModerationBallotRuntimeError::ResourceExhausted { .. }
        ));
        assert_eq!(ballots.snapshot(), ballots_before);
    }
}
