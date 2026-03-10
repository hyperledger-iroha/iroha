//! Lane compliance policy evaluation and loading.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use iroha_crypto::privacy::LaneCommitmentId;
use iroha_data_model::{
    account::AccountId,
    nexus::{
        DataSpaceId, LaneCompliancePolicy, LaneCompliancePolicyId, LaneComplianceRule, LaneId,
        ParticipantSelector, UniversalAccountId,
    },
};
use iroha_logger::warn;
use norito::codec::DecodeAll;

use crate::interlane::LanePrivacyRegistryHandle;

/// Static engine that evaluates lane compliance policies.
#[derive(Debug)]
pub struct LaneComplianceEngine {
    policies: BTreeMap<LaneId, Arc<LaneCompliancePolicy>>,
    audit_only: bool,
}

static EMPTY_PRIVACY_COMMITMENTS: LazyLock<BTreeSet<LaneCommitmentId>> =
    LazyLock::new(BTreeSet::new);

impl LaneComplianceEngine {
    /// Construct an engine from explicit policy definitions.
    ///
    /// # Errors
    /// Returns [`LaneComplianceLoadError`] when duplicate lane identifiers are encountered.
    pub fn from_policies(
        policies: Vec<LaneCompliancePolicy>,
        audit_only: bool,
    ) -> Result<Self, LaneComplianceLoadError> {
        let mut map = BTreeMap::new();
        for policy in policies {
            let lane_id = policy.lane_id;
            if map.insert(lane_id, Arc::new(policy)).is_some() {
                return Err(LaneComplianceLoadError::DuplicateLane { lane_id });
            }
        }
        Ok(Self {
            policies: map,
            audit_only,
        })
    }

    /// Load Norito-encoded policy bundles from the supplied directory.
    ///
    /// # Errors
    /// Returns [`LaneComplianceLoadError`] when the directory cannot be read, decoded, or no
    /// policies are present.
    pub fn from_directory(dir: &Path, audit_only: bool) -> Result<Self, LaneComplianceLoadError> {
        if !dir.exists() {
            return Err(LaneComplianceLoadError::MissingDirectory(dir.to_path_buf()));
        }
        if !dir.is_dir() {
            return Err(LaneComplianceLoadError::NotADirectory(dir.to_path_buf()));
        }

        let mut policies = Vec::new();
        for entry in fs::read_dir(dir).map_err(|source| LaneComplianceLoadError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| LaneComplianceLoadError::ReadDir {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path).map_err(|source| LaneComplianceLoadError::Io {
                path: path.clone(),
                source,
            })?;
            let mut slice: &[u8] = &bytes;
            let policy = LaneCompliancePolicy::decode_all(&mut slice).map_err(|source| {
                LaneComplianceLoadError::Decode {
                    path: path.clone(),
                    source,
                }
            })?;
            policies.push(policy);
        }

        if policies.is_empty() {
            return Err(LaneComplianceLoadError::Empty(dir.to_path_buf()));
        }

        Self::from_policies(policies, audit_only)
    }

    /// Evaluate the policy if known for the given lane.
    #[must_use]
    pub fn evaluate(&self, ctx: &LaneComplianceContext<'_>) -> LaneComplianceEvaluation {
        let Some(policy) = self.policies.get(&ctx.lane_id) else {
            return LaneComplianceEvaluation::NotConfigured;
        };
        if let Some(rule) = Self::match_rule(&policy.deny, ctx) {
            return LaneComplianceEvaluation::Denied(LaneComplianceDecisionRecord::new(
                policy.id,
                ctx.lane_id,
                ctx.dataspace_id,
                ctx.authority.clone(),
                LaneComplianceDecision::Deny,
                rule.reason_code()
                    .map(str::to_string)
                    .or_else(|| Some("lane compliance deny rule matched".to_string())),
            ));
        }

        if policy.allow.is_empty() {
            return LaneComplianceEvaluation::Allowed(LaneComplianceDecisionRecord::new(
                policy.id,
                ctx.lane_id,
                ctx.dataspace_id,
                ctx.authority.clone(),
                LaneComplianceDecision::Allow,
                None,
            ));
        }

        if let Some(rule) = Self::match_rule(&policy.allow, ctx) {
            return LaneComplianceEvaluation::Allowed(LaneComplianceDecisionRecord::new(
                policy.id,
                ctx.lane_id,
                ctx.dataspace_id,
                ctx.authority.clone(),
                LaneComplianceDecision::Allow,
                rule.reason_code().map(str::to_string),
            ));
        }

        LaneComplianceEvaluation::Denied(LaneComplianceDecisionRecord::new(
            policy.id,
            ctx.lane_id,
            ctx.dataspace_id,
            ctx.authority.clone(),
            LaneComplianceDecision::Deny,
            Some("no lane compliance allow rule matched".to_string()),
        ))
    }

