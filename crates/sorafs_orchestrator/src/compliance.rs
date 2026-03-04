//! Compliance helpers for enforcing SoraNet Governance Action Report (GAR)
//! policy constraints within the orchestrator.
//!
//! The roadmap item SNNet-9 introduces jurisdictional carve-outs and
//! blinded-CID opt-out lists that require the orchestrator to force direct mode
//! fetches even when SoraNet transports are otherwise available. This module
//! centralises the data model so configuration surfaces can ingest governance
//! policies and the fetch pipeline can make deterministic decisions.
//!
//! The compliance helpers now surface operator attestation artefacts alongside
//! an auditable checklist so governance tooling can confirm why a SoraNet
//! transport was overridden and which assurances were provided by each
//! operator.

use std::collections::BTreeSet;

use iroha_core::prelude::Hash;

/// Decision emitted after evaluating the compliance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceDecision {
    /// No override required; the configured transport/anonymity policies remain in effect.
    Proceed,
    /// Force direct-only transport because of a governance restriction.
    ForceDirect {
        /// Reason why SoraNet must be bypassed.
        reason: ComplianceReason,
    },
}

impl ComplianceDecision {
    /// Returns the compliance reason when the decision forces direct-only mode.
    #[must_use]
    pub const fn reason(self) -> Option<ComplianceReason> {
        match self {
            Self::ForceDirect { reason } => Some(reason),
            Self::Proceed => None,
        }
    }
}

/// Reasons why SoraNet transport must be bypassed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceReason {
    /// Operator jurisdiction opted out of SoraNet anonymity transport.
    JurisdictionOptOut,
    /// Manifest (blinded CID) is covered by a GAR opt-out list.
    BlindedCidOptOut,
}

/// Compliance policy derived from governance artefacts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompliancePolicy {
    operator_jurisdictions: Vec<String>,
    jurisdiction_opt_outs: BTreeSet<String>,
    blinded_cid_opt_outs: BTreeSet<[u8; Hash::LENGTH]>,
    audit_contacts: Vec<String>,
    attestations: Vec<OperatorAttestation>,
}

impl CompliancePolicy {
    /// Populate the operator jurisdictions enforced by this policy.
    pub fn set_operator_jurisdictions(&mut self, jurisdictions: Vec<String>) {
        self.operator_jurisdictions = jurisdictions;
    }

    /// Populate the jurisdiction opt-out list (ISO-3166 alpha-2 codes).
    pub fn set_jurisdiction_opt_outs(&mut self, codes: BTreeSet<String>) {
        self.jurisdiction_opt_outs = codes;
    }

    /// Populate the blinded-CID opt-out list.
    pub fn set_blinded_cid_opt_outs(&mut self, entries: BTreeSet<[u8; Hash::LENGTH]>) {
        self.blinded_cid_opt_outs = entries;
    }

    /// Populate audit contact URIs associated with the policy.
    pub fn set_audit_contacts(&mut self, contacts: Vec<String>) {
        self.audit_contacts = contacts;
    }

    /// Append an operator attestation artefact. Duplicate entries are retained
    /// so auditors can inspect the full attestation history.
    pub fn add_attestation(&mut self, attestation: OperatorAttestation) {
        self.attestations.push(attestation);
    }

    /// Replace the attestation set with the supplied artefacts.
    pub fn set_attestations(&mut self, attestations: Vec<OperatorAttestation>) {
        self.attestations = attestations;
    }

