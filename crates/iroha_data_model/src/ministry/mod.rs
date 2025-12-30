//! Ministry of Information transparency data structures.
//!
//! This module hosts payloads shared between the Ministry transparency tooling
//! (dashboards, release automation) and the governance ledger. The
//! `TransparencyReleaseV1` struct captures the canonical metadata required to
//! anchor a quarterly transparency bundle in the governance DAG.

#![allow(clippy::module_name_repetitions)]

mod jury;
use std::{collections::BTreeSet, string::String, vec::Vec};

use iroha_schema::IntoSchema;
pub use jury::*;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Decode, Encode};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

/// Governance payload describing a Ministry transparency release bundle.
///
/// Releases refer to the signed `transparency_manifest.json` artefact emitted by
/// the publishing pipeline. The manifest digest is recorded using BLAKE2b-256 so
/// governance can recompute and verify the manifest locally before accepting
/// the release. The `SoraFS` CID references the published CAR bundle that
/// contains the sanitized metrics and dashboards.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TransparencyReleaseV1 {
    /// Quarter identifier (e.g., `2026-Q3`) associated with the published bundle.
    pub quarter: String,
    /// UTC timestamp (milliseconds since Unix epoch) when the release manifest was generated.
    pub generated_at_unix_ms: u64,
    /// BLAKE2b-256 digest of `transparency_manifest.json`.
    pub manifest_digest_blake2b_256: [u8; 32],
    /// Raw `SoraFS` root CID bytes (multihash digest) referencing the published CAR.
    pub sorafs_root_cid: Vec<u8>,
    /// Optional dashboards git SHA (raw bytes) pinned alongside the release.
    pub dashboards_git_sha: Option<Vec<u8>>,
    /// Optional operator note recorded with the release (e.g., manual redactions).
    pub note: Option<String>,
}

/// Schema version for [`AgendaProposalV1`].
pub const AGENDA_PROPOSAL_VERSION_V1: u16 = 1;

/// Citizen agenda proposal submitted to the Ministry of Information.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AgendaProposalV1 {
    /// Schema version; must equal [`AGENDA_PROPOSAL_VERSION_V1`].
    pub version: u16,
    /// Stable identifier (`AC-YYYY-###`).
    pub proposal_id: String,
    /// UTC timestamp when the proposal was submitted (milliseconds since Unix epoch).
    pub submitted_at_unix_ms: u64,
    /// BCP‑47 language tag for the human-readable summary.
    pub language: String,
    /// Requested action (e.g., add/remove entry, amend policy).
    #[cfg_attr(feature = "json", norito(with = "crate::ministry::json::action"))]
    pub action: AgendaProposalAction,
    /// Human-readable summary text.
    pub summary: AgendaProposalSummary,
    /// Classification/triage tags (lowercase strings).
    #[norito(default)]
    pub tags: Vec<String>,
    /// Targets impacted by the proposal (hash families + digests).
    #[norito(default)]
    pub targets: Vec<AgendaProposalTarget>,
    /// Evidence attachments supporting the proposal.
    #[norito(default)]
    pub evidence: Vec<AgendaEvidenceAttachment>,
    /// Submitter metadata.
    pub submitter: AgendaProposalSubmitter,
    /// Optional manual duplicate hints supplied by the author.
    #[norito(default)]
    pub duplicates: Vec<String>,
}

impl AgendaProposalV1 {
    /// Validate basic invariants for an agenda proposal.
    ///
    /// # Errors
    ///
    /// Returns [`AgendaProposalValidationError`] when the schema version mismatches or when
    /// required fields are missing/invalid.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), AgendaProposalValidationError> {
        if self.version != AGENDA_PROPOSAL_VERSION_V1 {
            return Err(AgendaProposalValidationError::UnsupportedVersion {
                expected: AGENDA_PROPOSAL_VERSION_V1,
                found: self.version,
            });
        }
        if self.proposal_id.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingProposalId);
        }
        if !self.proposal_id.starts_with("AC-") {
            return Err(AgendaProposalValidationError::InvalidProposalIdFormat {
                value: self.proposal_id.clone(),
            });
        }
        if self.submitted_at_unix_ms == 0 {
            return Err(AgendaProposalValidationError::MissingSubmissionTimestamp);
        }
        if !is_valid_language_tag(&self.language) {
            return Err(AgendaProposalValidationError::InvalidLanguageTag {
                value: self.language.clone(),
            });
        }
        if self.summary.title.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingSummaryField { field: "title" });
        }
        if self.summary.motivation.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingSummaryField {
                field: "motivation",
            });
        }
        if self.summary.expected_impact.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingSummaryField {
                field: "expected_impact",
            });
        }
        if self.submitter.name.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingSubmitterField { field: "name" });
        }
        if self.submitter.contact.trim().is_empty() {
            return Err(AgendaProposalValidationError::MissingSubmitterField { field: "contact" });
        }
        if self.tags.iter().any(|tag| !is_allowed_tag(tag)) {
            let invalid = self
                .tags
                .iter()
                .find(|tag| !is_allowed_tag(tag))
                .cloned()
                .unwrap_or_default();
            return Err(AgendaProposalValidationError::InvalidTag { value: invalid });
        }
        if self.targets.is_empty() {
            return Err(AgendaProposalValidationError::MissingTargets);
        }
        let mut fingerprints = BTreeSet::new();
        for (index, target) in self.targets.iter().enumerate() {
            if target.label.trim().is_empty() {
                return Err(AgendaProposalValidationError::MissingTargetLabel { index });
            }
            if target.reason.trim().is_empty() {
                return Err(AgendaProposalValidationError::MissingTargetReason { index });
            }
            if !is_valid_hash_family(&target.hash_family) {
                return Err(AgendaProposalValidationError::InvalidHashFamily {
                    index,
                    value: target.hash_family.clone(),
                });
            }
            if !is_hex(&target.hash_hex) {
                return Err(AgendaProposalValidationError::InvalidHashHex {
                    index,
                    value: target.hash_hex.clone(),
                });
            }
            if target.hash_hex.len() < 32 {
                return Err(AgendaProposalValidationError::TargetDigestTooShort { index });
            }
            let fingerprint = target.fingerprint_key();
            if !fingerprints.insert(fingerprint.clone()) {
                return Err(AgendaProposalValidationError::DuplicateTarget { index, fingerprint });
            }
        }
        if self.evidence.is_empty() {
            return Err(AgendaProposalValidationError::MissingEvidence);
        }
        for (index, attachment) in self.evidence.iter().enumerate() {
            if attachment.uri.trim().is_empty() {
                return Err(AgendaProposalValidationError::MissingEvidenceUri { index });
            }
            if attachment.requires_digest() {
                let digest = attachment
                    .digest_blake3_hex
                    .as_ref()
                    .filter(|value| is_hex(value) && value.len() == 64);
                if digest.is_none() {
                    return Err(AgendaProposalValidationError::MissingEvidenceDigest { index });
                }
            }
            if let Some(digest) = &attachment.digest_blake3_hex
                && !is_hex(digest)
            {
                return Err(AgendaProposalValidationError::InvalidEvidenceDigest {
                    index,
                    value: digest.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Agenda proposal action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "action", content = "value", rename_all = "kebab-case")]
pub enum AgendaProposalAction {
    /// Request to add new entries to the denylist.
    AddToDenylist,
    /// Request to remove entries from the denylist.
    RemoveFromDenylist,
    /// Request to amend Ministry policy or enforcement procedures.
    AmendPolicy,
}

/// Human-readable summary metadata for the proposal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AgendaProposalSummary {
    /// Short title for the proposal.
    pub title: String,
    /// Motivation/justification paragraph.
    pub motivation: String,
    /// Expected impact assessment.
    pub expected_impact: String,
}

/// Target entry referenced by the proposal (e.g., perceptual hash digest).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AgendaProposalTarget {
    /// Display label describing the entry (e.g., file set).
    pub label: String,
    /// Hash family identifier (e.g., `blake3-256`).
    pub hash_family: String,
    /// Hex-encoded digest for the target entry.
    pub hash_hex: String,
    /// Reason/rationale for including this target.
    pub reason: String,
}

