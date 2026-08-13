//! SoraFS moderation screening, quarantine, and evidence-viewer runtimes.
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::{Component, Path},
};
use iroha_config::parameters::{ProductionRuntimeHandleError, validate_production_runtime_handle};
use iroha_crypto::{
    PublicKey,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use iroha_data_model::sorafs::moderation::{
    AdversarialCorpusManifestV1, ModerationCommitteeAggregateV1, ModerationReproManifestV1,
    ModerationSignedScreeningResultV1, ModerationTrustPolicyV1,
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use rand::{TryRngCore, rngs::OsRng};
use thiserror::Error;
const MODERATION_SCREENING_RECORD_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.screening-record.v1";
const MODERATION_QUARANTINE_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-record.v1";
const MODERATION_QUARANTINE_OBJECT_ID_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-object-id.v1";
const MODERATION_QUARANTINE_OBJECT_AAD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.quarantine-object.chunk-aad.v1";
const MODERATION_QUARANTINE_OBJECT_CIPHERTEXT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.quarantine-object.ciphertext.v1";
const MODERATION_QUARANTINE_OBJECT_WRAP_CONTEXT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.quarantine-object.wrap-context.v1";
const MODERATION_SCREENING_ADMISSION_RECEIPT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.screening.admission-receipt.v1";
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
/// Maximum records returned from each collection in one V1 local moderation read view.
///
/// This matches the first-release SoraFS HTTP list-page ceiling. Runtime read
/// views enforce it as well so non-HTTP callers cannot accidentally turn a
/// paginated projection back into a full retained-state clone.
pub const MODERATION_READ_VIEW_MAX_RECORDS_V1: usize = 500;
pub(crate) const MODERATION_EVIDENCE_VIEWER_AUDIT_REPORT_VERSION_V1: u16 = 1;
/// Schema version for durable authenticated-screening admission receipts.
pub const MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1: u16 = 1;
/// Schema version for the config-authoritative moderation trust bundle.
pub const MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1: u16 = 1;
pub(crate) const MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_ANCHORS_V1: usize = 64;
pub(crate) const MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1: u16 = 1;
pub(crate) const MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1: &str = "chacha20-poly1305-chunked-v1";
pub(crate) const MODERATION_QUARANTINE_OBJECTS_DIR: &str = "objects";
pub(crate) const MODERATION_QUARANTINE_OBJECT_EXT: &str = "qobj";
pub(crate) const MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1: u32 = 64 * 1024;
pub(crate) const MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1: u64 = 32 * 1024 * 1024;
const MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1: usize = 512;
const MODERATION_QUARANTINE_OBJECT_MAX_CONTENT_TYPE_BYTES_V1: usize = 256;
const MODERATION_QUARANTINE_OBJECT_MAX_KEY_HANDLE_BYTES_V1: usize = 512;
const MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1: usize = 64 * 1024;
const MODERATION_QUARANTINE_OBJECT_AEAD_TAG_BYTES_V1: usize = 16;
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
/// Bounded read view of the local moderation model registry.
///
/// Total counts describe authoritative retained state while the record vectors
/// contain at most the caller's already-admitted response limit. This keeps
/// HTTP readback allocation proportional to the response instead of cloning
/// the full durable registry before pagination.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModerationModelRegistryReadView {
    /// Total admitted reproducibility manifests.
    pub reproducibility_manifest_count: usize,
    /// Total admitted adversarial corpus manifests.
    pub adversarial_corpus_count: usize,
    /// First reproducibility manifest records in canonical map order.
    pub reproducibility_manifests: Vec<ModerationReproRegistryRecord>,
    /// First adversarial corpus records in canonical map order.
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
/// Canonical authenticated authority accepted by the V1 screening admission gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationAuthenticatedScreeningEvidenceV1 {
    /// One runner-signed result. This form is accepted only when the governed
    /// result quorum is exactly one.
    Signed(ModerationSignedScreeningResultV1),
    /// An exact canonical aggregate plus every signed member result needed to
    /// reconstruct and authenticate it.
    Committee {
        /// Aggregate claimed by the submitter.
        aggregate: ModerationCommitteeAggregateV1,
        /// Complete bounded signed member inventory.
        signed_results: Vec<ModerationSignedScreeningResultV1>,
    },
}
/// One replay-scoped request to admit authenticated screening evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationAuthenticatedScreeningRequestV1 {
    /// Non-zero caller idempotency key. Reuse with different evidence is invalid.
    pub idempotency_key: [u8; 32],
    /// Signed result or fully reconstructable committee aggregate.
    pub evidence: ModerationAuthenticatedScreeningEvidenceV1,
}
/// Authenticated, canonical local screening material ready for durable admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationVerifiedScreeningAdmissionV1 {
    /// Caller replay scope.
    pub idempotency_key: [u8; 32],
    /// Exact signed-result evidence digest or committee aggregate digest.
    pub authority_digest: [u8; 32],
    /// Payload-free authority label.
    pub authority_kind: &'static str,
    /// Canonical local projection input derived only after authentication.
    pub screening: ModerationScreeningInput,
}
/// Durable replay receipt for one authenticated screening admission.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningAdmissionReceiptV1 {
    /// Schema version.
    pub version: u16,
    /// Non-zero caller idempotency key.
    pub idempotency_key: [u8; 32],
    /// Exact signed-result evidence digest or committee aggregate digest.
    pub authority_digest: [u8; 32],
    /// Canonical authority label (`signed_result` or `committee_aggregate`).
    pub authority_kind: String,
    /// Screening record created from the authenticated authority.
    pub screening_record_id: [u8; 16],
    /// Domain-separated digest of every preceding receipt field.
    pub receipt_digest: [u8; 32],
}
/// Outcome of durably admitting authenticated screening evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationAuthenticatedScreeningOutcomeV1 {
    /// Replay and authority binding committed with the screening snapshot.
    pub admission: ModerationScreeningAdmissionReceiptV1,
    /// Canonical screening/quarantine projection.
    pub screening: ModerationScreeningOutcome,
}
/// Authentication failure at the V1 screening admission boundary.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationScreeningAuthenticationError {
    /// Idempotency keys are mandatory and may not use an inert all-zero value.
    #[error("moderation screening idempotency key must be non-zero")]
    MissingIdempotencyKey,
    /// The externally anchored trust policy or manifest is invalid.
    #[error("moderation screening trust policy is invalid: {message}")]
    InvalidTrustPolicy {
        /// Payload-free validation detail.
        message: String,
    },
    /// A single signed result attempted to bypass a multi-signer policy.
    #[error("single moderation screening result cannot satisfy governed result quorum {required}")]
    CommitteeRequired {
        /// Governed distinct signer threshold.
        required: u16,
    },
    /// A runner-signed result failed signature, authorization, binding, score, or freshness checks.
    #[error("signed moderation screening result is invalid: {message}")]
    InvalidSignedResult {
        /// Payload-free validation detail.
        message: String,
    },
    /// Committee member authentication or deterministic aggregation failed.
    #[error("moderation screening committee aggregate is invalid: {message}")]
    InvalidCommittee {
        /// Payload-free validation detail.
        message: String,
    },
    /// The submitted aggregate is not byte-for-byte equal to the canonical
    /// aggregate reconstructed from its member signatures.
    #[error("submitted moderation committee aggregate is not canonical")]
    NonCanonicalAggregate,
    /// The authenticated verdict is outside the canonical first-release set.
    #[error("authenticated moderation verdict `{verdict}` is unsupported")]
    UnsupportedVerdict {
        /// Rejected verdict label.
        verdict: String,
    },
    /// Canonical screening-policy digest derivation failed.
    #[error("failed to derive moderation screening policy digest: {message}")]
    PolicyDigest {
        /// Encoding detail.
        message: String,
    },
    /// No active externally anchored screening authority is installed.
    #[error("moderation screening authority is unavailable: {message}")]
    AuthorityUnavailable {
        /// Payload-free availability detail.
        message: String,
    },
    /// A policy update attempted to replace the active authority with an older policy.
    #[error(
        "moderation screening policy rollback rejected: active issue time {active_issued_at_unix}, candidate issue time {candidate_issued_at_unix}"
    )]
    PolicyRollback {
        /// Active policy issue timestamp.
        active_issued_at_unix: u64,
        /// Candidate policy issue timestamp.
        candidate_issued_at_unix: u64,
    },
    /// A policy with the same issue time conflicted with the active policy digest.
    #[error("moderation screening policy equivocation rejected at issue time {issued_at_unix}")]
    PolicyEquivocation {
        /// Conflicting policy issue timestamp.
        issued_at_unix: u64,
    },
}
/// Failure while authenticating or durably admitting screening evidence.
#[derive(Debug, Error)]
pub enum ModerationAuthenticatedScreeningAdmissionError {
    /// Signature, quorum, governance-anchor, binding, freshness, or replay
    /// evidence validation failed before local mutation.
    #[error(transparent)]
    Authentication(#[from] ModerationScreeningAuthenticationError),
    /// The authenticated result could not be committed to local durable state.
    #[error(transparent)]
    Runtime(#[from] ModerationScreeningError),
}
/// Canonical, non-secret authority bundle loaded from an `iroha_config` path.
///
/// The deployment configuration separately pins the BLAKE3 digest of the
/// exact canonical Norito bytes, so replacing this local file cannot silently
/// change the active screening authority.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningAuthorityBundleV1 {
    /// Schema version.
    pub version: u16,
    /// Governance-signed reproducibility manifest.
    pub manifest: ModerationReproManifestV1,
    /// Governance-signed runner trust policy.
    pub policy: ModerationTrustPolicyV1,
    /// Strictly sorted, unique externally reviewed governance trust anchors.
    pub governance_trust_anchors: Vec<PublicKey>,
    /// Minimum distinct governance anchors required by the deployment.
    pub minimum_governance_quorum: u16,
}
impl ModerationScreeningAuthorityBundleV1 {
    /// Validate and convert the configured bundle into an active authority.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version, a non-canonical or
    /// unbounded anchor inventory, an invalid quorum, or any invalid
    /// manifest/policy/signature/validity binding.
    pub fn into_authority(
        self,
        now_unix: u64,
    ) -> Result<ModerationScreeningAuthorityV1, ModerationScreeningAuthenticationError> {
        if self.version != MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1 {
            return Err(ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                message: format!(
                    "authority bundle version must be {}, got {}",
                    MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1, self.version
                ),
            });
        }
        if self.governance_trust_anchors.is_empty()
            || self.governance_trust_anchors.len()
                > MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_ANCHORS_V1
        {
            return Err(ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                message: format!(
                    "authority bundle governance anchors must contain 1..={} entries",
                    MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_ANCHORS_V1
                ),
            });
        }
        if self
            .governance_trust_anchors
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                message: "authority bundle governance anchors must be strictly sorted and unique"
                    .to_owned(),
            });
        }
        if self.minimum_governance_quorum == 0
            || usize::from(self.minimum_governance_quorum) > self.governance_trust_anchors.len()
        {
            return Err(ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                message: "authority bundle minimum governance quorum is outside its anchor set"
                    .to_owned(),
            });
        }
        ModerationScreeningAuthorityV1::new(
            self.manifest,
            self.policy,
            self.governance_trust_anchors.into_iter().collect(),
            self.minimum_governance_quorum,
            now_unix,
        )
    }
}
/// Active, externally anchored authority used by the production screening API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationScreeningAuthorityV1 {
    manifest: ModerationReproManifestV1,
    policy: ModerationTrustPolicyV1,
    governance_trust_anchors: BTreeSet<PublicKey>,
    minimum_governance_quorum: u16,
}
impl ModerationScreeningAuthorityV1 {
    /// Validate and construct an active screening authority.
    ///
    /// # Errors
    ///
    /// Returns an error if the policy, manifest, externally trusted governance
    /// signatures, validity interval, or quorum is invalid at `now_unix`.
    pub fn new(
        manifest: ModerationReproManifestV1,
        policy: ModerationTrustPolicyV1,
        governance_trust_anchors: BTreeSet<PublicKey>,
        minimum_governance_quorum: u16,
        now_unix: u64,
    ) -> Result<Self, ModerationScreeningAuthenticationError> {
        policy
            .validate_with_trust_anchors(
                &manifest,
                &governance_trust_anchors,
                minimum_governance_quorum,
                now_unix,
            )
            .map_err(
                |error| ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                    message: error.to_string(),
                },
            )?;
        Ok(Self {
            manifest,
            policy,
            governance_trust_anchors,
            minimum_governance_quorum,
        })
    }
    /// Exact active trust-policy digest.
    #[must_use]
    pub fn policy_digest(&self) -> [u8; 32] {
        self.policy.body.policy_digest
    }
    /// Active trust-policy issue timestamp.
    #[must_use]
    pub fn policy_issued_at_unix(&self) -> u64 {
        self.policy.body.issued_at_unix
    }
    /// Exact active reproducibility manifest identifier.
    #[must_use]
    pub fn manifest_id(&self) -> [u8; 16] {
        self.manifest.body.manifest_id
    }
    pub(crate) fn verify(
        &self,
        request: ModerationAuthenticatedScreeningRequestV1,
        now_unix: u64,
    ) -> Result<ModerationVerifiedScreeningAdmissionV1, ModerationScreeningAuthenticationError>
    {
        verify_authenticated_moderation_screening_v1(
            request,
            &self.manifest,
            &self.policy,
            &self.governance_trust_anchors,
            self.minimum_governance_quorum,
            now_unix,
        )
    }
}
/// Authenticate one canonical screening result or committee aggregate against
/// externally trusted governance anchors.
///
/// This is the only production-grade conversion into
/// [`ModerationScreeningInput`]. Callers must durably bind
/// `idempotency_key -> authority_digest` before accepting a retry.
pub fn verify_authenticated_moderation_screening_v1(
    request: ModerationAuthenticatedScreeningRequestV1,
    manifest: &ModerationReproManifestV1,
    policy: &ModerationTrustPolicyV1,
    governance_trust_anchors: &BTreeSet<PublicKey>,
    minimum_governance_quorum: u16,
    now_unix: u64,
) -> Result<ModerationVerifiedScreeningAdmissionV1, ModerationScreeningAuthenticationError> {
    if request.idempotency_key == [0; 32] {
        return Err(ModerationScreeningAuthenticationError::MissingIdempotencyKey);
    }
    policy
        .validate_with_trust_anchors(
            manifest,
            governance_trust_anchors,
            minimum_governance_quorum,
            now_unix,
        )
        .map_err(
            |error| ModerationScreeningAuthenticationError::InvalidTrustPolicy {
                message: error.to_string(),
            },
        )?;
    let policy_digest = manifest
        .body
        .computed_screening_policy_digest()
        .map_err(
            |error| ModerationScreeningAuthenticationError::PolicyDigest {
                message: error.to_string(),
            },
        )?;
    let (
        authority_digest,
        authority_kind,
        subject,
        subject_digest,
        combined_score_bps,
        verdict,
        screened_at_unix,
        notes,
    ) = match request.evidence {
        ModerationAuthenticatedScreeningEvidenceV1::Signed(result) => {
            if policy.body.result_quorum != 1 {
                return Err(ModerationScreeningAuthenticationError::CommitteeRequired {
                    required: policy.body.result_quorum,
                });
            }
            result
                .validate(manifest, policy, now_unix)
                .map_err(
                    |error| ModerationScreeningAuthenticationError::InvalidSignedResult {
                        message: error.to_string(),
                    },
                )?;
            (
                result.body.evidence_digest,
                "signed_result",
                result.body.subject,
                result.body.subject_digest,
                result.body.combined_score_bps,
                result.body.verdict,
                result.body.screened_at_unix,
                result.body.notes,
            )
        }
        ModerationAuthenticatedScreeningEvidenceV1::Committee {
            aggregate,
            signed_results,
        } => {
            let canonical = ModerationCommitteeAggregateV1::aggregate_authenticated(
                manifest,
                policy,
                governance_trust_anchors,
                minimum_governance_quorum,
                &signed_results,
                now_unix,
            )
            .map_err(|error| {
                ModerationScreeningAuthenticationError::InvalidCommittee {
                    message: error.to_string(),
                }
            })?;
            if aggregate != canonical {
                return Err(ModerationScreeningAuthenticationError::NonCanonicalAggregate);
            }
            (
                canonical.aggregate_digest,
                "committee_aggregate",
                canonical.subject,
                canonical.subject_digest,
                canonical.aggregated_score_bps,
                canonical.verdict,
                canonical.aggregated_at_unix,
                None,
            )
        }
    };
    let verdict = match verdict.as_str() {
        "pass" => ModerationScreeningVerdict::Pass,
        "quarantine" => ModerationScreeningVerdict::Quarantine,
        "escalate" => ModerationScreeningVerdict::Escalate,
        _ => {
            return Err(ModerationScreeningAuthenticationError::UnsupportedVerdict { verdict });
        }
    };
    Ok(ModerationVerifiedScreeningAdmissionV1 {
        idempotency_key: request.idempotency_key,
        authority_digest,
        authority_kind,
        screening: ModerationScreeningInput {
            subject,
            subject_digest,
            manifest_id: manifest.body.manifest_id,
            runner_hash: manifest.body.runner_hash,
            combined_score_bps,
            verdict,
            screened_at_unix,
            evidence_digest: Some(authority_digest),
            policy_digest: Some(policy_digest),
            notes,
        },
    })
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
#[derive(Clone, PartialEq, Eq)]
pub struct ModerationQuarantineObjectInput {
    /// Stable quarantine id that owns this payload object.
    pub quarantine_id: [u8; 16],
    /// Plaintext payload bytes to seal locally.
    pub payload: Vec<u8>,
    /// Unix timestamp (seconds) when the payload was captured.
    pub captured_at_unix: u64,
    /// Optional media/content type label for operator review.
    ///
    /// V1 accepts only a coarse allowlisted media label. It must never contain
    /// filenames, parameters, identities, or private data.
    pub content_type: Option<String>,
    /// Reserved in V1 and required to be absent. Private notes belong inside
    /// the encrypted payload, never in plaintext envelope metadata.
    pub notes: Option<String>,
}
impl std::fmt::Debug for ModerationQuarantineObjectInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationQuarantineObjectInput")
            .field("quarantine_id", &self.quarantine_id)
            .field("payload", &"<redacted>")
            .field("payload_len", &self.payload.len())
            .field("captured_at_unix", &self.captured_at_unix)
            .field(
                "content_type_len",
                &self.content_type.as_deref().map(str::len),
            )
            .field("notes_len", &self.notes.as_deref().map(str::len))
            .finish()
    }
}
impl Drop for ModerationQuarantineObjectInput {
    fn drop(&mut self) {
        self.payload.fill(0);
        let _ = std::hint::black_box(&self.payload);
        scrub_optional_quarantine_text(&mut self.content_type);
        scrub_optional_quarantine_text(&mut self.notes);
    }
}
fn scrub_optional_quarantine_text(value: &mut Option<String>) {
    if let Some(value) = value.take() {
        scrub_owned_quarantine_text(value);
    }
}
fn scrub_owned_quarantine_text(value: String) {
    let mut bytes = value.into_bytes();
    bytes.fill(0);
    let _ = std::hint::black_box(&bytes);
}
/// Persisted index record for one encrypted local quarantine payload object.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationQuarantineObjectRecord {
    /// Stable quarantine id that owns this payload object.
    pub quarantine_id: [u8; 16],
    /// Stable object id derived from immutable sealing metadata before encryption.
    ///
    /// The ciphertext digest is bound and validated separately by this record.
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
    /// Reserved V1 field. Canonical records require this to be absent because
    /// private notes belong inside the encrypted payload.
    pub notes: Option<String>,
    /// Local at-rest encryption algorithm label.
    pub encryption_algorithm: String,
    /// Random per-object nonce prefix. The checked chunk index forms the final
    /// 96-bit ChaCha20-Poly1305 nonce.
    pub nonce_prefix: [u8; 8],
    /// Maximum plaintext bytes carried by each independently authenticated chunk.
    pub chunk_plaintext_bytes: u32,
    /// Number of independently authenticated chunks in the envelope.
    pub chunk_count: u32,
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
/// Authenticated byte range from a local quarantine object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineObjectRangePayload {
    /// Persisted object index record.
    pub record: ModerationQuarantineObjectRecord,
    /// Inclusive plaintext byte offset.
    pub start: u64,
    /// Exclusive plaintext byte offset.
    pub end: u64,
    /// Decrypted bytes in `start..end`.
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
    /// Authenticated admission receipts sorted by idempotency key.
    pub authenticated_admissions: Vec<ModerationScreeningAdmissionReceiptV1>,
}
/// Bounded read view of local screening and quarantine state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModerationScreeningReadView {
    /// Total authenticated screening admissions retained locally.
    pub authenticated_admission_count: usize,
    /// Total screening records retained locally.
    pub screening_count: usize,
    /// Total quarantine records retained locally.
    pub quarantine_count: usize,
    /// First screening records in canonical map order.
    pub screening_records: Vec<ModerationScreeningRecord>,
    /// First quarantine records in canonical map order.
    pub quarantine_records: Vec<ModerationQuarantineRecord>,
}
/// Bounded read view of the local quarantine queue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModerationQuarantineReadView {
    /// Total quarantine records retained locally.
    pub quarantine_count: usize,
    /// First quarantine records in canonical map order.
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
    /// An idempotency key was reused with different authenticated evidence.
    #[error(
        "moderation screening idempotency key `{idempotency_key_hex}` conflicts with its durable receipt"
    )]
    ConflictingIdempotencyKey {
        /// Conflicting idempotency key as lowercase hex.
        idempotency_key_hex: String,
    },
    /// Authenticated evidence was replayed under a different idempotency key.
    #[error(
        "moderation screening authority `{authority_digest_hex}` was already admitted under idempotency key `{existing_idempotency_key_hex}`"
    )]
    ReplayedAuthority {
        /// Replayed authority digest as lowercase hex.
        authority_digest_hex: String,
        /// Original idempotency key as lowercase hex.
        existing_idempotency_key_hex: String,
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
    /// A runtime-only PKCS#11/KMS wrapping provider is unavailable.
    #[error("moderation quarantine object key wrapper is unavailable")]
    KeyWrapperUnavailable,
    /// The runtime-only PKCS#11/KMS provider failed its exact public binding.
    #[error("moderation quarantine object key wrapper qualification failed")]
    KeyWrapperUnqualified,
    /// The configured PKCS#11/KMS wrapping operation failed closed.
    #[error("moderation quarantine object key operation failed for `{key_id}`: {failure}")]
    KeyWrapping {
        /// Opaque, non-secret PKCS#11/KMS key handle.
        key_id: String,
        /// Stable payload-free provider failure class.
        failure: ModerationQuarantineKeyOperationErrorV1,
    },
    /// The requested authenticated plaintext range is invalid.
    #[error(
        "moderation quarantine object range {start}..{end} exceeds plaintext length {payload_len}"
    )]
    InvalidRange {
        /// Inclusive byte offset.
        start: u64,
        /// Exclusive byte offset.
        end: u64,
        /// Authenticated plaintext length.
        payload_len: u64,
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
impl ModerationQuarantineObjectError {
    /// Construct a stable payload-free key-operation failure.
    #[must_use]
    pub fn key_operation_failure(
        key_id: String,
        failure: ModerationQuarantineKeyOperationErrorV1,
    ) -> Self {
        Self::KeyWrapping { key_id, failure }
    }
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
        let family_count = u32::try_from(manifest.families.len()).map_err(|_| {
            ModerationModelRegistryError::InvalidCorpusManifest {
                message: "family count does not fit u32".to_owned(),
            }
        })?;
        let variant_count = manifest
            .families
            .iter()
            .try_fold(0_usize, |total, family| {
                total.checked_add(family.variants.len())
            })
            .and_then(|total| u32::try_from(total).ok())
            .ok_or_else(|| ModerationModelRegistryError::InvalidCorpusManifest {
                message: "variant count overflows u32".to_owned(),
            })?;
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
    pub(crate) fn read_view(&self, limit: usize) -> ModerationModelRegistryReadView {
        let limit = limit.min(MODERATION_READ_VIEW_MAX_RECORDS_V1);
        ModerationModelRegistryReadView {
            reproducibility_manifest_count: self.repro_manifests.len(),
            adversarial_corpus_count: self.corpora.len(),
            reproducibility_manifests: self.repro_manifests.values().take(limit).cloned().collect(),
            adversarial_corpora: self.corpora.values().take(limit).cloned().collect(),
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
    authenticated_admissions: BTreeMap<[u8; 32], ModerationScreeningAdmissionReceiptV1>,
    admitted_authorities: BTreeMap<[u8; 32], [u8; 32]>,
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
            authenticated_admissions: BTreeMap::new(),
            admitted_authorities: BTreeMap::new(),
            entry_limit: entry_limit.max(1),
        }
    }
    pub(crate) fn record_authenticated_screening(
        &mut self,
        verified: ModerationVerifiedScreeningAdmissionV1,
    ) -> Result<ModerationAuthenticatedScreeningOutcomeV1, ModerationScreeningError> {
        if verified.idempotency_key == [0; 32] || verified.authority_digest == [0; 32] {
            return Err(ModerationScreeningError::InvalidInput {
                message:
                    "authenticated screening idempotency and authority digests must be non-zero"
                        .to_owned(),
            });
        }
        validate_screening_authority_kind(verified.authority_kind)
            .map_err(|message| ModerationScreeningError::InvalidInput { message })?;
        if let Some(existing) = self.authenticated_admissions.get(&verified.idempotency_key) {
            if existing.authority_digest != verified.authority_digest
                || existing.authority_kind != verified.authority_kind
            {
                return Err(ModerationScreeningError::ConflictingIdempotencyKey {
                    idempotency_key_hex: hex::encode(verified.idempotency_key),
                });
            }
            let screening = self.screening_outcome(existing.screening_record_id)?;
            return Ok(ModerationAuthenticatedScreeningOutcomeV1 {
                admission: existing.clone(),
                screening,
            });
        }
        if let Some(existing_idempotency_key) =
            self.admitted_authorities.get(&verified.authority_digest)
        {
            return Err(ModerationScreeningError::ReplayedAuthority {
                authority_digest_hex: hex::encode(verified.authority_digest),
                existing_idempotency_key_hex: hex::encode(existing_idempotency_key),
            });
        }
        if self.authenticated_admissions.len() >= self.entry_limit {
            return Err(ModerationScreeningError::ResourceExhausted {
                resource: "authenticated_screening_admissions",
                limit: self.entry_limit,
            });
        }
        if verified.screening.evidence_digest != Some(verified.authority_digest) {
            return Err(ModerationScreeningError::InvalidInput {
                message:
                    "authenticated screening projection must retain the exact authority digest"
                        .to_owned(),
            });
        }
        let idempotency_key = verified.idempotency_key;
        let authority_digest = verified.authority_digest;
        let authority_kind = verified.authority_kind.to_owned();
        let screening = self.record_screening(verified.screening)?;
        let admission = screening_admission_receipt(
            idempotency_key,
            authority_digest,
            authority_kind,
            screening.record.record_id,
        );
        self.admitted_authorities
            .insert(authority_digest, idempotency_key);
        self.authenticated_admissions
            .insert(idempotency_key, admission.clone());
        Ok(ModerationAuthenticatedScreeningOutcomeV1 {
            admission,
            screening,
        })
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
            authenticated_admissions: self.authenticated_admissions.values().cloned().collect(),
        }
    }
    pub(crate) fn read_view(&self, limit: usize) -> ModerationScreeningReadView {
        let limit = limit.min(MODERATION_READ_VIEW_MAX_RECORDS_V1);
        ModerationScreeningReadView {
            authenticated_admission_count: self.authenticated_admissions.len(),
            screening_count: self.screening_records.len(),
            quarantine_count: self.quarantine_records.len(),
            screening_records: self
                .screening_records
                .values()
                .take(limit)
                .cloned()
                .collect(),
            quarantine_records: self
                .quarantine_records
                .values()
                .take(limit)
                .cloned()
                .collect(),
        }
    }
    pub(crate) fn quarantine_read_view(&self, limit: usize) -> ModerationQuarantineReadView {
        let limit = limit.min(MODERATION_READ_VIEW_MAX_RECORDS_V1);
        ModerationQuarantineReadView {
            quarantine_count: self.quarantine_records.len(),
            quarantine_records: self
                .quarantine_records
                .values()
                .take(limit)
                .cloned()
                .collect(),
        }
    }
    pub(crate) fn quarantine_record(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Option<ModerationQuarantineRecord> {
        self.quarantine_records.get(quarantine_id).cloned()
    }
    fn screening_outcome(
        &self,
        screening_record_id: [u8; 16],
    ) -> Result<ModerationScreeningOutcome, ModerationScreeningError> {
        let record = self
            .screening_records
            .get(&screening_record_id)
            .cloned()
            .ok_or_else(|| ModerationScreeningError::InvalidSnapshot {
                message: format!(
                    "authenticated admission references unknown screening record `{}`",
                    hex::encode(screening_record_id)
                ),
            })?;
        let quarantine = record
            .verdict
            .requires_quarantine_record()
            .then(|| quarantine_record_from_screening(&record))
            .and_then(|expected| {
                self.quarantine_records
                    .get(&expected.quarantine_id)
                    .cloned()
            });
        Ok(ModerationScreeningOutcome { record, quarantine })
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
            (
                "authenticated_screening_admissions",
                snapshot.authenticated_admissions.len(),
            ),
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
        let mut authenticated_admissions = BTreeMap::new();
        let mut admitted_authorities = BTreeMap::new();
        for admission in snapshot.authenticated_admissions {
            let screening_record = screening_records
                .get(&admission.screening_record_id)
                .ok_or_else(|| ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "authenticated admission `{}` references unknown screening record `{}`",
                        hex::encode(admission.idempotency_key),
                        hex::encode(admission.screening_record_id)
                    ),
                })?;
            validate_screening_admission_receipt(&admission, screening_record)?;
            if authenticated_admissions
                .insert(admission.idempotency_key, admission.clone())
                .is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate authenticated screening idempotency key".to_owned(),
                });
            }
            if admitted_authorities
                .insert(admission.authority_digest, admission.idempotency_key)
                .is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate authenticated screening authority digest".to_owned(),
                });
            }
        }
        self.screening_records = screening_records;
        self.quarantine_records = quarantine_records;
        self.authenticated_admissions = authenticated_admissions;
        self.admitted_authorities = admitted_authorities;
        Ok(())
    }
}
fn screening_admission_receipt(
    idempotency_key: [u8; 32],
    authority_digest: [u8; 32],
    authority_kind: String,
    screening_record_id: [u8; 16],
) -> ModerationScreeningAdmissionReceiptV1 {
    let receipt_digest = screening_admission_receipt_digest(
        MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1,
        idempotency_key,
        authority_digest,
        &authority_kind,
        screening_record_id,
    );
    ModerationScreeningAdmissionReceiptV1 {
        version: MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1,
        idempotency_key,
        authority_digest,
        authority_kind,
        screening_record_id,
        receipt_digest,
    }
}
fn screening_admission_receipt_digest(
    version: u16,
    idempotency_key: [u8; 32],
    authority_digest: [u8; 32],
    authority_kind: &str,
    screening_record_id: [u8; 16],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SCREENING_ADMISSION_RECEIPT_DOMAIN_V1);
    hasher.update(&version.to_le_bytes());
    hasher.update(&idempotency_key);
    hasher.update(&authority_digest);
    hasher.update(&len_to_u64(authority_kind.len()).to_le_bytes());
    hasher.update(authority_kind.as_bytes());
    hasher.update(&screening_record_id);
    *hasher.finalize().as_bytes()
}
fn validate_screening_authority_kind(authority_kind: &str) -> Result<(), String> {
    if matches!(authority_kind, "signed_result" | "committee_aggregate") {
        Ok(())
    } else {
        Err(format!(
            "authenticated screening authority kind `{authority_kind}` is not canonical"
        ))
    }
}
fn validate_screening_admission_receipt(
    admission: &ModerationScreeningAdmissionReceiptV1,
    screening_record: &ModerationScreeningRecord,
) -> Result<(), ModerationScreeningError> {
    if admission.version != MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "authenticated screening admission uses unsupported version {}",
                admission.version
            ),
        });
    }
    if admission.idempotency_key == [0; 32] || admission.authority_digest == [0; 32] {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message:
                "authenticated screening admission has an all-zero idempotency or authority digest"
                    .to_owned(),
        });
    }
    validate_screening_authority_kind(&admission.authority_kind)
        .map_err(|message| ModerationScreeningError::InvalidSnapshot { message })?;
    if screening_record.evidence_digest != Some(admission.authority_digest) {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "authenticated screening admission `{}` does not match its record evidence digest",
                hex::encode(admission.idempotency_key)
            ),
        });
    }
    let expected_digest = screening_admission_receipt_digest(
        admission.version,
        admission.idempotency_key,
        admission.authority_digest,
        &admission.authority_kind,
        admission.screening_record_id,
    );
    if admission.receipt_digest != expected_digest {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "authenticated screening admission `{}` has an invalid receipt digest",
                hex::encode(admission.idempotency_key)
            ),
        });
    }
    Ok(())
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
/// Public, non-secret qualification for one quarantine-key provider.
///
/// The revision identifies the deployment-owned adapter and its public policy
/// revision. The digest binds that exact public policy without exposing
/// credentials, key material, vendor diagnostics, or private configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationQuarantineKeyProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}
impl ModerationQuarantineKeyProviderQualificationV1 {
    /// Construct one provider qualification observation.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }
    /// Return the non-zero deployment adapter and public-policy revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Return the non-zero digest of the provider's public policy.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }
    fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}
