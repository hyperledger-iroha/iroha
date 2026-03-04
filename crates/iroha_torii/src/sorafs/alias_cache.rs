//! Alias proof caching policy and evaluation helpers for SoraFS gateways.

use std::time::Duration;

use http::header::HeaderValue;
pub use sorafs_manifest::alias_cache::{
    AliasCachePolicy, AliasProofError, AliasProofEvaluation, AliasProofState, decode_alias_proof,
    unix_now_secs,
};
use sorafs_manifest::pin_registry::AliasProofBundleV1;

use crate::sorafs::registry::{GovernanceSummary, ManifestLineageSummary};

/// Grace window overrides layered atop the base alias cache policy.
#[derive(Debug, Clone, Copy)]
pub struct AliasCacheEnforcement {
    /// Grace period after an approved successor before predecessor proofs are refused.
    pub successor_grace: Duration,
    /// Grace period applied to governance-triggered rotations.
    pub governance_grace: Duration,
}

impl AliasCacheEnforcement {
    /// Returns the successor grace period in seconds.
    #[must_use]
    pub fn successor_grace_secs(self) -> u64 {
        self.successor_grace.as_secs()
    }

    /// Returns the governance grace period in seconds.
    #[must_use]
    pub fn governance_grace_secs(self) -> u64 {
        self.governance_grace.as_secs()
    }
}

/// Creates an [`AliasCachePolicy`] from the runtime configuration.
#[must_use]
pub fn policy_from_config(
    config: &iroha_config::parameters::actual::SorafsAliasCachePolicy,
) -> AliasCachePolicy {
    AliasCachePolicy::new(
        config.positive_ttl,
        config.refresh_window,
        config.hard_expiry,
        config.negative_ttl,
        config.revocation_ttl,
        config.rotation_max_age,
        config.successor_grace,
        config.governance_grace,
    )
}

/// Creates an [`AliasCacheEnforcement`] descriptor from the runtime configuration.
#[must_use]
pub fn enforcement_from_config(
    config: &iroha_config::parameters::actual::SorafsAliasCachePolicy,
) -> AliasCacheEnforcement {
    AliasCacheEnforcement {
        successor_grace: config.successor_grace,
        governance_grace: config.governance_grace,
    }
}

/// Convenience extension providing duration accessors in seconds.
pub trait AliasCachePolicyExt {
    /// Returns the configured positive TTL (seconds).
    fn positive_ttl_secs(&self) -> u64;
    /// Returns the configured refresh window (seconds).
    fn refresh_window_secs(&self) -> u64;
    /// Returns the configured hard expiry (seconds).
    fn hard_expiry_secs(&self) -> u64;
    /// Returns the configured rotation ceiling (seconds).
    fn rotation_max_age_secs(&self) -> u64;
    /// Returns the configured negative cache TTL (seconds).
    fn negative_ttl_secs(&self) -> u64;
    /// Returns the configured revocation TTL (seconds).
    fn revocation_ttl_secs(&self) -> u64;
    /// Returns the configured successor grace period (seconds).
    fn successor_grace_secs(&self) -> u64;
    /// Returns the configured governance grace period (seconds).
    fn governance_grace_secs(&self) -> u64;
}

impl AliasCachePolicyExt for AliasCachePolicy {
    fn positive_ttl_secs(&self) -> u64 {
        self.positive_ttl().as_secs()
    }

    fn refresh_window_secs(&self) -> u64 {
        self.refresh_window().as_secs()
    }

    fn hard_expiry_secs(&self) -> u64 {
        self.hard_expiry().as_secs()
    }

    fn rotation_max_age_secs(&self) -> u64 {
        self.rotation_max_age().as_secs()
    }

    fn negative_ttl_secs(&self) -> u64 {
        self.negative_ttl().as_secs()
    }

    fn revocation_ttl_secs(&self) -> u64 {
        self.revocation_ttl().as_secs()
    }

    fn successor_grace_secs(&self) -> u64 {
        self.successor_grace().as_secs()
    }

    fn governance_grace_secs(&self) -> u64 {
        self.governance_grace().as_secs()
    }
}