impl AgendaProposalTarget {
    /// Deterministic fingerprint combining hash family and digest.
    pub fn fingerprint_key(&self) -> String {
        format!(
            "{}:{}",
            self.hash_family.to_lowercase(),
            self.hash_hex.to_lowercase()
        )
    }
}

/// Evidence attachment supporting the proposal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AgendaEvidenceAttachment {
    /// Evidence kind (URL, `SoraFS` CID, Torii case, etc.).
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::ministry::json::evidence_kind")
    )]
    pub kind: AgendaEvidenceKind,
    /// URI or identifier for the evidence.
    pub uri: String,
    /// Optional BLAKE3 digest when the attachment refers to static content.
    #[norito(default)]
    pub digest_blake3_hex: Option<String>,
    /// Optional human-readable description.
    #[norito(default)]
    pub description: Option<String>,
}

impl AgendaEvidenceAttachment {
    fn requires_digest(&self) -> bool {
        matches!(
            self.kind,
            AgendaEvidenceKind::SorafsCid | AgendaEvidenceKind::Attachment
        )
    }
}

/// Evidence categories supported by the validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "evidence", content = "value", rename_all = "kebab-case")]
pub enum AgendaEvidenceKind {
    /// HTTP(S) or Torii REST URL.
    Url,
    /// Torii case identifier (e.g., `CASE-2026-04-123`).
    ToriiCase,
    /// `SoraFS` CID referencing an encrypted bundle.
    SorafsCid,
    /// Discrete attachment tracked by digest (e.g., offline files).
    Attachment,
}

/// Submitter metadata recorded with the proposal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AgendaProposalSubmitter {
    /// Individual or organization name.
    pub name: String,
    /// Contact channel (email, Matrix handle, etc.).
    pub contact: String,
    /// Optional organization.
    #[norito(default)]
    pub organization: Option<String>,
    /// Optional PGP fingerprint or other verification material.
    #[norito(default)]
    pub pgp_fingerprint: Option<String>,
}

/// Validation errors surfaced by [`AgendaProposalV1::validate`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgendaProposalValidationError {
    /// Unsupported schema version.
    #[error("agenda proposal schema version {found} not supported (expected {expected})")]
    UnsupportedVersion {
        /// Version required by the validator.
        expected: u16,
        /// Version supplied in the payload.
        found: u16,
    },
    /// Missing identifier.
    #[error("agenda proposal id must not be empty")]
    MissingProposalId,
    /// Identifier format invalid.
    #[error("agenda proposal id `{value}` must follow AC-YYYY-### format")]
    InvalidProposalIdFormat {
        /// Rejected proposal id.
        value: String,
    },
    /// Missing timestamp.
    #[error("agenda proposal submitted_at_unix_ms must be non-zero")]
    MissingSubmissionTimestamp,
    /// Invalid language tag.
    #[error("invalid BCP-47 language tag `{value}`")]
    InvalidLanguageTag {
        /// Tag that failed validation.
        value: String,
    },
    /// Missing summary fields.
    #[error("agenda proposal summary.{field} must not be empty")]
    MissingSummaryField {
        /// Summary field that was empty or missing.
        field: &'static str,
    },
    /// Missing submitter metadata.
    #[error("agenda proposal submitter.{field} must not be empty")]
    MissingSubmitterField {
        /// Submitter field that failed validation.
        field: &'static str,
    },
    /// Invalid tag.
    #[error("unsupported agenda proposal tag `{value}`")]
    InvalidTag {
        /// Tag label that is not allowed.
        value: String,
    },
    /// Proposal lacks targets.
    #[error("agenda proposal must reference at least one target hash")]
    MissingTargets,
    /// Target label missing.
    #[error("agenda proposal target #{index} missing label")]
    MissingTargetLabel {
        /// Zero-based target index in the request.
        index: usize,
    },
    /// Target reason missing.
    #[error("agenda proposal target #{index} missing reason")]
    MissingTargetReason {
        /// Zero-based target index in the request.
        index: usize,
    },
    /// Invalid hash family label.
    #[error("agenda proposal target #{index} hash family `{value}` is invalid")]
    InvalidHashFamily {
        /// Zero-based target index in the request.
        index: usize,
        /// Provided hash family name.
        value: String,
    },
    /// Invalid hash hex encoding.
    #[error("agenda proposal target #{index} hash hex `{value}` is not valid hex")]
    InvalidHashHex {
        /// Zero-based target index in the request.
        index: usize,
        /// Provided digest string.
        value: String,
    },
    /// Digest too short.
    #[error("agenda proposal target #{index} digest must be at least 16 bytes")]
    TargetDigestTooShort {
        /// Zero-based target index in the request.
        index: usize,
    },
    /// Duplicate target fingerprint.
    #[error("agenda proposal target #{index} duplicates fingerprint `{fingerprint}`")]
    DuplicateTarget {
        /// Zero-based target index in the request.
        index: usize,
        /// Canonical fingerprint that already appeared earlier.
        fingerprint: String,
    },
    /// Missing evidence entries.
    #[error("agenda proposal must include at least one evidence attachment")]
    MissingEvidence,
    /// Missing evidence URI.
    #[error("agenda proposal evidence #{index} missing uri")]
    MissingEvidenceUri {
        /// Zero-based evidence index in the request.
        index: usize,
    },
    /// Missing digest for evidence requiring one.
    #[error("agenda proposal evidence #{index} must include digest_blake3_hex")]
    MissingEvidenceDigest {
        /// Zero-based evidence index in the request.
        index: usize,
    },
    /// Invalid digest encoding.
    #[error("agenda proposal evidence #{index} digest `{value}` is not valid hex")]
    InvalidEvidenceDigest {
        /// Zero-based evidence index in the request.
        index: usize,
        /// Provided digest string.
        value: String,
    },
}