    fn match_rule<'a>(
        rules: &'a [LaneComplianceRule],
        ctx: &LaneComplianceContext<'_>,
    ) -> Option<&'a LaneComplianceRule> {
        rules
            .iter()
            .find(|rule| selector_matches(&rule.selector, ctx))
    }

    /// Whether the engine is running in audit-only mode.
    #[must_use]
    pub fn audit_only(&self) -> bool {
        self.audit_only
    }
}

fn selector_matches(selector: &ParticipantSelector, ctx: &LaneComplianceContext<'_>) -> bool {
    if let Some(account) = selector.account.as_ref()
        && account != ctx.authority
    {
        return false;
    }

    if selector.domain.is_some() {
        return false;
    }

    if selector.domain_prefix.is_some() {
        return false;
    }

    if let Some(required_uaid) = selector.uaid.as_ref() {
        match ctx.uaid {
            Some(current) if current == required_uaid => {}
            _ => return false,
        }
    }

    if let Some(prefix) = selector.uaid_prefix.as_ref() {
        match ctx.uaid {
            Some(current)
                if current
                    .as_hash()
                    .as_ref()
                    .starts_with(prefix.as_hash().as_ref()) => {}
            _ => return false,
        }
    }

    if let Some(tag) = selector.capability_tag.as_deref() {
        if !ctx
            .capability_tags
            .iter()
            .any(|capability| capability == tag)
        {
            return false;
        }
    }

    if !selector.privacy_commitments_any_of.is_empty() {
        let has_verified = selector
            .privacy_commitments_any_of
            .iter()
            .any(|id| ctx.verified_privacy_commitments.contains(id));
        if !has_verified {
            return false;
        }
    }

    true
}

/// Metadata describing the evaluation context.
#[derive(Debug)]
pub struct LaneComplianceContext<'a> {
    /// Lane assigned to the transaction.
    pub lane_id: LaneId,
    /// Dataspace assigned to the transaction.
    pub dataspace_id: DataSpaceId,
    /// Transaction authority.
    pub authority: &'a AccountId,
    /// UAID derived for the authority (if known).
    pub uaid: Option<&'a UniversalAccountId>,
    /// Capability tags attached to the transaction or manifest.
    pub capability_tags: &'a [String],
    /// Snapshot of the privacy commitment registry, if available.
    pub lane_privacy_registry: Option<LanePrivacyRegistryHandle>,
    /// Commitments proven by attached lane privacy witnesses.
    pub verified_privacy_commitments: &'a BTreeSet<LaneCommitmentId>,
}

impl<'a> LaneComplianceContext<'a> {
    /// Convenience constructor for contexts without capability tags.
    #[must_use]
    pub fn new(lane_id: LaneId, dataspace_id: DataSpaceId, authority: &'a AccountId) -> Self {
        Self {
            lane_id,
            dataspace_id,
            authority,
            uaid: None,
            capability_tags: &[],
            lane_privacy_registry: None,
            verified_privacy_commitments: &EMPTY_PRIVACY_COMMITMENTS,
        }
    }
}

/// Outcome of evaluating a transaction against a lane policy.
#[derive(Debug)]
pub enum LaneComplianceEvaluation {
    /// No policy configured for the lane.
    NotConfigured,
    /// Transaction satisfied the policy.
    Allowed(LaneComplianceDecisionRecord),
    /// Transaction violates the policy.
    Denied(LaneComplianceDecisionRecord),
}

impl LaneComplianceEvaluation {
    /// Access the decision record (if available).
    #[must_use]
    pub fn record(&self) -> Option<&LaneComplianceDecisionRecord> {
        match self {
            Self::Allowed(record) | Self::Denied(record) => Some(record),
            Self::NotConfigured => None,
        }
    }
}

/// Record describing a single decision.
#[derive(Debug, Clone)]
pub struct LaneComplianceDecisionRecord {
    /// Policy identifier evaluated.
    pub policy_id: LaneCompliancePolicyId,
    /// Lane identifier evaluated.
    pub lane_id: LaneId,
    /// Dataspace identifier evaluated.
    pub dataspace_id: DataSpaceId,
    /// Transaction authority.
    pub authority: AccountId,
    /// Decision outcome.
    pub decision: LaneComplianceDecision,
    /// Optional human-readable reason.
    pub reason: Option<String>,
}