impl AliasCachePolicyExt for iroha_config::parameters::actual::SorafsAliasCachePolicy {
    fn positive_ttl_secs(&self) -> u64 {
        self.positive_ttl.as_secs()
    }

    fn refresh_window_secs(&self) -> u64 {
        self.refresh_window.as_secs()
    }

    fn hard_expiry_secs(&self) -> u64 {
        self.hard_expiry.as_secs()
    }

    fn rotation_max_age_secs(&self) -> u64 {
        self.rotation_max_age.as_secs()
    }

    fn negative_ttl_secs(&self) -> u64 {
        self.negative_ttl.as_secs()
    }

    fn revocation_ttl_secs(&self) -> u64 {
        self.revocation_ttl.as_secs()
    }

    fn successor_grace_secs(&self) -> u64 {
        self.successor_grace.as_secs()
    }

    fn governance_grace_secs(&self) -> u64 {
        self.governance_grace.as_secs()
    }
}

fn push_reason(
    decision: &mut CacheDecision,
    reason: impl Into<String>,
    outcome: CacheDecisionOutcome,
) {
    push_reason_with_deadline(decision, reason, outcome, None);
}

fn push_reason_with_deadline(
    decision: &mut CacheDecision,
    reason: impl Into<String>,
    outcome: CacheDecisionOutcome,
    deadline: Option<u64>,
) {
    let reason = reason.into();
    if !decision.reasons.iter().any(|existing| existing == &reason) {
        decision.reasons.push(reason);
    }
    decision.outcome.elevate(outcome);
    if let Some(until) = deadline {
        match decision.serve_until_unix {
            Some(current) if until >= current => {}
            _ => decision.serve_until_unix = Some(until),
        }
    }
}