const ALLOWED_TAGS: &[&str] = &[
    "csam",
    "malware",
    "fraud",
    "harassment",
    "impersonation",
    "policy-escalation",
    "terrorism",
    "spam",
];

fn is_allowed_tag(tag: &str) -> bool {
    let normalized = tag.trim().to_lowercase();
    !normalized.is_empty() && ALLOWED_TAGS.contains(&normalized.as_str())
}

fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_hash_family(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 48
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn is_valid_language_tag(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 2 || trimmed.len() > 32 {
        return false;
    }
    trimmed
        .split('-')
        .all(|segment| segment.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn is_valid_brief_id(value: &str) -> bool {
    if !value.starts_with("VB-") {
        return false;
    }
    let rest = &value[3..];
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
}

fn parse_rfc3339_timestamp(value: &str) -> bool {
    OffsetDateTime::parse(value.trim(), &Rfc3339).is_ok()
}

/// Schema version for [`VolunteerBriefV1`].
pub const VOLUNTEER_BRIEF_VERSION_V1: u16 = 1;

/// Structured volunteer brief describing a stance for/against a blacklist change (MINFO-3).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerBriefV1 {
    /// Schema version; must equal [`VOLUNTEER_BRIEF_VERSION_V1`].
    pub version: u16,
    /// Stable brief identifier (e.g., `VB-2026-04-EN-01`).
    pub brief_id: String,
    /// Identifier of the referenced proposal (e.g., `BLACKLIST-2026-04-12`).
    pub proposal_id: String,
    /// Submission language (BCP‑47).
    pub language: String,
    /// Volunteer stance.
    pub stance: VolunteerBriefStance,
    /// Submission timestamp in RFC 3339 format.
    pub submitted_at: String,
    /// Author metadata.
    pub author: VolunteerBriefAuthor,
    /// Summary block rendered beside the fact table.
    pub summary: VolunteerBriefSummary,
    /// Machine-readable fact table rows backing the stance.
    pub fact_table: Vec<VolunteerFactRow>,
    /// Conflict disclosure entries.
    #[norito(default)]
    pub disclosures: Vec<VolunteerDisclosure>,
    /// Moderation metadata applied during intake.
    #[norito(default)]
    pub moderation: VolunteerBriefModeration,
}

impl VolunteerBriefV1 {
    /// Validate the volunteer brief according to the MINFO-3 contract.
    ///
    /// # Errors
    ///
    /// Returns [`VolunteerBriefValidationError`] whenever a required field is missing
    /// or an invariant defined in the MINFO-3 template is violated.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), VolunteerBriefValidationError> {
        if self.version != VOLUNTEER_BRIEF_VERSION_V1 {
            return Err(VolunteerBriefValidationError::UnsupportedVersion {
                expected: VOLUNTEER_BRIEF_VERSION_V1,
                found: self.version,
            });
        }
        if !is_valid_brief_id(&self.brief_id) {
            return Err(VolunteerBriefValidationError::InvalidBriefId {
                value: self.brief_id.clone(),
            });
        }
        if self.proposal_id.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingProposalId);
        }
        if !is_valid_language_tag(&self.language) {
            return Err(VolunteerBriefValidationError::InvalidLanguageTag {
                value: self.language.clone(),
            });
        }
        if !parse_rfc3339_timestamp(&self.submitted_at) {
            return Err(VolunteerBriefValidationError::InvalidSubmittedAt {
                value: self.submitted_at.clone(),
            });
        }
        self.author.validate()?;
        self.summary.validate()?;
        if self.fact_table.is_empty() {
            return Err(VolunteerBriefValidationError::MissingFactRows);
        }
        let mut claims = BTreeSet::new();
        for (index, row) in self.fact_table.iter().enumerate() {
            row.validate(index)?;
            if !claims.insert(row.claim_id.clone()) {
                return Err(VolunteerBriefValidationError::DuplicateFactClaimId {
                    claim_id: row.claim_id.clone(),
                });
            }
        }
        if self.disclosures.is_empty() && !self.author.no_conflicts_certified {
            return Err(VolunteerBriefValidationError::MissingDisclosureOrCertification);
        }
        for (index, disclosure) in self.disclosures.iter().enumerate() {
            disclosure.validate(index)?;
        }
        self.moderation.validate()?;
        Ok(())
    }
}

/// Volunteer stance labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "stance", content = "value", rename_all = "kebab-case")
)]
pub enum VolunteerBriefStance {
    /// Supports the referenced action.
    #[cfg_attr(feature = "json", norito(rename = "support"))]
    Support,
    /// Opposes the referenced action.
    #[cfg_attr(feature = "json", norito(rename = "oppose"))]
    Oppose,
    /// Provides additional context without a direct preference.
    #[cfg_attr(feature = "json", norito(rename = "context"))]
    Context,
}

/// Author metadata associated with a volunteer brief.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerBriefAuthor {
    /// Author name.
    pub name: String,
    /// Optional organisation or affiliation.
    #[norito(default)]
    pub organization: Option<String>,
    /// Contact channel (redacted from public dashboards).
    pub contact: String,
    /// Author certifies that no conflicts exist.
    #[norito(default)]
    pub no_conflicts_certified: bool,
}

impl VolunteerBriefAuthor {
    fn validate(&self) -> Result<(), VolunteerBriefValidationError> {
        if self.name.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingAuthorField { field: "name" });
        }
        if self.contact.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingAuthorField { field: "contact" });
        }
        Ok(())
    }
}

/// Summary presented alongside the fact table.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerBriefSummary {
    /// Summary title.
    pub title: String,
    /// Short abstract supporting the stance.
    #[cfg_attr(feature = "json", norito(rename = "abstract"))]
    pub abstract_text: String,
    /// Requested governance action.
    pub requested_action: String,
}

impl VolunteerBriefSummary {
    fn validate(&self) -> Result<(), VolunteerBriefValidationError> {
        if self.title.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingSummaryField { field: "title" });
        }
        let abstract_len = self.abstract_text.trim().chars().count();
        if abstract_len == 0 {
            return Err(VolunteerBriefValidationError::MissingSummaryField { field: "abstract" });
        }
        if abstract_len > 2_000 {
            return Err(VolunteerBriefValidationError::SummaryAbstractTooLong {
                length: abstract_len,
            });
        }
        if self.requested_action.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingSummaryField {
                field: "requested_action",
            });
        }
        Ok(())
    }
}

/// Fact row describing a claim, status, and supporting evidence.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerFactRow {
    /// Stable claim identifier (`VB-YYYY-##-F1`).
    pub claim_id: String,
    /// Human-readable claim text.
    pub claim: String,
    /// Validation status recorded by the volunteer.
    pub status: VolunteerFactStatus,
    /// Impact categories affected by the claim.
    pub impact: Vec<VolunteerFactImpact>,
    /// Supporting citations (URLs, Torii case IDs, CID references, etc.).
    #[norito(default)]
    pub citations: Vec<String>,
    /// Optional BLAKE3 digest of supporting documents.
    #[norito(default)]
    pub evidence_digest: Option<String>,
}