    /// Returns true when the policy carries no overrides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operator_jurisdictions.is_empty()
            && self.jurisdiction_opt_outs.is_empty()
            && self.blinded_cid_opt_outs.is_empty()
            && self.audit_contacts.is_empty()
            && self.attestations.is_empty()
    }

    /// Returns the configured operator jurisdictions.
    #[must_use]
    pub fn operator_jurisdictions(&self) -> &[String] {
        &self.operator_jurisdictions
    }

    /// Provides an iterator over jurisdiction opt-out codes.
    pub fn jurisdiction_opt_outs(&self) -> impl Iterator<Item = &str> {
        self.jurisdiction_opt_outs.iter().map(String::as_str)
    }

    /// Provides an iterator over blinded-CID opt-out entries.
    pub fn blinded_cid_opt_outs(&self) -> impl Iterator<Item = &[u8; Hash::LENGTH]> {
        self.blinded_cid_opt_outs.iter()
    }

    /// Returns the configured audit contact URIs.
    #[must_use]
    pub fn audit_contacts(&self) -> &[String] {
        &self.audit_contacts
    }

    /// Returns all registered operator attestation artefacts.
    #[must_use]
    pub fn attestations(&self) -> &[OperatorAttestation] {
        &self.attestations
    }

    /// Evaluate the compliance policy for the provided payload digest.
    ///
    /// The digest corresponds to the payload manifest (blinded CID) handed to
    /// the orchestrator. Jurisdictional opt-outs win over manifest-level
    /// listings to make it explicit when an operator environment is forced into
    /// direct-only mode.
    #[must_use]
    pub fn decision_for_digest(&self, digest: &[u8; Hash::LENGTH]) -> ComplianceDecision {
        if self
            .operator_jurisdictions
            .iter()
            .any(|code| self.jurisdiction_opt_outs.contains(code))
        {
            return ComplianceDecision::ForceDirect {
                reason: ComplianceReason::JurisdictionOptOut,
            };
        }
        if self.blinded_cid_opt_outs.contains(digest) {
            return ComplianceDecision::ForceDirect {
                reason: ComplianceReason::BlindedCidOptOut,
            };
        }

        ComplianceDecision::Proceed
    }

    /// Produce an audit checklist describing the decision, the matched
    /// overrides, and relevant operator attestations. This enables upstream
    /// tooling to render dashboards and compliance packages without duplicating
    /// policy logic.
    #[must_use]
    pub fn checklist_for_digest(&self, digest: &[u8; Hash::LENGTH]) -> AuditChecklist {
        let decision = self.decision_for_digest(digest);
        let matched_jurisdictions: Vec<String> = self
            .operator_jurisdictions
            .iter()
            .filter(|code| self.jurisdiction_opt_outs.contains(*code))
            .cloned()
            .collect();

        let digest_listed = self.blinded_cid_opt_outs.contains(digest);
        let attestations: Vec<OperatorAttestation> = if matched_jurisdictions.is_empty() {
            self.attestations.clone()
        } else {
            self.attestations
                .iter()
                .filter(|attestation| {
                    attestation
                        .jurisdiction()
                        .map(|code| matched_jurisdictions.iter().any(|m| m == code))
                        .unwrap_or(true)
                })
                .cloned()
                .collect()
        };

        AuditChecklist {
            decision,
            jurisdictions: matched_jurisdictions,
            blinded_cid_listed: digest_listed,
            attestations,
            audit_contacts: self.audit_contacts.clone(),
        }
    }
}

/// Structured representation of an operator attestation artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorAttestation {
    jurisdiction: Option<String>,
    document_uri: String,
    digest: [u8; Hash::LENGTH],
    issued_at_ms: u64,
    expires_at_ms: Option<u64>,
}

impl OperatorAttestation {
    /// Construct a new attestation artefact.
    #[must_use]
    pub fn new(
        jurisdiction: Option<String>,
        document_uri: impl Into<String>,
        digest: [u8; Hash::LENGTH],
        issued_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> Self {
        Self {
            jurisdiction,
            document_uri: document_uri.into(),
            digest,
            issued_at_ms,
            expires_at_ms,
        }
    }

    /// Returns the jurisdiction this attestation covers, if any.
    #[must_use]
    pub fn jurisdiction(&self) -> Option<&str> {
        self.jurisdiction.as_deref()
    }

    /// Returns the attestation document URI.
    #[must_use]
    pub fn document_uri(&self) -> &str {
        &self.document_uri
    }

    /// Returns the attestation digest.
    #[must_use]
    pub const fn digest(&self) -> &[u8; Hash::LENGTH] {
        &self.digest
    }

    /// Returns the timestamp (milliseconds since Unix epoch) when the attestation was issued.
    #[must_use]
    pub const fn issued_at_ms(&self) -> u64 {
        self.issued_at_ms
    }

    /// Returns the optional expiry timestamp for the attestation.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<u64> {
        self.expires_at_ms
    }
}

/// Audit checklist reported to governance tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChecklist {
    decision: ComplianceDecision,
    jurisdictions: Vec<String>,
    blinded_cid_listed: bool,
    attestations: Vec<OperatorAttestation>,
    audit_contacts: Vec<String>,
}