/// Evaluate cache enforcement decisions using successor and governance metadata.
#[must_use]
pub(crate) fn evaluate_cache_decision(
    evaluation: &AliasProofEvaluation,
    lineage: &ManifestLineageSummary,
    governance: &GovernanceSummary,
    enforcement: AliasCacheEnforcement,
    now_secs: u64,
) -> CacheDecision {
    let mut decision = CacheDecision {
        outcome: CacheDecisionOutcome::Serve,
        reasons: Vec::new(),
        serve_until_unix: None,
        successor: SuccessorAssessment::from_lineage(lineage),
        governance: GovernanceAssessment::from_summary(governance),
    };

    let ttl_deadline = Some(evaluation.expires_at_unix);
    match evaluation.state {
        AliasProofState::Fresh => {}
        AliasProofState::RefreshWindow => {
            push_reason_with_deadline(
                &mut decision,
                "RefreshWindow",
                CacheDecisionOutcome::Hold,
                ttl_deadline,
            );
        }
        AliasProofState::Expired => {
            push_reason(&mut decision, "ExpiredTTL", CacheDecisionOutcome::Refuse);
        }
        AliasProofState::HardExpired => {
            push_reason(&mut decision, "HardExpired", CacheDecisionOutcome::Refuse);
        }
    }

    if evaluation.rotation_due {
        push_reason(&mut decision, "RotationDue", CacheDecisionOutcome::Hold);
    }

    if let Some(reason) = decision.governance.active_reason() {
        let grace_secs = enforcement.governance_grace_secs();
        match decision.governance.effective_at_unix {
            Some(effective) if grace_secs > 0 => {
                let grace_deadline = effective.saturating_add(grace_secs);
                if now_secs < grace_deadline {
                    push_reason_with_deadline(
                        &mut decision,
                        "GovernanceGrace",
                        CacheDecisionOutcome::Hold,
                        Some(grace_deadline),
                    );
                    push_reason(&mut decision, reason, CacheDecisionOutcome::Hold);
                } else {
                    push_reason(&mut decision, reason, CacheDecisionOutcome::Refuse);
                }
            }
            Some(effective) => {
                if now_secs < effective {
                    push_reason_with_deadline(
                        &mut decision,
                        "GovernanceGrace",
                        CacheDecisionOutcome::Hold,
                        Some(effective),
                    );
                    push_reason(&mut decision, reason, CacheDecisionOutcome::Hold);
                } else {
                    push_reason(&mut decision, reason, CacheDecisionOutcome::Refuse);
                }
            }
            None => {
                push_reason(&mut decision, reason, CacheDecisionOutcome::Refuse);
            }
        }
    }

    let successor_snapshot = decision.successor.clone();
    let manifest_missing = successor_snapshot
        .anomalies
        .iter()
        .any(|reason| reason.as_str() == "ManifestMissing");
    let lineage_depth_exceeded = successor_snapshot
        .anomalies
        .iter()
        .any(|reason| reason.as_str() == "LineageDepthExceeded");
    let lineage_cycle_detected = successor_snapshot
        .anomalies
        .iter()
        .any(|reason| reason.as_str() == "LineageCycleDetected");
    let successor_fork_resolved = successor_snapshot
        .anomalies
        .iter()
        .any(|reason| reason.as_str() == "SuccessorForkResolved");

    if manifest_missing {
        push_reason(
            &mut decision,
            "ManifestMissing",
            CacheDecisionOutcome::Refuse,
        );
    } else if lineage_depth_exceeded {
        push_reason(
            &mut decision,
            "LineageDepthExceeded",
            CacheDecisionOutcome::Refuse,
        );
    } else if lineage_cycle_detected {
        push_reason(
            &mut decision,
            "LineageCycleDetected",
            CacheDecisionOutcome::Refuse,
        );
    } else if successor_fork_resolved {
        push_reason(
            &mut decision,
            "SuccessorForkResolved",
            CacheDecisionOutcome::Hold,
        );
    }

    if let Some(approved_at) = successor_snapshot.approved_at_unix {
        let grace_secs = enforcement.successor_grace.as_secs();
        if now_secs < approved_at {
            push_reason_with_deadline(
                &mut decision,
                "ApprovedSuccessorPending",
                CacheDecisionOutcome::Hold,
                Some(approved_at),
            );
        } else if grace_secs > 0 {
            let deadline = approved_at.saturating_add(grace_secs);
            if now_secs < deadline {
                push_reason_with_deadline(
                    &mut decision,
                    "ApprovedSuccessorGrace",
                    CacheDecisionOutcome::Hold,
                    Some(deadline),
                );
            } else {
                push_reason(
                    &mut decision,
                    "ApprovedSuccessor",
                    CacheDecisionOutcome::Refuse,
                );
            }
        } else {
            push_reason(
                &mut decision,
                "ApprovedSuccessor",
                CacheDecisionOutcome::Refuse,
            );
        }
    } else if successor_snapshot.approved {
        push_reason(
            &mut decision,
            "ApprovedSuccessor",
            CacheDecisionOutcome::Refuse,
        );
        push_reason(
            &mut decision,
            "MissingTimestamp",
            CacheDecisionOutcome::Refuse,
        );
    } else if successor_snapshot.exists {
        push_reason(
            &mut decision,
            "PendingSuccessor",
            CacheDecisionOutcome::Hold,
        );
    }

    decision.reasons.sort();
    decision.reasons.dedup();
    decision
}

/// Result of applying alias cache enforcement rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheDecisionOutcome {
    /// Alias proof may be served without mitigation.
    Serve,
    /// Alias proof must be held while remediation or refresh completes.
    Hold,
    /// Alias proof must be refused to the caller.
    Refuse,
}

impl CacheDecisionOutcome {
    fn elevate(&mut self, other: Self) {
        if other.severity() > self.severity() {
            *self = other;
        }
    }

    fn severity(self) -> u8 {
        match self {
            CacheDecisionOutcome::Serve => 0,
            CacheDecisionOutcome::Hold => 1,
            CacheDecisionOutcome::Refuse => 2,
        }
    }
}

/// Aggregate evaluation describing how a gateway should respond to an alias proof.
#[derive(Debug, Clone)]
pub struct CacheDecision {
    /// Final outcome after evaluating policy, successor, and governance hints.
    pub outcome: CacheDecisionOutcome,
    /// Collected reason codes that justify the final outcome.
    pub reasons: Vec<String>,
    /// Optional UNIX timestamp (seconds) indicating until when the current proof may be served.
    pub serve_until_unix: Option<u64>,
    /// Summary of successor manifest information used for decision making.
    pub successor: SuccessorAssessment,
    /// Summary of governance state used for decision making.
    pub governance: GovernanceAssessment,
}