impl VolunteerFactRow {
    fn validate(&self, index: usize) -> Result<(), VolunteerBriefValidationError> {
        if self.claim_id.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingFactField {
                index,
                field: "claim_id",
            });
        }
        if self.claim.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingFactField {
                index,
                field: "claim",
            });
        }
        if self.impact.is_empty() {
            return Err(VolunteerBriefValidationError::MissingFactImpact { index });
        }
        for value in &self.citations {
            if value.trim().is_empty() {
                return Err(VolunteerBriefValidationError::EmptyCitation { index });
            }
        }
        if let Some(digest) = &self.evidence_digest
            && (digest.len() != 64 || !is_hex(digest))
        {
            return Err(VolunteerBriefValidationError::InvalidEvidenceDigest {
                index,
                value: digest.clone(),
            });
        }
        Ok(())
    }
}

/// Fact status enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "kebab-case")
)]
pub enum VolunteerFactStatus {
    /// Fact corroborated by evidence.
    #[cfg_attr(feature = "json", norito(rename = "corroborated"))]
    Corroborated,
    /// Fact is disputed by conflicting evidence.
    #[cfg_attr(feature = "json", norito(rename = "disputed"))]
    Disputed,
    /// Fact provides neutral context.
    #[cfg_attr(feature = "json", norito(rename = "context-only"))]
    ContextOnly,
}

/// Impact categories referenced by fact rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "impact", content = "value", rename_all = "kebab-case")
)]
pub enum VolunteerFactImpact {
    /// Governance-related impact.
    #[cfg_attr(feature = "json", norito(rename = "governance"))]
    Governance,
    /// Technical impact.
    #[cfg_attr(feature = "json", norito(rename = "technical"))]
    Technical,
    /// Compliance impact.
    #[cfg_attr(feature = "json", norito(rename = "compliance"))]
    Compliance,
    /// Community impact.
    #[cfg_attr(feature = "json", norito(rename = "community"))]
    Community,
}

/// Conflict disclosure entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerDisclosure {
    /// Disclosure classification.
    #[cfg_attr(feature = "json", norito(rename = "type"))]
    pub disclosure_type: VolunteerDisclosureType,
    /// Entity associated with the disclosure.
    pub entity: String,
    /// Relationship summary.
    pub relationship: String,
    /// Additional details.
    pub details: String,
    /// Optional evidence reference supporting the disclosure.
    #[norito(default)]
    pub evidence: Option<String>,
}

impl VolunteerDisclosure {
    fn validate(&self, index: usize) -> Result<(), VolunteerBriefValidationError> {
        if self.entity.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingDisclosureField {
                index,
                field: "entity",
            });
        }
        if self.relationship.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingDisclosureField {
                index,
                field: "relationship",
            });
        }
        if self.details.trim().is_empty() {
            return Err(VolunteerBriefValidationError::MissingDisclosureField {
                index,
                field: "details",
            });
        }
        if let Some(evidence) = &self.evidence
            && evidence.trim().is_empty()
        {
            return Err(VolunteerBriefValidationError::MissingDisclosureField {
                index,
                field: "evidence",
            });
        }
        Ok(())
    }
}

/// Disclosure type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "disclosure", content = "value", rename_all = "kebab-case")
)]
pub enum VolunteerDisclosureType {
    /// Financial disclosure.
    #[cfg_attr(feature = "json", norito(rename = "financial"))]
    Financial,
    /// Employment relationship.
    #[cfg_attr(feature = "json", norito(rename = "employment"))]
    Employment,
    /// Governance/board position.
    #[cfg_attr(feature = "json", norito(rename = "governance"))]
    Governance,
    /// Familial relationship.
    #[cfg_attr(feature = "json", norito(rename = "family"))]
    Family,
    /// Other disclosure category.
    #[cfg_attr(feature = "json", norito(rename = "other"))]
    Other,
}

/// Moderation metadata capturing off-topic or triage notes.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VolunteerBriefModeration {
    /// Whether the submission is considered off-topic.
    #[norito(default)]
    pub off_topic: bool,
    /// Moderation tags applied to the submission.
    #[norito(default)]
    pub tags: Vec<VolunteerModerationTag>,
    /// Optional moderator notes (≤512 characters).
    #[norito(default)]
    pub notes: Option<String>,
}

impl VolunteerBriefModeration {
    fn validate(&self) -> Result<(), VolunteerBriefValidationError> {
        if let Some(notes) = &self.notes {
            let len = notes.trim().chars().count();
            if len > 512 {
                return Err(VolunteerBriefValidationError::ModerationNotesTooLong { length: len });
            }
        }
        Ok(())
    }
}

/// Moderation tags available to reviewers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "tag", content = "value", rename_all = "kebab-case")
)]
pub enum VolunteerModerationTag {
    /// Duplicate submission.
    #[cfg_attr(feature = "json", norito(rename = "duplicate"))]
    Duplicate,
    /// Needs translation before publishing.
    #[cfg_attr(feature = "json", norito(rename = "needs-translation"))]
    NeedsTranslation,
    /// Requires follow-up with the author.
    #[cfg_attr(feature = "json", norito(rename = "needs-follow-up"))]
    NeedsFollowUp,
    /// Detected spam.
    #[cfg_attr(feature = "json", norito(rename = "spam"))]
    Spam,
    /// Indicates astroturfing behaviour.
    #[cfg_attr(feature = "json", norito(rename = "astroturf"))]
    Astroturf,
    /// Escalated policy concern.
    #[cfg_attr(feature = "json", norito(rename = "policy-escalation"))]
    PolicyEscalation,
}