/// Fixed, payload-free readiness failures returned by a quarantine-key provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModerationQuarantineKeyProviderReadinessErrorV1 {
    /// The provider or one of its runtime-only credentials is unavailable.
    #[error("moderation quarantine key provider unavailable")]
    Unavailable,
    /// The provider is revoked, stale, unauthorized, or otherwise ineligible.
    #[error("moderation quarantine key provider rejected qualification")]
    Rejected,
}
/// Stable, payload-free failure classes for quarantine-key operations.
///
/// An adapter must classify protected provider diagnostics at its own boundary,
/// scrub the diagnostic with
/// [`Self::after_scrubbing_provider_diagnostic`], and expose only one of these
/// variants. `Unavailable`, `Rejected`, and `StaleOrRevoked` are definitive:
/// the adapter knows that the requested wrap did not complete.
/// `Ambiguous` is reserved for a wrap request that may have reached the
/// provider before transport was lost. The caller must not replay that request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModerationQuarantineKeyOperationErrorV1 {
    /// The provider was unreachable before it could accept the operation.
    #[error("moderation quarantine key operation unavailable")]
    Unavailable,
    /// The provider definitively rejected the operation.
    #[error("moderation quarantine key operation rejected")]
    Rejected,
    /// The referenced key or governing provider policy is stale or revoked.
    #[error("moderation quarantine key or policy is stale or revoked")]
    StaleOrRevoked,
    /// A wrap may have completed after dispatch, so replay is unsafe.
    #[error("moderation quarantine key wrap outcome is ambiguous")]
    Ambiguous,
}
impl ModerationQuarantineKeyOperationErrorV1 {
    /// Scrub one protected provider diagnostic and return this fixed class.
    ///
    /// Provider diagnostics may contain credentials, key material, tenant
    /// identifiers, or private policy detail. This method consumes and
    /// overwrites that text before it can cross the runtime adapter boundary.
    #[must_use]
    pub fn after_scrubbing_provider_diagnostic(self, provider_detail: String) -> Self {
        scrub_owned_quarantine_text(provider_detail);
        self
    }
}
/// Stable, payload-free quarantine-key provider qualification failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModerationQuarantineKeyProviderQualificationErrorV1 {
    /// The configured opaque provider handle is malformed.
    #[error("configured moderation quarantine key provider handle is invalid")]
    InvalidConfiguredHandle,
    /// The configured handle is explicitly marked for test or development use.
    #[error("configured moderation quarantine key provider handle is test-marked")]
    TestMarkedConfiguredHandle,
    /// The injected provider's opaque handle is malformed.
    #[error("injected moderation quarantine key provider handle is invalid")]
    InvalidProviderHandle,
    /// The injected provider advertises a test- or development-marked handle.
    #[error("injected moderation quarantine key provider handle is test-marked")]
    TestMarkedProviderHandle,
    /// The configured provider revision or public-policy digest is zero.
    #[error("configured moderation quarantine key provider qualification is invalid")]
    InvalidConfiguredQualification,
    /// The injected provider does not match the configured stable handle.
    #[error("moderation quarantine key provider handle does not match configuration")]
    SubstitutedProvider,
    /// Qualification could not prove that the provider is current and usable.
    #[error("moderation quarantine key provider is unavailable, stale, or unqualified")]
    UnavailableOrStale,
    /// The provider returned a zero revision or all-zero public-policy digest.
    #[error("moderation quarantine key provider returned an invalid qualification")]
    InvalidQualification,
    /// The provider does not match the independently governed qualification.
    #[error("moderation quarantine key provider qualification does not match configuration")]
    QualificationMismatch,
    /// The provider identity or public policy changed around an external operation.
    #[error("moderation quarantine key provider identity or policy changed during operation")]
    IdentityOrPolicyChanged,
}
/// Independently configured exact binding for one quarantine-key provider.
///
/// Keep this value next to the injected runtime provider and construct it only
/// from stable `iroha_config` fields. Credentials, private keys, tokens, and
/// provider diagnostics must never enter this binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineKeyProviderBindingV1 {
    provider_handle: String,
    expected_qualification: ModerationQuarantineKeyProviderQualificationV1,
}
impl ModerationQuarantineKeyProviderBindingV1 {
    /// Construct and validate an independently governed provider binding.
    ///
    /// # Errors
    ///
    /// Fails when the configured handle is malformed or test-marked, or when
    /// the configured revision or policy digest is zero.
    pub fn try_new(
        provider_handle: String,
        expected_qualification: ModerationQuarantineKeyProviderQualificationV1,
    ) -> Result<Self, ModerationQuarantineKeyProviderQualificationErrorV1> {
        validate_moderation_quarantine_key_provider_handle(&provider_handle, true)?;
        if !expected_qualification.is_valid() {
            return Err(
                ModerationQuarantineKeyProviderQualificationErrorV1::InvalidConfiguredQualification,
            );
        }
        Ok(Self {
            provider_handle,
            expected_qualification,
        })
    }
    /// Return the configured stable opaque provider handle.
    #[must_use]
    pub fn provider_handle(&self) -> &str {
        &self.provider_handle
    }
    /// Return the configured public provider qualification.
    #[must_use]
    pub const fn expected_qualification(&self) -> ModerationQuarantineKeyProviderQualificationV1 {
        self.expected_qualification
    }
    /// Qualify an injected provider against this exact configured binding.
    ///
    /// # Errors
    ///
    /// Fails for invalid or test-marked provider handles, substitutions,
    /// unavailable or stale providers, invalid observations, and qualification
    /// mismatches.
    pub fn qualify(
        &self,
        provider: &dyn ModerationQuarantineKeyWrapper,
    ) -> Result<(), ModerationQuarantineKeyProviderQualificationErrorV1> {
        validate_moderation_quarantine_key_provider_handle(provider.provider_handle(), false)?;
        if provider.provider_handle() != self.provider_handle {
            return Err(ModerationQuarantineKeyProviderQualificationErrorV1::SubstitutedProvider);
        }
        let qualification = provider
            .qualification()
            .map_err(|_| ModerationQuarantineKeyProviderQualificationErrorV1::UnavailableOrStale)?;
        if !qualification.is_valid() {
            return Err(ModerationQuarantineKeyProviderQualificationErrorV1::InvalidQualification);
        }
        if qualification != self.expected_qualification {
            return Err(ModerationQuarantineKeyProviderQualificationErrorV1::QualificationMismatch);
        }
        if provider.provider_handle() != self.provider_handle {
            return Err(
                ModerationQuarantineKeyProviderQualificationErrorV1::IdentityOrPolicyChanged,
            );
        }
        Ok(())
    }
    fn revalidate(
        &self,
        provider: &dyn ModerationQuarantineKeyWrapper,
    ) -> Result<(), ModerationQuarantineKeyProviderQualificationErrorV1> {
        if provider.provider_handle() != self.provider_handle {
            return Err(
                ModerationQuarantineKeyProviderQualificationErrorV1::IdentityOrPolicyChanged,
            );
        }
        let qualification = provider
            .qualification()
            .map_err(|_| ModerationQuarantineKeyProviderQualificationErrorV1::UnavailableOrStale)?;
        if !qualification.is_valid() {
            return Err(ModerationQuarantineKeyProviderQualificationErrorV1::InvalidQualification);
        }
        if provider.provider_handle() != self.provider_handle
            || qualification != self.expected_qualification
        {
            return Err(
                ModerationQuarantineKeyProviderQualificationErrorV1::IdentityOrPolicyChanged,
            );
        }
        Ok(())
    }
}
/// Runtime-only adapter for wrapping per-object data-encryption keys.
///
/// Production implementations are expected to call PKCS#11 or a KMS and keep
/// all key material outside the process configuration and durable object
/// envelope. Implementations must authenticate `context_digest` during both
/// wrap and unwrap so a wrapped DEK cannot be replayed onto another object.
/// `qualification` must fail for unavailable, revoked, stale, substituted, or
/// test-marked adapters, and its revision or digest must change whenever the
/// public adapter/key policy changes.
///
/// Wrapping is treated as mutating for retry purposes. An implementation must
/// not retry after dispatch when it cannot prove whether the provider completed
/// the request; it returns
/// [`ModerationQuarantineKeyOperationErrorV1::Ambiguous`] instead. The object
/// runtime never retries a wrap. Failed sealing discards its fresh DEK; failed
/// rewrapping leaves the authoritative envelope unchanged and requires external
/// reconciliation before another attempt. No ambiguous provider result is
/// persisted as a completed envelope.
///
/// Unwrapping is read-only and idempotent for an exact key id, context digest,
/// and wrapped DEK. Implementations must not return `Ambiguous` from
/// [`Self::unwrap_dek`]; transport uncertainty is `Unavailable` and may be
/// retried safely at a higher read boundary. The object runtime itself performs
/// each unwrap at most once per invocation.
pub trait ModerationQuarantineKeyWrapper: Send + Sync + std::fmt::Debug {
    /// Return the stable, non-secret deployment handle for this provider.
    fn provider_handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    >;
    /// Return the active non-secret PKCS#11/KMS wrapping-key handle.
    fn active_key_id(&self) -> &str;
    /// Wrap one freshly generated 256-bit DEK for durable storage.
    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1>;
    /// Unwrap one DEK using the exact key handle persisted in its envelope.
    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1>;
}
fn validate_moderation_quarantine_key_provider_handle(
    handle: &str,
    configured: bool,
) -> Result<(), ModerationQuarantineKeyProviderQualificationErrorV1> {
    validate_production_runtime_handle(handle).map_err(|error| match (configured, error) {
        (true, ProductionRuntimeHandleError::InvalidSyntax) => {
            ModerationQuarantineKeyProviderQualificationErrorV1::InvalidConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::InvalidSyntax) => {
            ModerationQuarantineKeyProviderQualificationErrorV1::InvalidProviderHandle
        }
        (true, ProductionRuntimeHandleError::TestMarked) => {
            ModerationQuarantineKeyProviderQualificationErrorV1::TestMarkedConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::TestMarked) => {
            ModerationQuarantineKeyProviderQualificationErrorV1::TestMarkedProviderHandle
        }
    })
}
fn map_moderation_quarantine_key_provider_qualification_error(
    _error: ModerationQuarantineKeyProviderQualificationErrorV1,
) -> ModerationQuarantineObjectError {
    ModerationQuarantineObjectError::KeyWrapperUnqualified
}
pub(crate) fn validate_moderation_quarantine_key_wrapper(
    binding: &ModerationQuarantineKeyProviderBindingV1,
    key_wrapper: &dyn ModerationQuarantineKeyWrapper,
) -> Result<(), ModerationQuarantineObjectError> {
    binding
        .qualify(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let key_id = clean_wrapping_key_id(key_wrapper.active_key_id())?;
    binding
        .revalidate(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let revalidated_key_id = clean_wrapping_key_id(key_wrapper.active_key_id())
        .map_err(|_| ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
    if revalidated_key_id != key_id {
        return Err(ModerationQuarantineObjectError::KeyWrapperUnqualified);
    }
    Ok(())
}
/// One independently authenticated ciphertext chunk.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct ModerationQuarantineCiphertextChunkV1 {
    /// Zero-based nonce and ordering index.
    pub index: u32,
    /// Plaintext byte offset.
    pub plaintext_offset: u64,
    /// Plaintext bytes protected by this chunk.
    pub plaintext_len: u32,
    /// ChaCha20-Poly1305 ciphertext followed by its 16-byte tag.
    pub ciphertext: Vec<u8>,
}
/// Canonical V1 chunked ChaCha20-Poly1305 quarantine object envelope.
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
    /// Reserved V1 field; validation requires `None`.
    pub notes: Option<String>,
    /// Opaque non-secret PKCS#11/KMS key handle.
    pub wrapping_key_id: String,
    /// Provider-produced, context-bound wrapped per-object DEK.
    pub wrapped_dek: Vec<u8>,
    /// Random 64-bit prefix combined with a checked 32-bit chunk index.
    pub nonce_prefix: [u8; 8],
    pub chunk_plaintext_bytes: u32,
    pub chunks: Vec<ModerationQuarantineCiphertextChunkV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationQuarantineImmutableMetadataV1 {
    version: u16,
    algorithm: String,
    quarantine_id: [u8; 16],
    payload_digest: [u8; 32],
    payload_len: u64,
    captured_at_unix: u64,
    content_type: Option<String>,
    /// Reserved V1 field; validation requires `None`.
    notes: Option<String>,
    nonce_prefix: [u8; 8],
    chunk_plaintext_bytes: u32,
    chunk_count: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationQuarantineAadHeaderV1 {
    metadata: ModerationQuarantineImmutableMetadataV1,
    object_id: [u8; 16],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationQuarantineChunkAadV1 {
    header_digest: [u8; 32],
    index: u32,
    plaintext_offset: u64,
    plaintext_len: u32,
}
struct ModerationQuarantineDek([u8; 32]);
impl Drop for ModerationQuarantineDek {
    fn drop(&mut self) {
        self.0.fill(0);
        let _ = std::hint::black_box(&self.0);
    }
}
struct ModerationQuarantineWrappedDek(Vec<u8>);
impl ModerationQuarantineWrappedDek {
    fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}
impl Drop for ModerationQuarantineWrappedDek {
    fn drop(&mut self) {
        self.0.fill(0);
        let _ = std::hint::black_box(&self.0);
    }
}
struct ModerationQuarantinePlaintext(Vec<u8>);
impl ModerationQuarantinePlaintext {
    fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}
impl Drop for ModerationQuarantinePlaintext {
    fn drop(&mut self) {
        self.0.fill(0);
        let _ = std::hint::black_box(&self.0);
    }
}
pub(crate) fn seal_moderation_quarantine_object(
    input: ModerationQuarantineObjectInput,
    key_provider_binding: &ModerationQuarantineKeyProviderBindingV1,
    key_wrapper: &dyn ModerationQuarantineKeyWrapper,
) -> Result<(ModerationQuarantineObjectRecord, Vec<u8>), ModerationQuarantineObjectError> {
    let mut cleaned = normalize_moderation_quarantine_object_input(input)?;
    let payload_digest = *blake3::hash(&cleaned.payload).as_bytes();
    let payload_len = u64::try_from(cleaned.payload.len()).map_err(|_| {
        ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_payload_bytes",
            limit: usize::try_from(MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1)
                .unwrap_or(usize::MAX),
        }
    })?;
    let chunk_plaintext_bytes = MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1;
    let chunk_size = usize::try_from(chunk_plaintext_bytes).map_err(|_| {
        ModerationQuarantineObjectError::InvalidInput {
            message: "canonical chunk size does not fit this platform".to_owned(),
        }
    })?;
    let chunk_count = u32::try_from(cleaned.payload.len().div_ceil(chunk_size)).map_err(|_| {
        ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_chunks",
            limit: MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1,
        }
    })?;
    let mut dek = ModerationQuarantineDek([0_u8; 32]);
    fill_nonzero_random(&mut dek.0, "data-encryption key")?;
    let mut nonce_prefix = [0_u8; 8];
    fill_nonzero_random(&mut nonce_prefix, "nonce prefix")?;
    let metadata = ModerationQuarantineImmutableMetadataV1 {
        version: MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1,
        algorithm: MODERATION_QUARANTINE_OBJECT_ALGORITHM_V1.to_owned(),
        quarantine_id: cleaned.quarantine_id,
        payload_digest,
        payload_len,
        captured_at_unix: cleaned.captured_at_unix,
        content_type: cleaned.content_type.take(),
        notes: cleaned.notes.take(),
        nonce_prefix,
        chunk_plaintext_bytes,
        chunk_count,
    };
    let object_id = moderation_quarantine_object_id(&metadata)?;
    let header = ModerationQuarantineAadHeaderV1 {
        metadata,
        object_id,
    };
    let header_digest = moderation_quarantine_aad_header_digest(&header)?;
    key_provider_binding
        .revalidate(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let wrapping_key_id = clean_wrapping_key_id(key_wrapper.active_key_id())?;
    let wrapped_dek = key_wrapper
        .wrap_dek(moderation_quarantine_wrap_context_digest(&header)?, &dek.0)
        .map(ModerationQuarantineWrappedDek)
        .map_err(|failure| {
            ModerationQuarantineObjectError::key_operation_failure(wrapping_key_id.clone(), failure)
        })?;
    key_provider_binding
        .revalidate(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let revalidated_key_id = clean_wrapping_key_id(key_wrapper.active_key_id())
        .map_err(|_| ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
    if revalidated_key_id != wrapping_key_id {
        return Err(ModerationQuarantineObjectError::KeyWrapperUnqualified);
    }
    validate_wrapped_dek(&wrapped_dek.0)?;
    let wrapped_dek = wrapped_dek.into_vec();
    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&dek.0).map_err(|error| {
            ModerationQuarantineObjectError::InvalidInput {
                message: format!("failed to initialize ChaCha20-Poly1305: {error}"),
            }
        })?;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(usize::try_from(chunk_count).unwrap_or(usize::MAX))
        .map_err(|_| ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_chunks",
            limit: MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1,
        })?;
    for (index, plaintext) in cleaned.payload.chunks(chunk_size).enumerate() {
        let index = u32::try_from(index).map_err(|_| {
            ModerationQuarantineObjectError::ResourceExhausted {
                resource: "quarantine_object_chunks",
                limit: MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1,
            }
        })?;
        let plaintext_offset = u64::from(index)
            .checked_mul(u64::from(chunk_plaintext_bytes))
            .ok_or_else(|| ModerationQuarantineObjectError::InvalidInput {
                message: "quarantine object chunk offset overflow".to_owned(),
            })?;
        let plaintext_len = u32::try_from(plaintext.len()).map_err(|_| {
            ModerationQuarantineObjectError::InvalidInput {
                message: "quarantine object chunk length does not fit u32".to_owned(),
            }
        })?;
        let aad =
            moderation_quarantine_chunk_aad(header_digest, index, plaintext_offset, plaintext_len)?;
        let nonce = moderation_quarantine_chunk_nonce(nonce_prefix, index);
        let ciphertext = encryptor
            .encrypt(nonce.as_slice(), aad.as_slice(), plaintext)
            .map_err(|error| ModerationQuarantineObjectError::InvalidInput {
                message: format!("ChaCha20-Poly1305 encryption failed: {error}"),
            })?;
        chunks.push(ModerationQuarantineCiphertextChunkV1 {
            index,
            plaintext_offset,
            plaintext_len,
            ciphertext,
        });
    }
    let ciphertext_digest = moderation_quarantine_ciphertext_digest(&chunks);
    let envelope = ModerationQuarantineObjectEnvelopeV1 {
        version: header.metadata.version,
        algorithm: header.metadata.algorithm,
        quarantine_id: header.metadata.quarantine_id,
        object_id,
        payload_digest: header.metadata.payload_digest,
        ciphertext_digest,
        payload_len: header.metadata.payload_len,
        captured_at_unix: header.metadata.captured_at_unix,
        content_type: header.metadata.content_type,
        notes: header.metadata.notes,
        wrapping_key_id,
        wrapped_dek,
        nonce_prefix: header.metadata.nonce_prefix,
        chunk_plaintext_bytes: header.metadata.chunk_plaintext_bytes,
        chunks,
    };
    let envelope_path =
        moderation_quarantine_object_relative_path(envelope.quarantine_id, envelope.object_id);
    let record = moderation_quarantine_object_record_from_envelope(&envelope, envelope_path)?;
    let bytes =
        norito::to_bytes(&envelope).map_err(|err| ModerationQuarantineObjectError::Codec {
            message: err.to_string(),
        })?;
    Ok((record, bytes))
}
pub(crate) fn open_moderation_quarantine_object(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    record: &ModerationQuarantineObjectRecord,
    key_provider_binding: &ModerationQuarantineKeyProviderBindingV1,
    key_wrapper: &dyn ModerationQuarantineKeyWrapper,
) -> Result<Vec<u8>, ModerationQuarantineObjectError> {
    let payload = ModerationQuarantinePlaintext(open_moderation_quarantine_object_range(
        envelope,
        record,
        key_provider_binding,
        key_wrapper,
        0..envelope.payload_len,
    )?);
    if *blake3::hash(&payload.0).as_bytes() != envelope.payload_digest {
        return Err(authentication_failed(envelope.quarantine_id));
    }
    Ok(payload.into_vec())
}
/// Authenticate and decrypt only chunks intersecting `range`.
///
/// Every returned byte is covered by ChaCha20-Poly1305 with immutable object
/// metadata, chunk index, offset, and length in AAD. A reordered or substituted
/// chunk therefore fails before any plaintext is returned.
pub(crate) fn open_moderation_quarantine_object_range(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    record: &ModerationQuarantineObjectRecord,
    key_provider_binding: &ModerationQuarantineKeyProviderBindingV1,
    key_wrapper: &dyn ModerationQuarantineKeyWrapper,
    range: Range<u64>,
) -> Result<Vec<u8>, ModerationQuarantineObjectError> {
    validate_quarantine_object_envelope(envelope)?;
    let rebuilt = moderation_quarantine_object_record_from_envelope(
        envelope,
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
    if range.start > range.end || range.end > envelope.payload_len {
        return Err(ModerationQuarantineObjectError::InvalidRange {
            start: range.start,
            end: range.end,
            payload_len: envelope.payload_len,
        });
    }
    let header = quarantine_aad_header_from_envelope(envelope)?;
    let header_digest = moderation_quarantine_aad_header_digest(&header)?;
    let wrap_context = moderation_quarantine_wrap_context_digest(&header)?;
    key_provider_binding
        .revalidate(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let dek = key_wrapper
        .unwrap_dek(
            &envelope.wrapping_key_id,
            wrap_context,
            &envelope.wrapped_dek,
        )
        .map(ModerationQuarantineDek)
        .map_err(|failure| {
            ModerationQuarantineObjectError::key_operation_failure(
                envelope.wrapping_key_id.clone(),
                failure,
            )
        })?;
    key_provider_binding
        .revalidate(key_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&dek.0)
        .map_err(|_| authentication_failed(envelope.quarantine_id))?;
    let output_len = usize::try_from(range.end - range.start).map_err(|_| {
        ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_range_bytes",
            limit: usize::try_from(MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1)
                .unwrap_or(usize::MAX),
        }
    })?;
    let mut output = ModerationQuarantinePlaintext(Vec::new());
    output.0.try_reserve_exact(output_len).map_err(|_| {
        ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_range_bytes",
            limit: usize::try_from(MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1)
                .unwrap_or(usize::MAX),
        }
    })?;
    for chunk in &envelope.chunks {
        let chunk_end = chunk
            .plaintext_offset
            .checked_add(u64::from(chunk.plaintext_len))
            .ok_or_else(|| ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object chunk end overflow".to_owned(),
            })?;
        if chunk_end <= range.start || chunk.plaintext_offset >= range.end {
            continue;
        }
        let aad = moderation_quarantine_chunk_aad(
            header_digest,
            chunk.index,
            chunk.plaintext_offset,
            chunk.plaintext_len,
        )?;
        let nonce = moderation_quarantine_chunk_nonce(envelope.nonce_prefix, chunk.index);
        let plaintext = ModerationQuarantinePlaintext(
            decryptor
                .decrypt(
                    nonce.as_slice(),
                    aad.as_slice(),
                    chunk.ciphertext.as_slice(),
                )
                .map_err(|_| authentication_failed(envelope.quarantine_id))?,
        );
        if plaintext.0.len() != usize::try_from(chunk.plaintext_len).unwrap_or(usize::MAX) {
            return Err(authentication_failed(envelope.quarantine_id));
        }
        let copy_start = range.start.max(chunk.plaintext_offset) - chunk.plaintext_offset;
        let copy_end = range.end.min(chunk_end) - chunk.plaintext_offset;
        let copy_start = usize::try_from(copy_start)
            .map_err(|_| authentication_failed(envelope.quarantine_id))?;
        let copy_end =
            usize::try_from(copy_end).map_err(|_| authentication_failed(envelope.quarantine_id))?;
        output
            .0
            .extend_from_slice(&plaintext.0[copy_start..copy_end]);
    }
    if output.0.len() != output_len {
        return Err(authentication_failed(envelope.quarantine_id));
    }
    Ok(output.into_vec())
}
/// Rewrap a per-object DEK without decrypting or rewriting ciphertext chunks.
pub(crate) fn rewrap_moderation_quarantine_object(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    record: &ModerationQuarantineObjectRecord,
    current_key_provider_binding: &ModerationQuarantineKeyProviderBindingV1,
    current_wrapper: &dyn ModerationQuarantineKeyWrapper,
    replacement_key_provider_binding: &ModerationQuarantineKeyProviderBindingV1,
    replacement_wrapper: &dyn ModerationQuarantineKeyWrapper,
) -> Result<(ModerationQuarantineObjectRecord, Vec<u8>), ModerationQuarantineObjectError> {
    validate_quarantine_object_envelope(envelope)?;
    let rebuilt = moderation_quarantine_object_record_from_envelope(
        envelope,
        moderation_quarantine_object_relative_path(envelope.quarantine_id, envelope.object_id),
    )?;
    if &rebuilt != record {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: "cannot rewrap an envelope that differs from its durable index".to_owned(),
        });
    }
    let header = quarantine_aad_header_from_envelope(envelope)?;
    let context = moderation_quarantine_wrap_context_digest(&header)?;
    current_key_provider_binding
        .revalidate(current_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let dek = current_wrapper
        .unwrap_dek(&envelope.wrapping_key_id, context, &envelope.wrapped_dek)
        .map(ModerationQuarantineDek)
        .map_err(|failure| {
            ModerationQuarantineObjectError::key_operation_failure(
                envelope.wrapping_key_id.clone(),
                failure,
            )
        })?;
    current_key_provider_binding
        .revalidate(current_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    authenticate_moderation_quarantine_ciphertext(envelope, &dek.0)?;
    replacement_key_provider_binding
        .revalidate(replacement_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let replacement_key_id = clean_wrapping_key_id(replacement_wrapper.active_key_id())?;
    let replacement_wrapped_dek = replacement_wrapper
        .wrap_dek(context, &dek.0)
        .map(ModerationQuarantineWrappedDek)
        .map_err(|failure| {
            ModerationQuarantineObjectError::key_operation_failure(
                replacement_key_id.clone(),
                failure,
            )
        })?;
    replacement_key_provider_binding
        .revalidate(replacement_wrapper)
        .map_err(map_moderation_quarantine_key_provider_qualification_error)?;
    let revalidated_replacement_key_id = clean_wrapping_key_id(replacement_wrapper.active_key_id())
        .map_err(|_| ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
    if revalidated_replacement_key_id != replacement_key_id {
        return Err(ModerationQuarantineObjectError::KeyWrapperUnqualified);
    }
    validate_wrapped_dek(&replacement_wrapped_dek.0)?;
    let replacement_wrapped_dek = replacement_wrapped_dek.into_vec();
    let mut replacement = envelope.clone();
    replacement.wrapping_key_id = replacement_key_id;
    replacement.wrapped_dek = replacement_wrapped_dek;
    validate_quarantine_object_envelope(&replacement)?;
    debug_assert_eq!(replacement.object_id, envelope.object_id);
    debug_assert_eq!(replacement.ciphertext_digest, envelope.ciphertext_digest);
    debug_assert_eq!(replacement.chunks, envelope.chunks);
    let replacement_record = moderation_quarantine_object_record_from_envelope(
        &replacement,
        record.envelope_path.clone(),
    )?;
    let bytes =
        norito::to_bytes(&replacement).map_err(|error| ModerationQuarantineObjectError::Codec {
            message: error.to_string(),
        })?;
    Ok((replacement_record, bytes))
}
fn authenticate_moderation_quarantine_ciphertext(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
    dek: &[u8; 32],
) -> Result<(), ModerationQuarantineObjectError> {
    let header = quarantine_aad_header_from_envelope(envelope)?;
    let header_digest = moderation_quarantine_aad_header_digest(&header)?;
    let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(dek)
        .map_err(|_| authentication_failed(envelope.quarantine_id))?;
    let mut payload_hasher = blake3::Hasher::new();
    let mut authenticated_len = 0_u64;
    for chunk in &envelope.chunks {
        let aad = moderation_quarantine_chunk_aad(
            header_digest,
            chunk.index,
            chunk.plaintext_offset,
            chunk.plaintext_len,
        )?;
        let nonce = moderation_quarantine_chunk_nonce(envelope.nonce_prefix, chunk.index);
        let plaintext = ModerationQuarantinePlaintext(
            decryptor
                .decrypt(
                    nonce.as_slice(),
                    aad.as_slice(),
                    chunk.ciphertext.as_slice(),
                )
                .map_err(|_| authentication_failed(envelope.quarantine_id))?,
        );
        if plaintext.0.len() != usize::try_from(chunk.plaintext_len).unwrap_or(usize::MAX) {
            return Err(authentication_failed(envelope.quarantine_id));
        }
        authenticated_len = authenticated_len
            .checked_add(u64::from(chunk.plaintext_len))
            .ok_or_else(|| authentication_failed(envelope.quarantine_id))?;
        payload_hasher.update(&plaintext.0);
    }
    if authenticated_len != envelope.payload_len
        || payload_hasher.finalize().as_bytes() != &envelope.payload_digest
    {
        return Err(authentication_failed(envelope.quarantine_id));
    }
    Ok(())
}
pub(crate) fn normalize_moderation_quarantine_object_input(
    mut input: ModerationQuarantineObjectInput,
) -> Result<ModerationQuarantineObjectInput, ModerationQuarantineObjectError> {
    if input.captured_at_unix == 0 {
        return Err(ModerationQuarantineObjectError::InvalidInput {
            message: "captured_at_unix must be non-zero".to_string(),
        });
    }
    if input.payload.is_empty() {
        return Err(ModerationQuarantineObjectError::InvalidInput {
            message: "quarantine payload must not be empty".to_owned(),
        });
    }
    if len_to_u64(input.payload.len()) > MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1 {
        return Err(ModerationQuarantineObjectError::ResourceExhausted {
            resource: "quarantine_object_payload_bytes",
            limit: usize::try_from(MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1)
                .unwrap_or(usize::MAX),
        });
    }
    if input.notes.is_some() {
        return Err(ModerationQuarantineObjectError::InvalidInput {
            message:
                "plaintext quarantine object notes are forbidden in V1; include private notes inside the encrypted payload"
                    .to_owned(),
        });
    }
    let content_type = clean_optional_quarantine_content_type(
        input.content_type.take(),
        MODERATION_QUARANTINE_OBJECT_MAX_CONTENT_TYPE_BYTES_V1,
    )?;
    let payload = std::mem::take(&mut input.payload);
    Ok(ModerationQuarantineObjectInput {
        quarantine_id: input.quarantine_id,
        payload,
        captured_at_unix: input.captured_at_unix,
        content_type,
        notes: None,
    })
}
fn clean_optional_quarantine_content_type(
    value: Option<String>,
    max_bytes: usize,
) -> Result<Option<String>, ModerationQuarantineObjectError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !is_canonical_quarantine_content_type(&value, max_bytes) {
        scrub_owned_quarantine_text(value);
        return Err(ModerationQuarantineObjectError::InvalidInput {
            message: format!(
                "content_type must be a V1 allowlisted coarse media label of at most {max_bytes} bytes without parameters or private data"
            ),
        });
    }
    Ok(Some(value))
}
fn is_canonical_quarantine_content_type(value: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
        && matches!(
            value,
            "application/octet-stream"
                | "application/json"
                | "application/pdf"
                | "audio/mpeg"
                | "audio/ogg"
                | "audio/wav"
                | "image/gif"
                | "image/jpeg"
                | "image/png"
                | "image/webp"
                | "text/plain"
                | "video/mp4"
                | "video/webm"
        )
}
pub(crate) fn validate_quarantine_object_envelope(
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
    if envelope.quarantine_id == [0; 16]
        || envelope.object_id == [0; 16]
        || envelope.payload_digest == [0; 32]
        || envelope.ciphertext_digest == [0; 32]
        || envelope.nonce_prefix == [0; 8]
    {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: "quarantine object envelope has a zero identity, digest, or nonce prefix"
                .to_owned(),
        });
    }
    if envelope.payload_len == 0
        || envelope.payload_len > MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1
    {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` payload length is outside V1 bounds",
                hex::encode(envelope.object_id)
            ),
        });
    }
    if envelope
        .content_type
        .as_deref()
        .is_some_and(|content_type| {
            !is_canonical_quarantine_content_type(
                content_type,
                MODERATION_QUARANTINE_OBJECT_MAX_CONTENT_TYPE_BYTES_V1,
            )
        })
    {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: "quarantine object content_type is not a V1 allowlisted coarse media label"
                .to_owned(),
        });
    }
    if envelope.notes.is_some() {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: "plaintext quarantine object notes are forbidden in V1".to_owned(),
        });
    }
    validate_wrapping_key_id_text(&envelope.wrapping_key_id)
        .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
    validate_wrapped_dek(&envelope.wrapped_dek)?;
    if envelope.chunk_plaintext_bytes != MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` uses non-canonical chunk size {}",
                hex::encode(envelope.object_id),
                envelope.chunk_plaintext_bytes
            ),
        });
    }
    let expected_chunk_count_u64 = envelope
        .payload_len
        .div_ceil(u64::from(envelope.chunk_plaintext_bytes));
    let expected_chunk_count = usize::try_from(expected_chunk_count_u64).map_err(|_| {
        ModerationQuarantineObjectError::InvalidSnapshot {
            message: "quarantine object chunk count does not fit this platform".to_owned(),
        }
    })?;
    if expected_chunk_count == 0
        || expected_chunk_count > MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1
        || envelope.chunks.len() != expected_chunk_count
    {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` has {} chunks; expected {expected_chunk_count}",
                hex::encode(envelope.object_id),
                envelope.chunks.len()
            ),
        });
    }
    let header = quarantine_aad_header_from_envelope(envelope)?;
    let expected_object_id = moderation_quarantine_object_id(&header.metadata)?;
    if envelope.object_id != expected_object_id {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` id does not match immutable metadata",
                hex::encode(envelope.object_id)
            ),
        });
    }
    let mut expected_offset = 0_u64;
    for (expected_index, chunk) in envelope.chunks.iter().enumerate() {
        let expected_index = u32::try_from(expected_index).map_err(|_| {
            ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object chunk index does not fit u32".to_owned(),
            }
        })?;
        if chunk.index != expected_index || chunk.plaintext_offset != expected_offset {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: format!(
                    "object envelope `{}` chunks are reordered or have non-contiguous offsets",
                    hex::encode(envelope.object_id)
                ),
            });
        }
        if chunk.plaintext_len == 0
            || chunk.plaintext_len > envelope.chunk_plaintext_bytes
            || (expected_index + 1 != u32::try_from(expected_chunk_count).unwrap_or(u32::MAX)
                && chunk.plaintext_len != envelope.chunk_plaintext_bytes)
        {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: format!(
                    "object envelope `{}` chunk {} has a non-canonical plaintext length",
                    hex::encode(envelope.object_id),
                    chunk.index
                ),
            });
        }
        let expected_ciphertext_len = usize::try_from(chunk.plaintext_len)
            .ok()
            .and_then(|len| len.checked_add(MODERATION_QUARANTINE_OBJECT_AEAD_TAG_BYTES_V1))
            .ok_or_else(|| ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object ciphertext length overflow".to_owned(),
            })?;
        if chunk.ciphertext.len() != expected_ciphertext_len {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: format!(
                    "object envelope `{}` chunk {} has invalid AEAD ciphertext length",
                    hex::encode(envelope.object_id),
                    chunk.index
                ),
            });
        }
        expected_offset = expected_offset
            .checked_add(u64::from(chunk.plaintext_len))
            .ok_or_else(|| ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object plaintext offset overflow".to_owned(),
            })?;
    }
    if expected_offset != envelope.payload_len {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "object envelope `{}` chunks do not cover payload_len",
                hex::encode(envelope.object_id)
            ),
        });
    }
    if moderation_quarantine_ciphertext_digest(&envelope.chunks) != envelope.ciphertext_digest {
        return Err(authentication_failed(envelope.quarantine_id));
    }
    Ok(())
}
pub(crate) fn moderation_quarantine_object_record_from_envelope(
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
        nonce_prefix: envelope.nonce_prefix,
        chunk_plaintext_bytes: envelope.chunk_plaintext_bytes,
        chunk_count: u32::try_from(envelope.chunks.len()).map_err(|_| {
            ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object chunk count does not fit u32".to_owned(),
            }
        })?,
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
    if record.quarantine_id == [0; 16]
        || record.object_id == [0; 16]
        || record.payload_digest == [0; 32]
        || record.ciphertext_digest == [0; 32]
        || record.nonce_prefix == [0; 8]
    {
        return Err(format!(
            "quarantine object `{}` has a zero identity, digest, or nonce prefix",
            hex::encode(record.object_id)
        ));
    }
    if record.payload_len == 0
        || record.payload_len > MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1
    {
        return Err(format!(
            "quarantine object `{}` payload length is outside V1 bounds",
            hex::encode(record.object_id)
        ));
    }
    if record.content_type.as_deref().is_some_and(|content_type| {
        !is_canonical_quarantine_content_type(
            content_type,
            MODERATION_QUARANTINE_OBJECT_MAX_CONTENT_TYPE_BYTES_V1,
        )
    }) {
        return Err(format!(
            "quarantine object `{}` content_type is not a V1 allowlisted coarse media label",
            hex::encode(record.object_id)
        ));
    }
    if record.notes.is_some() {
        return Err(format!(
            "quarantine object `{}` contains forbidden plaintext notes",
            hex::encode(record.object_id)
        ));
    }
    if record.chunk_plaintext_bytes != MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 {
        return Err(format!(
            "quarantine object `{}` uses non-canonical chunk size {}",
            hex::encode(record.object_id),
            record.chunk_plaintext_bytes
        ));
    }
    let expected_chunk_count = record
        .payload_len
        .div_ceil(u64::from(record.chunk_plaintext_bytes));
    if u64::from(record.chunk_count) != expected_chunk_count
        || usize::try_from(record.chunk_count).unwrap_or(usize::MAX)
            > MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1
    {
        return Err(format!(
            "quarantine object `{}` has invalid chunk count {}",
            hex::encode(record.object_id),
            record.chunk_count
        ));
    }
    let metadata = ModerationQuarantineImmutableMetadataV1 {
        version: MODERATION_QUARANTINE_OBJECT_ENVELOPE_VERSION_V1,
        algorithm: record.encryption_algorithm.clone(),
        quarantine_id: record.quarantine_id,
        payload_digest: record.payload_digest,
        payload_len: record.payload_len,
        captured_at_unix: record.captured_at_unix,
        content_type: record.content_type.clone(),
        notes: record.notes.clone(),
        nonce_prefix: record.nonce_prefix,
        chunk_plaintext_bytes: record.chunk_plaintext_bytes,
        chunk_count: record.chunk_count,
    };
    let expected_id = moderation_quarantine_object_id(&metadata)
        .map_err(|error| format!("failed to encode quarantine object metadata: {error}"))?;
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
fn fill_nonzero_random(
    output: &mut [u8],
    label: &str,
) -> Result<(), ModerationQuarantineObjectError> {
    let mut rng = OsRng;
    for _ in 0..4 {
        rng.try_fill_bytes(output).map_err(|error| {
            ModerationQuarantineObjectError::InvalidInput {
                message: format!("failed to generate quarantine object {label}: {error}"),
            }
        })?;
        if output.iter().any(|byte| *byte != 0) {
            return Ok(());
        }
    }
    Err(ModerationQuarantineObjectError::InvalidInput {
        message: format!("failed to generate non-zero quarantine object {label}"),
    })
}
fn clean_wrapping_key_id(key_id: &str) -> Result<String, ModerationQuarantineObjectError> {
    validate_wrapping_key_id_text(key_id)
        .map_err(|message| ModerationQuarantineObjectError::InvalidInput { message })?;
    Ok(key_id.to_owned())
}
fn validate_wrapping_key_id_text(key_id: &str) -> Result<(), String> {
    if key_id.is_empty()
        || key_id.len() > MODERATION_QUARANTINE_OBJECT_MAX_KEY_HANDLE_BYTES_V1
        || key_id.trim() != key_id
        || key_id.chars().any(char::is_control)
        || !(key_id.starts_with("pkcs11:") || key_id.starts_with("kms:"))
    {
        return Err(format!(
            "wrapping key id must be a canonical `pkcs11:` or `kms:` handle of at most {} printable bytes",
            MODERATION_QUARANTINE_OBJECT_MAX_KEY_HANDLE_BYTES_V1
        ));
    }
    Ok(())
}
fn validate_wrapped_dek(wrapped_dek: &[u8]) -> Result<(), ModerationQuarantineObjectError> {
    if wrapped_dek.is_empty()
        || wrapped_dek.len() > MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1
    {
        return Err(ModerationQuarantineObjectError::InvalidSnapshot {
            message: format!(
                "wrapped DEK length must be within 1..={}",
                MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1
            ),
        });
    }
    Ok(())
}
fn quarantine_immutable_metadata_from_envelope(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
) -> Result<ModerationQuarantineImmutableMetadataV1, ModerationQuarantineObjectError> {
    Ok(ModerationQuarantineImmutableMetadataV1 {
        version: envelope.version,
        algorithm: envelope.algorithm.clone(),
        quarantine_id: envelope.quarantine_id,
        payload_digest: envelope.payload_digest,
        payload_len: envelope.payload_len,
        captured_at_unix: envelope.captured_at_unix,
        content_type: envelope.content_type.clone(),
        notes: envelope.notes.clone(),
        nonce_prefix: envelope.nonce_prefix,
        chunk_plaintext_bytes: envelope.chunk_plaintext_bytes,
        chunk_count: u32::try_from(envelope.chunks.len()).map_err(|_| {
            ModerationQuarantineObjectError::InvalidSnapshot {
                message: "quarantine object chunk count does not fit u32".to_owned(),
            }
        })?,
    })
}
fn quarantine_aad_header_from_envelope(
    envelope: &ModerationQuarantineObjectEnvelopeV1,
) -> Result<ModerationQuarantineAadHeaderV1, ModerationQuarantineObjectError> {
    Ok(ModerationQuarantineAadHeaderV1 {
        metadata: quarantine_immutable_metadata_from_envelope(envelope)?,
        object_id: envelope.object_id,
    })
}
fn moderation_quarantine_object_id(
    metadata: &ModerationQuarantineImmutableMetadataV1,
) -> Result<[u8; 16], ModerationQuarantineObjectError> {
    let encoded =
        norito::to_bytes(metadata).map_err(|error| ModerationQuarantineObjectError::Codec {
            message: error.to_string(),
        })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_ID_DOMAIN_V1);
    hasher.update(&encoded);
    let digest = hasher.finalize();
    let mut object_id = [0_u8; 16];
    object_id.copy_from_slice(&digest.as_bytes()[..16]);
    Ok(object_id)
}
fn moderation_quarantine_aad_header_digest(
    header: &ModerationQuarantineAadHeaderV1,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let encoded =
        norito::to_bytes(header).map_err(|error| ModerationQuarantineObjectError::Codec {
            message: error.to_string(),
        })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_AAD_DOMAIN_V1);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}