impl CacheDecision {
    /// Returns a human-readable status label blending cache enforcement with proof state.
    #[must_use]
    pub fn status_label(&self, evaluation: &AliasProofEvaluation) -> String {
        match self.outcome {
            CacheDecisionOutcome::Refuse => {
                if self
                    .reasons
                    .iter()
                    .any(|reason| reason == "LineageCycleDetected")
                {
                    "lineage-invalid".to_owned()
                } else if self
                    .reasons
                    .iter()
                    .any(|reason| reason.starts_with("Governance"))
                {
                    "governance-refused".to_owned()
                } else if self
                    .reasons
                    .iter()
                    .any(|reason| reason.starts_with("ApprovedSuccessor"))
                {
                    "successor-refused".to_owned()
                } else {
                    evaluation.status_label().to_owned()
                }
            }
            CacheDecisionOutcome::Hold => {
                if self
                    .reasons
                    .iter()
                    .any(|reason| reason == "ApprovedSuccessorGrace")
                {
                    "refresh-successor".to_owned()
                } else if self
                    .reasons
                    .iter()
                    .any(|reason| reason == "GovernanceGrace")
                {
                    "refresh-governance".to_owned()
                } else if self
                    .reasons
                    .iter()
                    .any(|reason| reason == "PendingSuccessor")
                {
                    "pending-successor".to_owned()
                } else {
                    evaluation.status_label().to_owned()
                }
            }
            CacheDecisionOutcome::Serve => evaluation.status_label().to_owned(),
        }
    }
}

/// Projection of successor manifest state relevant to cache enforcement.
#[derive(Debug, Clone)]
pub struct SuccessorAssessment {
    /// Indicates whether a successor manifest exists in the lineage.
    pub exists: bool,
    /// Hex encoding of the lineage head manifest.
    pub head_hex: Option<String>,
    /// True when the immediate successor has been approved by governance.
    pub approved: bool,
    /// UNIX timestamp (seconds) when the approved successor becomes active.
    pub approved_at_unix: Option<u64>,
    /// Distance in blocks from the evaluated manifest to the lineage head.
    pub depth_to_head: u32,
    /// Collected anomaly codes discovered during lineage inspection.
    pub anomalies: Vec<String>,
}

impl SuccessorAssessment {
    fn from_lineage(lineage: &ManifestLineageSummary) -> Self {
        Self {
            exists: lineage.immediate_successor.is_some(),
            head_hex: Some(lineage.head_hex.clone()),
            approved: lineage.approved_successor.is_some(),
            approved_at_unix: lineage
                .approved_successor
                .as_ref()
                .and_then(|succ| succ.status_timestamp_unix),
            depth_to_head: lineage.depth_to_head,
            anomalies: lineage.anomalies.clone(),
        }
    }
}

/// Projection of governance decisions affecting the evaluated alias.
#[derive(Debug, Clone)]
pub struct GovernanceAssessment {
    /// Reference identifiers (CIDs or digests) tied to the governance decision.
    pub ref_ids: Vec<String>,
    /// True when governance explicitly revoked the alias or manifest.
    pub revoked: bool,
    /// True when governance froze the alias or manifest.
    pub frozen: bool,
    /// True when governance rotated signer sets without explicit revoke/freeze.
    pub rotated: bool,
    /// Optional UNIX timestamp (seconds) when the governance decision takes effect.
    pub effective_at_unix: Option<u64>,
}

impl GovernanceAssessment {
    fn from_summary(summary: &GovernanceSummary) -> Self {
        let mut refs = Vec::new();
        for reference in &summary.references {
            if let Some(cid) = &reference.cid {
                refs.push(cid.clone());
            } else if let Some(digest) = &reference.manifest_digest_hex {
                refs.push(digest.clone());
            }
        }
        refs.sort();
        refs.dedup();

        let revoked = summary.revoked.is_some();
        let frozen = summary.frozen.is_some();
        let rotated = summary.rotated.is_some();
        let effective_at_unix = summary
            .revoked
            .as_ref()
            .and_then(|reference| reference.effective_at_unix)
            .or_else(|| {
                summary
                    .frozen
                    .as_ref()
                    .and_then(|reference| reference.effective_at_unix)
            })
            .or_else(|| {
                summary
                    .rotated
                    .as_ref()
                    .and_then(|reference| reference.effective_at_unix)
            });

        Self {
            ref_ids: refs,
            revoked,
            frozen,
            rotated,
            effective_at_unix,
        }
    }