/// Validation errors returned by [`VolunteerBriefV1::validate`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum VolunteerBriefValidationError {
    /// Unsupported schema version.
    #[error("unsupported volunteer brief version {found} (expected {expected})")]
    UnsupportedVersion {
        /// Version required by the validator.
        expected: u16,
        /// Version observed in the payload.
        found: u16,
    },
    /// Brief identifier is invalid.
    #[error(
        "volunteer brief id `{value}` must begin with `VB-` and use uppercase letters, digits, or hyphens"
    )]
    InvalidBriefId {
        /// Identifier that failed validation.
        value: String,
    },
    /// Missing proposal identifier.
    #[error("volunteer brief proposal_id is required")]
    MissingProposalId,
    /// Invalid language tag.
    #[error("volunteer brief language tag `{value}` is invalid")]
    InvalidLanguageTag {
        /// Provided language tag.
        value: String,
    },
    /// Submitted at timestamp is invalid.
    #[error("volunteer brief submitted_at `{value}` must be RFC3339")]
    InvalidSubmittedAt {
        /// Timestamp string that failed parsing.
        value: String,
    },
    /// Missing author data.
    #[error("volunteer brief author field `{field}` is required")]
    MissingAuthorField {
        /// Author field missing a value.
        field: &'static str,
    },
    /// Missing summary data.
    #[error("volunteer brief summary field `{field}` is required")]
    MissingSummaryField {
        /// Summary field missing a value.
        field: &'static str,
    },
    /// Abstract too long.
    #[error("volunteer brief summary abstract exceeds 2000 characters (found {length})")]
    SummaryAbstractTooLong {
        /// Observed abstract length.
        length: usize,
    },
    /// No fact rows provided.
    #[error("volunteer brief fact_table must contain at least one row")]
    MissingFactRows,
    /// Duplicate fact claim identifiers.
    #[error("volunteer brief fact_table contains duplicate claim_id `{claim_id}`")]
    DuplicateFactClaimId {
        /// Duplicate claim identifier.
        claim_id: String,
    },
    /// Missing fact field.
    #[error("volunteer fact row #{index} field `{field}` is required")]
    MissingFactField {
        /// Zero-based index of the fact row.
        index: usize,
        /// Field missing a value.
        field: &'static str,
    },
    /// Missing fact impact set.
    #[error("volunteer fact row #{index} must include at least one impact entry")]
    MissingFactImpact {
        /// Zero-based index of the fact row.
        index: usize,
    },
    /// Empty citation entry.
    #[error("volunteer fact row #{index} citations must not contain empty entries")]
    EmptyCitation {
        /// Zero-based index of the fact row.
        index: usize,
    },
    /// Invalid digest encoding.
    #[error("volunteer fact row #{index} evidence digest `{value}` must be 64 hex characters")]
    InvalidEvidenceDigest {
        /// Zero-based index of the fact row.
        index: usize,
        /// Digest string that failed validation.
        value: String,
    },
    /// Missing disclosure info.
    #[error("volunteer disclosure #{index} field `{field}` is required")]
    MissingDisclosureField {
        /// Zero-based disclosure index.
        index: usize,
        /// Missing disclosure field.
        field: &'static str,
    },
    /// Disclosures missing when certification not provided.
    #[error(
        "volunteer brief must include at least one disclosure or set author.no_conflicts_certified=true"
    )]
    MissingDisclosureOrCertification,
    /// Moderation notes too long.
    #[error("volunteer brief moderation notes exceed 512 characters (found {length})")]
    ModerationNotesTooLong {
        /// Observed moderation note length.
        length: usize,
    },
}

/// Schema version for [`ReviewPanelSummaryV1`].
pub const REVIEW_PANEL_SUMMARY_VERSION_V1: u16 = 1;

/// Neutral referendum summary emitted by the review panel workflow (MINFO-4a).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelSummaryV1 {
    /// Schema version; must equal [`REVIEW_PANEL_SUMMARY_VERSION_V1`].
    pub version: u16,
    /// Identifier of the agenda proposal under review.
    pub proposal_id: String,
    /// Review panel round identifier (`RP-YYYY-##`).
    pub panel_round_id: String,
    /// BCP‑47 language tag for the generated summary.
    pub language: String,
    /// Unix timestamp (milliseconds) when the summary was generated.
    pub generated_at_unix_ms: u64,
    /// Neutral overview describing the request and decision context.
    pub overview: ReviewPanelOverview,
    /// Distribution of briefs per stance consumed by the panel.
    #[norito(default)]
    pub stance_distribution: Vec<ReviewPanelStanceCount>,
    /// Key highlights extracted from volunteer briefs and evidence.
    #[norito(default)]
    pub highlights: Vec<ReviewPanelHighlight>,
    /// Referenced AI reproducibility manifest.
    pub ai_manifest: ReviewPanelAiEvidence,
    /// Volunteer brief metadata referenced by the summary.
    #[norito(default)]
    pub volunteer_references: Vec<ReviewPanelVolunteerReference>,
    /// Lint warnings surfaced during synthesis (e.g., missing citations).
    #[norito(default)]
    pub warnings: Vec<String>,
}

/// Summary text emitted by the review panel.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelOverview {
    /// Title for the referendum packet.
    pub title: String,
    /// Neutral summary paragraph describing the request.
    pub neutral_summary: String,
    /// Notes about how the panel expects the policy jury to proceed.
    pub decision_context: String,
}

/// Count of briefs/fact rows per stance (support/oppose/context).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelStanceCount {
    /// Stance label (`support`, `oppose`, `context`).
    pub stance: String,
    /// Number of briefs referencing this stance.
    pub brief_count: u32,
    /// Number of fact rows collected for this stance.
    pub fact_row_count: u32,
}

/// Highlight summarising a specific fact row or evidence bundle.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelHighlight {
    /// Stable highlight identifier (e.g., `support-1`).
    pub id: String,
    /// Stance associated with the highlight.
    pub stance: String,
    /// Canonical fact/claim identifier referenced by the highlight.
    pub claim_id: String,
    /// Neutral statement summarising the fact row and its impact.
    pub statement: String,
    /// Citations that back the highlight.
    #[norito(default)]
    pub citations: Vec<ReviewPanelCitation>,
}

/// Citation referencing volunteer facts, AI manifests, or proposal evidence.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelCitation {
    /// Citation classification.
    pub kind: ReviewPanelCitationKind,
    /// Identifier of the referenced artefact (fact id, manifest id, etc.).
    pub reference_id: String,
    /// Optional URI associated with the citation.
    #[norito(default)]
    pub uri: Option<String>,
}

/// Citation kinds used in review panel summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ReviewPanelCitationKind {
    /// Citation references a volunteer fact row.
    VolunteerFact,
    /// Citation references the AI reproducibility manifest.
    AiManifest,
    /// Citation references an evidence attachment from the proposal.
    ProposalEvidence,
}

/// Reference to the reproducibility manifest used by the panel.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelAiEvidence {
    /// Manifest UUID referenced by the panel.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub manifest_id: [u8; 16],
    /// Runner version string.
    pub runtime_version: String,
    /// Unix timestamp (seconds) when the manifest was issued.
    pub issued_at_unix: u64,
    /// Quarantine threshold used by the AI committee (basis points).
    pub quarantine_threshold: u16,
    /// Escalation threshold used by the AI committee (basis points).
    pub escalate_threshold: u16,
}

/// Metadata about volunteer briefs feeding the neutral summary.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReviewPanelVolunteerReference {
    /// Volunteer brief identifier.
    pub brief_id: String,
    /// Stance supported by the brief.
    pub stance: String,
    /// Language of the submission (BCP‑47).
    pub language: String,
    /// Number of fact rows in the submission.
    pub fact_rows: u32,
    /// Number of fact rows that included citations.
    pub cited_rows: u32,
}

/// Schema version for [`ReferendumPacketV1`].
pub const REFERENDUM_PACKET_VERSION_V1: u16 = 1;