fn moderation_quarantine_wrap_context_digest(
    header: &ModerationQuarantineAadHeaderV1,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let header_digest = moderation_quarantine_aad_header_digest(header)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_WRAP_CONTEXT_DOMAIN_V1);
    hasher.update(&header_digest);
    Ok(*hasher.finalize().as_bytes())
}
fn moderation_quarantine_chunk_aad(
    header_digest: [u8; 32],
    index: u32,
    plaintext_offset: u64,
    plaintext_len: u32,
) -> Result<Vec<u8>, ModerationQuarantineObjectError> {
    norito::to_bytes(&ModerationQuarantineChunkAadV1 {
        header_digest,
        index,
        plaintext_offset,
        plaintext_len,
    })
    .map_err(|error| ModerationQuarantineObjectError::Codec {
        message: error.to_string(),
    })
}
fn moderation_quarantine_chunk_nonce(nonce_prefix: [u8; 8], index: u32) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[..8].copy_from_slice(&nonce_prefix);
    nonce[8..].copy_from_slice(&index.to_be_bytes());
    nonce
}
fn moderation_quarantine_ciphertext_digest(
    chunks: &[ModerationQuarantineCiphertextChunkV1],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_OBJECT_CIPHERTEXT_DOMAIN_V1);
    hasher.update(&len_to_u64(chunks.len()).to_le_bytes());
    for chunk in chunks {
        hasher.update(&chunk.index.to_le_bytes());
        hasher.update(&chunk.plaintext_offset.to_le_bytes());
        hasher.update(&chunk.plaintext_len.to_le_bytes());
        hasher.update(&len_to_u64(chunk.ciphertext.len()).to_le_bytes());
        hasher.update(&chunk.ciphertext);
    }
    *hasher.finalize().as_bytes()
}
fn authentication_failed(quarantine_id: [u8; 16]) -> ModerationQuarantineObjectError {
    ModerationQuarantineObjectError::AuthenticationFailed {
        quarantine_id_hex: hex::encode(quarantine_id),
    }
}
pub(crate) fn evidence_viewer_session_record_from_input(
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
pub(crate) fn validate_evidence_viewer_session_record(
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
#[cfg(test)]
#[path = "moderation_model_tests.rs"]
mod tests;