impl LaneComplianceDecisionRecord {
    fn new(
        policy_id: LaneCompliancePolicyId,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        authority: AccountId,
        decision: LaneComplianceDecision,
        reason: Option<String>,
    ) -> Self {
        Self {
            policy_id,
            lane_id,
            dataspace_id,
            authority,
            decision,
            reason,
        }
    }
}

/// Decision kind emitted by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneComplianceDecision {
    /// Transaction allowed.
    Allow,
    /// Transaction denied.
    Deny,
}

/// Errors produced when loading lane compliance policies.
#[derive(Debug, thiserror::Error)]
pub enum LaneComplianceLoadError {
    /// Directory is missing.
    #[error("lane compliance policy directory {0:?} does not exist")]
    MissingDirectory(PathBuf),
    /// Path is not a directory.
    #[error("lane compliance policy path {0:?} is not a directory")]
    NotADirectory(PathBuf),
    /// Failed to enumerate entries in the directory.
    #[error("failed to read lane compliance directory {path:?}")]
    ReadDir {
        /// Directory that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// I/O failure while reading a policy file.
    #[error("failed to read lane compliance policy {path:?}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode a Norito policy bundle.
    #[error("failed to decode lane compliance policy {path:?}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: norito::codec::Error,
    },
    /// Duplicate lane identifier detected.
    #[error("duplicate lane compliance policy for lane {lane_id}")]
    DuplicateLane {
        /// Lane identifier.
        lane_id: LaneId,
    },
    /// Directory contained no policies.
    #[error("lane compliance directory {0:?} does not contain any policies")]
    Empty(PathBuf),
}