/// Complete referendum dossier emitted after the review panel workflow (MINFO-4).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumPacketV1 {
    /// Schema version; must equal [`REFERENDUM_PACKET_VERSION_V1`].
    pub version: u16,
    /// Agenda proposal under vote.
    pub proposal: AgendaProposalV1,
    /// Neutral review panel summary including AI manifest references.
    pub review_summary: ReviewPanelSummaryV1,
    /// Deterministic sortition evidence used to seat the panel.
    pub sortition: ReferendumSortitionEvidence,
    /// Selected panelists along with the Merkle proofs for their draw.
    #[norito(default)]
    pub panelists: Vec<ReferendumPanelist>,
    /// Hash-family impact summary derived from the duplicate registry + policy snapshot.
    pub impact_summary: ReferendumImpactSummary,
}

/// Evidence describing the panel sortition draw.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumSortitionEvidence {
    /// Sortition algorithm identifier.
    pub algorithm: String,
    /// Seed used for the deterministic draw (hex).
    pub seed_hex: String,
    /// Merkle root hash (hex) over the eligible roster.
    pub merkle_root_hex: String,
    /// BLAKE3 digest of the roster manifest (hex).
    pub roster_digest_blake3_hex: String,
    /// SHA256 digest of the roster manifest (hex).
    pub roster_digest_sha256_hex: String,
    /// Number of slots drawn from the roster.
    pub slots: u32,
    /// Number of eligible members considered by the draw.
    pub eligible_members: u32,
}

/// Selected panelist plus the data required to audit their slot.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumPanelist {
    /// Stable roster identifier.
    pub member_id: String,
    /// Role (citizen, observer, etc.).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Affiliated organization, if provided.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Contact channel for follow-ups.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Weight assigned in the roster.
    pub weight: u64,
    /// Slot index inside the draw.
    pub draw_position: u32,
    /// Zero-based index of the member within the eligible roster.
    pub eligible_index: u32,
    /// Original roster index before filtering.
    pub original_index: u32,
    /// Entropy used for the draw (hex).
    pub draw_entropy_hex: String,
    /// Leaf hash committed to by the Merkle proof (hex).
    pub leaf_hash_hex: String,
    /// Merkle proof hashes (hex) used to verify the selected member.
    #[norito(default)]
    pub merkle_proof: Vec<String>,
}

/// Impact summary derived from `cargo xtask ministry-agenda impact`.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumImpactSummary {
    /// RFC3339 timestamp when the impact report was generated.
    pub report_generated_at: String,
    /// Total targets analyzed for the referenced proposal.
    pub total_targets: u32,
    /// Number of duplicate-registry conflicts.
    pub registry_conflicts: u32,
    /// Number of policy-snapshot conflicts.
    pub policy_conflicts: u32,
    /// Breakdown per hash family.
    #[norito(default)]
    pub hash_families: Vec<ReferendumImpactHashFamily>,
    /// Detailed conflict entries.
    #[norito(default)]
    pub conflicts: Vec<ReferendumImpactConflict>,
}

/// Aggregate impact stats for a single hash family.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumImpactHashFamily {
    /// Hash family label (e.g., `blake3-256`).
    pub hash_family: String,
    /// Number of targets referencing the family.
    pub targets: u32,
    /// Registry conflicts for the family.
    pub registry_conflicts: u32,
    /// Policy conflicts for the family.
    pub policy_conflicts: u32,
}

/// Detailed conflict row mirroring the impact report structure.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReferendumImpactConflict {
    /// Source of the conflict (duplicate registry or policy snapshot).
    pub source: ReferendumImpactConflictSource,
    /// Hash family of the conflicting entry.
    pub hash_family: String,
    /// Hash hex of the conflicting entry.
    pub hash_hex: String,
    /// Reference identifier (proposal id or policy id).
    pub reference: String,
    /// Optional note attached to the conflict.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Conflict source enumeration mirrored from the impact report helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "source", content = "value"))]
#[norito(rename_all = "snake_case")]
pub enum ReferendumImpactConflictSource {
    /// Fingerprint already exists in the duplicate registry.
    DuplicateRegistry,
    /// Fingerprint already exists within the policy snapshot.
    PolicySnapshot,
}

/// Re-export commonly used ministry types.
pub mod prelude {
    pub use super::{
        AgendaEvidenceAttachment, AgendaEvidenceKind, AgendaProposalAction,
        AgendaProposalSubmitter, AgendaProposalSummary, AgendaProposalTarget, AgendaProposalV1,
        AgendaProposalValidationError, ReferendumImpactConflict, ReferendumImpactConflictSource,
        ReferendumImpactHashFamily, ReferendumImpactSummary, ReferendumPacketV1,
        ReferendumPanelist, ReferendumSortitionEvidence, ReviewPanelAiEvidence,
        ReviewPanelCitation, ReviewPanelCitationKind, ReviewPanelHighlight, ReviewPanelOverview,
        ReviewPanelStanceCount, ReviewPanelSummaryV1, ReviewPanelVolunteerReference,
        TransparencyReleaseV1, VolunteerBriefAuthor, VolunteerBriefModeration,
        VolunteerBriefStance, VolunteerBriefSummary, VolunteerBriefV1,
        VolunteerBriefValidationError, VolunteerDisclosure, VolunteerDisclosureType,
        VolunteerFactImpact, VolunteerFactRow, VolunteerFactStatus, VolunteerModerationTag,
    };
}

#[cfg(feature = "json")]
mod json {
    use norito::json::{self, JsonSerialize, Parser};

    use super::{AgendaEvidenceKind, AgendaProposalAction};

    pub mod action {
        use super::*;

        #[allow(clippy::trivially_copy_pass_by_ref)] // Norito serializer signature
        pub fn serialize(value: &AgendaProposalAction, out: &mut String) {
            let label = match value {
                AgendaProposalAction::AddToDenylist => "add-to-denylist",
                AgendaProposalAction::RemoveFromDenylist => "remove-from-denylist",
                AgendaProposalAction::AmendPolicy => "amend-policy",
            };
            JsonSerialize::json_serialize(label, out);
        }

        pub fn deserialize(parser: &mut Parser<'_>) -> Result<AgendaProposalAction, json::Error> {
            let label = parser.parse_string()?;
            match label.as_str() {
                "add-to-denylist" => Ok(AgendaProposalAction::AddToDenylist),
                "remove-from-denylist" => Ok(AgendaProposalAction::RemoveFromDenylist),
                "amend-policy" => Ok(AgendaProposalAction::AmendPolicy),
                other => Err(json::Error::unknown_field(other)),
            }
        }
    }

    pub mod evidence_kind {
        use super::*;

        #[allow(clippy::trivially_copy_pass_by_ref)] // Norito serializer signature
        pub fn serialize(value: &AgendaEvidenceKind, out: &mut String) {
            let label = match value {
                AgendaEvidenceKind::Url => "url",
                AgendaEvidenceKind::ToriiCase => "torii-case",
                AgendaEvidenceKind::SorafsCid => "sorafs-cid",
                AgendaEvidenceKind::Attachment => "attachment",
            };
            JsonSerialize::json_serialize(label, out);
        }