impl AuditChecklist {
    /// The final compliance decision generated for the digest.
    #[must_use]
    pub const fn decision(&self) -> ComplianceDecision {
        self.decision
    }

    /// Jurisdiction codes that triggered the override.
    #[must_use]
    pub fn jurisdictions(&self) -> &[String] {
        &self.jurisdictions
    }

    /// Returns true when the manifest digest appears on the blinded CID opt-out list.
    #[must_use]
    pub const fn blinded_cid_listed(&self) -> bool {
        self.blinded_cid_listed
    }

    /// Attestation artefacts relevant for this decision.
    #[must_use]
    pub fn attestations(&self) -> &[OperatorAttestation] {
        &self.attestations
    }

    /// Audit contact URIs associated with the policy.
    #[must_use]
    pub fn audit_contacts(&self) -> &[String] {
        &self.audit_contacts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_policy_proceeds() {
        let policy = CompliancePolicy::default();
        let digest = Hash::new(b"empty");
        let decision = policy.decision_for_digest(digest.as_ref());
        assert_eq!(decision, ComplianceDecision::Proceed);
    }

    #[test]
    fn jurisdiction_opt_out_forces_direct() {
        let mut policy = CompliancePolicy::default();
        policy.set_operator_jurisdictions(vec!["US".to_string()]);
        policy.set_jurisdiction_opt_outs(BTreeSet::from(["US".to_string()]));
        let digest = Hash::new(b"demo");
        let decision = policy.decision_for_digest(digest.as_ref());
        assert_eq!(
            decision,
            ComplianceDecision::ForceDirect {
                reason: ComplianceReason::JurisdictionOptOut
            }
        );
    }

    #[test]
    fn blinded_cid_opt_out_forces_direct() {
        let mut policy = CompliancePolicy::default();
        let digest = Hash::new(b"demo");
        policy.set_blinded_cid_opt_outs(BTreeSet::from([*digest.as_ref()]));
        let decision = policy.decision_for_digest(digest.as_ref());
        assert_eq!(
            decision,
            ComplianceDecision::ForceDirect {
                reason: ComplianceReason::BlindedCidOptOut
            }
        );
    }

    #[test]
    fn checklist_reports_jurisdiction_matches_and_attestations() {
        let mut policy = CompliancePolicy::default();
        policy.set_operator_jurisdictions(vec!["US".to_string(), "CA".to_string()]);
        policy.set_jurisdiction_opt_outs(BTreeSet::from(["US".to_string()]));
        policy.set_audit_contacts(vec!["mailto:gar-ops@example.com".to_string()]);

        let digest = Hash::new(b"jurisdiction-attestation");
        let us_attestation = OperatorAttestation::new(
            Some("US".to_string()),
            "https://example.com/us-gar.pdf",
            *Hash::new(b"us-attestation").as_ref(),
            1_704_000_000_000,
            Some(1_706_000_000_000),
        );
        let global_attestation = OperatorAttestation::new(
            None,
            "file://global/checklist.txt",
            *Hash::new(b"global-attestation").as_ref(),
            1_702_000_000_000,
            None,
        );
        policy.set_attestations(vec![us_attestation.clone(), global_attestation.clone()]);

        let checklist = policy.checklist_for_digest(digest.as_ref());
        assert_eq!(
            checklist.decision(),
            ComplianceDecision::ForceDirect {
                reason: ComplianceReason::JurisdictionOptOut
            }
        );
        assert_eq!(checklist.jurisdictions(), &["US".to_string()]);
        assert!(!checklist.blinded_cid_listed());
        assert_eq!(
            checklist.attestations(),
            &[us_attestation, global_attestation]
        );
        assert_eq!(
            checklist.audit_contacts(),
            &["mailto:gar-ops@example.com".to_string()]
        );
    }

    #[test]
    fn checklist_marks_digest_opt_out() {
        let mut policy = CompliancePolicy::default();
        let digest = Hash::new(b"cid-optout");
        policy.set_blinded_cid_opt_outs(BTreeSet::from([*digest.as_ref()]));

        let checklist = policy.checklist_for_digest(digest.as_ref());
        assert!(checklist.blinded_cid_listed());
        assert!(checklist.jurisdictions().is_empty());
        assert_eq!(checklist.attestations(), &[]);
    }
}