    fn active_reason(&self) -> Option<&'static str> {
        if self.revoked {
            Some("GovernanceRevoked")
        } else if self.frozen {
            Some("GovernanceFrozen")
        } else if self.rotated {
            Some("GovernanceRotated")
        } else {
            None
        }
    }
}

/// HTTP-specific helpers for alias cache policies.
pub trait AliasCachePolicyHttpExt {
    /// Builds the recommended `Cache-Control` header for positive responses.
    fn cache_control_header(&self) -> HeaderValue;
    /// Builds the `Cache-Control` header for negative cache responses (`404`).
    fn negative_cache_control_header(&self) -> HeaderValue;
    /// Builds the `Cache-Control` header for revoked aliases (`410`).
    fn revocation_cache_control_header(&self) -> HeaderValue;
}

impl AliasCachePolicyHttpExt for AliasCachePolicy {
    fn cache_control_header(&self) -> HeaderValue {
        let value = format!(
            "max-age={}, stale-while-revalidate={}",
            self.positive_ttl().as_secs(),
            self.refresh_window().as_secs()
        );
        HeaderValue::from_str(&value).expect("cache-control value must be ASCII")
    }

    fn negative_cache_control_header(&self) -> HeaderValue {
        let value = format!("max-age={}", self.negative_ttl().as_secs());
        HeaderValue::from_str(&value).expect("cache-control value must be ASCII")
    }

    fn revocation_cache_control_header(&self) -> HeaderValue {
        let value = format!("max-age={}", self.revocation_ttl().as_secs());
        HeaderValue::from_str(&value).expect("cache-control value must be ASCII")
    }
}

/// HTTP header conversions for alias proof evaluations.
pub trait AliasProofEvaluationExt {
    /// Builds the `Age` header value reflecting the proof age.
    fn age_header(&self) -> HeaderValue;
    /// Builds the optional Warning header for refresh/expiry states.
    fn warning_header(&self) -> Option<HeaderValue>;
    /// Builds the `Sora-Proof-Status` header.
    fn proof_status_header(&self) -> HeaderValue;
}

impl AliasProofEvaluationExt for AliasProofEvaluation {
    fn age_header(&self) -> HeaderValue {
        HeaderValue::from_str(&self.age.as_secs().to_string())
            .expect("age header must be ASCII digits")
    }