        pub fn deserialize(parser: &mut Parser<'_>) -> Result<AgendaEvidenceKind, json::Error> {
            let label = parser.parse_string()?;
            match label.as_str() {
                "url" => Ok(AgendaEvidenceKind::Url),
                "torii-case" => Ok(AgendaEvidenceKind::ToriiCase),
                "sorafs-cid" => Ok(AgendaEvidenceKind::SorafsCid),
                "attachment" => Ok(AgendaEvidenceKind::Attachment),
                other => Err(json::Error::unknown_field(other)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, json, to_bytes};

    use super::*;

    fn sample_review_summary() -> ReviewPanelSummaryV1 {
        ReviewPanelSummaryV1 {
            version: REVIEW_PANEL_SUMMARY_VERSION_V1,
            proposal_id: "AC-2026-001".into(),
            panel_round_id: "RP-2026-05".into(),
            language: "en".into(),
            generated_at_unix_ms: 1_780_000_123_456,
            overview: ReviewPanelOverview {
                title: "Review Panel Summary — AC-2026-001".into(),
                neutral_summary: "Panel reviewed the citizen request and the referenced evidence."
                    .into(),
                decision_context: "Forwarded to policy jury with heightened scrutiny.".into(),
            },
            stance_distribution: vec![ReviewPanelStanceCount {
                stance: "support".into(),
                brief_count: 2,
                fact_row_count: 4,
            }],
            highlights: vec![ReviewPanelHighlight {
                id: "support-1".into(),
                stance: "support".into(),
                claim_id: "VB-2026-04-F1".into(),
                statement:
                    "Volunteer briefs cite regulator takedown orders covering the same hash family."
                        .into(),
                citations: vec![
                    ReviewPanelCitation {
                        kind: ReviewPanelCitationKind::VolunteerFact,
                        reference_id: "VB-2026-04-F1".into(),
                        uri: Some("https://example.invalid/case/123".into()),
                    },
                    ReviewPanelCitation {
                        kind: ReviewPanelCitationKind::AiManifest,
                        reference_id: "5f90d3a833f545498f7f9915d5f3d210".into(),
                        uri: None,
                    },
                ],
            }],
            ai_manifest: ReviewPanelAiEvidence {
                manifest_id: [0xAA; 16],
                runtime_version: "sorafs-ai-runner 0.5.0".into(),
                issued_at_unix: 1_780_000_000,
                quarantine_threshold: 7800,
                escalate_threshold: 3200,
            },
            volunteer_references: vec![ReviewPanelVolunteerReference {
                brief_id: "VB-2026-04".into(),
                stance: "support".into(),
                language: "en".into(),
                fact_rows: 3,
                cited_rows: 3,
            }],
            warnings: vec!["context briefs missing".into()],
        }
    }

    #[test]
    fn transparency_release_roundtrips() {
        let payload = TransparencyReleaseV1 {
            quarter: "2026-Q3".into(),
            generated_at_unix_ms: 1_780_000_000_000,
            manifest_digest_blake2b_256: [0xAB; 32],
            sorafs_root_cid: vec![0xAA, 0xBB, 0xCC],
            dashboards_git_sha: Some(vec![0x11; 20]),
            note: Some("DP sanitizer re-run after hotfix".into()),
        };
        let bytes = to_bytes(&payload).expect("encode transparency payload");
        let decoded: TransparencyReleaseV1 =
            decode_from_bytes(&bytes).expect("decode transparency payload");
        assert_eq!(payload, decoded);
    }

    fn sample_proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-001".into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Add abusive hash family".into(),
                motivation: "Multiple citizen reports for the same hash.".into(),
                expected_impact: "Removes known malicious payload".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "Sample entry".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d".into(),
                reason: "Flagged by moderators".into(),
            }],
            evidence: vec![
                AgendaEvidenceAttachment {
                    kind: AgendaEvidenceKind::SorafsCid,
                    uri: "sorafs://bafybei.../artifact.car".into(),
                    digest_blake3_hex: Some(
                        "f1c02fb3bb194a9add242c3dfdf0bb2d94f9f3e1cf11f4a7d79d4012e4d0c2ad".into(),
                    ),
                    description: Some("Encrypted artifact bundle".into()),
                },
                AgendaEvidenceAttachment {
                    kind: AgendaEvidenceKind::Url,
                    uri: "https://example.invalid/case/123".into(),
                    digest_blake3_hex: None,
                    description: Some("Incident report".into()),
                },
            ],
            submitter: AgendaProposalSubmitter {
                name: "Citizen 42".into(),
                contact: "citizen42@example.org".into(),
                organization: Some("Wonderland Watch".into()),
                pgp_fingerprint: None,
            },
            duplicates: vec!["AC-2025-014".into()],
        }
    }

    #[test]
    fn agenda_proposal_validate_succeeds() {
        let proposal = sample_proposal();
        assert!(proposal.validate().is_ok());
    }

    #[test]
    fn agenda_proposal_rejects_duplicate_targets() {
        let mut proposal = sample_proposal();
        proposal.targets.push(proposal.targets[0].clone());
        let err = proposal.validate().expect_err("duplicates should fail");
        assert!(matches!(
            err,
            AgendaProposalValidationError::DuplicateTarget { .. }
        ));
    }

    #[test]
    fn agenda_proposal_json_matches_external_schema() {
        let proposal = sample_proposal();
        let json_bytes = json::to_vec_pretty(&proposal).expect("serialize agenda proposal to JSON");
        let json = String::from_utf8(json_bytes).expect("utf-8 json");
        assert!(json.contains("\"action\": \"add-to-denylist\""));
        assert!(json.contains("\"kind\": \"sorafs-cid\""));
        let decoded: AgendaProposalV1 =
            norito::json::from_str(&json).expect("deserialize agenda proposal JSON");
        assert_eq!(decoded, proposal);
    }

    #[test]
    fn review_panel_summary_roundtrips() {
        let summary = sample_review_summary();
        let bytes = to_bytes(&summary).expect("encode review panel summary");
        let decoded: ReviewPanelSummaryV1 =
            decode_from_bytes(&bytes).expect("decode review panel summary");
        assert_eq!(summary, decoded);
    }

    fn sample_volunteer_brief() -> VolunteerBriefV1 {
        VolunteerBriefV1 {
            version: VOLUNTEER_BRIEF_VERSION_V1,
            brief_id: "VB-2026-04-EN-01".into(),
            proposal_id: "BLACKLIST-2026-04-12".into(),
            language: "en".into(),
            stance: VolunteerBriefStance::Support,
            submitted_at: "2026-04-15T11:24:00Z".into(),
            author: VolunteerBriefAuthor {
                name: "Makoto K.".into(),
                organization: Some("Sora Commons Guild".into()),
                contact: "makoto@example.org".into(),
                no_conflicts_certified: false,
            },
            summary: VolunteerBriefSummary {
                title: "Support delisting stale phishing domain".into(),
                abstract_text: "Phishing domain has been inactive for two months; telemetry confirms inactive threat window.".into(),
                requested_action: "Approve delisting after replica confirmation.".into(),
            },
            fact_table: vec![
                VolunteerFactRow {
                    claim_id: "VB-2026-04-F1".into(),
                    claim: "Torii case TR-2219 concluded with verified restitution.".into(),
                    status: VolunteerFactStatus::Corroborated,
                    impact: vec![VolunteerFactImpact::Governance, VolunteerFactImpact::Compliance],
                    citations: vec!["torii://case/TR-2219".into()],
                    evidence_digest: Some(
                        "d7d1c1f0e8a6f8c7d2b49392dd4ddc4076aef9b1403fab8a9d53d2d3b029f3ad"
                            .into(),
                    ),
                },
                VolunteerFactRow {
                    claim_id: "VB-2026-04-F2".into(),
                    claim: "Zero traffic observed over 30-day cooling window.".into(),
                    status: VolunteerFactStatus::Corroborated,
                    impact: vec![VolunteerFactImpact::Technical],
                    citations: vec!["https://dashboards.sora.net/render/telemetry".into()],
                    evidence_digest: None,
                },
            ],
            disclosures: vec![VolunteerDisclosure {
                disclosure_type: VolunteerDisclosureType::Employment,
                entity: "Sora Commons Guild".into(),
                relationship: "Incident response contractor".into(),
                details: "Hourly retainer to manage phishing takedown reports.".into(),
                evidence: Some("https://sora.commons/contracts/contract-2025-11.pdf".into()),
            }],
            moderation: VolunteerBriefModeration {
                off_topic: false,
                tags: vec![VolunteerModerationTag::NeedsTranslation],
                notes: Some("Pending Japanese translation before publication.".into()),
            },
        }
    }

    #[test]
    fn volunteer_brief_validation_succeeds() {
        let brief = sample_volunteer_brief();
        assert!(brief.validate().is_ok());
    }

    #[test]
    fn volunteer_brief_requires_disclosure_or_certification() {
        let mut brief = sample_volunteer_brief();
        brief.disclosures.clear();
        brief.author.no_conflicts_certified = false;
        let err = brief.validate().expect_err("disclosures required");
        assert!(matches!(
            err,
            VolunteerBriefValidationError::MissingDisclosureOrCertification
        ));
    }

    #[test]
    fn volunteer_brief_rejects_duplicate_claim_ids() {
        let mut brief = sample_volunteer_brief();
        let mut duplicate = brief.fact_table[0].clone();
        duplicate.claim_id = brief.fact_table[1].claim_id.clone();
        brief.fact_table.push(duplicate);
        let err = brief
            .validate()
            .expect_err("duplicate claim ids must be rejected");
        assert!(matches!(
            err,
            VolunteerBriefValidationError::DuplicateFactClaimId { .. }
        ));
    }

    fn sample_referendum_packet() -> ReferendumPacketV1 {
        ReferendumPacketV1 {
            version: REFERENDUM_PACKET_VERSION_V1,
            proposal: sample_proposal(),
            review_summary: sample_review_summary(),
            sortition: ReferendumSortitionEvidence {
                algorithm: "agenda-sortition-blake3-v1".into(),
                seed_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                merkle_root_hex: "c9c5d48ebf297d73195dd73cdca3b02283d47fac5beb8a2f0f4cf7d4cf5a3b77"
                    .into(),
                roster_digest_blake3_hex:
                    "7c6f6cbfbcb8eec595655ffb5f84355a84772b65d741f6a53fb966ab0c4b5858".into(),
                roster_digest_sha256_hex:
                    "1ecf1fd54548617b7329627221dfe9af0ef23e704cd9c4235d2ff8ce6f8c2cdf".into(),
                slots: 2,
                eligible_members: 3,
            },
            panelists: vec![
                ReferendumPanelist {
                    member_id: "citizen:ada".into(),
                    role: Some("citizen".into()),
                    organization: Some("Artemis Cooperative".into()),
                    contact: Some("ada@example.org".into()),
                    weight: 2,
                    draw_position: 0,
                    eligible_index: 0,
                    original_index: 0,
                    draw_entropy_hex:
                        "a7e8ee16b74a6ed6c5043621f0ee82e9a2bd69882bff572b7fc608bec3e9a9ef".into(),
                    leaf_hash_hex:
                        "4a807d728c14b2a0f1cb42a8e96ad0b7d5e2f7d37973062dbf8f5eeeed2d2aeb".into(),
                    merkle_proof: vec![
                        "ed7941cb505dbb09c059bdec9f2acefe8237b3356f5b10d2b3a5c51c88629875".into(),
                        "d5850df94543f1ef9e2cb84660b1c3d93b5a2f8bd92afc6f7cc0260f2fa7089b".into(),
                    ],
                },
                ReferendumPanelist {
                    member_id: "citizen:bea".into(),
                    role: Some("observer".into()),
                    organization: Some("Inspectorate".into()),
                    contact: None,
                    weight: 1,
                    draw_position: 1,
                    eligible_index: 2,
                    original_index: 2,
                    draw_entropy_hex:
                        "4c5e7a0f36de9974ee5d8b848b0f916d3c2a39b38a4a48dcdc2805326b0bd708".into(),
                    leaf_hash_hex:
                        "a0ffca1a26d57fd65b746d4b97a2fc836df7c5e353788f62c7568b5c4b0a5d88".into(),
                    merkle_proof: vec![
                        "7eb0ec24bbce9215ea4ed6f0f9059ecb8ecde25d3672f7c002848631186c34b4".into(),
                        "d5850df94543f1ef9e2cb84660b1c3d93b5a2f8bd92afc6f7cc0260f2fa7089b".into(),
                    ],
                },
            ],
            impact_summary: ReferendumImpactSummary {
                report_generated_at: "2026-03-31T12:00:00Z".into(),
                total_targets: 1,
                registry_conflicts: 1,
                policy_conflicts: 0,
                hash_families: vec![ReferendumImpactHashFamily {
                    hash_family: "blake3-256".into(),
                    targets: 1,
                    registry_conflicts: 1,
                    policy_conflicts: 0,
                }],
                conflicts: vec![ReferendumImpactConflict {
                    source: ReferendumImpactConflictSource::DuplicateRegistry,
                    hash_family: "blake3-256".into(),
                    hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d"
                        .into(),
                    reference: "AC-2025-014".into(),
                    note: Some("Already quarantined".into()),
                }],
            },
        }
    }

    #[test]
    fn referendum_packet_roundtrips() {
        let packet = sample_referendum_packet();
        let bytes = to_bytes(&packet).expect("encode referendum packet");
        let decoded: ReferendumPacketV1 =
            decode_from_bytes(&bytes).expect("decode referendum packet");
        assert_eq!(packet, decoded);
    }
}