impl LaneComplianceDecisionRecord {
    /// Helper for logging evaluation summaries.
    pub fn log(&self, audit_only: bool) {
        let mode = if audit_only { "audit_only" } else { "enforced" };
        match self.decision {
            LaneComplianceDecision::Allow => {
                if let Some(reason) = &self.reason {
                    warn!(
                        lane = %self.lane_id.as_u32(),
                        dataspace = %self.dataspace_id.as_u64(),
                        mode,
                        authority = %self.authority,
                        reason,
                        policy = ?self.policy_id.as_hash(),
                        "lane compliance allow decision recorded",
                    );
                }
            }
            LaneComplianceDecision::Deny => {
                warn!(
                    lane = %self.lane_id.as_u32(),
                    dataspace = %self.dataspace_id.as_u64(),
                    mode,
                    authority = %self.authority,
                    reason = %self.reason.as_deref().unwrap_or("lane compliance deny rule"),
                    policy = ?self.policy_id.as_hash(),
                    "lane compliance deny decision recorded",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::PathBuf};

    use iroha_crypto::{
        Algorithm, Hash, KeyPair,
        privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment},
    };
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        nexus::{AuditControls, DataSpaceId, JurisdictionSet, LaneStorageProfile, LaneVisibility},
    };

    use super::*;
    use crate::{governance::manifest::LaneManifestStatus, interlane::LanePrivacyRegistry};

    fn account(name: &str, domain: &str) -> AccountId {
        let seed_literal = format!("{name}::{domain}");
        let mut seed = seed_literal.into_bytes();
        if seed.is_empty() {
            seed.extend_from_slice(b"lane-compliance-account");
        }
        let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_policy(
        lane_id: LaneId,
        allow: &[AccountId],
        deny: &[AccountId],
    ) -> LaneCompliancePolicy {
        let mut hash_bytes = [0u8; 32];
        hash_bytes[..4].copy_from_slice(&lane_id.as_u32().to_le_bytes());
        LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed(hash_bytes)),
            version: 1,
            lane_id,
            dataspace_id: DataSpaceId::GLOBAL,
            jurisdiction: JurisdictionSet::default(),
            deny: deny
                .iter()
                .cloned()
                .map(|account| LaneComplianceRule {
                    selector: ParticipantSelector {
                        account: Some(account),
                        ..ParticipantSelector::default()
                    },
                    reason_code: Some("deny".to_string()),
                    jurisdiction_override: JurisdictionSet::default(),
                })
                .collect(),
            allow: allow
                .iter()
                .cloned()
                .map(|account| LaneComplianceRule {
                    selector: ParticipantSelector {
                        account: Some(account),
                        ..ParticipantSelector::default()
                    },
                    reason_code: Some("allow".to_string()),
                    jurisdiction_override: JurisdictionSet::default(),
                })
                .collect(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        }
    }

    #[test]
    fn allow_rule_matches() {
        let alpha = account("alice", "wonderland");
        let beta = account("bob", "wonderland");
        let policy = sample_policy(
            LaneId::SINGLE,
            std::slice::from_ref(&alpha),
            std::slice::from_ref(&beta),
        );
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");

        let ctx = LaneComplianceContext {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            authority: &alpha,
            uaid: None,
            capability_tags: &[],
            lane_privacy_registry: None,
            verified_privacy_commitments: &EMPTY_PRIVACY_COMMITMENTS,
        };
        let evaluation = engine.evaluate(&ctx);
        matches!(evaluation, LaneComplianceEvaluation::Allowed(_))
            .then_some(())
            .expect("allowed");

        let ctx_beta = LaneComplianceContext {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            authority: &beta,
            uaid: None,
            capability_tags: &[],
            lane_privacy_registry: None,
            verified_privacy_commitments: &EMPTY_PRIVACY_COMMITMENTS,
        };
        let evaluation = engine.evaluate(&ctx_beta);
        matches!(evaluation, LaneComplianceEvaluation::Denied(_))
            .then_some(())
            .expect("denied");
    }

    #[test]
    fn duplicate_lane_rejected() {
        let alpha = account("alice", "wonderland");
        let policy = sample_policy(LaneId::SINGLE, std::slice::from_ref(&alpha), &[]);
        let duplicate = sample_policy(LaneId::SINGLE, &[alpha], &[]);
        let err = LaneComplianceEngine::from_policies(vec![policy, duplicate], false)
            .expect_err("duplicate lane must fail");
        assert!(matches!(
            err,
            LaneComplianceLoadError::DuplicateLane { lane_id } if lane_id == LaneId::SINGLE
        ));
    }

    #[test]
    fn selector_requires_privacy_commitment() {
        let alpha = account("alice", "wonderland");
        let policy = LaneCompliancePolicy {
            allow: vec![LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(alpha.clone()),
                    privacy_commitments_any_of: vec![LaneCommitmentId::new(7)],
                    ..ParticipantSelector::default()
                },
                reason_code: Some("allow".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            }],
            deny: Vec::new(),
            ..sample_policy(LaneId::SINGLE, &[], &[])
        };
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");

        let statuses = vec![LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "confidential".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::CommitmentOnly,
            governance: None,
            manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
            governance_rules: None,
            privacy_commitments: vec![LanePrivacyCommitment::merkle(
                LaneCommitmentId::new(7),
                MerkleCommitment::from_root_bytes([0xAA; 32], 8),
            )],
        }];
        let registry = LanePrivacyRegistry::from_statuses(&statuses);
        let ctx = LaneComplianceContext {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            authority: &alpha,
            uaid: None,
            capability_tags: &[],
            lane_privacy_registry: Some(Arc::new(registry)),
            verified_privacy_commitments: &EMPTY_PRIVACY_COMMITMENTS,
        };
        let mut verified = BTreeSet::new();
        verified.insert(LaneCommitmentId::new(7));
        let ctx_with_proof = LaneComplianceContext {
            verified_privacy_commitments: &verified,
            ..ctx
        };
        assert!(matches!(
            engine.evaluate(&ctx_with_proof),
            LaneComplianceEvaluation::Allowed(_)
        ));

        let empty_verified = BTreeSet::new();
        let ctx_missing_proof = LaneComplianceContext {
            verified_privacy_commitments: &empty_verified,
            ..ctx_with_proof
        };
        assert!(matches!(
            engine.evaluate(&ctx_missing_proof),
            LaneComplianceEvaluation::Denied(_)
        ));
    }

    #[test]
    fn selector_matches_capability_tag() {
        let alpha = account("alice", "wonderland");
        let policy = LaneCompliancePolicy {
            allow: vec![LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(alpha.clone()),
                    capability_tag: Some("fx-cleared".to_string()),
                    ..ParticipantSelector::default()
                },
                reason_code: Some("allow".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            }],
            deny: Vec::new(),
            ..sample_policy(LaneId::SINGLE, &[], &[])
        };
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");

        let ctx_missing_tag = LaneComplianceContext {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            authority: &alpha,
            uaid: None,
            capability_tags: &[],
            lane_privacy_registry: None,
            verified_privacy_commitments: &EMPTY_PRIVACY_COMMITMENTS,
        };
        assert!(matches!(
            engine.evaluate(&ctx_missing_tag),
            LaneComplianceEvaluation::Denied(_)
        ));

        let tags = vec!["fx-cleared".to_string()];
        let ctx_with_tag = LaneComplianceContext {
            capability_tags: &tags,
            ..ctx_missing_tag
        };
        assert!(matches!(
            engine.evaluate(&ctx_with_tag),
            LaneComplianceEvaluation::Allowed(_)
        ));
    }
}