    fn warning_header(&self) -> Option<HeaderValue> {
        match self.state {
            AliasProofState::RefreshWindow => Some(HeaderValue::from_static(
                r#"199 - "alias proof refresh in-flight""#,
            )),
            AliasProofState::Expired => {
                Some(HeaderValue::from_static(r#"110 - "alias proof stale""#))
            }
            AliasProofState::HardExpired => {
                Some(HeaderValue::from_static(r#"111 - "alias proof expired""#))
            }
            AliasProofState::Fresh => None,
        }
    }

    fn proof_status_header(&self) -> HeaderValue {
        HeaderValue::from_str(self.status_label()).expect("status label must be ASCII")
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_config::parameters::actual::SorafsAliasCachePolicy as ConfigPolicy;
    use sorafs_manifest::{
        CouncilSignature,
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
        },
    };

    use super::*;
    use crate::sorafs::registry::{
        GovernanceRefKind, GovernanceReference, GovernanceSummary, ManifestLineageSummary,
        approved_successor_for_tests,
    };

    fn sample_bundle(generated: u64, expires: u64) -> AliasProofBundleV1 {
        let binding = AliasBindingV1 {
            alias: "docs/sora".into(),
            manifest_cid: vec![0x42, 0x24],
            bound_at: 1,
            expiry_epoch: 10,
        };
        let mut bundle = AliasProofBundleV1 {
            binding,
            registry_root: [0; 32],
            registry_height: 1,
            generated_at_unix: generated,
            expires_at_unix: expires,
            merkle_path: Vec::new(),
            council_signatures: Vec::new(),
        };
        bundle.registry_root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)
            .expect("compute alias merkle root");

        let digest = alias_proof_signature_digest(&bundle);
        let signing_key = SigningKey::from_bytes(&[0xAB; 32]);
        let signature = signing_key.sign(&digest);
        bundle.council_signatures.push(CouncilSignature {
            signer: signing_key.verifying_key().to_bytes(),
            signature: signature.to_bytes().to_vec(),
        });
        bundle
    }

    fn sample_policy() -> AliasCachePolicy {
        AliasCachePolicy::new(
            Duration::from_mins(10),
            Duration::from_mins(2),
            Duration::from_mins(15),
            Duration::from_mins(1),
            Duration::from_mins(5),
            Duration::from_hours(6),
            Duration::from_mins(5),
            Duration::ZERO,
        )
    }

    fn evaluation_at(now: u64, generated_at: u64, expires_at: u64) -> AliasProofEvaluation {
        AliasProofEvaluation {
            state: AliasProofState::Fresh,
            rotation_due: false,
            age: Duration::from_secs(now.saturating_sub(generated_at)),
            generated_at_unix: generated_at,
            expires_at_unix: expires_at,
            expires_in: Some(Duration::from_secs(expires_at.saturating_sub(now))),
        }
    }

    #[test]
    fn evaluates_fresh_refresh_and_expired_states() {
        let policy = sample_policy();
        let now = 1_000_000;

        let fresh = sample_bundle(now - 120, now + 600);
        let eval_fresh = policy.evaluate(&fresh, now);
        assert_eq!(eval_fresh.state, AliasProofState::Fresh);
        assert!(!eval_fresh.rotation_due);

        let refresh = sample_bundle(now - 550, now + 600);
        let eval_refresh = policy.evaluate(&refresh, now);
        assert_eq!(eval_refresh.state, AliasProofState::RefreshWindow);

        let expired = sample_bundle(now - 720, now + 600);
        let eval_expired = policy.evaluate(&expired, now);
        assert_eq!(eval_expired.state, AliasProofState::Expired);

        let hard = sample_bundle(now - 1_200, now + 600);
        let eval_hard = policy.evaluate(&hard, now);
        assert_eq!(eval_hard.state, AliasProofState::HardExpired);

        let expiry = sample_bundle(now - 300, now - 1);
        let eval_expiry = policy.evaluate(&expiry, now);
        assert_eq!(eval_expiry.state, AliasProofState::HardExpired);
    }

    #[test]
    fn rotation_due_flag_triggers_when_configured() {
        let config = ConfigPolicy {
            positive_ttl: Duration::from_mins(20),
            refresh_window: Duration::from_mins(1),
            hard_expiry: Duration::from_hours(1),
            rotation_max_age: Duration::from_mins(5),
            ..ConfigPolicy::default()
        };
        let policy = policy_from_config(&config);
        let now = 50_000;
        let bundle = sample_bundle(now - 350, now + 10_000);
        let eval = policy.evaluate(&bundle, now);
        assert_eq!(eval.state, AliasProofState::Fresh);
        assert!(eval.rotation_due);
    }

    #[test]
    fn cache_control_header_matches_policy() {
        let policy = sample_policy();
        let header = policy
            .cache_control_header()
            .to_str()
            .expect("header value ascii")
            .to_owned();
        assert_eq!(header, "max-age=600, stale-while-revalidate=120");
    }

    #[test]
    fn policy_from_config_propagates_grace_windows() {
        let config = ConfigPolicy {
            successor_grace: Duration::from_secs(123),
            governance_grace: Duration::from_secs(11),
            ..ConfigPolicy::default()
        };
        let policy = policy_from_config(&config);
        assert_eq!(policy.successor_grace().as_secs(), 123);
        assert_eq!(policy.governance_grace().as_secs(), 11);
        let enforcement = enforcement_from_config(&config);
        assert_eq!(enforcement.successor_grace_secs(), 123);
        assert_eq!(enforcement.governance_grace_secs(), 11);
    }

    #[test]
    fn manifest_missing_anomaly_forces_refusal() {
        let evaluation = evaluation_at(10_000, 9_500, 12_000);
        let lineage = ManifestLineageSummary {
            successor_of_hex: None,
            head_hex: "deadbeef".into(),
            depth_to_head: 0,
            approved_successor: None,
            immediate_successor: None,
            anomalies: vec!["ManifestMissing".to_string()],
        };
        let governance = GovernanceSummary {
            references: Vec::new(),
            revoked: None,
            frozen: None,
            rotated: None,
        };
        let enforcement = AliasCacheEnforcement {
            successor_grace: Duration::from_secs(0),
            governance_grace: Duration::from_secs(0),
        };
        let decision =
            evaluate_cache_decision(&evaluation, &lineage, &governance, enforcement, 10_000);
        assert_eq!(
            decision.outcome,
            CacheDecisionOutcome::Refuse,
            "manifest-missing anomaly must refuse serving the proof"
        );
        assert!(
            decision
                .reasons
                .iter()
                .any(|reason| reason == "ManifestMissing"),
            "expected ManifestMissing reason in {:?}",
            decision.reasons
        );
    }

    #[test]
    fn lineage_depth_limit_anomaly_forces_refusal() {
        let evaluation = evaluation_at(10_000, 9_500, 12_000);
        let lineage = ManifestLineageSummary {
            successor_of_hex: None,
            head_hex: "deadbeef".into(),
            depth_to_head: 0,
            approved_successor: None,
            immediate_successor: None,
            anomalies: vec!["LineageDepthExceeded".to_string()],
        };
        let governance = GovernanceSummary {
            references: Vec::new(),
            revoked: None,
            frozen: None,
            rotated: None,
        };
        let enforcement = AliasCacheEnforcement {
            successor_grace: Duration::from_secs(0),
            governance_grace: Duration::from_secs(0),
        };
        let decision =
            evaluate_cache_decision(&evaluation, &lineage, &governance, enforcement, 10_000);
        assert_eq!(
            decision.outcome,
            CacheDecisionOutcome::Refuse,
            "lineage depth anomalies must refuse serving the proof"
        );
        assert!(
            decision
                .reasons
                .iter()
                .any(|reason| reason == "LineageDepthExceeded"),
            "expected LineageDepthExceeded reason in {:?}",
            decision.reasons
        );
    }

    #[test]
    fn proof_headers_reflect_evaluation_state() {
        let policy = sample_policy();
        let now = 42_000;
        let bundle = sample_bundle(now - 610, now + 600);
        let evaluation = policy.evaluate(&bundle, now);
        assert_eq!(evaluation.age_header().to_str().unwrap(), "610");
        assert_eq!(
            evaluation
                .warning_header()
                .map(|value| value.to_str().unwrap().to_owned()),
            Some(r#"110 - "alias proof stale""#.into())
        );
        assert_eq!(
            evaluation.proof_status_header().to_str().unwrap(),
            "expired"
        );
    }

    #[test]
    fn decode_alias_proof_validates_bundle() {
        let bytes = encode_valid_bundle();
        let decoded = decode_alias_proof(&bytes).expect("decode alias proof");
        assert_eq!(decoded.binding.alias, "docs/sora");
    }

    #[test]
    fn successor_grace_transitions_from_hold_to_refuse() {
        let approved_epoch = 64;
        let approved_at = 1_700_000_000;
        let successor = approved_successor_for_tests("succ", approved_epoch, Some(approved_at));
        let lineage = ManifestLineageSummary {
            successor_of_hex: Some("prev".to_owned()),
            head_hex: "succ".to_owned(),
            depth_to_head: 1,
            approved_successor: Some(successor.clone()),
            immediate_successor: Some(successor),
            anomalies: Vec::new(),
        };
        let governance = GovernanceSummary {
            references: Vec::new(),
            revoked: None,
            frozen: None,
            rotated: None,
        };
        let enforcement = AliasCacheEnforcement {
            successor_grace: Duration::from_mins(5),
            governance_grace: Duration::from_secs(0),
        };
        let generated_at = approved_at.saturating_sub(600);
        let expires_at = approved_at + 3_600;

        let within_grace = approved_at + 120;
        let eval_within = evaluation_at(within_grace, generated_at, expires_at);
        let decision_within = evaluate_cache_decision(
            &eval_within,
            &lineage,
            &governance,
            enforcement,
            within_grace,
        );
        assert_eq!(decision_within.outcome, CacheDecisionOutcome::Hold);
        assert!(
            decision_within
                .reasons
                .contains(&"ApprovedSuccessorGrace".to_owned())
        );
        assert_eq!(
            decision_within.serve_until_unix,
            Some(approved_at + enforcement.successor_grace_secs())
        );
        assert_eq!(
            decision_within.status_label(&eval_within),
            "refresh-successor"
        );

        let past_grace = approved_at + enforcement.successor_grace_secs() + 90;
        let eval_past = evaluation_at(past_grace, generated_at, expires_at);
        let decision_past =
            evaluate_cache_decision(&eval_past, &lineage, &governance, enforcement, past_grace);
        assert_eq!(decision_past.outcome, CacheDecisionOutcome::Refuse);
        assert!(
            decision_past
                .reasons
                .contains(&"ApprovedSuccessor".to_owned())
        );
        assert_eq!(decision_past.status_label(&eval_past), "successor-refused");
        assert!(decision_past.serve_until_unix.is_none());
    }

    #[test]
    fn governance_grace_transitions_to_refusal() {
        let effective_at = 1_700_000_500;
        let rotation_reference = GovernanceReference {
            cid: Some("cid:rotate".to_owned()),
            kind: GovernanceRefKind::AliasRotate,
            effective_at_unix: Some(effective_at),
            alias_label: Some("docs/main".to_owned()),
            manifest_digest_hex: Some("prev".to_owned()),
            signers: vec!["council".to_owned()],
        };
        let governance = GovernanceSummary {
            references: vec![rotation_reference.clone()],
            revoked: None,
            frozen: None,
            rotated: Some(rotation_reference),
        };
        let lineage = ManifestLineageSummary {
            successor_of_hex: None,
            head_hex: "prev".to_owned(),
            depth_to_head: 0,
            approved_successor: None,
            immediate_successor: None,
            anomalies: Vec::new(),
        };
        let enforcement = AliasCacheEnforcement {
            successor_grace: Duration::from_secs(0),
            governance_grace: Duration::from_mins(3),
        };
        let generated_at = effective_at.saturating_sub(300);
        let expires_at = effective_at + 3_000;

        let within_grace = effective_at + 60;
        let eval_within = evaluation_at(within_grace, generated_at, expires_at);
        let decision_within = evaluate_cache_decision(
            &eval_within,
            &lineage,
            &governance,
            enforcement,
            within_grace,
        );
        assert_eq!(decision_within.outcome, CacheDecisionOutcome::Hold);
        assert!(
            decision_within
                .reasons
                .contains(&"GovernanceGrace".to_owned())
        );
        assert!(
            decision_within
                .reasons
                .contains(&"GovernanceRotated".to_owned())
        );
        assert_eq!(
            decision_within.serve_until_unix,
            Some(effective_at + enforcement.governance_grace_secs())
        );
        assert_eq!(
            decision_within.status_label(&eval_within),
            "refresh-governance"
        );

        let past_grace = effective_at + enforcement.governance_grace_secs() + 45;
        let eval_past = evaluation_at(past_grace, generated_at, expires_at);
        let decision_past =
            evaluate_cache_decision(&eval_past, &lineage, &governance, enforcement, past_grace);
        assert_eq!(decision_past.outcome, CacheDecisionOutcome::Refuse);
        assert!(
            decision_past
                .reasons
                .contains(&"GovernanceRotated".to_owned())
        );
        assert_eq!(decision_past.status_label(&eval_past), "governance-refused");
        assert!(decision_past.serve_until_unix.is_none());
    }

    fn encode_valid_bundle() -> Vec<u8> {
        let bundle = sample_bundle(100, 200);
        norito::to_bytes(&bundle).expect("encode alias bundle")
    }
}
