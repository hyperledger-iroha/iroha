//! Governed SoraFS gateway-compliance catalog admission and durable promotion.
//!
//! The controller deliberately owns no credentials and performs no ambient DNS
//! or HTTP access. Feed transport and catalog signatures cross explicit runtime
//! boundaries so production embeddings can keep authentication material in
//! their KMS/HSM and pin the exact addresses used for each connection.
//! Enabled feed transports attest a stable V1 handle, revision, and canonical
//! hostname/SPKI policy digest before checkpoint state is opened and again
//! before and after every feed operation.
use super::provider::{GatewayProviderBindingErrorV1, GatewayProviderBindingV1};
use blake3::Hasher;
use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use flate2::read::GzDecoder;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{
    cmp::Ordering,
    collections::BTreeSet,
    fmt::Debug,
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    num::NonZeroU16,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use url::{Host, Url};
/// V1 schema version for compliance catalog payloads.
pub const GATEWAY_COMPLIANCE_CATALOG_VERSION_V1: u8 = 1;
/// V1 schema version for catalog signatures.
pub const GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1: u8 = 1;
/// V1 schema version for gateway acknowledgements.
pub const GATEWAY_COMPLIANCE_ACK_VERSION_V1: u8 = 1;
/// V1 schema version for rollback authorizations.
pub const GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1: u8 = 1;
/// V1 schema version for canonical feed documents.
pub const GATEWAY_COMPLIANCE_FEED_VERSION_V1: u8 = 1;
/// V1 schema version for durable controller checkpoints.
pub const GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1: u8 = 1;
/// Maximum catalog entries across all entry families.
pub const MAX_GATEWAY_COMPLIANCE_ENTRIES_V1: usize = 65_536;
/// Maximum trusted governance or gateway signers.
pub const MAX_GATEWAY_COMPLIANCE_SIGNERS_V1: usize = 128;
/// Maximum retained gateway acknowledgements for one candidate.
pub const MAX_GATEWAY_COMPLIANCE_ACKS_V1: usize = 128;
/// Maximum catalog/feed bytes before decoding.
pub const MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum durable checkpoint bytes.
pub const MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum bounded controller history records.
pub const MAX_GATEWAY_COMPLIANCE_HISTORY_V1: usize = 4_096;
/// Maximum durable mutation-idempotency records.
pub const MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1: usize = 4_096;
const CATALOG_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-catalog-v1";
const ACK_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-ack-v1";
const ROLLBACK_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-rollback-v1";
const TRUST_POLICY_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-trust-policy-v1";
const CATALOG_DIGEST_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-catalog-digest-v1";
const FEED_DIGEST_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-feed-v1";
const FEED_TRANSPORT_POLICY_DOMAIN_V1: &[u8] =
    b"sorafs-gateway-compliance-feed-transport-policy-v1";
const CHECKPOINT_STORE_GENERATION_DOMAIN_V1: &[u8] =
    b"sorafs-gateway-compliance-checkpoint-store-generation-v1";
/// Stable runtime handle required from the V1 authenticated feed transport.
pub const GATEWAY_COMPLIANCE_FEED_TRANSPORT_HANDLE_V1: &str =
    "sorafs.gateway.compliance.feed-https.v1";
/// Exact V1 runtime adapter revision.
pub const GATEWAY_COMPLIANCE_FEED_TRANSPORT_REVISION_V1: u64 = 1;
/// One strong Ed25519 identity authorized by policy.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceTrustedSignerV1 {
    /// Stable lowercase identity.
    pub signer_id: String,
    /// Canonical Ed25519 public key.
    pub public_key: [u8; 32],
}
/// Config-derived threshold trust policy.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceTrustPolicyV1 {
    /// Non-zero governance policy identity.
    pub policy_id: [u8; 32],
    /// Minimum distinct governance approvals.
    pub catalog_threshold: u16,
    /// Governance catalog signers in strictly increasing identifier order,
    /// disjoint from gateway acknowledgement identities and keys.
    pub catalog_signers: Vec<GatewayComplianceTrustedSignerV1>,
    /// Revoked governance signer identifiers in strictly increasing order.
    pub revoked_catalog_signer_ids: Vec<String>,
    /// Minimum distinct positive gateway acknowledgements before promotion.
    pub gateway_ack_threshold: u16,
    /// Gateway acknowledgement signers in strictly increasing identifier
    /// order, disjoint from catalog approval identities and keys.
    pub gateway_signers: Vec<GatewayComplianceTrustedSignerV1>,
    /// Revoked gateway signer identifiers in strictly increasing order.
    pub revoked_gateway_signer_ids: Vec<String>,
}
impl GatewayComplianceTrustPolicyV1 {
    /// Validate the complete threshold and revocation policy.
    pub fn validate(&self) -> Result<(), GatewayComplianceError> {
        if self.policy_id.iter().all(|byte| *byte == 0) {
            return Err(GatewayComplianceError::InvalidPolicy(
                "policy_id must not be all zeroes".into(),
            ));
        }
        validate_signer_inventory(
            &self.catalog_signers,
            &self.revoked_catalog_signer_ids,
            self.catalog_threshold,
            "catalog",
        )?;
        validate_signer_inventory(
            &self.gateway_signers,
            &self.revoked_gateway_signer_ids,
            self.gateway_ack_threshold,
            "gateway acknowledgement",
        )?;
        validate_disjoint_signer_roles(&self.catalog_signers, &self.gateway_signers)
    }
    /// Return the domain-separated canonical policy digest.
    pub fn canonical_digest(&self) -> Result<[u8; 32], GatewayComplianceError> {
        self.validate()?;
        hash_canonical(
            TRUST_POLICY_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
    fn catalog_signer(&self, signer_id: &str) -> Option<&GatewayComplianceTrustedSignerV1> {
        find_signer(&self.catalog_signers, signer_id)
    }
    fn gateway_signer(&self, signer_id: &str) -> Option<&GatewayComplianceTrustedSignerV1> {
        find_signer(&self.gateway_signers, signer_id)
    }
}
/// Canonical compliance subject family.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GatewayComplianceSubjectKindV1 {
    /// Admitted provider identifier.
    Provider,
    /// Manifest BLAKE3 digest.
    ManifestDigest,
    /// Canonical base32 CID.
    Cid,
    /// Canonical URL.
    Url,
}
/// Baseline deny rule from an admitted feed.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct GatewayComplianceBaselineRuleV1 {
    /// Stable rule identity.
    pub rule_id: String,
    /// `global`, `region:<id>`, or `gateway:<id>`.
    pub scope: String,
    /// Subject family.
    pub subject_kind: GatewayComplianceSubjectKindV1,
    /// Canonical subject representation.
    pub subject: String,
    /// Stable source feed identity.
    pub source_id: String,
    /// Payload-free reason code.
    pub reason_code: String,
    /// Optional scoped toggle controlling this baseline rule.
    pub toggle_id: Option<String>,
    /// Inclusive activation Unix second.
    pub effective_from_unix: u64,
    /// Exclusive expiry Unix second, when present.
    pub expires_at_unix: Option<u64>,
}
/// Accepted appeal that allows one otherwise denied subject.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct GatewayComplianceAppealOverrideV1 {
    /// Stable appeal identity.
    pub appeal_id: String,
    /// Scope of the override.
    pub scope: String,
    /// Subject family.
    pub subject_kind: GatewayComplianceSubjectKindV1,
    /// Canonical subject representation.
    pub subject: String,
    /// Digest of the finalized accepted-appeal decision.
    pub decision_digest: [u8; 32],
    /// Inclusive activation Unix second.
    pub effective_from_unix: u64,
    /// Exclusive expiry Unix second.
    pub expires_at_unix: u64,
}
/// Legal or safety hold that cannot be bypassed by an appeal or toggle.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct GatewayComplianceLegalSafetyHoldV1 {
    /// Stable hold identity.
    pub hold_id: String,
    /// Scope of the hold.
    pub scope: String,
    /// Subject family.
    pub subject_kind: GatewayComplianceSubjectKindV1,
    /// Canonical subject representation.
    pub subject: String,
    /// Payload-free authority reference.
    pub authority_reference: String,
    /// Inclusive activation Unix second.
    pub effective_from_unix: u64,
    /// Exclusive expiry Unix second, when present.
    pub expires_at_unix: Option<u64>,
}
/// Threshold-approved scoped policy toggle.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct GatewayComplianceToggleV1 {
    /// Stable toggle identity.
    pub toggle_id: String,
    /// Scope of the toggle.
    pub scope: String,
    /// Whether the controlled baseline rule family is enabled.
    pub enabled: bool,
    /// Payload-free governance approval reference.
    pub approval_reference: String,
    /// Inclusive activation Unix second.
    pub effective_from_unix: u64,
    /// Exclusive expiry Unix second.
    pub expires_at_unix: u64,
}
/// Digest anchor for one normalized source feed.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct GatewayComplianceSourceAnchorV1 {
    /// Configured feed identifier.
    pub feed_id: String,
    /// Domain-separated digest of the canonical normalized feed.
    pub feed_digest: [u8; 32],
    /// Source feed generation Unix second.
    pub generated_at_unix: u64,
}
/// Unsigned, deterministic catalog payload.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceCatalogPayloadV1 {
    /// Schema version.
    pub version: u8,
    /// Strictly increasing sequence.
    pub sequence: u64,
    /// Exact digest of the previous promoted chain head.
    pub predecessor_digest: Option<[u8; 32]>,
    /// Digest of the config-derived trust policy.
    pub policy_digest: [u8; 32],
    /// Catalog creation Unix second.
    pub generated_at_unix: u64,
    /// Exclusive catalog expiry Unix second.
    pub valid_until_unix: u64,
    /// Source feed digest inventory in strict feed-id order.
    pub source_anchors: Vec<GatewayComplianceSourceAnchorV1>,
    /// Baseline deny rules in canonical order.
    pub baseline_rules: Vec<GatewayComplianceBaselineRuleV1>,
    /// Accepted appeal overrides in canonical order.
    pub appeal_overrides: Vec<GatewayComplianceAppealOverrideV1>,
    /// Legal/safety holds in canonical order.
    pub legal_safety_holds: Vec<GatewayComplianceLegalSafetyHoldV1>,
    /// Scoped toggles in canonical order.
    pub toggles: Vec<GatewayComplianceToggleV1>,
}
impl GatewayComplianceCatalogPayloadV1 {
    /// Normalize all bounded fields and deterministically sort each inventory.
    pub fn normalize(mut self) -> Result<Self, GatewayComplianceError> {
        for anchor in &mut self.source_anchors {
            anchor.feed_id = normalize_token(&anchor.feed_id, "feed_id")?;
            if anchor.feed_digest.iter().all(|byte| *byte == 0) || anchor.generated_at_unix == 0 {
                return Err(GatewayComplianceError::InvalidCatalog(
                    "source feed digest and generation time must be non-zero".into(),
                ));
            }
        }
        for rule in &mut self.baseline_rules {
            normalize_baseline_rule(rule)?;
        }
        for appeal in &mut self.appeal_overrides {
            normalize_appeal(appeal)?;
        }
        for hold in &mut self.legal_safety_holds {
            normalize_hold(hold)?;
        }
        for toggle in &mut self.toggles {
            normalize_toggle(toggle)?;
        }
        self.source_anchors.sort();
        self.baseline_rules.sort();
        self.appeal_overrides.sort();
        self.legal_safety_holds.sort();
        self.toggles.sort();
        reject_duplicate_keys(
            &self.source_anchors,
            |entry| entry.feed_id.as_str(),
            "source_anchors",
        )?;
        reject_duplicate_keys(
            &self.baseline_rules,
            |entry| entry.rule_id.as_str(),
            "baseline_rules",
        )?;
        reject_duplicate_keys(
            &self.appeal_overrides,
            |entry| entry.appeal_id.as_str(),
            "appeal_overrides",
        )?;
        reject_duplicate_keys(
            &self.legal_safety_holds,
            |entry| entry.hold_id.as_str(),
            "legal_safety_holds",
        )?;
        reject_duplicate_toggle_scope(&self.toggles)?;
        Ok(self)
    }
    /// Validate strict canonical shape and resource bounds.
    pub fn validate(&self) -> Result<(), GatewayComplianceError> {
        if self.version != GATEWAY_COMPLIANCE_CATALOG_VERSION_V1 {
            return Err(GatewayComplianceError::InvalidCatalog(format!(
                "unsupported catalog version {}",
                self.version
            )));
        }
        if self.sequence == 0 {
            return Err(GatewayComplianceError::InvalidCatalog(
                "catalog sequence must be non-zero".into(),
            ));
        }
        if self.policy_digest.iter().all(|byte| *byte == 0) {
            return Err(GatewayComplianceError::InvalidCatalog(
                "policy digest must not be all zeroes".into(),
            ));
        }
        if self.generated_at_unix == 0 || self.valid_until_unix <= self.generated_at_unix {
            return Err(GatewayComplianceError::InvalidCatalog(
                "catalog validity interval is invalid".into(),
            ));
        }
        let entry_count = self
            .baseline_rules
            .len()
            .saturating_add(self.appeal_overrides.len())
            .saturating_add(self.legal_safety_holds.len())
            .saturating_add(self.toggles.len());
        if entry_count > MAX_GATEWAY_COMPLIANCE_ENTRIES_V1 {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "catalog entries",
                found: entry_count,
                maximum: MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
            });
        }
        let normalized = self.clone().normalize()?;
        if normalized != *self {
            return Err(GatewayComplianceError::NonCanonical(
                "catalog inventories or fields".into(),
            ));
        }
        encode_bounded(self, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1)?;
        Ok(())
    }
    /// Return the exact domain-separated catalog identifier.
    pub fn catalog_digest(&self) -> Result<[u8; 32], GatewayComplianceError> {
        self.validate()?;
        hash_canonical(
            CATALOG_DIGEST_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
    /// Return the digest governance signers approve.
    pub fn signing_digest(&self) -> Result<[u8; 32], GatewayComplianceError> {
        self.validate()?;
        hash_canonical(
            CATALOG_SIGNING_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
}
/// One governance approval on a catalog.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceCatalogApprovalV1 {
    /// Schema version.
    pub version: u8,
    /// Trusted governance signer identity.
    pub signer_id: String,
    /// Strong Ed25519 signature of the catalog signing digest.
    pub signature: [u8; 64],
}
/// Threshold-signed predecessor-bound catalog.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceCatalogV1 {
    /// Unsigned canonical payload.
    pub payload: GatewayComplianceCatalogPayloadV1,
    /// Distinct governance approvals in strict signer-id order.
    pub approvals: Vec<GatewayComplianceCatalogApprovalV1>,
}
impl GatewayComplianceCatalogV1 {
    /// Verify canonical shape, trust-policy binding, freshness, and quorum.
    pub fn verify(
        &self,
        policy: &GatewayComplianceTrustPolicyV1,
        observed_at_unix: u64,
        max_clock_skew_secs: u64,
    ) -> Result<[u8; 32], GatewayComplianceError> {
        policy.validate()?;
        self.payload.validate()?;
        if self.payload.policy_digest != policy.canonical_digest()? {
            return Err(GatewayComplianceError::PolicyDigestMismatch);
        }
        validate_catalog_freshness(&self.payload, observed_at_unix, max_clock_skew_secs)?;
        if self.approvals.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "catalog approvals",
                found: self.approvals.len(),
                maximum: MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
            });
        }
        if self.approvals.len() < usize::from(policy.catalog_threshold) {
            return Err(GatewayComplianceError::QuorumNotMet {
                found: self.approvals.len(),
                required: policy.catalog_threshold,
            });
        }
        let digest = self.payload.signing_digest()?;
        let mut previous: Option<&str> = None;
        for approval in &self.approvals {
            if approval.version != GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1 {
                return Err(GatewayComplianceError::InvalidSignature {
                    signer_id: approval.signer_id.clone(),
                    reason: "unsupported approval version".into(),
                });
            }
            validate_token(&approval.signer_id, "signer_id")?;
            if let Some(previous) = previous {
                match previous.cmp(approval.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(GatewayComplianceError::DuplicateSigner(
                            approval.signer_id.clone(),
                        ));
                    }
                    Ordering::Greater => {
                        return Err(GatewayComplianceError::NonCanonical(
                            "catalog approval order".into(),
                        ));
                    }
                    Ordering::Less => {}
                }
            }
            let trusted = policy.catalog_signer(&approval.signer_id).ok_or_else(|| {
                GatewayComplianceError::UntrustedSigner(approval.signer_id.clone())
            })?;
            if contains_sorted(
                &policy.revoked_catalog_signer_ids,
                approval.signer_id.as_str(),
            ) {
                return Err(GatewayComplianceError::RevokedSigner(
                    approval.signer_id.clone(),
                ));
            }
            verify_ed25519(
                &trusted.public_key,
                &approval.signature,
                &digest,
                &approval.signer_id,
            )?;
            previous = Some(&approval.signer_id);
        }
        encode_bounded(self, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1)?;
        self.payload.catalog_digest()
    }
}
/// Payload signed by one regional gateway after staging a catalog.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceAcknowledgementPayloadV1 {
    /// Schema version.
    pub version: u8,
    /// Trusted gateway identity.
    pub gateway_id: String,
    /// Exact staged catalog digest.
    pub catalog_digest: [u8; 32],
    /// Gateway observation Unix second.
    pub observed_at_unix: u64,
    /// Whether local validation and reload succeeded.
    pub accepted: bool,
    /// Payload-free rejection code when `accepted` is false.
    pub rejection_code: Option<String>,
}
/// Signed regional gateway acknowledgement.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceAcknowledgementV1 {
    /// Signed acknowledgement payload.
    pub payload: GatewayComplianceAcknowledgementPayloadV1,
    /// Gateway Ed25519 signature.
    pub signature: [u8; 64],
}
impl GatewayComplianceAcknowledgementV1 {
    fn verify(
        &self,
        policy: &GatewayComplianceTrustPolicyV1,
        expected_catalog_digest: [u8; 32],
        observed_at_unix: u64,
        max_clock_skew_secs: u64,
    ) -> Result<(), GatewayComplianceError> {
        if self.payload.version != GATEWAY_COMPLIANCE_ACK_VERSION_V1 {
            return Err(GatewayComplianceError::InvalidAcknowledgement(
                "unsupported acknowledgement version".into(),
            ));
        }
        validate_token(&self.payload.gateway_id, "gateway_id")?;
        if self.payload.catalog_digest != expected_catalog_digest {
            return Err(GatewayComplianceError::InvalidAcknowledgement(
                "catalog digest mismatch".into(),
            ));
        }
        if self.payload.observed_at_unix == 0
            || self.payload.observed_at_unix > observed_at_unix.saturating_add(max_clock_skew_secs)
            || self
                .payload
                .observed_at_unix
                .saturating_add(max_clock_skew_secs)
                < observed_at_unix
        {
            return Err(GatewayComplianceError::InvalidAcknowledgement(
                "acknowledgement timestamp is invalid".into(),
            ));
        }
        match (self.payload.accepted, self.payload.rejection_code.as_ref()) {
            (true, None) => {}
            (false, Some(code)) => {
                validate_token(code, "rejection_code")?;
            }
            _ => {
                return Err(GatewayComplianceError::InvalidAcknowledgement(
                    "accepted acknowledgements omit rejection_code; rejected acknowledgements require it"
                        .into(),
                ));
            }
        }
        let trusted = policy
            .gateway_signer(&self.payload.gateway_id)
            .ok_or_else(|| {
                GatewayComplianceError::UntrustedSigner(self.payload.gateway_id.clone())
            })?;
        if contains_sorted(
            &policy.revoked_gateway_signer_ids,
            self.payload.gateway_id.as_str(),
        ) {
            return Err(GatewayComplianceError::RevokedSigner(
                self.payload.gateway_id.clone(),
            ));
        }
        let digest = hash_canonical(
            ACK_SIGNING_DOMAIN_V1,
            &self.payload,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )?;
        verify_ed25519(
            &trusted.public_key,
            &self.signature,
            &digest,
            &self.payload.gateway_id,
        )
    }
}
/// Unsigned rollback command.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceRollbackPayloadV1 {
    /// Schema version.
    pub version: u8,
    /// Replay-resistant governance operation identity.
    pub operation_id: [u8; 32],
    /// Current serving catalog digest.
    pub from_catalog_digest: [u8; 32],
    /// Previous last-known-good catalog digest.
    pub to_catalog_digest: [u8; 32],
    /// Payload-free reason code.
    pub reason_code: String,
    /// Governance authorization Unix second.
    pub authorized_at_unix: u64,
}
/// Threshold-approved rollback authorization.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceRollbackV1 {
    /// Signed rollback payload.
    pub payload: GatewayComplianceRollbackPayloadV1,
    /// Governance approvals in strict signer-id order.
    pub approvals: Vec<GatewayComplianceCatalogApprovalV1>,
}
/// Canonical normalized external feed document.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceFeedDocumentV1 {
    /// Schema version.
    pub version: u8,
    /// Configured feed identity.
    pub feed_id: String,
    /// Source generation Unix second.
    pub generated_at_unix: u64,
    /// Baseline rules.
    pub baseline_rules: Vec<GatewayComplianceBaselineRuleV1>,
    /// Accepted appeal overrides.
    pub appeal_overrides: Vec<GatewayComplianceAppealOverrideV1>,
    /// Legal/safety holds.
    pub legal_safety_holds: Vec<GatewayComplianceLegalSafetyHoldV1>,
    /// Scoped toggles.
    pub toggles: Vec<GatewayComplianceToggleV1>,
}
impl GatewayComplianceFeedDocumentV1 {
    /// Normalize and validate one external feed.
    pub fn normalize(mut self) -> Result<Self, GatewayComplianceError> {
        if self.version != GATEWAY_COMPLIANCE_FEED_VERSION_V1 {
            return Err(GatewayComplianceError::InvalidFeed(
                "unsupported feed version".into(),
            ));
        }
        self.feed_id = normalize_token(&self.feed_id, "feed_id")?;
        if self.generated_at_unix == 0 {
            return Err(GatewayComplianceError::InvalidFeed(
                "feed generated_at_unix must be non-zero".into(),
            ));
        }
        let payload = GatewayComplianceCatalogPayloadV1 {
            version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
            sequence: 1,
            predecessor_digest: None,
            policy_digest: [1; 32],
            generated_at_unix: self.generated_at_unix,
            valid_until_unix: self.generated_at_unix.saturating_add(1),
            source_anchors: Vec::new(),
            baseline_rules: self.baseline_rules,
            appeal_overrides: self.appeal_overrides,
            legal_safety_holds: self.legal_safety_holds,
            toggles: self.toggles,
        }
        .normalize()?;
        self.baseline_rules = payload.baseline_rules;
        self.appeal_overrides = payload.appeal_overrides;
        self.legal_safety_holds = payload.legal_safety_holds;
        self.toggles = payload.toggles;
        let entry_count = self
            .baseline_rules
            .len()
            .saturating_add(self.appeal_overrides.len())
            .saturating_add(self.legal_safety_holds.len())
            .saturating_add(self.toggles.len());
        if entry_count > MAX_GATEWAY_COMPLIANCE_ENTRIES_V1 {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "feed entries",
                found: entry_count,
                maximum: MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
            });
        }
        encode_bounded(&self, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1)?;
        Ok(self)
    }
    /// Return a digest of the canonical normalized feed.
    pub fn canonical_digest(&self) -> Result<[u8; 32], GatewayComplianceError> {
        let normalized = self.clone().normalize()?;
        if normalized != *self {
            return Err(GatewayComplianceError::NonCanonical(
                "external feed document".into(),
            ));
        }
        hash_canonical(
            FEED_DIGEST_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
}
/// One allowlisted HTTPS host and pinned TLS identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayComplianceFeedHostPolicy {
    /// Canonical lowercase DNS hostname.
    pub hostname: String,
    /// Non-empty set of accepted SHA-256 SPKI digests.
    pub accepted_spki_sha256: BTreeSet<[u8; 32]>,
}
/// Config-derived feed endpoint policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayComplianceFeedPolicy {
    /// Stable feed identity.
    pub feed_id: String,
    /// Exact canonical initial HTTPS URL.
    pub url: String,
    /// Whether catalog construction requires this feed.
    pub required: bool,
    /// Exact redirect host allowlist, including the initial host.
    pub hosts: Vec<GatewayComplianceFeedHostPolicy>,
}
/// Bounded feed-fetch policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayComplianceFetchLimits {
    /// Maximum encoded response bytes.
    pub max_encoded_bytes: usize,
    /// Maximum decoded response bytes.
    pub max_decoded_bytes: usize,
    /// Maximum redirect count.
    pub max_redirects: u8,
    /// Maximum distinct DNS answers.
    pub max_dns_addresses: usize,
    /// Connect timeout given to the injected transport.
    pub connect_timeout: Duration,
    /// Total operation deadline given to the injected transport.
    pub total_timeout: Duration,
}
impl Default for GatewayComplianceFetchLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 4 * 1024 * 1024,
            max_decoded_bytes: 16 * 1024 * 1024,
            max_redirects: 3,
            max_dns_addresses: 8,
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(20),
        }
    }
}
/// Content encodings admitted from compliance feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayComplianceContentEncoding {
    /// Uncompressed bytes.
    Identity,
    /// Gzip-compressed bytes.
    Gzip,
    /// Zstandard-compressed bytes.
    Zstd,
}
/// Address-pinned request handed to a runtime transport.
#[derive(Debug, Clone)]
pub struct GatewayComplianceFetchRequest {
    /// Exact validated HTTPS URL.
    pub url: Url,
    /// Validated DNS answers the transport must exclusively use.
    pub pinned_addresses: Vec<IpAddr>,
    /// Per-connection timeout.
    pub connect_timeout: Duration,
    /// Overall timeout.
    pub total_timeout: Duration,
    /// Maximum response bytes the transport may buffer.
    pub max_encoded_bytes: usize,
}
/// Address- and trust-bound response returned by a runtime transport.
#[derive(Clone)]
pub struct GatewayComplianceFetchResponse {
    /// HTTP status.
    pub status: u16,
    /// Redirect location for 3xx responses.
    pub redirect_location: Option<String>,
    /// Exact connected peer address.
    pub connected_address: IpAddr,
    /// SHA-256 digest of the authenticated peer SPKI.
    pub peer_spki_sha256: [u8; 32],
    /// Declared content encoding.
    pub content_encoding: GatewayComplianceContentEncoding,
    /// Encoded response bytes.
    pub body: Vec<u8>,
    /// Transport-observed elapsed time.
    pub elapsed: Duration,
}
impl Debug for GatewayComplianceFetchResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GatewayComplianceFetchResponse")
            .field("status", &self.status)
            .field(
                "redirect_location_bytes",
                &self.redirect_location.as_ref().map(String::len),
            )
            .field("connected_address", &self.connected_address)
            .field("peer_spki_sha256", &self.peer_spki_sha256)
            .field("content_encoding", &self.content_encoding)
            .field("body_bytes", &self.body.len())
            .field("elapsed", &self.elapsed)
            .finish()
    }
}
/// Runtime-owned authenticated DNS and HTTPS transport.
pub trait GatewayComplianceFeedTransport: Send + Sync {
    /// Return the provider identity currently serving this runtime boundary.
    ///
    /// Implementations must obtain this identity from the same provider
    /// instance that performs [`Self::resolve`] and [`Self::fetch`]. Provider
    /// diagnostics must remain behind this boundary; callers intentionally see
    /// only the payload-free [`GatewayComplianceFeedTransportProbeError`].
    fn qualification(
        &self,
    ) -> Result<GatewayComplianceFeedTransportIdentityV1, GatewayComplianceFeedTransportProbeError>;
    /// Resolve a DNS hostname using the runtime's pinned resolver.
    fn resolve(
        &self,
        hostname: &str,
        timeout: Duration,
    ) -> Result<Vec<IpAddr>, GatewayComplianceError>;
    /// Fetch through only the supplied pinned addresses, without automatic
    /// redirects or transparent decompression.
    fn fetch(
        &self,
        request: &GatewayComplianceFetchRequest,
    ) -> Result<GatewayComplianceFetchResponse, GatewayComplianceError>;
}
/// Payload-free identity reported by one authenticated feed transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayComplianceFeedTransportIdentityV1 {
    /// Stable adapter contract handle. This is never a credential or URL.
    pub provider_handle: String,
    /// Monotonic deployed adapter revision.
    pub revision: u64,
    /// Domain-separated digest of the exact hostname/SPKI trust inventory.
    pub policy_digest: [u8; 32],
    /// Explicit marker set by test, mock, development, or placeholder adapters.
    pub test_marked: bool,
}
/// Redacted failure returned when a runtime provider cannot attest its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("gateway compliance feed transport qualification failed")]
pub struct GatewayComplianceFeedTransportProbeError;
#[derive(Debug, NoritoSerialize)]
struct GatewayComplianceFeedTransportPolicyDigestV1 {
    version: u8,
    hosts: Vec<GatewayComplianceFeedTransportHostDigestV1>,
}
#[derive(Debug, NoritoSerialize)]
struct GatewayComplianceFeedTransportHostDigestV1 {
    hostname: String,
    accepted_spki_sha256: Vec<[u8; 32]>,
}
/// Controller configuration assembled exclusively from resolved `iroha_config`.
#[derive(Debug, Clone)]
pub struct GatewayComplianceControllerConfig {
    /// Threshold signature and acknowledgement policy.
    pub trust_policy: GatewayComplianceTrustPolicyV1,
    /// Exact regional serving scope for this independently administered gateway.
    pub region_scope: String,
    /// Exact gateway serving scope bound to one active gateway signer identity.
    pub gateway_scope: String,
    /// Configured external feeds in strict feed-id order.
    pub feeds: Vec<GatewayComplianceFeedPolicy>,
    /// Exact independently configured runtime feed-transport provider.
    ///
    /// Production launchers must provide this binding. `None` is reserved for
    /// the test-only controller core, which cannot perform external feed I/O.
    pub feed_transport_provider: Option<GatewayProviderBindingV1>,
    /// Fetch and decompression limits.
    pub fetch_limits: GatewayComplianceFetchLimits,
    /// Maximum accepted future timestamp skew.
    pub max_clock_skew_secs: u64,
    /// Maximum age of any source feed at catalog generation.
    pub max_feed_age_secs: u64,
    /// Maximum catalog validity interval.
    pub max_catalog_validity_secs: u64,
    /// Maximum durable history length.
    pub max_history_entries: usize,
}
impl GatewayComplianceControllerConfig {
    /// Validate bounds, ordering, HTTPS allowlists, and pins.
    pub fn validate(&self) -> Result<(), GatewayComplianceError> {
        self.trust_policy.validate()?;
        if normalize_scope(&self.region_scope).ok().as_deref() != Some(self.region_scope.as_str())
            || !self.region_scope.starts_with("region:")
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "region_scope must be one canonical region:<id> scope".into(),
            ));
        }
        if normalize_scope(&self.gateway_scope).ok().as_deref() != Some(self.gateway_scope.as_str())
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "gateway_scope must be one canonical gateway:<id> scope".into(),
            ));
        }
        let gateway_id = self.gateway_scope.strip_prefix("gateway:").ok_or_else(|| {
            GatewayComplianceError::InvalidPolicy(
                "gateway_scope must be one canonical gateway:<id> scope".into(),
            )
        })?;
        if self
            .trust_policy
            .gateway_signers
            .binary_search_by(|signer| signer.signer_id.as_str().cmp(gateway_id))
            .is_err()
            || self
                .trust_policy
                .revoked_gateway_signer_ids
                .binary_search_by(|signer_id| signer_id.as_str().cmp(gateway_id))
                .is_ok()
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "gateway_scope must name one active configured gateway signer".into(),
            ));
        }
        if self.feeds.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "configured feeds",
                found: self.feeds.len(),
                maximum: MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
            });
        }
        if self.fetch_limits.max_encoded_bytes == 0
            || self.fetch_limits.max_decoded_bytes == 0
            || self.fetch_limits.max_encoded_bytes > MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1
            || self.fetch_limits.max_decoded_bytes > MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1
            || self.fetch_limits.max_redirects > 8
            || self.fetch_limits.max_dns_addresses == 0
            || self.fetch_limits.max_dns_addresses > 32
            || self.fetch_limits.connect_timeout.is_zero()
            || self.fetch_limits.connect_timeout > Duration::from_secs(30)
            || self.fetch_limits.total_timeout < self.fetch_limits.connect_timeout
            || self.fetch_limits.total_timeout > Duration::from_secs(120)
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "invalid compliance fetch limits".into(),
            ));
        }
        if self.max_history_entries == 0
            || self.max_history_entries > MAX_GATEWAY_COMPLIANCE_HISTORY_V1
            || self.max_clock_skew_secs > 3_600
            || self.max_feed_age_secs == 0
            || self.max_feed_age_secs > 30 * 24 * 60 * 60
            || self.max_catalog_validity_secs == 0
            || self.max_catalog_validity_secs > 30 * 24 * 60 * 60
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "invalid compliance controller bounds".into(),
            ));
        }
        let mut previous: Option<&str> = None;
        for feed in &self.feeds {
            validate_token(&feed.feed_id, "feed_id")?;
            if previous.is_some_and(|value| value >= feed.feed_id.as_str()) {
                return Err(GatewayComplianceError::NonCanonical(
                    "configured feed order".into(),
                ));
            }
            if feed.hosts.is_empty() {
                return Err(GatewayComplianceError::InvalidPolicy(format!(
                    "feed `{}` has no HTTPS host allowlist",
                    feed.feed_id
                )));
            }
            let mut previous_host: Option<&str> = None;
            for host in &feed.hosts {
                validate_dns_hostname(&host.hostname)?;
                if previous_host.is_some_and(|value| value >= host.hostname.as_str()) {
                    return Err(GatewayComplianceError::NonCanonical(format!(
                        "feed `{}` host allowlist",
                        feed.feed_id
                    )));
                }
                if host.accepted_spki_sha256.is_empty()
                    || host
                        .accepted_spki_sha256
                        .iter()
                        .any(|digest| digest.iter().all(|byte| *byte == 0))
                {
                    return Err(GatewayComplianceError::InvalidPolicy(format!(
                        "feed `{}` host `{}` has no valid trust pin",
                        feed.feed_id, host.hostname
                    )));
                }
                previous_host = Some(&host.hostname);
            }
            validate_feed_url(feed, &feed.url)?;
            previous = Some(&feed.feed_id);
        }
        if let Some(binding) = self.feed_transport_provider.as_ref()
            && binding.policy_digest() != self.feed_transport_policy_digest()?
        {
            return Err(GatewayComplianceError::InvalidPolicy(
                "feed transport provider policy digest does not bind the exact configured hostname/SPKI inventory"
                    .into(),
            ));
        }
        Ok(())
    }
    fn feed(&self, feed_id: &str) -> Option<&GatewayComplianceFeedPolicy> {
        self.feeds
            .binary_search_by(|feed| feed.feed_id.as_str().cmp(feed_id))
            .ok()
            .map(|index| &self.feeds[index])
    }
    fn expected_feed_transport_identity(
        &self,
    ) -> Result<GatewayComplianceFeedTransportIdentityV1, GatewayComplianceError> {
        let binding = self.feed_transport_provider.as_ref().ok_or_else(|| {
            GatewayComplianceError::InvalidPolicy(
                "feed transport provider binding is required for external feed access".into(),
            )
        })?;
        if binding.policy_digest() != self.feed_transport_policy_digest()? {
            return Err(GatewayComplianceError::InvalidPolicy(
                "feed transport provider policy digest does not bind the exact configured hostname/SPKI inventory"
                    .into(),
            ));
        }
        Ok(GatewayComplianceFeedTransportIdentityV1 {
            provider_handle: binding.provider_handle().to_owned(),
            revision: binding.revision(),
            policy_digest: binding.policy_digest(),
            test_marked: false,
        })
    }
    fn feed_transport_policy_digest(&self) -> Result<[u8; 32], GatewayComplianceError> {
        let mut pins_by_hostname = std::collections::BTreeMap::new();
        for feed in &self.feeds {
            for host in &feed.hosts {
                if let Some(existing) = pins_by_hostname
                    .insert(host.hostname.clone(), host.accepted_spki_sha256.clone())
                    && existing != host.accepted_spki_sha256
                {
                    return Err(GatewayComplianceError::InvalidPolicy(
                        "one compliance hostname has conflicting trust inventories".into(),
                    ));
                }
            }
        }
        gateway_compliance_feed_transport_policy_digest(&pins_by_hostname)
    }
}
/// Compute the V1 domain-separated digest of a canonical hostname/SPKI policy.
///
/// Runtime adapter implementations use this helper to attest the exact
/// non-secret trust inventory they enforce. Hostnames and pin sets are
/// traversed in `BTreeMap`/`BTreeSet` order, so every platform produces the
/// same digest.
///
/// # Errors
///
/// Returns an error when the inventory is empty, noncanonical, or contains an
/// empty or all-zero pin set.
pub fn gateway_compliance_feed_transport_policy_digest(
    pins_by_hostname: &std::collections::BTreeMap<String, BTreeSet<[u8; 32]>>,
) -> Result<[u8; 32], GatewayComplianceError> {
    if pins_by_hostname.is_empty() {
        return Err(GatewayComplianceError::InvalidPolicy(
            "compliance feed transport trust inventory must not be empty".into(),
        ));
    }
    let mut hosts = Vec::with_capacity(pins_by_hostname.len());
    for (hostname, pins) in pins_by_hostname {
        validate_dns_hostname(hostname)?;
        if pins.is_empty() || pins.iter().any(|pin| pin.iter().all(|byte| *byte == 0)) {
            return Err(GatewayComplianceError::InvalidPolicy(
                "invalid compliance feed transport trust inventory".into(),
            ));
        }
        hosts.push(GatewayComplianceFeedTransportHostDigestV1 {
            hostname: hostname.clone(),
            accepted_spki_sha256: pins.iter().copied().collect(),
        });
    }
    hash_canonical(
        FEED_TRANSPORT_POLICY_DOMAIN_V1,
        &GatewayComplianceFeedTransportPolicyDigestV1 { version: 1, hosts },
        MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
    )
}
/// Effective request disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayComplianceDisposition {
    /// Serve unless another gateway policy rejects.
    Allow,
    /// Reject under the selected compliance rule.
    Deny,
}
/// Policy tier that produced an effective decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayComplianceDecisionSource {
    /// No catalog entry matched.
    NoMatch,
    /// Baseline deny rule matched.
    Baseline,
    /// Finalized accepted appeal overrode a baseline deny.
    AcceptedAppeal,
    /// Legal or safety hold took absolute precedence.
    LegalSafetyHold,
}
/// Payload-free deterministic evaluation output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayComplianceDecision {
    /// Effective disposition.
    pub disposition: GatewayComplianceDisposition,
    /// Winning precedence tier.
    pub source: GatewayComplianceDecisionSource,
    /// Winning rule/appeal/hold identity.
    pub reference_id: Option<String>,
    /// Serving catalog digest, when a catalog is active.
    pub catalog_digest: Option<[u8; 32]>,
    /// Monotonic serving-catalog sequence.
    pub catalog_sequence: u64,
    /// Exclusive serving-catalog expiry as a Unix second.
    pub catalog_valid_until_unix: u64,
}
/// Durable promotion or rollback record.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceHistoryRecordV1 {
    /// Replay-resistant operation identity.
    pub operation_id: [u8; 32],
    /// Serving catalog before the action, when present.
    pub previous_serving_digest: Option<[u8; 32]>,
    /// Serving catalog after the action.
    pub serving_digest: [u8; 32],
    /// Action Unix second.
    pub recorded_at_unix: u64,
    /// `promotion` or `rollback`.
    pub action: String,
    /// Payload-free reason code.
    pub reason_code: String,
}
/// Exact durable gateway-compliance mutation kind.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GatewayComplianceMutationKindV1 {
    /// Stage a threshold-signed candidate catalog.
    Stage,
    /// Record a signed regional-gateway acknowledgement.
    Acknowledge,
    /// Promote the staged catalog.
    Promote,
    /// Roll back to the last-known-good catalog.
    Rollback,
}
impl GatewayComplianceMutationKindV1 {
    fn history_action(self) -> Option<&'static str> {
        match self {
            Self::Stage | Self::Acknowledge => None,
            Self::Promote => Some("promotion"),
            Self::Rollback => Some("rollback"),
        }
    }
}
/// Cryptographic idempotency binding supplied by the authenticated API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayComplianceMutationBindingV1 {
    /// Digest-form operation key. Raw caller-provided keys are never persisted.
    pub key_digest: [u8; 32],
    /// Digest of the exact canonical method, target, and request bytes.
    pub request_digest: [u8; 32],
}
/// Stable response returned for an initial mutation and every exact replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayComplianceMutationResultV1 {
    /// Catalog affected by the committed mutation.
    pub catalog_digest: [u8; 32],
    /// Unix second at which the initial mutation committed.
    pub recorded_at_unix: u64,
}
/// Durable replay binding for one successful mutation.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct GatewayComplianceIdempotencyRecordV1 {
    /// Digest-form operation key.
    pub key_digest: [u8; 32],
    /// Digest of the exact canonical request.
    pub request_digest: [u8; 32],
    /// Mutation performed by the request.
    pub operation: GatewayComplianceMutationKindV1,
    /// Catalog affected by the mutation.
    pub catalog_digest: [u8; 32],
    /// Unix second at which the initial mutation committed.
    pub recorded_at_unix: u64,
}
/// Durable controller state.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct GatewayComplianceCheckpointV1 {
    /// Schema version.
    pub version: u8,
    /// Monotonic durable mutation revision. Exact idempotency replays do not advance it.
    pub revision: u64,
    /// Bound trust-policy digest.
    pub policy_digest: [u8; 32],
    /// Most recently promoted predecessor-chain head.
    pub chain_head: Option<GatewayComplianceCatalogV1>,
    /// Catalog currently served by this gateway.
    pub serving: Option<GatewayComplianceCatalogV1>,
    /// Last-known-good catalog available for rollback.
    pub previous_serving: Option<GatewayComplianceCatalogV1>,
    /// Staged candidate awaiting gateway acknowledgements.
    pub candidate: Option<GatewayComplianceCatalogV1>,
    /// Signed acknowledgements for the staged candidate.
    pub acknowledgements: Vec<GatewayComplianceAcknowledgementV1>,
    /// Bounded immutable promotion/rollback history.
    pub history: Vec<GatewayComplianceHistoryRecordV1>,
    /// Bounded immutable exact-request replay registry.
    pub idempotency_records: Vec<GatewayComplianceIdempotencyRecordV1>,
}
impl GatewayComplianceCheckpointV1 {
    fn empty(policy_digest: [u8; 32]) -> Self {
        Self {
            version: GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1,
            revision: 0,
            policy_digest,
            chain_head: None,
            serving: None,
            previous_serving: None,
            candidate: None,
            acknowledgements: Vec::new(),
            history: Vec::new(),
            idempotency_records: Vec::new(),
        }
    }
}
/// Opaque generation of the exact durable checkpoint bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayComplianceStoreGeneration {
    /// No checkpoint exists.
    Absent,
    /// Domain-separated digest of the exact checkpoint bytes.
    Present([u8; 32]),
}
/// One exact snapshot loaded under an active checkpoint-store lease.
#[derive(Debug)]
pub struct GatewayComplianceStoreSnapshot {
    /// Opaque exact-byte generation.
    pub generation: GatewayComplianceStoreGeneration,
    /// Exact checkpoint bytes, or `None` when the checkpoint is absent.
    pub bytes: Option<Vec<u8>>,
}
/// Process-lifetime exclusive lease over one durable checkpoint backend.
pub trait GatewayComplianceStoreLease: Debug + Send + Sync {
    /// Load the exact current checkpoint snapshot.
    fn load(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError>;
    /// Atomically replace the checkpoint only when its exact generation matches.
    fn compare_and_store(
        &self,
        expected: GatewayComplianceStoreGeneration,
        bytes: &[u8],
    ) -> Result<GatewayComplianceStoreGeneration, GatewayComplianceError>;
}
/// Durable checkpoint backend. A controller must hold its exclusive lease for
/// its complete lifetime.
pub trait GatewayComplianceStore: Debug + Send + Sync {
    /// Acquire the one active process-lifetime lease.
    fn try_acquire(&self) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError>;
}
#[cfg(test)]
#[derive(Debug, Default)]
struct TestGatewayComplianceStore {
    state: Arc<std::sync::Mutex<TestGatewayComplianceStoreState>>,
}
#[cfg(test)]
#[derive(Debug, Default)]
struct TestGatewayComplianceStoreState {
    bytes: Option<Vec<u8>>,
    leased: bool,
}
#[cfg(test)]
#[derive(Debug)]
struct TestGatewayComplianceStoreLease {
    state: Arc<std::sync::Mutex<TestGatewayComplianceStoreState>>,
}
#[cfg(test)]
impl Drop for TestGatewayComplianceStoreLease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.leased = false;
        }
    }
}
#[cfg(test)]
impl GatewayComplianceStore for TestGatewayComplianceStore {
    fn try_acquire(&self) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| GatewayComplianceError::StatePoisoned)?;
        if state.leased {
            return Err(GatewayComplianceError::LeaseHeld);
        }
        state.leased = true;
        drop(state);
        Ok(Box::new(TestGatewayComplianceStoreLease {
            state: Arc::clone(&self.state),
        }))
    }
}
#[cfg(test)]
impl GatewayComplianceStoreLease for TestGatewayComplianceStoreLease {
    fn load(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
        let state = self
            .state
            .lock()
            .map_err(|_| GatewayComplianceError::StatePoisoned)?;
        Ok(checkpoint_store_snapshot(state.bytes.clone()))
    }
    fn compare_and_store(
        &self,
        expected: GatewayComplianceStoreGeneration,
        bytes: &[u8],
    ) -> Result<GatewayComplianceStoreGeneration, GatewayComplianceError> {
        if bytes.len() > MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1 {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "compliance checkpoint bytes",
                found: bytes.len(),
                maximum: MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1,
            });
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| GatewayComplianceError::StatePoisoned)?;
        if checkpoint_store_generation(state.bytes.as_deref()) != expected {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        state.bytes = Some(bytes.to_vec());
        Ok(checkpoint_store_generation(Some(bytes)))
    }
}
/// Filesystem store with an exclusive stable-file lease, exact-byte CAS,
/// no-follow temp creation, fsync, and atomic replacement.
#[derive(Debug, Clone)]
pub struct FileGatewayComplianceStore {
    path: PathBuf,
    max_bytes: usize,
    #[cfg(test)]
    test_hook: Arc<FileGatewayComplianceStoreTestHook>,
}
#[cfg(test)]
#[derive(Debug, Default)]
struct FileGatewayComplianceStoreTestHook {
    before_persist_replacement: std::sync::Mutex<Option<Vec<u8>>>,
}
impl FileGatewayComplianceStore {
    /// Construct an absolute-path store. The parent directory must be
    /// provisioned ahead of startup, must not traverse symlinks, and must not
    /// be group- or world-writable.
    pub fn new(path: PathBuf) -> Result<Self, GatewayComplianceError> {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(GatewayComplianceError::Persistence(
                "compliance checkpoint path must be an absolute file path".into(),
            ));
        }
        let parent = path.parent().ok_or_else(|| {
            GatewayComplianceError::Persistence("compliance checkpoint path has no parent".into())
        })?;
        validate_checkpoint_parent(parent)?;
        Ok(Self {
            path,
            max_bytes: MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1,
            #[cfg(test)]
            test_hook: Arc::new(FileGatewayComplianceStoreTestHook::default()),
        })
    }
    #[cfg(test)]
    fn replace_before_next_persist(&self, bytes: Vec<u8>) {
        *self
            .test_hook
            .before_persist_replacement
            .lock()
            .expect("file checkpoint test hook lock") = Some(bytes);
    }
}
#[derive(Debug)]
struct FileGatewayComplianceStoreLease {
    path: PathBuf,
    max_bytes: usize,
    _lock_file: File,
    #[cfg(test)]
    test_hook: Arc<FileGatewayComplianceStoreTestHook>,
}
impl GatewayComplianceStore for FileGatewayComplianceStore {
    fn try_acquire(&self) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError> {
        let parent = self.path.parent().ok_or_else(|| {
            GatewayComplianceError::Persistence("compliance checkpoint path has no parent".into())
        })?;
        validate_checkpoint_parent(parent)?;
        let lock_path = checkpoint_lock_path(&self.path)?;
        validate_output_file(&lock_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        set_no_follow(&mut options);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o640);
        }
        let lock_file = options
            .open(&lock_path)
            .map_err(|error| persistence_io("open checkpoint lease file", error))?;
        validate_opened_file(
            &lock_path,
            &lock_file,
            "checkpoint lease file changed while opening",
        )?;
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(GatewayComplianceError::LeaseHeld);
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(persistence_io("lock checkpoint lease file", error));
            }
        }
        validate_opened_file(
            &lock_path,
            &lock_file,
            "checkpoint lease file changed after locking",
        )?;
        Ok(Box::new(FileGatewayComplianceStoreLease {
            path: self.path.clone(),
            max_bytes: self.max_bytes,
            _lock_file: lock_file,
            #[cfg(test)]
            test_hook: Arc::clone(&self.test_hook),
        }))
    }
}
impl GatewayComplianceStoreLease for FileGatewayComplianceStoreLease {
    fn load(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
        read_checkpoint_snapshot(&self.path, self.max_bytes)
    }
    fn compare_and_store(
        &self,
        expected: GatewayComplianceStoreGeneration,
        bytes: &[u8],
    ) -> Result<GatewayComplianceStoreGeneration, GatewayComplianceError> {
        if bytes.len() > self.max_bytes {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "compliance checkpoint bytes",
                found: bytes.len(),
                maximum: self.max_bytes,
            });
        }
        let parent = self.path.parent().ok_or_else(|| {
            GatewayComplianceError::Persistence("compliance checkpoint path has no parent".into())
        })?;
        let current_generation = read_checkpoint_snapshot(&self.path, self.max_bytes)?.generation;
        if current_generation != expected {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        validate_checkpoint_parent(parent)?;
        validate_output_file(&self.path)?;
        let filename = self.path.file_name().ok_or_else(|| {
            GatewayComplianceError::Persistence("compliance checkpoint path has no filename".into())
        })?;
        let mut prefix = std::ffi::OsString::from(".");
        prefix.push(filename);
        prefix.push(".tmp-");
        let mut temporary = tempfile::Builder::new()
            .prefix(&prefix)
            .tempfile_in(parent)
            .map_err(|error| persistence_io("create checkpoint temp file", error))?;
        #[cfg(unix)]
        temporary
            .as_file()
            .set_permissions({
                use std::os::unix::fs::PermissionsExt as _;
                fs::Permissions::from_mode(0o640)
            })
            .map_err(|error| persistence_io("set checkpoint temp permissions", error))?;
        let temporary_metadata = temporary
            .as_file()
            .metadata()
            .map_err(|error| persistence_io("inspect checkpoint temp file", error))?;
        if !temporary_metadata.is_file() || hard_link_count(&temporary_metadata) != 1 {
            return Err(GatewayComplianceError::Persistence(
                "compliance checkpoint temp must be an unlinked regular file".into(),
            ));
        }
        temporary
            .write_all(bytes)
            .map_err(|error| persistence_io("write checkpoint temp file", error))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| persistence_io("fsync checkpoint temp file", error))?;
        validate_checkpoint_parent(parent)?;
        validate_output_file(&self.path)?;
        #[cfg(test)]
        if let Some(replacement) = self
            .test_hook
            .before_persist_replacement
            .lock()
            .map_err(|_| GatewayComplianceError::StatePoisoned)?
            .take()
        {
            fs::write(&self.path, replacement)
                .map_err(|error| persistence_io("run checkpoint test hook", error))?;
        }
        let current_generation = read_checkpoint_snapshot(&self.path, self.max_bytes)?.generation;
        if current_generation != expected {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        // The stable lease serializes compliant writers. Standard filesystem
        // APIs do not provide an atomic compare-bytes-and-rename operation, so
        // a non-cooperating same-user writer can still race this final check
        // and be overwritten. Post-replacement exact-byte verification catches
        // interference that changes the final durable result, but cannot
        // observe an intervening value overwritten by the rename.
        let persisted = temporary
            .persist(&self.path)
            .map_err(|error| persistence_io("replace checkpoint", error.error))?;
        let persisted_metadata = persisted
            .metadata()
            .map_err(|_| GatewayComplianceError::CheckpointConflict)?;
        let path_metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| GatewayComplianceError::CheckpointConflict)?;
        if !persisted_metadata.is_file()
            || path_metadata.file_type().is_symlink()
            || !path_metadata.is_file()
            || hard_link_count(&persisted_metadata) != 1
            || hard_link_count(&path_metadata) != 1
            || !same_file_identity(&persisted_metadata, &path_metadata)
        {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| GatewayComplianceError::CheckpointConflict)?;
        let generation = checkpoint_store_generation(Some(bytes));
        let durable = read_checkpoint_snapshot(&self.path, self.max_bytes)
            .map_err(|_| GatewayComplianceError::CheckpointConflict)?;
        if durable.generation != generation || durable.bytes.as_deref() != Some(bytes) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        Ok(generation)
    }
}
#[derive(Debug, Clone)]
struct GatewayComplianceControllerState {
    checkpoint: GatewayComplianceCheckpointV1,
    generation: GatewayComplianceStoreGeneration,
}
impl std::ops::Deref for GatewayComplianceControllerState {
    type Target = GatewayComplianceCheckpointV1;
    fn deref(&self) -> &Self::Target {
        &self.checkpoint
    }
}
impl std::ops::DerefMut for GatewayComplianceControllerState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.checkpoint
    }
}
/// Thread-safe governed compliance controller.
#[derive(Debug)]
pub struct GatewayComplianceController {
    config: GatewayComplianceControllerConfig,
    expected_feed_transport_identity: Option<GatewayComplianceFeedTransportIdentityV1>,
    lease: Box<dyn GatewayComplianceStoreLease>,
    state: RwLock<GatewayComplianceControllerState>,
    fenced: AtomicBool,
}
impl GatewayComplianceController {
    /// Load or initialize a controller without enabling external feed access.
    ///
    /// Production launchers must use [`Self::new_with_feed_transport`]. This
    /// private constructor exists for controller-core tests and for the
    /// test-only allow-all policy; [`Self::fetch_feed`] fails closed when no
    /// transport identity was bound at construction.
    #[cfg(test)]
    fn new(
        config: GatewayComplianceControllerConfig,
        store: Arc<dyn GatewayComplianceStore>,
    ) -> Result<Self, GatewayComplianceError> {
        Self::new_inner(config, store, None)
    }
    /// Qualify an authenticated feed transport before opening durable state,
    /// then load or initialize the controller.
    ///
    /// # Errors
    ///
    /// Returns an error when controller policy is invalid, provider identity
    /// does not exactly match it, the provider is unavailable or test-marked,
    /// or the durable checkpoint cannot be acquired and validated.
    pub fn new_with_feed_transport(
        config: GatewayComplianceControllerConfig,
        store: Arc<dyn GatewayComplianceStore>,
        transport: &dyn GatewayComplianceFeedTransport,
    ) -> Result<Self, GatewayComplianceError> {
        config.validate()?;
        let expected_feed_transport_identity = config.expected_feed_transport_identity()?;
        qualify_feed_transport(&expected_feed_transport_identity, transport)?;
        Self::new_inner(config, store, Some(expected_feed_transport_identity))
    }
    fn new_inner(
        config: GatewayComplianceControllerConfig,
        store: Arc<dyn GatewayComplianceStore>,
        expected_feed_transport_identity: Option<GatewayComplianceFeedTransportIdentityV1>,
    ) -> Result<Self, GatewayComplianceError> {
        config.validate()?;
        let policy_digest = config.trust_policy.canonical_digest()?;
        let lease = store.try_acquire()?;
        let snapshot = lease.load()?;
        let checkpoint = match snapshot.bytes {
            Some(bytes) => decode_checkpoint(&bytes)?,
            None => GatewayComplianceCheckpointV1::empty(policy_digest),
        };
        validate_checkpoint(&checkpoint, &config, 1)?;
        Ok(Self {
            config,
            expected_feed_transport_identity,
            lease,
            state: RwLock::new(GatewayComplianceControllerState {
                checkpoint,
                generation: snapshot.generation,
            }),
            fenced: AtomicBool::new(false),
        })
    }
    /// Fetch, pin, decompress, decode, and normalize one configured feed.
    pub fn fetch_feed(
        &self,
        feed_id: &str,
        transport: &dyn GatewayComplianceFeedTransport,
    ) -> Result<GatewayComplianceFeedDocumentV1, GatewayComplianceError> {
        let expected = self
            .expected_feed_transport_identity
            .as_ref()
            .ok_or(GatewayComplianceError::FeedTransportNotQualified)?;
        qualify_feed_transport(expected, transport)?;
        let feed = self
            .config
            .feed(feed_id)
            .ok_or_else(|| GatewayComplianceError::UnknownFeed(feed_id.to_owned()))?;
        let bytes = fetch_feed_bytes(feed, self.config.fetch_limits, expected, transport)?;
        // Discard even fully bounded/authenticated response bytes if the
        // runtime provider rotated or was substituted during network I/O.
        qualify_feed_transport(expected, transport)?;
        let document: GatewayComplianceFeedDocumentV1 =
            norito::json::from_slice(&bytes).map_err(|error| {
                GatewayComplianceError::InvalidFeed(format!("invalid canonical feed JSON: {error}"))
            })?;
        if document.feed_id != feed.feed_id {
            return Err(GatewayComplianceError::InvalidFeed(
                "feed identity does not match configured endpoint".into(),
            ));
        }
        document.normalize()
    }
    /// Deterministically merge normalized feeds into an unsigned catalog.
    pub fn build_catalog_payload(
        &self,
        sequence: u64,
        predecessor_digest: Option<[u8; 32]>,
        generated_at_unix: u64,
        valid_until_unix: u64,
        feeds: &[GatewayComplianceFeedDocumentV1],
    ) -> Result<GatewayComplianceCatalogPayloadV1, GatewayComplianceError> {
        let mut seen = BTreeSet::new();
        let mut anchors = Vec::with_capacity(feeds.len());
        let mut baseline_rules = Vec::new();
        let mut appeal_overrides = Vec::new();
        let mut legal_safety_holds = Vec::new();
        let mut toggles = Vec::new();
        for feed in feeds {
            let normalized = feed.clone().normalize()?;
            if normalized != *feed {
                return Err(GatewayComplianceError::NonCanonical(format!(
                    "feed `{}`",
                    feed.feed_id
                )));
            }
            if self.config.feed(&feed.feed_id).is_none() {
                return Err(GatewayComplianceError::UnknownFeed(feed.feed_id.clone()));
            }
            if !seen.insert(feed.feed_id.clone()) {
                return Err(GatewayComplianceError::InvalidFeed(format!(
                    "duplicate feed `{}`",
                    feed.feed_id
                )));
            }
            anchors.push(GatewayComplianceSourceAnchorV1 {
                feed_id: feed.feed_id.clone(),
                feed_digest: feed.canonical_digest()?,
                generated_at_unix: feed.generated_at_unix,
            });
            baseline_rules.extend(feed.baseline_rules.clone());
            appeal_overrides.extend(feed.appeal_overrides.clone());
            legal_safety_holds.extend(feed.legal_safety_holds.clone());
            toggles.extend(feed.toggles.clone());
        }
        for configured in &self.config.feeds {
            if configured.required && !seen.contains(&configured.feed_id) {
                return Err(GatewayComplianceError::MissingRequiredFeed(
                    configured.feed_id.clone(),
                ));
            }
        }
        let payload = GatewayComplianceCatalogPayloadV1 {
            version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
            sequence,
            predecessor_digest,
            policy_digest: self.config.trust_policy.canonical_digest()?,
            generated_at_unix,
            valid_until_unix,
            source_anchors: anchors,
            baseline_rules,
            appeal_overrides,
            legal_safety_holds,
            toggles,
        }
        .normalize()?;
        validate_catalog_against_config(&payload, &self.config)?;
        Ok(payload)
    }
    /// Durably stage a threshold-signed candidate under an exact request
    /// binding. Exact replays return the original durable result even after a
    /// later promotion; same-key substitution and same-sequence equivocation
    /// are rejected.
    pub fn stage_catalog(
        &self,
        catalog: GatewayComplianceCatalogV1,
        observed_at_unix: u64,
        binding: GatewayComplianceMutationBindingV1,
    ) -> Result<GatewayComplianceMutationResultV1, GatewayComplianceError> {
        let mut guard = self.write_state()?;
        if let Some(result) =
            replay_mutation(&guard, GatewayComplianceMutationKindV1::Stage, binding)?
        {
            return Ok(result);
        }
        validate_new_mutation(&guard, binding, observed_at_unix)?;
        let digest = catalog.verify(
            &self.config.trust_policy,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        validate_catalog_against_config(&catalog.payload, &self.config)?;
        validate_catalog_transition(guard.chain_head.as_ref(), &catalog)?;
        let mut replace_candidate = true;
        if let Some(candidate) = guard.candidate.as_ref() {
            let existing = candidate.payload.catalog_digest()?;
            if existing == digest {
                replace_candidate = false;
            }
            if existing != digest && candidate.payload.sequence == catalog.payload.sequence {
                return Err(GatewayComplianceError::CatalogEquivocation {
                    sequence: catalog.payload.sequence,
                });
            }
        }
        let mut next = guard.clone();
        if replace_candidate {
            next.candidate = Some(catalog);
            next.acknowledgements.clear();
        }
        append_mutation_record(
            &mut next,
            GatewayComplianceMutationKindV1::Stage,
            binding,
            digest,
            observed_at_unix,
        );
        self.commit(&mut guard, next)?;
        Ok(GatewayComplianceMutationResultV1 {
            catalog_digest: digest,
            recorded_at_unix: observed_at_unix,
        })
    }
    /// Durably record one signed gateway acknowledgement under an exact
    /// request binding.
    pub fn acknowledge(
        &self,
        acknowledgement: GatewayComplianceAcknowledgementV1,
        observed_at_unix: u64,
        binding: GatewayComplianceMutationBindingV1,
    ) -> Result<GatewayComplianceMutationResultV1, GatewayComplianceError> {
        let mut guard = self.write_state()?;
        if let Some(result) = replay_mutation(
            &guard,
            GatewayComplianceMutationKindV1::Acknowledge,
            binding,
        )? {
            return Ok(result);
        }
        validate_new_mutation(&guard, binding, observed_at_unix)?;
        let candidate = guard
            .candidate
            .as_ref()
            .ok_or(GatewayComplianceError::NoStagedCatalog)?;
        let digest = candidate.payload.catalog_digest()?;
        acknowledgement.verify(
            &self.config.trust_policy,
            digest,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        let mut append_acknowledgement = true;
        if let Some(existing) = guard
            .acknowledgements
            .iter()
            .find(|entry| entry.payload.gateway_id == acknowledgement.payload.gateway_id)
        {
            if existing == &acknowledgement {
                append_acknowledgement = false;
            } else {
                return Err(GatewayComplianceError::GatewayEquivocation(
                    acknowledgement.payload.gateway_id.clone(),
                ));
            }
        }
        if append_acknowledgement && guard.acknowledgements.len() >= MAX_GATEWAY_COMPLIANCE_ACKS_V1
        {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "gateway acknowledgements",
                found: guard.acknowledgements.len().saturating_add(1),
                maximum: MAX_GATEWAY_COMPLIANCE_ACKS_V1,
            });
        }
        let mut next = guard.clone();
        if append_acknowledgement {
            next.acknowledgements.push(acknowledgement);
            next.acknowledgements
                .sort_by(|left, right| left.payload.gateway_id.cmp(&right.payload.gateway_id));
        }
        append_mutation_record(
            &mut next,
            GatewayComplianceMutationKindV1::Acknowledge,
            binding,
            digest,
            observed_at_unix,
        );
        self.commit(&mut guard, next)?;
        Ok(GatewayComplianceMutationResultV1 {
            catalog_digest: digest,
            recorded_at_unix: observed_at_unix,
        })
    }
    /// Promote the exact expected staged catalog after the configured
    /// regional-gateway quorum.
    pub fn promote(
        &self,
        expected_catalog_digest: [u8; 32],
        expected_sequence: u64,
        observed_at_unix: u64,
        binding: GatewayComplianceMutationBindingV1,
    ) -> Result<GatewayComplianceMutationResultV1, GatewayComplianceError> {
        let mut guard = self.write_state()?;
        if let Some(result) =
            replay_mutation(&guard, GatewayComplianceMutationKindV1::Promote, binding)?
        {
            return Ok(result);
        }
        validate_new_mutation(&guard, binding, observed_at_unix)?;
        let candidate = guard
            .candidate
            .as_ref()
            .ok_or(GatewayComplianceError::NoStagedCatalog)?;
        let digest = candidate.verify(
            &self.config.trust_policy,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        validate_catalog_against_config(&candidate.payload, &self.config)?;
        validate_catalog_transition(guard.chain_head.as_ref(), candidate)?;
        if digest != expected_catalog_digest || candidate.payload.sequence != expected_sequence {
            return Err(GatewayComplianceError::PromotionTargetMismatch);
        }
        let mut accepted = 0;
        for acknowledgement in &guard.acknowledgements {
            if !acknowledgement.payload.accepted {
                continue;
            }
            acknowledgement.verify(
                &self.config.trust_policy,
                digest,
                observed_at_unix,
                self.config.max_clock_skew_secs,
            )?;
            accepted += 1;
        }
        if accepted < usize::from(self.config.trust_policy.gateway_ack_threshold) {
            return Err(GatewayComplianceError::GatewayQuorumNotMet {
                found: accepted,
                required: self.config.trust_policy.gateway_ack_threshold,
            });
        }
        if guard.history.len() >= self.config.max_history_entries {
            return Err(GatewayComplianceError::HistoryFull);
        }
        let previous_digest = guard
            .serving
            .as_ref()
            .map(|catalog| catalog.payload.catalog_digest())
            .transpose()?;
        let mut next = guard.clone();
        let promoted = next
            .candidate
            .take()
            .ok_or(GatewayComplianceError::NoStagedCatalog)?;
        next.previous_serving = next.serving.take();
        next.serving = Some(promoted.clone());
        next.chain_head = Some(promoted);
        next.acknowledgements.clear();
        next.history.push(GatewayComplianceHistoryRecordV1 {
            operation_id: binding.key_digest,
            previous_serving_digest: previous_digest,
            serving_digest: digest,
            recorded_at_unix: observed_at_unix,
            action: "promotion".into(),
            reason_code: "gateway-quorum".into(),
        });
        append_mutation_record(
            &mut next,
            GatewayComplianceMutationKindV1::Promote,
            binding,
            digest,
            observed_at_unix,
        );
        self.commit(&mut guard, next)?;
        Ok(GatewayComplianceMutationResultV1 {
            catalog_digest: digest,
            recorded_at_unix: observed_at_unix,
        })
    }
    /// Roll the serving pointer back to the last-known-good catalog while
    /// preserving the promoted predecessor-chain head.
    pub fn rollback(
        &self,
        authorization: &GatewayComplianceRollbackV1,
        observed_at_unix: u64,
        binding: GatewayComplianceMutationBindingV1,
    ) -> Result<GatewayComplianceMutationResultV1, GatewayComplianceError> {
        if binding.key_digest != authorization.payload.operation_id {
            return Err(GatewayComplianceError::IdempotencyConflict);
        }
        let mut guard = self.write_state()?;
        if let Some(result) =
            replay_mutation(&guard, GatewayComplianceMutationKindV1::Rollback, binding)?
        {
            return Ok(result);
        }
        validate_new_mutation(&guard, binding, observed_at_unix)?;
        verify_rollback(
            authorization,
            &self.config.trust_policy,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        let current = guard
            .serving
            .as_ref()
            .ok_or(GatewayComplianceError::NoServingCatalog)?;
        let previous = guard
            .previous_serving
            .as_ref()
            .ok_or(GatewayComplianceError::NoLastKnownGood)?;
        let current_digest = current.payload.catalog_digest()?;
        let previous_digest = previous.payload.catalog_digest()?;
        if authorization.payload.from_catalog_digest != current_digest
            || authorization.payload.to_catalog_digest != previous_digest
        {
            return Err(GatewayComplianceError::RollbackTargetMismatch);
        }
        previous.verify(
            &self.config.trust_policy,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        validate_catalog_against_config(&previous.payload, &self.config)?;
        if guard.history.len() >= self.config.max_history_entries {
            return Err(GatewayComplianceError::HistoryFull);
        }
        let mut next = guard.clone();
        let old_serving = next
            .serving
            .take()
            .ok_or(GatewayComplianceError::NoServingCatalog)?;
        let restored = next
            .previous_serving
            .take()
            .ok_or(GatewayComplianceError::NoLastKnownGood)?;
        next.serving = Some(restored);
        next.previous_serving = Some(old_serving);
        next.history.push(GatewayComplianceHistoryRecordV1 {
            operation_id: authorization.payload.operation_id,
            previous_serving_digest: Some(current_digest),
            serving_digest: previous_digest,
            recorded_at_unix: observed_at_unix,
            action: "rollback".into(),
            reason_code: authorization.payload.reason_code.clone(),
        });
        append_mutation_record(
            &mut next,
            GatewayComplianceMutationKindV1::Rollback,
            binding,
            previous_digest,
            observed_at_unix,
        );
        self.commit(&mut guard, next)?;
        Ok(GatewayComplianceMutationResultV1 {
            catalog_digest: previous_digest,
            recorded_at_unix: observed_at_unix,
        })
    }
    /// Evaluate the active catalog with mandatory precedence:
    /// legal/safety hold, accepted appeal, then baseline policy.
    pub fn evaluate(
        &self,
        scope: &str,
        subject_kind: GatewayComplianceSubjectKindV1,
        subject: &str,
        observed_at_unix: u64,
    ) -> Result<GatewayComplianceDecision, GatewayComplianceError> {
        let scope = normalize_scope(scope)?;
        self.evaluate_scopes(&[scope.as_str()], subject_kind, subject, observed_at_unix)
    }
    /// Evaluate the complete serving scope for this gateway while preserving
    /// precedence across global, regional, and gateway-specific records.
    pub fn evaluate_serving(
        &self,
        subject_kind: GatewayComplianceSubjectKindV1,
        subject: &str,
        observed_at_unix: u64,
    ) -> Result<GatewayComplianceDecision, GatewayComplianceError> {
        self.evaluate_scopes(
            &[
                self.config.region_scope.as_str(),
                self.config.gateway_scope.as_str(),
            ],
            subject_kind,
            subject,
            observed_at_unix,
        )
    }
    fn evaluate_scopes(
        &self,
        scopes: &[&str],
        subject_kind: GatewayComplianceSubjectKindV1,
        subject: &str,
        observed_at_unix: u64,
    ) -> Result<GatewayComplianceDecision, GatewayComplianceError> {
        let subject = normalize_subject(subject_kind, subject)?;
        let guard = self.read_state()?;
        let catalog = guard
            .serving
            .as_ref()
            .ok_or(GatewayComplianceError::NoServingCatalog)?;
        let catalog_digest = catalog.payload.catalog_digest()?;
        validate_catalog_freshness(
            &catalog.payload,
            observed_at_unix,
            self.config.max_clock_skew_secs,
        )?;
        if let Some(hold) = catalog.payload.legal_safety_holds.iter().find(|hold| {
            scopes_match(&hold.scope, scopes)
                && hold.subject_kind == subject_kind
                && hold.subject == subject
                && active_interval(
                    hold.effective_from_unix,
                    hold.expires_at_unix,
                    observed_at_unix,
                )
        }) {
            return Ok(GatewayComplianceDecision {
                disposition: GatewayComplianceDisposition::Deny,
                source: GatewayComplianceDecisionSource::LegalSafetyHold,
                reference_id: Some(hold.hold_id.clone()),
                catalog_digest: Some(catalog_digest),
                catalog_sequence: catalog.payload.sequence,
                catalog_valid_until_unix: catalog.payload.valid_until_unix,
            });
        }
        if let Some(appeal) = catalog.payload.appeal_overrides.iter().find(|appeal| {
            scopes_match(&appeal.scope, scopes)
                && appeal.subject_kind == subject_kind
                && appeal.subject == subject
                && active_interval(
                    appeal.effective_from_unix,
                    Some(appeal.expires_at_unix),
                    observed_at_unix,
                )
        }) {
            return Ok(GatewayComplianceDecision {
                disposition: GatewayComplianceDisposition::Allow,
                source: GatewayComplianceDecisionSource::AcceptedAppeal,
                reference_id: Some(appeal.appeal_id.clone()),
                catalog_digest: Some(catalog_digest),
                catalog_sequence: catalog.payload.sequence,
                catalog_valid_until_unix: catalog.payload.valid_until_unix,
            });
        }
        if let Some(rule) = catalog.payload.baseline_rules.iter().find(|rule| {
            scopes_match(&rule.scope, scopes)
                && rule.subject_kind == subject_kind
                && rule.subject == subject
                && active_interval(
                    rule.effective_from_unix,
                    rule.expires_at_unix,
                    observed_at_unix,
                )
                && rule.toggle_id.as_ref().is_none_or(|toggle_id| {
                    toggle_enabled(
                        &catalog.payload.toggles,
                        toggle_id,
                        &rule.scope,
                        observed_at_unix,
                    )
                })
        }) {
            return Ok(GatewayComplianceDecision {
                disposition: GatewayComplianceDisposition::Deny,
                source: GatewayComplianceDecisionSource::Baseline,
                reference_id: Some(rule.rule_id.clone()),
                catalog_digest: Some(catalog_digest),
                catalog_sequence: catalog.payload.sequence,
                catalog_valid_until_unix: catalog.payload.valid_until_unix,
            });
        }
        Ok(GatewayComplianceDecision {
            disposition: GatewayComplianceDisposition::Allow,
            source: GatewayComplianceDecisionSource::NoMatch,
            reference_id: None,
            catalog_digest: Some(catalog_digest),
            catalog_sequence: catalog.payload.sequence,
            catalog_valid_until_unix: catalog.payload.valid_until_unix,
        })
    }
    /// Return a payload-safe state snapshot for authenticated control/read APIs.
    pub fn checkpoint(&self) -> Result<GatewayComplianceCheckpointV1, GatewayComplianceError> {
        Ok(self.read_state()?.checkpoint.clone())
    }
    fn read_state(
        &self,
    ) -> Result<RwLockReadGuard<'_, GatewayComplianceControllerState>, GatewayComplianceError> {
        if self.fenced.load(AtomicOrdering::Acquire) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        let guard = self.state.read();
        if self.fenced.load(AtomicOrdering::Acquire) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        Ok(guard)
    }
    fn write_state(
        &self,
    ) -> Result<RwLockWriteGuard<'_, GatewayComplianceControllerState>, GatewayComplianceError>
    {
        if self.fenced.load(AtomicOrdering::Acquire) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        let guard = self.state.write();
        if self.fenced.load(AtomicOrdering::Acquire) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        Ok(guard)
    }
    fn commit(
        &self,
        guard: &mut RwLockWriteGuard<'_, GatewayComplianceControllerState>,
        mut next: GatewayComplianceControllerState,
    ) -> Result<(), GatewayComplianceError> {
        next.checkpoint.revision = guard.checkpoint.revision.checked_add(1).ok_or_else(|| {
            GatewayComplianceError::InvalidCheckpoint("checkpoint revision overflow".into())
        })?;
        validate_checkpoint(&next.checkpoint, &self.config, 1)?;
        let bytes = encode_bounded(&next.checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)?;
        next.generation = match self.lease.compare_and_store(guard.generation, &bytes) {
            Ok(generation) => generation,
            Err(GatewayComplianceError::CheckpointConflict) => {
                self.fenced.store(true, AtomicOrdering::Release);
                return Err(GatewayComplianceError::CheckpointConflict);
            }
            Err(error) => return Err(error),
        };
        **guard = next;
        Ok(())
    }
}
fn validate_mutation_binding(
    binding: GatewayComplianceMutationBindingV1,
) -> Result<(), GatewayComplianceError> {
    if binding.key_digest.iter().all(|byte| *byte == 0)
        || binding.request_digest.iter().all(|byte| *byte == 0)
    {
        return Err(GatewayComplianceError::IdempotencyConflict);
    }
    Ok(())
}
fn replay_mutation(
    checkpoint: &GatewayComplianceCheckpointV1,
    operation: GatewayComplianceMutationKindV1,
    binding: GatewayComplianceMutationBindingV1,
) -> Result<Option<GatewayComplianceMutationResultV1>, GatewayComplianceError> {
    validate_mutation_binding(binding)?;
    let Some(record) = checkpoint
        .idempotency_records
        .iter()
        .find(|record| record.key_digest == binding.key_digest)
    else {
        return Ok(None);
    };
    if record.operation != operation || record.request_digest != binding.request_digest {
        return Err(GatewayComplianceError::IdempotencyConflict);
    }
    Ok(Some(GatewayComplianceMutationResultV1 {
        catalog_digest: record.catalog_digest,
        recorded_at_unix: record.recorded_at_unix,
    }))
}
fn validate_new_mutation(
    checkpoint: &GatewayComplianceCheckpointV1,
    binding: GatewayComplianceMutationBindingV1,
    observed_at_unix: u64,
) -> Result<(), GatewayComplianceError> {
    validate_mutation_binding(binding)?;
    if checkpoint.idempotency_records.len() >= MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1 {
        return Err(GatewayComplianceError::IdempotencyRegistryFull);
    }
    if observed_at_unix == 0
        || checkpoint
            .idempotency_records
            .last()
            .is_some_and(|record| observed_at_unix < record.recorded_at_unix)
    {
        return Err(GatewayComplianceError::MutationTimeInvalid);
    }
    Ok(())
}
fn append_mutation_record(
    checkpoint: &mut GatewayComplianceCheckpointV1,
    operation: GatewayComplianceMutationKindV1,
    binding: GatewayComplianceMutationBindingV1,
    catalog_digest: [u8; 32],
    recorded_at_unix: u64,
) {
    checkpoint
        .idempotency_records
        .push(GatewayComplianceIdempotencyRecordV1 {
            key_digest: binding.key_digest,
            request_digest: binding.request_digest,
            operation,
            catalog_digest,
            recorded_at_unix,
        });
}
/// Build a fresh governed empty catalog for Torii request tests.
#[cfg(test)]
pub(crate) fn allow_all_gateway_compliance_controller_for_tests() -> Arc<GatewayComplianceController>
{
    use ed25519_dalek::{Signer as _, SigningKey};
    let catalog_key = SigningKey::from_bytes(&[0xC1; 32]);
    let gateway_key = SigningKey::from_bytes(&[0xD2; 32]);
    let trust_policy = GatewayComplianceTrustPolicyV1 {
        policy_id: [0xE3; 32],
        catalog_threshold: 1,
        catalog_signers: vec![GatewayComplianceTrustedSignerV1 {
            signer_id: "catalog-test".into(),
            public_key: catalog_key.verifying_key().to_bytes(),
        }],
        revoked_catalog_signer_ids: Vec::new(),
        gateway_ack_threshold: 1,
        gateway_signers: vec![GatewayComplianceTrustedSignerV1 {
            signer_id: "gateway-test".into(),
            public_key: gateway_key.verifying_key().to_bytes(),
        }],
        revoked_gateway_signer_ids: Vec::new(),
    };
    let config = GatewayComplianceControllerConfig {
        trust_policy: trust_policy.clone(),
        region_scope: "region:test".into(),
        gateway_scope: "gateway:gateway-test".into(),
        feeds: Vec::new(),
        feed_transport_provider: None,
        fetch_limits: GatewayComplianceFetchLimits::default(),
        max_clock_skew_secs: 300,
        max_feed_age_secs: 3_600,
        max_catalog_validity_secs: 86_400,
        max_history_entries: 16,
    };
    let observed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("test clock must follow the Unix epoch")
        .as_secs()
        .max(1);
    let payload = GatewayComplianceCatalogPayloadV1 {
        version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
        sequence: 1,
        predecessor_digest: None,
        policy_digest: trust_policy
            .canonical_digest()
            .expect("test trust policy must be canonical"),
        generated_at_unix: observed_at_unix,
        valid_until_unix: observed_at_unix.saturating_add(86_400),
        source_anchors: Vec::new(),
        baseline_rules: Vec::new(),
        appeal_overrides: Vec::new(),
        legal_safety_holds: Vec::new(),
        toggles: Vec::new(),
    };
    let signing_digest = payload
        .signing_digest()
        .expect("test catalog must be canonical");
    let catalog = GatewayComplianceCatalogV1 {
        payload,
        approvals: vec![GatewayComplianceCatalogApprovalV1 {
            version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
            signer_id: "catalog-test".into(),
            signature: catalog_key.sign(&signing_digest).to_bytes(),
        }],
    };
    let controller = Arc::new(
        GatewayComplianceController::new(config, Arc::new(TestGatewayComplianceStore::default()))
            .expect("test compliance controller must initialize"),
    );
    let catalog_digest = controller
        .stage_catalog(
            catalog,
            observed_at_unix,
            GatewayComplianceMutationBindingV1 {
                key_digest: [0x01; 32],
                request_digest: [0x81; 32],
            },
        )
        .expect("test catalog must stage")
        .catalog_digest;
    let acknowledgement_payload = GatewayComplianceAcknowledgementPayloadV1 {
        version: GATEWAY_COMPLIANCE_ACK_VERSION_V1,
        gateway_id: "gateway-test".into(),
        catalog_digest,
        observed_at_unix,
        accepted: true,
        rejection_code: None,
    };
    let acknowledgement_digest = hash_canonical(
        ACK_SIGNING_DOMAIN_V1,
        &acknowledgement_payload,
        MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
    )
    .expect("test acknowledgement must encode");
    controller
        .acknowledge(
            GatewayComplianceAcknowledgementV1 {
                payload: acknowledgement_payload,
                signature: gateway_key.sign(&acknowledgement_digest).to_bytes(),
            },
            observed_at_unix,
            GatewayComplianceMutationBindingV1 {
                key_digest: [0x02; 32],
                request_digest: [0x82; 32],
            },
        )
        .expect("test acknowledgement must commit");
    controller
        .promote(
            catalog_digest,
            1,
            observed_at_unix,
            GatewayComplianceMutationBindingV1 {
                key_digest: [0x03; 32],
                request_digest: [0x83; 32],
            },
        )
        .expect("test catalog must promote");
    controller
}
fn validate_signer_inventory(
    signers: &[GatewayComplianceTrustedSignerV1],
    revoked: &[String],
    threshold: u16,
    label: &'static str,
) -> Result<(), GatewayComplianceError> {
    if signers.is_empty() || signers.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
        return Err(GatewayComplianceError::InvalidPolicy(format!(
            "{label} signer count is invalid"
        )));
    }
    let mut previous: Option<&str> = None;
    let mut keys = BTreeSet::new();
    for signer in signers {
        validate_token(&signer.signer_id, "signer_id")?;
        if previous.is_some_and(|value| value >= signer.signer_id.as_str()) {
            return Err(GatewayComplianceError::NonCanonical(format!(
                "{label} signer order"
            )));
        }
        let verifying_key = VerifyingKey::from_bytes(&signer.public_key).map_err(|error| {
            GatewayComplianceError::InvalidPolicy(format!(
                "{label} signer `{}` has invalid Ed25519 key: {error}",
                signer.signer_id
            ))
        })?;
        if verifying_key.is_weak() {
            return Err(GatewayComplianceError::InvalidPolicy(format!(
                "{label} signer `{}` uses a weak Ed25519 key",
                signer.signer_id
            )));
        }
        if !keys.insert(signer.public_key) {
            return Err(GatewayComplianceError::InvalidPolicy(format!(
                "{label} signer public keys must be unique"
            )));
        }
        previous = Some(&signer.signer_id);
    }
    let mut previous_revoked: Option<&str> = None;
    for signer_id in revoked {
        validate_token(signer_id, "revoked signer_id")?;
        if previous_revoked.is_some_and(|value| value >= signer_id.as_str()) {
            return Err(GatewayComplianceError::NonCanonical(format!(
                "{label} revocation order"
            )));
        }
        if find_signer(signers, signer_id).is_none() {
            return Err(GatewayComplianceError::InvalidPolicy(format!(
                "{label} revocation names an unknown signer"
            )));
        }
        previous_revoked = Some(signer_id);
    }
    let active = signers.len().saturating_sub(revoked.len());
    let threshold = usize::from(
        NonZeroU16::new(threshold)
            .ok_or_else(|| {
                GatewayComplianceError::InvalidPolicy(format!("{label} threshold must be non-zero"))
            })?
            .get(),
    );
    if threshold > active {
        return Err(GatewayComplianceError::InvalidPolicy(format!(
            "{label} threshold exceeds active signer count"
        )));
    }
    Ok(())
}
fn validate_disjoint_signer_roles(
    catalog_signers: &[GatewayComplianceTrustedSignerV1],
    gateway_signers: &[GatewayComplianceTrustedSignerV1],
) -> Result<(), GatewayComplianceError> {
    let catalog_ids = catalog_signers
        .iter()
        .map(|signer| signer.signer_id.as_str())
        .collect::<BTreeSet<_>>();
    let catalog_keys = catalog_signers
        .iter()
        .map(|signer| signer.public_key)
        .collect::<BTreeSet<_>>();
    if gateway_signers.iter().any(|signer| {
        catalog_ids.contains(signer.signer_id.as_str()) || catalog_keys.contains(&signer.public_key)
    }) {
        return Err(GatewayComplianceError::InvalidPolicy(
            "catalog and gateway acknowledgement signer identities must be administratively disjoint"
                .into(),
        ));
    }
    Ok(())
}
fn find_signer<'a>(
    signers: &'a [GatewayComplianceTrustedSignerV1],
    signer_id: &str,
) -> Option<&'a GatewayComplianceTrustedSignerV1> {
    signers
        .binary_search_by(|signer| signer.signer_id.as_str().cmp(signer_id))
        .ok()
        .map(|index| &signers[index])
}
fn contains_sorted(values: &[String], needle: &str) -> bool {
    values
        .binary_search_by(|value| value.as_str().cmp(needle))
        .is_ok()
}
fn normalize_baseline_rule(
    rule: &mut GatewayComplianceBaselineRuleV1,
) -> Result<(), GatewayComplianceError> {
    rule.rule_id = normalize_token(&rule.rule_id, "rule_id")?;
    rule.scope = normalize_scope(&rule.scope)?;
    rule.subject = normalize_subject(rule.subject_kind, &rule.subject)?;
    rule.source_id = normalize_token(&rule.source_id, "source_id")?;
    rule.reason_code = normalize_token(&rule.reason_code, "reason_code")?;
    rule.toggle_id = rule
        .toggle_id
        .as_deref()
        .map(|value| normalize_token(value, "toggle_id"))
        .transpose()?;
    validate_interval(rule.effective_from_unix, rule.expires_at_unix)
}
fn normalize_appeal(
    appeal: &mut GatewayComplianceAppealOverrideV1,
) -> Result<(), GatewayComplianceError> {
    appeal.appeal_id = normalize_token(&appeal.appeal_id, "appeal_id")?;
    appeal.scope = normalize_scope(&appeal.scope)?;
    appeal.subject = normalize_subject(appeal.subject_kind, &appeal.subject)?;
    if appeal.decision_digest.iter().all(|byte| *byte == 0) {
        return Err(GatewayComplianceError::InvalidCatalog(
            "accepted appeal decision digest must not be all zeroes".into(),
        ));
    }
    validate_interval(appeal.effective_from_unix, Some(appeal.expires_at_unix))
}
fn normalize_hold(
    hold: &mut GatewayComplianceLegalSafetyHoldV1,
) -> Result<(), GatewayComplianceError> {
    hold.hold_id = normalize_token(&hold.hold_id, "hold_id")?;
    hold.scope = normalize_scope(&hold.scope)?;
    hold.subject = normalize_subject(hold.subject_kind, &hold.subject)?;
    hold.authority_reference = normalize_token(&hold.authority_reference, "authority_reference")?;
    validate_interval(hold.effective_from_unix, hold.expires_at_unix)
}
fn normalize_toggle(toggle: &mut GatewayComplianceToggleV1) -> Result<(), GatewayComplianceError> {
    toggle.toggle_id = normalize_token(&toggle.toggle_id, "toggle_id")?;
    toggle.scope = normalize_scope(&toggle.scope)?;
    toggle.approval_reference = normalize_token(&toggle.approval_reference, "approval_reference")?;
    validate_interval(toggle.effective_from_unix, Some(toggle.expires_at_unix))
}
fn validate_interval(
    effective_from_unix: u64,
    expires_at_unix: Option<u64>,
) -> Result<(), GatewayComplianceError> {
    if effective_from_unix == 0
        || expires_at_unix.is_some_and(|expiry| expiry <= effective_from_unix)
    {
        return Err(GatewayComplianceError::InvalidCatalog(
            "compliance entry validity interval is invalid".into(),
        ));
    }
    Ok(())
}
fn normalize_token(value: &str, field: &'static str) -> Result<String, GatewayComplianceError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 128
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b':')
        })
    {
        return Err(GatewayComplianceError::InvalidCatalog(format!(
            "{field} is not a canonical bounded token"
        )));
    }
    Ok(normalized)
}
fn validate_token(value: &str, field: &'static str) -> Result<(), GatewayComplianceError> {
    if normalize_token(value, field)? != value {
        return Err(GatewayComplianceError::NonCanonical(field.into()));
    }
    Ok(())
}
fn normalize_scope(value: &str) -> Result<String, GatewayComplianceError> {
    let normalized = normalize_token(value, "scope")?;
    if normalized == "global"
        || normalized
            .strip_prefix("region:")
            .is_some_and(|suffix| !suffix.is_empty())
        || normalized
            .strip_prefix("gateway:")
            .is_some_and(|suffix| !suffix.is_empty())
    {
        Ok(normalized)
    } else {
        Err(GatewayComplianceError::InvalidCatalog(
            "scope must be global, region:<id>, or gateway:<id>".into(),
        ))
    }
}
fn normalize_subject(
    kind: GatewayComplianceSubjectKindV1,
    value: &str,
) -> Result<String, GatewayComplianceError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 2_048
        || !trimmed.is_ascii()
        || trimmed.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(GatewayComplianceError::InvalidCatalog(
            "compliance subject is not bounded canonical ASCII".into(),
        ));
    }
    match kind {
        GatewayComplianceSubjectKindV1::Provider
        | GatewayComplianceSubjectKindV1::ManifestDigest => normalize_hex(trimmed, 64),
        GatewayComplianceSubjectKindV1::Cid => normalize_cid(trimmed),
        GatewayComplianceSubjectKindV1::Url => normalize_subject_url(trimmed),
    }
}
fn normalize_cid(value: &str) -> Result<String, GatewayComplianceError> {
    let encoded = value.strip_prefix('b').ok_or_else(|| {
        GatewayComplianceError::InvalidCatalog(
            "CID subjects must use the lowercase base32 multibase prefix".into(),
        )
    })?;
    if encoded.is_empty()
        || encoded
            .bytes()
            .any(|byte| !matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
    {
        return Err(GatewayComplianceError::InvalidCatalog(
            "CID subjects must use canonical lowercase base32 without padding".into(),
        ));
    }
    let decoded = decode_base32_lower(encoded)?;
    if encode_base32_lower(&decoded) != encoded {
        return Err(GatewayComplianceError::InvalidCatalog(
            "CID subject is not a canonical base32 round-trip".into(),
        ));
    }
    Ok(value.to_owned())
}
fn decode_base32_lower(value: &str) -> Result<Vec<u8>, GatewayComplianceError> {
    let mut output = Vec::with_capacity(value.len().saturating_mul(5) / 8);
    let mut accumulator = 0_u16;
    let mut bits = 0_u8;
    for byte in value.bytes() {
        let digit = match byte {
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => {
                return Err(GatewayComplianceError::InvalidCatalog(
                    "CID subject contains a non-base32 digit".into(),
                ));
            }
        };
        accumulator = (accumulator << 5) | u16::from(digit);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);
            accumulator &= (1_u16 << bits).saturating_sub(1);
        }
    }
    if bits != 0 && accumulator != 0 {
        return Err(GatewayComplianceError::InvalidCatalog(
            "CID subject contains non-zero base32 padding bits".into(),
        ));
    }
    Ok(output)
}
fn encode_base32_lower(value: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut output = String::with_capacity(value.len().saturating_mul(8).div_ceil(5));
    let mut accumulator = 0_u16;
    let mut bits = 0_u8;
    for byte in value {
        accumulator = (accumulator << 8) | u16::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(char::from(
                ALPHABET[usize::from((accumulator >> bits) & 0x1f)],
            ));
            accumulator &= (1_u16 << bits).saturating_sub(1);
        }
    }
    if bits != 0 {
        let digit = usize::from((accumulator << (5 - bits)) & 0x1f);
        output.push(char::from(ALPHABET[digit]));
    }
    output
}
fn normalize_hex(value: &str, expected_length: usize) -> Result<String, GatewayComplianceError> {
    let normalized = value.to_ascii_lowercase();
    if normalized.len() != expected_length
        || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GatewayComplianceError::InvalidCatalog(format!(
            "hex subject must contain exactly {expected_length} digits"
        )));
    }
    Ok(normalized)
}
fn normalize_subject_url(value: &str) -> Result<String, GatewayComplianceError> {
    let parsed = Url::parse(value).map_err(|error| {
        GatewayComplianceError::InvalidCatalog(format!("invalid URL subject: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
    {
        return Err(GatewayComplianceError::InvalidCatalog(
            "URL subject contains forbidden components".into(),
        ));
    }
    Ok(parsed.to_string())
}
fn reject_duplicate_keys<T, F>(
    values: &[T],
    key: F,
    field: &'static str,
) -> Result<(), GatewayComplianceError>
where
    F: Fn(&T) -> &str,
{
    if values
        .windows(2)
        .any(|window| key(&window[0]) == key(&window[1]))
    {
        return Err(GatewayComplianceError::InvalidCatalog(format!(
            "{field} contains duplicate identities"
        )));
    }
    Ok(())
}
fn reject_duplicate_toggle_scope(
    toggles: &[GatewayComplianceToggleV1],
) -> Result<(), GatewayComplianceError> {
    if toggles.windows(2).any(|window| {
        window[0].toggle_id == window[1].toggle_id && window[0].scope == window[1].scope
    }) {
        return Err(GatewayComplianceError::InvalidCatalog(
            "toggles contains duplicate id/scope pairs".into(),
        ));
    }
    Ok(())
}
fn verify_ed25519(
    public_key: &[u8; 32],
    signature: &[u8; 64],
    message: &[u8],
    signer_id: &str,
) -> Result<(), GatewayComplianceError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|error| {
        GatewayComplianceError::InvalidSignature {
            signer_id: signer_id.to_owned(),
            reason: error.to_string(),
        }
    })?;
    let signature = Ed25519Signature::from_bytes(signature);
    key.verify_strict(message, &signature).map_err(|error| {
        GatewayComplianceError::InvalidSignature {
            signer_id: signer_id.to_owned(),
            reason: error.to_string(),
        }
    })
}
fn validate_catalog_transition(
    previous: Option<&GatewayComplianceCatalogV1>,
    next: &GatewayComplianceCatalogV1,
) -> Result<(), GatewayComplianceError> {
    match previous {
        None => {
            if next.payload.sequence != 1 || next.payload.predecessor_digest.is_some() {
                return Err(GatewayComplianceError::InvalidPredecessor);
            }
        }
        Some(previous) => {
            let expected_sequence = previous
                .payload
                .sequence
                .checked_add(1)
                .ok_or(GatewayComplianceError::SequenceOverflow)?;
            if next.payload.sequence != expected_sequence
                || next.payload.predecessor_digest != Some(previous.payload.catalog_digest()?)
            {
                return Err(GatewayComplianceError::InvalidPredecessor);
            }
        }
    }
    Ok(())
}
fn validate_catalog_freshness(
    payload: &GatewayComplianceCatalogPayloadV1,
    observed_at_unix: u64,
    max_clock_skew_secs: u64,
) -> Result<(), GatewayComplianceError> {
    if observed_at_unix == 0
        || payload.generated_at_unix > observed_at_unix.saturating_add(max_clock_skew_secs)
        || observed_at_unix >= payload.valid_until_unix
    {
        return Err(GatewayComplianceError::CatalogNotFresh);
    }
    Ok(())
}
fn validate_catalog_against_config(
    payload: &GatewayComplianceCatalogPayloadV1,
    config: &GatewayComplianceControllerConfig,
) -> Result<(), GatewayComplianceError> {
    let maximum_valid_until = payload
        .generated_at_unix
        .checked_add(config.max_catalog_validity_secs)
        .ok_or(GatewayComplianceError::TimeOverflow)?;
    if payload.valid_until_unix > maximum_valid_until {
        return Err(GatewayComplianceError::InvalidCatalog(
            "catalog validity exceeds the configured maximum".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for anchor in &payload.source_anchors {
        if config.feed(&anchor.feed_id).is_none() {
            return Err(GatewayComplianceError::UnknownFeed(anchor.feed_id.clone()));
        }
        if !seen.insert(anchor.feed_id.clone()) {
            return Err(GatewayComplianceError::InvalidCatalog(format!(
                "duplicate source feed `{}`",
                anchor.feed_id
            )));
        }
        if anchor.generated_at_unix
            > payload
                .generated_at_unix
                .saturating_add(config.max_clock_skew_secs)
            || anchor
                .generated_at_unix
                .saturating_add(config.max_feed_age_secs)
                < payload.generated_at_unix
        {
            return Err(GatewayComplianceError::InvalidCatalog(format!(
                "source feed `{}` is stale or future-dated",
                anchor.feed_id
            )));
        }
    }
    for feed in &config.feeds {
        if feed.required && !seen.contains(&feed.feed_id) {
            return Err(GatewayComplianceError::MissingRequiredFeed(
                feed.feed_id.clone(),
            ));
        }
    }
    Ok(())
}
fn verify_rollback(
    authorization: &GatewayComplianceRollbackV1,
    policy: &GatewayComplianceTrustPolicyV1,
    observed_at_unix: u64,
    max_clock_skew_secs: u64,
) -> Result<(), GatewayComplianceError> {
    let payload = &authorization.payload;
    if payload.version != GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1
        || payload.operation_id.iter().all(|byte| *byte == 0)
        || payload.from_catalog_digest.iter().all(|byte| *byte == 0)
        || payload.to_catalog_digest.iter().all(|byte| *byte == 0)
        || payload.from_catalog_digest == payload.to_catalog_digest
        || payload.authorized_at_unix == 0
        || payload.authorized_at_unix > observed_at_unix.saturating_add(max_clock_skew_secs)
        || payload
            .authorized_at_unix
            .saturating_add(max_clock_skew_secs)
            < observed_at_unix
    {
        return Err(GatewayComplianceError::InvalidRollback(
            "rollback payload is malformed or stale".into(),
        ));
    }
    validate_token(&payload.reason_code, "reason_code")?;
    if authorization.approvals.len() < usize::from(policy.catalog_threshold) {
        return Err(GatewayComplianceError::QuorumNotMet {
            found: authorization.approvals.len(),
            required: policy.catalog_threshold,
        });
    }
    if authorization.approvals.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
        return Err(GatewayComplianceError::ResourceLimit {
            resource: "rollback approvals",
            found: authorization.approvals.len(),
            maximum: MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
        });
    }
    let digest = hash_canonical(
        ROLLBACK_SIGNING_DOMAIN_V1,
        payload,
        MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
    )?;
    let mut previous: Option<&str> = None;
    for approval in &authorization.approvals {
        if approval.version != GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1 {
            return Err(GatewayComplianceError::InvalidRollback(
                "rollback approval version is unsupported".into(),
            ));
        }
        if previous.is_some_and(|value| value >= approval.signer_id.as_str()) {
            return Err(GatewayComplianceError::NonCanonical(
                "rollback approval order".into(),
            ));
        }
        let trusted = policy
            .catalog_signer(&approval.signer_id)
            .ok_or_else(|| GatewayComplianceError::UntrustedSigner(approval.signer_id.clone()))?;
        if contains_sorted(
            &policy.revoked_catalog_signer_ids,
            approval.signer_id.as_str(),
        ) {
            return Err(GatewayComplianceError::RevokedSigner(
                approval.signer_id.clone(),
            ));
        }
        verify_ed25519(
            &trusted.public_key,
            &approval.signature,
            &digest,
            &approval.signer_id,
        )?;
        previous = Some(&approval.signer_id);
    }
    Ok(())
}
fn validate_checkpoint(
    checkpoint: &GatewayComplianceCheckpointV1,
    config: &GatewayComplianceControllerConfig,
    observed_at_unix: u64,
) -> Result<(), GatewayComplianceError> {
    if checkpoint.version != GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1 {
        return Err(GatewayComplianceError::InvalidCheckpoint(
            "unsupported checkpoint version".into(),
        ));
    }
    let durable_mutation_count =
        u64::try_from(checkpoint.idempotency_records.len()).map_err(|_| {
            GatewayComplianceError::InvalidCheckpoint(
                "checkpoint mutation inventory length overflow".into(),
            )
        })?;
    if checkpoint.revision != durable_mutation_count {
        return Err(GatewayComplianceError::InvalidCheckpoint(
            "checkpoint revision must equal its durable mutation inventory length".into(),
        ));
    }
    if checkpoint.policy_digest != config.trust_policy.canonical_digest()? {
        return Err(GatewayComplianceError::PolicyDigestMismatch);
    }
    if checkpoint.history.len() > config.max_history_entries
        || checkpoint.acknowledgements.len() > MAX_GATEWAY_COMPLIANCE_ACKS_V1
        || checkpoint.idempotency_records.len() > MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1
    {
        return Err(GatewayComplianceError::InvalidCheckpoint(
            "checkpoint inventory exceeds configured bounds".into(),
        ));
    }
    for catalog in [
        checkpoint.chain_head.as_ref(),
        checkpoint.serving.as_ref(),
        checkpoint.previous_serving.as_ref(),
        checkpoint.candidate.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        catalog.verify(
            &config.trust_policy,
            catalog.payload.generated_at_unix,
            config.max_clock_skew_secs,
        )?;
        if catalog.payload.policy_digest != checkpoint.policy_digest {
            return Err(GatewayComplianceError::PolicyDigestMismatch);
        }
        validate_catalog_against_config(&catalog.payload, config)?;
    }
    let chain_head_digest = checkpoint
        .chain_head
        .as_ref()
        .map(|catalog| catalog.payload.catalog_digest())
        .transpose()?;
    let serving_digest = checkpoint
        .serving
        .as_ref()
        .map(|catalog| catalog.payload.catalog_digest())
        .transpose()?;
    let previous_serving_digest = checkpoint
        .previous_serving
        .as_ref()
        .map(|catalog| catalog.payload.catalog_digest())
        .transpose()?;
    match (
        checkpoint.chain_head.as_ref(),
        checkpoint.serving.as_ref(),
        checkpoint.previous_serving.as_ref(),
    ) {
        (None, None, None) => {}
        (Some(_), Some(_), previous) => {
            if ![serving_digest, previous_serving_digest]
                .into_iter()
                .flatten()
                .any(|digest| Some(digest) == chain_head_digest)
            {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "chain head is not one of the retained serving catalogs".into(),
                ));
            }
            if let Some(previous) = previous {
                let serving = checkpoint.serving.as_ref().expect("matched Some serving");
                if serving_digest == previous_serving_digest {
                    return Err(GatewayComplianceError::InvalidCheckpoint(
                        "serving and previous_serving catalogs must be distinct".into(),
                    ));
                }
                let (older, newer) = if serving.payload.sequence < previous.payload.sequence {
                    (serving, previous)
                } else {
                    (previous, serving)
                };
                if older.payload.sequence.checked_add(1) == Some(newer.payload.sequence) {
                    validate_catalog_transition(Some(older), newer).map_err(|_| {
                        GatewayComplianceError::InvalidCheckpoint(
                            "adjacent retained catalogs break predecessor lineage".into(),
                        )
                    })?;
                } else if chain_head_digest != serving_digest
                    || older.payload.sequence >= newer.payload.sequence
                {
                    return Err(GatewayComplianceError::InvalidCheckpoint(
                        "non-adjacent retained catalogs are inconsistent with a promotion after rollback"
                            .into(),
                    ));
                }
            }
        }
        _ => {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "chain_head, serving, and previous_serving topology is inconsistent".into(),
            ));
        }
    }
    if let Some(candidate) = checkpoint.candidate.as_ref() {
        validate_catalog_transition(checkpoint.chain_head.as_ref(), candidate)?;
        let digest = candidate.payload.catalog_digest()?;
        let mut previous_gateway: Option<&str> = None;
        for ack in &checkpoint.acknowledgements {
            if previous_gateway.is_some_and(|value| value >= ack.payload.gateway_id.as_str()) {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "acknowledgements are not strictly ordered".into(),
                ));
            }
            ack.verify(
                &config.trust_policy,
                digest,
                observed_at_unix.max(ack.payload.observed_at_unix),
                config.max_clock_skew_secs,
            )?;
            previous_gateway = Some(&ack.payload.gateway_id);
        }
    } else if !checkpoint.acknowledgements.is_empty() {
        return Err(GatewayComplianceError::InvalidCheckpoint(
            "acknowledgements exist without a candidate".into(),
        ));
    }
    let mut operation_ids = BTreeSet::new();
    let mut active_history_digest = None;
    let mut last_promotion_digest = None;
    let mut previous_recorded_at = 0;
    for record in &checkpoint.history {
        if record.operation_id.iter().all(|byte| *byte == 0)
            || record.serving_digest.iter().all(|byte| *byte == 0)
            || !operation_ids.insert(record.operation_id)
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "history contains zero digests or duplicate operation ids".into(),
            ));
        }
        validate_token(&record.action, "history action")?;
        validate_token(&record.reason_code, "history reason_code")?;
        if record.recorded_at_unix == 0 || record.recorded_at_unix < previous_recorded_at {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "history timestamps must be non-zero and monotonic".into(),
            ));
        }
        if record.previous_serving_digest != active_history_digest {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "history serving lineage is discontinuous".into(),
            ));
        }
        match record.action.as_str() {
            "promotion" => last_promotion_digest = Some(record.serving_digest),
            "rollback" if record.previous_serving_digest.is_some() => {}
            "rollback" => {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "rollback history requires a previous serving digest".into(),
                ));
            }
            _ => {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "history action must be promotion or rollback".into(),
                ));
            }
        }
        active_history_digest = Some(record.serving_digest);
        previous_recorded_at = record.recorded_at_unix;
    }
    if checkpoint.history.is_empty() {
        if chain_head_digest.is_some()
            || serving_digest.is_some()
            || previous_serving_digest.is_some()
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "serving catalogs require promotion history".into(),
            ));
        }
    } else {
        let last = checkpoint.history.last().expect("history is non-empty");
        if active_history_digest != serving_digest
            || last_promotion_digest != chain_head_digest
            || last.previous_serving_digest != previous_serving_digest
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "checkpoint pointers do not match durable history lineage".into(),
            ));
        }
    }
    let mut known_catalog_digests = BTreeSet::new();
    for catalog in [
        checkpoint.chain_head.as_ref(),
        checkpoint.serving.as_ref(),
        checkpoint.previous_serving.as_ref(),
        checkpoint.candidate.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        known_catalog_digests.insert(catalog.payload.catalog_digest()?);
    }
    for record in &checkpoint.history {
        known_catalog_digests.insert(record.serving_digest);
        if let Some(digest) = record.previous_serving_digest {
            known_catalog_digests.insert(digest);
        }
    }
    let mut idempotency_keys = BTreeSet::new();
    let mut previous_idempotency_timestamp = 0;
    for record in &checkpoint.idempotency_records {
        if record.key_digest.iter().all(|byte| *byte == 0)
            || record.request_digest.iter().all(|byte| *byte == 0)
            || record.catalog_digest.iter().all(|byte| *byte == 0)
            || !idempotency_keys.insert(record.key_digest)
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "idempotency records contain zero digests or duplicate keys".into(),
            ));
        }
        if record.recorded_at_unix == 0 || record.recorded_at_unix < previous_idempotency_timestamp
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "idempotency record timestamps must be non-zero and monotonic".into(),
            ));
        }
        if !known_catalog_digests.contains(&record.catalog_digest) {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "idempotency record references an unknown catalog".into(),
            ));
        }
        if let Some(action) = record.operation.history_action() {
            let Some(history) = checkpoint
                .history
                .iter()
                .find(|history| history.operation_id == record.key_digest)
            else {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "terminal mutation idempotency record has no matching history".into(),
                ));
            };
            if history.action != action
                || history.serving_digest != record.catalog_digest
                || history.recorded_at_unix != record.recorded_at_unix
            {
                return Err(GatewayComplianceError::InvalidCheckpoint(
                    "terminal mutation idempotency record disagrees with history".into(),
                ));
            }
        }
        previous_idempotency_timestamp = record.recorded_at_unix;
    }
    for history in &checkpoint.history {
        let expected_operation = match history.action.as_str() {
            "promotion" => GatewayComplianceMutationKindV1::Promote,
            "rollback" => GatewayComplianceMutationKindV1::Rollback,
            _ => unreachable!("history actions were validated above"),
        };
        let Some(record) = checkpoint
            .idempotency_records
            .iter()
            .find(|record| record.key_digest == history.operation_id)
        else {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "history has no matching idempotency record".into(),
            ));
        };
        if record.operation != expected_operation
            || record.catalog_digest != history.serving_digest
            || record.recorded_at_unix != history.recorded_at_unix
        {
            return Err(GatewayComplianceError::InvalidCheckpoint(
                "history disagrees with its idempotency record".into(),
            ));
        }
    }
    encode_bounded(checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)?;
    Ok(())
}
fn decode_checkpoint(
    bytes: &[u8],
) -> Result<GatewayComplianceCheckpointV1, GatewayComplianceError> {
    if bytes.len() > MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1 {
        return Err(GatewayComplianceError::ResourceLimit {
            resource: "compliance checkpoint bytes",
            found: bytes.len(),
            maximum: MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1,
        });
    }
    let checkpoint: GatewayComplianceCheckpointV1 = norito::decode_from_bytes_with_limits(
        bytes,
        norito::DecodeLimits::new(
            MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
            MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1,
            4_000_000,
            MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1 * 4,
            128,
        ),
    )
    .map_err(|error| GatewayComplianceError::InvalidCheckpoint(error.to_string()))?;
    let canonical = encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)?;
    if canonical != bytes {
        return Err(GatewayComplianceError::NonCanonical(
            "compliance checkpoint encoding".into(),
        ));
    }
    Ok(checkpoint)
}
fn fetch_feed_bytes(
    feed: &GatewayComplianceFeedPolicy,
    limits: GatewayComplianceFetchLimits,
    expected_transport: &GatewayComplianceFeedTransportIdentityV1,
    transport: &dyn GatewayComplianceFeedTransport,
) -> Result<Vec<u8>, GatewayComplianceError> {
    let started = Instant::now();
    let mut current = validate_feed_url(feed, &feed.url)?;
    for redirect_count in 0..=limits.max_redirects {
        let host = current
            .host_str()
            .ok_or_else(|| GatewayComplianceError::UnsafeUrl("missing host".into()))?;
        let remaining = remaining_fetch_time(started, limits.total_timeout)?;
        let mut addresses = qualified_feed_transport_resolve(
            expected_transport,
            transport,
            host,
            limits.connect_timeout.min(remaining),
        )?;
        let remaining = remaining_fetch_time(started, limits.total_timeout)?;
        normalize_resolved_addresses(&mut addresses, limits.max_dns_addresses)?;
        let response = qualified_feed_transport_fetch(
            expected_transport,
            transport,
            &GatewayComplianceFetchRequest {
                url: current.clone(),
                pinned_addresses: addresses.clone(),
                connect_timeout: limits.connect_timeout.min(remaining),
                total_timeout: remaining,
                max_encoded_bytes: limits.max_encoded_bytes,
            },
        )?;
        if response.elapsed > remaining {
            return Err(GatewayComplianceError::FetchTimeout);
        }
        remaining_fetch_time(started, limits.total_timeout)?;
        if !addresses.contains(&response.connected_address) {
            return Err(GatewayComplianceError::DnsRebinding);
        }
        let host_policy = host_policy(feed, host)?;
        if !host_policy
            .accepted_spki_sha256
            .contains(&response.peer_spki_sha256)
        {
            return Err(GatewayComplianceError::TrustPinMismatch);
        }
        if response.body.len() > limits.max_encoded_bytes {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "encoded feed bytes",
                found: response.body.len(),
                maximum: limits.max_encoded_bytes,
            });
        }
        let remaining = remaining_fetch_time(started, limits.total_timeout)?;
        let mut revalidated = qualified_feed_transport_resolve(
            expected_transport,
            transport,
            host,
            limits.connect_timeout.min(remaining),
        )?;
        remaining_fetch_time(started, limits.total_timeout)?;
        normalize_resolved_addresses(&mut revalidated, limits.max_dns_addresses)?;
        if revalidated != addresses {
            return Err(GatewayComplianceError::DnsRebinding);
        }
        if (300..400).contains(&response.status) {
            if redirect_count == limits.max_redirects {
                return Err(GatewayComplianceError::TooManyRedirects);
            }
            let location = response.redirect_location.ok_or_else(|| {
                GatewayComplianceError::InvalidFeed("redirect lacks Location".into())
            })?;
            let redirected = current.join(&location).map_err(|error| {
                GatewayComplianceError::UnsafeUrl(format!("invalid redirect: {error}"))
            })?;
            current = validate_feed_url(feed, redirected.as_str())?;
            continue;
        }
        if response.status != 200 || response.redirect_location.is_some() {
            return Err(GatewayComplianceError::InvalidFeed(format!(
                "feed returned HTTP {}",
                response.status
            )));
        }
        remaining_fetch_time(started, limits.total_timeout)?;
        let decoded = decompress_bounded(
            &response.body,
            response.content_encoding,
            limits.max_decoded_bytes,
        )?;
        remaining_fetch_time(started, limits.total_timeout)?;
        return Ok(decoded);
    }
    Err(GatewayComplianceError::TooManyRedirects)
}
fn qualified_feed_transport_resolve(
    expected: &GatewayComplianceFeedTransportIdentityV1,
    transport: &dyn GatewayComplianceFeedTransport,
    hostname: &str,
    timeout: Duration,
) -> Result<Vec<IpAddr>, GatewayComplianceError> {
    qualify_feed_transport(expected, transport)?;
    let result = transport.resolve(hostname, timeout);
    qualify_feed_transport(expected, transport)?;
    result.map_err(redact_feed_transport_operation_error)
}
fn qualified_feed_transport_fetch(
    expected: &GatewayComplianceFeedTransportIdentityV1,
    transport: &dyn GatewayComplianceFeedTransport,
    request: &GatewayComplianceFetchRequest,
) -> Result<GatewayComplianceFetchResponse, GatewayComplianceError> {
    qualify_feed_transport(expected, transport)?;
    let result = transport.fetch(request);
    qualify_feed_transport(expected, transport)?;
    result.map_err(redact_feed_transport_operation_error)
}
fn redact_feed_transport_operation_error(error: GatewayComplianceError) -> GatewayComplianceError {
    match error {
        GatewayComplianceError::FetchTimeout => GatewayComplianceError::FetchTimeout,
        GatewayComplianceError::DnsRebinding => GatewayComplianceError::DnsRebinding,
        GatewayComplianceError::NonPublicAddress => GatewayComplianceError::NonPublicAddress,
        GatewayComplianceError::UnsafeAddressSet { found, maximum } => {
            GatewayComplianceError::UnsafeAddressSet { found, maximum }
        }
        GatewayComplianceError::TrustPinMismatch => GatewayComplianceError::TrustPinMismatch,
        GatewayComplianceError::ResourceLimit {
            resource,
            found,
            maximum,
        } => GatewayComplianceError::ResourceLimit {
            resource,
            found,
            maximum,
        },
        _ => GatewayComplianceError::FeedTransportOperationFailed,
    }
}
fn remaining_fetch_time(
    started: Instant,
    total_timeout: Duration,
) -> Result<Duration, GatewayComplianceError> {
    total_timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(GatewayComplianceError::FetchTimeout)
}
fn qualify_feed_transport(
    expected: &GatewayComplianceFeedTransportIdentityV1,
    transport: &dyn GatewayComplianceFeedTransport,
) -> Result<(), GatewayComplianceError> {
    let observed = transport
        .qualification()
        .map_err(|_| GatewayComplianceError::FeedTransportUnavailable)?;
    if observed.test_marked {
        return Err(GatewayComplianceError::FeedTransportTestMarked);
    }
    let observed_binding = GatewayProviderBindingV1::try_new(
        observed.provider_handle,
        observed.revision,
        observed.policy_digest,
    )
    .map_err(|error| match error {
        GatewayProviderBindingErrorV1::TestMarkedHandle => {
            GatewayComplianceError::FeedTransportTestMarked
        }
        GatewayProviderBindingErrorV1::ZeroRevision => GatewayComplianceError::FeedTransportStale,
        GatewayProviderBindingErrorV1::InvalidHandle
        | GatewayProviderBindingErrorV1::ZeroPolicyDigest => {
            GatewayComplianceError::FeedTransportUnqualified
        }
    })?;
    if observed_binding.provider_handle() != expected.provider_handle {
        return Err(GatewayComplianceError::FeedTransportSubstituted);
    }
    if observed_binding.revision() != expected.revision {
        return Err(GatewayComplianceError::FeedTransportStale);
    }
    if observed_binding.policy_digest() != expected.policy_digest {
        return Err(GatewayComplianceError::FeedTransportSubstituted);
    }
    Ok(())
}
fn validate_feed_url(
    feed: &GatewayComplianceFeedPolicy,
    raw: &str,
) -> Result<Url, GatewayComplianceError> {
    if raw.len() > 2_048 {
        return Err(GatewayComplianceError::UnsafeUrl(
            "URL exceeds 2048 bytes".into(),
        ));
    }
    let parsed =
        Url::parse(raw).map_err(|error| GatewayComplianceError::UnsafeUrl(error.to_string()))?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.port().is_some_and(|port| port != 443)
    {
        return Err(GatewayComplianceError::UnsafeUrl(
            "feed URL must be credential-free HTTPS without query or fragment".into(),
        ));
    }
    let host = match parsed.host() {
        Some(Host::Domain(host)) => host,
        _ => {
            return Err(GatewayComplianceError::UnsafeUrl(
                "feed URL must use an allowlisted DNS hostname".into(),
            ));
        }
    };
    validate_dns_hostname(host)?;
    host_policy(feed, host)?;
    if parsed.path().contains("//")
        || raw.contains("%2f")
        || raw.contains("%2F")
        || raw.contains("%5c")
        || raw.contains("%5C")
        || raw.contains("/../")
        || raw.ends_with("/..")
    {
        return Err(GatewayComplianceError::UnsafeUrl(
            "feed URL contains ambiguous path separators or traversal".into(),
        ));
    }
    if parsed.as_str() != raw {
        return Err(GatewayComplianceError::UnsafeUrl(
            "feed URL is not in canonical URL spelling".into(),
        ));
    }
    Ok(parsed)
}
fn host_policy<'a>(
    feed: &'a GatewayComplianceFeedPolicy,
    host: &str,
) -> Result<&'a GatewayComplianceFeedHostPolicy, GatewayComplianceError> {
    feed.hosts
        .binary_search_by(|entry| entry.hostname.as_str().cmp(host))
        .ok()
        .map(|index| &feed.hosts[index])
        .ok_or_else(|| GatewayComplianceError::UnsafeUrl("host is not allowlisted".into()))
}
fn validate_dns_hostname(host: &str) -> Result<(), GatewayComplianceError> {
    if host.is_empty()
        || host.len() > 253
        || !host.contains('.')
        || host != host.to_ascii_lowercase()
        || host.ends_with('.')
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".onion")
        || host.parse::<IpAddr>().is_ok()
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
    {
        return Err(GatewayComplianceError::UnsafeUrl(
            "host is not a canonical public DNS name".into(),
        ));
    }
    Ok(())
}
fn normalize_resolved_addresses(
    addresses: &mut Vec<IpAddr>,
    maximum: usize,
) -> Result<(), GatewayComplianceError> {
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > maximum {
        return Err(GatewayComplianceError::UnsafeAddressSet {
            found: addresses.len(),
            maximum,
        });
    }
    if addresses.iter().any(|address| !is_public_ip(*address)) {
        return Err(GatewayComplianceError::NonPublicAddress);
    }
    Ok(())
}
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}
fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (18..=19).contains(&b)))
}
fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;
    let documentation_v2 = segments[0] == 0x3fff && (segments[1] & 0xf000) == 0;
    let orchid = segments[0] == 0x2001 && (segments[1] & 0xfff0) == 0x0010;
    let transition = (segments[0] == 0x2001 && segments[1] == 0)
        || segments[0] == 0x2002
        || ip.to_ipv4_mapped().is_some();
    !((segments[0] & 0xe000) != 0x2000
        || ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || documentation
        || documentation_v2
        || orchid
        || transition)
}
fn decompress_bounded(
    bytes: &[u8],
    encoding: GatewayComplianceContentEncoding,
    maximum: usize,
) -> Result<Vec<u8>, GatewayComplianceError> {
    match encoding {
        GatewayComplianceContentEncoding::Identity => {
            if bytes.len() > maximum {
                return Err(GatewayComplianceError::ResourceLimit {
                    resource: "decoded feed bytes",
                    found: bytes.len(),
                    maximum,
                });
            }
            Ok(bytes.to_vec())
        }
        GatewayComplianceContentEncoding::Gzip => {
            read_bounded(GzDecoder::new(Cursor::new(bytes)), maximum, "gzip")
        }
        GatewayComplianceContentEncoding::Zstd => {
            let decoder =
                zstd::stream::read::Decoder::new(Cursor::new(bytes)).map_err(|error| {
                    GatewayComplianceError::Decompression(format!("zstd header: {error}"))
                })?;
            read_bounded(decoder, maximum, "zstd")
        }
    }
}
fn read_bounded<R: Read>(
    reader: R,
    maximum: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, GatewayComplianceError> {
    let limit = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut output = Vec::with_capacity(maximum.min(64 * 1024));
    reader
        .take(limit)
        .read_to_end(&mut output)
        .map_err(|error| GatewayComplianceError::Decompression(format!("{algorithm}: {error}")))?;
    if output.len() > maximum {
        return Err(GatewayComplianceError::ResourceLimit {
            resource: "decoded feed bytes",
            found: output.len(),
            maximum,
        });
    }
    Ok(output)
}
fn active_interval(start: u64, end: Option<u64>, observed: u64) -> bool {
    start <= observed && end.is_none_or(|end| observed < end)
}
fn scopes_match(rule_scope: &str, request_scopes: &[&str]) -> bool {
    rule_scope == "global" || request_scopes.contains(&rule_scope)
}
fn toggle_enabled(
    toggles: &[GatewayComplianceToggleV1],
    toggle_id: &str,
    scope: &str,
    observed_at_unix: u64,
) -> bool {
    toggles
        .iter()
        .find(|toggle| {
            toggle.toggle_id == toggle_id
                && toggle.scope == scope
                && active_interval(
                    toggle.effective_from_unix,
                    Some(toggle.expires_at_unix),
                    observed_at_unix,
                )
        })
        .or_else(|| {
            toggles.iter().find(|toggle| {
                toggle.toggle_id == toggle_id
                    && toggle.scope == "global"
                    && active_interval(
                        toggle.effective_from_unix,
                        Some(toggle.expires_at_unix),
                        observed_at_unix,
                    )
            })
        })
        .is_none_or(|toggle| toggle.enabled)
}
fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
    maximum: usize,
) -> Result<[u8; 32], GatewayComplianceError> {
    let bytes = encode_bounded(value, maximum)?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| GatewayComplianceError::Encoding("payload length overflow".into()))?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&length.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
fn checkpoint_store_generation(bytes: Option<&[u8]>) -> GatewayComplianceStoreGeneration {
    let Some(bytes) = bytes else {
        return GatewayComplianceStoreGeneration::Absent;
    };
    let mut hasher = Hasher::new();
    hasher.update(CHECKPOINT_STORE_GENERATION_DOMAIN_V1);
    hasher.update(
        &u64::try_from(bytes.len())
            .expect("bounded checkpoint length fits u64")
            .to_le_bytes(),
    );
    hasher.update(bytes);
    GatewayComplianceStoreGeneration::Present(*hasher.finalize().as_bytes())
}
fn checkpoint_store_snapshot(bytes: Option<Vec<u8>>) -> GatewayComplianceStoreSnapshot {
    GatewayComplianceStoreSnapshot {
        generation: checkpoint_store_generation(bytes.as_deref()),
        bytes,
    }
}
fn encode_bounded<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, GatewayComplianceError> {
    match norito::core::to_bytes_bounded(value, maximum) {
        Ok(bytes) => Ok(bytes),
        Err(norito::core::BoundedEncodeError::FrameTooLarge { encoded_bytes, .. }) => {
            Err(GatewayComplianceError::ResourceLimit {
                resource: "canonical encoded bytes",
                found: encoded_bytes,
                maximum,
            })
        }
        Err(error) => Err(GatewayComplianceError::Encoding(error.to_string())),
    }
}
fn checkpoint_lock_path(path: &Path) -> Result<PathBuf, GatewayComplianceError> {
    let parent = path.parent().ok_or_else(|| {
        GatewayComplianceError::Persistence("compliance checkpoint path has no parent".into())
    })?;
    let filename = path.file_name().ok_or_else(|| {
        GatewayComplianceError::Persistence("compliance checkpoint path has no filename".into())
    })?;
    let mut lock_name = std::ffi::OsString::from(".");
    lock_name.push(filename);
    lock_name.push(".lock");
    Ok(parent.join(lock_name))
}
fn read_checkpoint_snapshot(
    path: &Path,
    max_bytes: usize,
) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
    let parent = path.parent().ok_or_else(|| {
        GatewayComplianceError::Persistence("compliance checkpoint path has no parent".into())
    })?;
    validate_checkpoint_parent(parent)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(checkpoint_store_snapshot(None));
        }
        Err(error) => return Err(persistence_io("inspect checkpoint", error)),
    };
    validate_checkpoint_file_metadata(&metadata)?;
    let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if length > max_bytes {
        return Err(GatewayComplianceError::ResourceLimit {
            resource: "compliance checkpoint bytes",
            found: length,
            maximum: max_bytes,
        });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = options
        .open(path)
        .map_err(|error| persistence_io("open checkpoint", error))?;
    validate_opened_file(path, &file, "compliance checkpoint changed while opening")?;
    let mut bytes = Vec::with_capacity(length);
    file.take(
        u64::try_from(max_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
    )
    .read_to_end(&mut bytes)
    .map_err(|error| persistence_io("read checkpoint", error))?;
    if bytes.len() > max_bytes {
        return Err(GatewayComplianceError::ResourceLimit {
            resource: "compliance checkpoint bytes",
            found: bytes.len(),
            maximum: max_bytes,
        });
    }
    Ok(checkpoint_store_snapshot(Some(bytes)))
}
fn validate_checkpoint_parent(path: &Path) -> Result<(), GatewayComplianceError> {
    validate_existing_directory_chain(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| persistence_io("inspect checkpoint parent", error))?;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(GatewayComplianceError::Persistence(
                "checkpoint parent must not be group- or world-writable".into(),
            ));
        }
    }
    Ok(())
}
fn validate_existing_directory_chain(path: &Path) -> Result<(), GatewayComplianceError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                current.push(component.as_os_str());
            }
            Component::CurDir | Component::ParentDir => {
                return Err(GatewayComplianceError::Persistence(
                    "checkpoint path contains traversal".into(),
                ));
            }
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| persistence_io("inspect checkpoint directory", error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(GatewayComplianceError::Persistence(format!(
                "checkpoint directory `{}` is not a regular directory",
                current.display()
            )));
        }
    }
    Ok(())
}
fn validate_checkpoint_file_metadata(
    metadata: &fs::Metadata,
) -> Result<(), GatewayComplianceError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() || hard_link_count(metadata) != 1 {
        return Err(GatewayComplianceError::Persistence(
            "compliance checkpoint must be a single-link regular non-symlink file".into(),
        ));
    }
    validate_file_permissions(metadata, "compliance checkpoint")
}
fn validate_output_file(path: &Path) -> Result<(), GatewayComplianceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || hard_link_count(&metadata) != 1 =>
        {
            Err(GatewayComplianceError::Persistence(
                "checkpoint output must be absent or a single-link regular non-symlink file".into(),
            ))
        }
        Ok(metadata) => validate_file_permissions(&metadata, "checkpoint output"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(persistence_io("inspect checkpoint output", error)),
    }
}
fn validate_opened_file(
    path: &Path,
    file: &File,
    changed_message: &'static str,
) -> Result<(), GatewayComplianceError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| persistence_io("inspect opened file path", error))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| persistence_io("inspect opened file", error))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !opened_metadata.is_file()
        || hard_link_count(&path_metadata) != 1
        || hard_link_count(&opened_metadata) != 1
        || !same_file_identity(&path_metadata, &opened_metadata)
    {
        return Err(GatewayComplianceError::Persistence(changed_message.into()));
    }
    validate_file_permissions(&path_metadata, "opened checkpoint file")?;
    validate_file_permissions(&opened_metadata, "opened checkpoint file")
}
#[cfg(unix)]
fn validate_file_permissions(
    metadata: &fs::Metadata,
    label: &'static str,
) -> Result<(), GatewayComplianceError> {
    use std::os::unix::fs::PermissionsExt as _;
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(GatewayComplianceError::Persistence(format!(
            "{label} must not be group- or world-writable"
        )));
    }
    Ok(())
}
#[cfg(not(unix))]
fn validate_file_permissions(
    _metadata: &fs::Metadata,
    _label: &'static str,
) -> Result<(), GatewayComplianceError> {
    Ok(())
}
#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt as _;
    options.custom_flags(libc::O_NOFOLLOW);
}
#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}
#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.is_file() == right.is_file()
}
#[cfg(unix)]
fn hard_link_count(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink()
}
#[cfg(not(unix))]
fn hard_link_count(_metadata: &fs::Metadata) -> u64 {
    1
}
fn persistence_io(action: &'static str, error: io::Error) -> GatewayComplianceError {
    GatewayComplianceError::Persistence(format!("{action}: {error}"))
}
/// Fail-closed compliance controller errors.
#[derive(Debug, Error)]
pub enum GatewayComplianceError {
    /// Trust or controller policy is malformed.
    #[error("invalid gateway compliance policy: {0}")]
    InvalidPolicy(String),
    /// Catalog shape or semantics are malformed.
    #[error("invalid gateway compliance catalog: {0}")]
    InvalidCatalog(String),
    /// Feed shape or response is malformed.
    #[error("invalid gateway compliance feed: {0}")]
    InvalidFeed(String),
    /// Durable checkpoint is invalid.
    #[error("invalid gateway compliance checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// A canonical inventory or string was not normalized.
    #[error("non-canonical gateway compliance value: {0}")]
    NonCanonical(String),
    /// A bounded resource exceeded policy.
    #[error("{resource} count/size {found} exceeds maximum {maximum}")]
    ResourceLimit {
        /// Bounded resource.
        resource: &'static str,
        /// Observed count/size.
        found: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// Norito encoding failed.
    #[error("gateway compliance encoding failed: {0}")]
    Encoding(String),
    /// Catalog policy digest differs from resolved config.
    #[error("gateway compliance policy digest mismatch")]
    PolicyDigestMismatch,
    /// Feed access was attempted on a controller without a startup-qualified transport.
    #[error("gateway compliance feed transport was not qualified at startup")]
    FeedTransportNotQualified,
    /// The runtime feed transport could not attest its identity.
    #[error("gateway compliance feed transport is unavailable")]
    FeedTransportUnavailable,
    /// The runtime feed transport returned an invalid public qualification.
    #[error("gateway compliance feed transport is unqualified")]
    FeedTransportUnqualified,
    /// The runtime feed transport failed without exposing provider diagnostics.
    #[error("gateway compliance feed transport operation failed")]
    FeedTransportOperationFailed,
    /// The runtime feed transport does not match the configured trust inventory.
    #[error("gateway compliance feed transport was substituted")]
    FeedTransportSubstituted,
    /// The runtime feed transport revision differs from the V1 contract.
    #[error("gateway compliance feed transport revision is stale")]
    FeedTransportStale,
    /// Test, mock, development, and placeholder providers are forbidden.
    #[error("gateway compliance feed transport is test-marked")]
    FeedTransportTestMarked,
    /// Catalog timestamp is stale or too far in the future.
    #[error("gateway compliance catalog is stale or future-dated")]
    CatalogNotFresh,
    /// Catalog approval quorum is incomplete.
    #[error("gateway compliance quorum not met: found {found}, required {required}")]
    QuorumNotMet {
        /// Valid approval count.
        found: usize,
        /// Required approval count.
        required: u16,
    },
    /// Regional gateway acknowledgement quorum is incomplete.
    #[error("gateway acknowledgement quorum not met: found {found}, required {required}")]
    GatewayQuorumNotMet {
        /// Positive acknowledgement count.
        found: usize,
        /// Required acknowledgement count.
        required: u16,
    },
    /// Signature identity is repeated.
    #[error("duplicate compliance signer `{0}`")]
    DuplicateSigner(String),
    /// Signer is not in the resolved trust policy.
    #[error("untrusted compliance signer `{0}`")]
    UntrustedSigner(String),
    /// Signer is explicitly revoked.
    #[error("revoked compliance signer `{0}`")]
    RevokedSigner(String),
    /// Signature verification failed.
    #[error("invalid compliance signature from `{signer_id}`: {reason}")]
    InvalidSignature {
        /// Signer identity.
        signer_id: String,
        /// Verification failure.
        reason: String,
    },
    /// Initial or successor linkage is invalid.
    #[error("gateway compliance catalog predecessor or sequence is invalid")]
    InvalidPredecessor,
    /// Sequence arithmetic overflowed.
    #[error("gateway compliance catalog sequence overflow")]
    SequenceOverflow,
    /// Timestamp arithmetic overflowed.
    #[error("gateway compliance timestamp overflow")]
    TimeOverflow,
    /// A same-sequence alternative candidate was staged.
    #[error("gateway compliance catalog equivocation at sequence {sequence}")]
    CatalogEquivocation {
        /// Conflicting sequence.
        sequence: u64,
    },
    /// Promotion expectation does not identify the staged catalog exactly.
    #[error("gateway compliance promotion target mismatch")]
    PromotionTargetMismatch,
    /// No candidate is staged.
    #[error("no gateway compliance catalog is staged")]
    NoStagedCatalog,
    /// Gateway acknowledgement is malformed.
    #[error("invalid gateway compliance acknowledgement: {0}")]
    InvalidAcknowledgement(String),
    /// Gateway submitted conflicting acknowledgements.
    #[error("gateway `{0}` submitted conflicting acknowledgements")]
    GatewayEquivocation(String),
    /// No serving catalog exists.
    #[error("no gateway compliance catalog is serving")]
    NoServingCatalog,
    /// No prior last-known-good catalog exists.
    #[error("no last-known-good gateway compliance catalog exists")]
    NoLastKnownGood,
    /// Rollback authorization is malformed.
    #[error("invalid gateway compliance rollback: {0}")]
    InvalidRollback(String),
    /// Rollback targets do not match serving/LKG state.
    #[error("gateway compliance rollback target mismatch")]
    RollbackTargetMismatch,
    /// An idempotency key is already bound to another request or action.
    #[error("gateway compliance idempotency binding conflict")]
    IdempotencyConflict,
    /// Durable replay protection is full and must be archived by an operator.
    #[error("gateway compliance idempotency registry reached its V1 bound")]
    IdempotencyRegistryFull,
    /// Mutation time is zero or regresses behind the last durable operation.
    #[error("gateway compliance mutation time is zero or regressed")]
    MutationTimeInvalid,
    /// Durable history must be archived before additional actions.
    #[error("gateway compliance history reached its configured bound")]
    HistoryFull,
    /// Configured feed does not exist.
    #[error("unknown gateway compliance feed `{0}`")]
    UnknownFeed(String),
    /// Required feed was omitted from catalog construction.
    #[error("required gateway compliance feed `{0}` is missing")]
    MissingRequiredFeed(String),
    /// URL violates HTTPS/allowlist rules.
    #[error("unsafe gateway compliance URL: {0}")]
    UnsafeUrl(String),
    /// DNS answer count is empty or excessive.
    #[error("gateway compliance DNS answer count {found} is outside 1..={maximum}")]
    UnsafeAddressSet {
        /// Observed count.
        found: usize,
        /// Maximum count.
        maximum: usize,
    },
    /// DNS resolved to non-public space.
    #[error("gateway compliance endpoint resolved to a non-public address")]
    NonPublicAddress,
    /// Connected address or revalidation does not match the pinned DNS set.
    #[error("gateway compliance DNS rebinding/address-set change detected")]
    DnsRebinding,
    /// Authenticated peer did not match configured pins.
    #[error("gateway compliance TLS trust pin mismatch")]
    TrustPinMismatch,
    /// Redirect limit was exceeded.
    #[error("gateway compliance redirect limit exceeded")]
    TooManyRedirects,
    /// Fetch exceeded configured time.
    #[error("gateway compliance fetch exceeded its deadline")]
    FetchTimeout,
    /// Bounded decompression failed.
    #[error("gateway compliance decompression failed: {0}")]
    Decompression(String),
    /// Durable storage failed.
    #[error("gateway compliance persistence failed: {0}")]
    Persistence(String),
    /// Another controller already owns the process-lifetime checkpoint lease.
    #[error("gateway compliance checkpoint lease is already held")]
    LeaseHeld,
    /// Exact checkpoint bytes changed or a durable replacement became indeterminate.
    #[error("gateway compliance checkpoint generation conflict")]
    CheckpointConflict,
    /// In-process state lock was poisoned.
    #[error("gateway compliance state lock poisoned")]
    StatePoisoned,
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};
    use flate2::{Compression, write::GzEncoder};
    use std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        io::Write as _,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering as TestAtomicOrdering},
        },
    };
    const NOW: u64 = 1_800_000_000;
    #[derive(Debug, Default)]
    struct MemoryStore {
        state: Arc<Mutex<MemoryStoreState>>,
        fail_next_store: Arc<AtomicBool>,
        fail_after_store: Arc<AtomicBool>,
    }
    #[derive(Debug, Default)]
    struct MemoryStoreState {
        bytes: Option<Vec<u8>>,
        leased: bool,
    }
    #[derive(Debug)]
    struct MemoryStoreLease {
        state: Arc<Mutex<MemoryStoreState>>,
        fail_next_store: Arc<AtomicBool>,
        fail_after_store: Arc<AtomicBool>,
    }
    impl Drop for MemoryStoreLease {
        fn drop(&mut self) {
            if let Ok(mut state) = self.state.lock() {
                state.leased = false;
            }
        }
    }
    impl MemoryStore {
        fn fail_next_store(&self) {
            self.fail_next_store.store(true, TestAtomicOrdering::SeqCst);
        }
        fn fail_after_store(&self) {
            self.fail_after_store
                .store(true, TestAtomicOrdering::SeqCst);
        }
        fn durable_bytes(&self) -> Option<Vec<u8>> {
            self.state.lock().expect("memory store lock").bytes.clone()
        }
        fn force_store(&self, bytes: Vec<u8>) {
            self.state.lock().expect("memory store lock").bytes = Some(bytes);
        }
    }
    impl GatewayComplianceStore for MemoryStore {
        fn try_acquire(
            &self,
        ) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError> {
            let mut state = self
                .state
                .lock()
                .map_err(|_| GatewayComplianceError::StatePoisoned)?;
            if state.leased {
                return Err(GatewayComplianceError::LeaseHeld);
            }
            state.leased = true;
            drop(state);
            Ok(Box::new(MemoryStoreLease {
                state: Arc::clone(&self.state),
                fail_next_store: Arc::clone(&self.fail_next_store),
                fail_after_store: Arc::clone(&self.fail_after_store),
            }))
        }
    }
    impl GatewayComplianceStoreLease for MemoryStoreLease {
        fn load(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
            let state = self
                .state
                .lock()
                .map_err(|_| GatewayComplianceError::StatePoisoned)?;
            Ok(checkpoint_store_snapshot(state.bytes.clone()))
        }
        fn compare_and_store(
            &self,
            expected: GatewayComplianceStoreGeneration,
            bytes: &[u8],
        ) -> Result<GatewayComplianceStoreGeneration, GatewayComplianceError> {
            if bytes.len() > MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1 {
                return Err(GatewayComplianceError::ResourceLimit {
                    resource: "compliance checkpoint bytes",
                    found: bytes.len(),
                    maximum: MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1,
                });
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| GatewayComplianceError::StatePoisoned)?;
            if checkpoint_store_generation(state.bytes.as_deref()) != expected {
                return Err(GatewayComplianceError::CheckpointConflict);
            }
            if self.fail_next_store.swap(false, TestAtomicOrdering::SeqCst) {
                return Err(GatewayComplianceError::Persistence(
                    "injected test store failure".into(),
                ));
            }
            state.bytes = Some(bytes.to_vec());
            if self
                .fail_after_store
                .swap(false, TestAtomicOrdering::SeqCst)
            {
                return Err(GatewayComplianceError::CheckpointConflict);
            }
            Ok(checkpoint_store_generation(Some(bytes)))
        }
    }
    fn catalog_keys() -> [SigningKey; 2] {
        [
            SigningKey::from_bytes(&[0x11; 32]),
            SigningKey::from_bytes(&[0x22; 32]),
        ]
    }
    fn gateway_keys() -> [SigningKey; 2] {
        [
            SigningKey::from_bytes(&[0x33; 32]),
            SigningKey::from_bytes(&[0x44; 32]),
        ]
    }
    fn trust_policy() -> GatewayComplianceTrustPolicyV1 {
        let catalog = catalog_keys();
        let gateways = gateway_keys();
        GatewayComplianceTrustPolicyV1 {
            policy_id: [0xA5; 32],
            catalog_threshold: 2,
            catalog_signers: vec![
                GatewayComplianceTrustedSignerV1 {
                    signer_id: "council-a".into(),
                    public_key: catalog[0].verifying_key().to_bytes(),
                },
                GatewayComplianceTrustedSignerV1 {
                    signer_id: "council-b".into(),
                    public_key: catalog[1].verifying_key().to_bytes(),
                },
            ],
            revoked_catalog_signer_ids: Vec::new(),
            gateway_ack_threshold: 2,
            gateway_signers: vec![
                GatewayComplianceTrustedSignerV1 {
                    signer_id: "gateway-eu".into(),
                    public_key: gateways[0].verifying_key().to_bytes(),
                },
                GatewayComplianceTrustedSignerV1 {
                    signer_id: "gateway-us".into(),
                    public_key: gateways[1].verifying_key().to_bytes(),
                },
            ],
            revoked_gateway_signer_ids: Vec::new(),
        }
    }
    fn config() -> GatewayComplianceControllerConfig {
        let feed = feed_policy();
        let policy_digest = gateway_compliance_feed_transport_policy_digest(&BTreeMap::from([(
            feed.hosts[0].hostname.clone(),
            feed.hosts[0].accepted_spki_sha256.clone(),
        )]))
        .expect("test feed transport policy digest");
        GatewayComplianceControllerConfig {
            trust_policy: trust_policy(),
            region_scope: "region:eu".into(),
            gateway_scope: "gateway:gateway-eu".into(),
            feeds: vec![feed],
            feed_transport_provider: Some(
                GatewayProviderBindingV1::try_new(
                    GATEWAY_COMPLIANCE_FEED_TRANSPORT_HANDLE_V1.to_owned(),
                    GATEWAY_COMPLIANCE_FEED_TRANSPORT_REVISION_V1,
                    policy_digest,
                )
                .expect("valid test feed transport provider binding"),
            ),
            fetch_limits: GatewayComplianceFetchLimits::default(),
            max_clock_skew_secs: 300,
            max_feed_age_secs: 3_600,
            max_catalog_validity_secs: 7_200,
            max_history_entries: 16,
        }
    }
    #[derive(Debug)]
    struct QualifiableTransport {
        identity: Mutex<GatewayComplianceFeedTransportIdentityV1>,
        probe_available: AtomicBool,
        drift_revision_on_resolve: AtomicBool,
        drift_revision_on_fetch: AtomicBool,
        drift_policy_on_fetch: AtomicBool,
        qualification_calls: AtomicUsize,
        resolve_calls: AtomicUsize,
    }
    impl QualifiableTransport {
        fn new(identity: GatewayComplianceFeedTransportIdentityV1) -> Self {
            Self {
                identity: Mutex::new(identity),
                probe_available: AtomicBool::new(true),
                drift_revision_on_resolve: AtomicBool::new(false),
                drift_revision_on_fetch: AtomicBool::new(false),
                drift_policy_on_fetch: AtomicBool::new(false),
                qualification_calls: AtomicUsize::new(0),
                resolve_calls: AtomicUsize::new(0),
            }
        }
        fn expected() -> Self {
            Self::new(
                config()
                    .expected_feed_transport_identity()
                    .expect("expected feed transport identity"),
            )
        }
    }
    impl GatewayComplianceFeedTransport for QualifiableTransport {
        fn qualification(
            &self,
        ) -> Result<
            GatewayComplianceFeedTransportIdentityV1,
            GatewayComplianceFeedTransportProbeError,
        > {
            self.qualification_calls
                .fetch_add(1, TestAtomicOrdering::SeqCst);
            if !self.probe_available.load(TestAtomicOrdering::SeqCst) {
                return Err(GatewayComplianceFeedTransportProbeError);
            }
            Ok(self
                .identity
                .lock()
                .expect("transport identity lock")
                .clone())
        }
        fn resolve(
            &self,
            _hostname: &str,
            _timeout: Duration,
        ) -> Result<Vec<IpAddr>, GatewayComplianceError> {
            self.resolve_calls.fetch_add(1, TestAtomicOrdering::SeqCst);
            if self
                .drift_revision_on_resolve
                .swap(false, TestAtomicOrdering::SeqCst)
            {
                self.identity
                    .lock()
                    .expect("transport identity lock")
                    .revision += 1;
            }
            Ok(vec!["93.184.216.34".parse().expect("public IP")])
        }
        fn fetch(
            &self,
            _request: &GatewayComplianceFetchRequest,
        ) -> Result<GatewayComplianceFetchResponse, GatewayComplianceError> {
            if self
                .drift_revision_on_fetch
                .swap(false, TestAtomicOrdering::SeqCst)
            {
                self.identity
                    .lock()
                    .expect("transport identity lock")
                    .revision += 1;
            }
            if self
                .drift_policy_on_fetch
                .swap(false, TestAtomicOrdering::SeqCst)
            {
                self.identity
                    .lock()
                    .expect("transport identity lock")
                    .policy_digest[0] ^= 0xFF;
            }
            Ok(fetch_response(Vec::new()))
        }
    }
    #[derive(Debug, Default)]
    struct UnexpectedStore {
        acquire_calls: AtomicUsize,
    }
    impl GatewayComplianceStore for UnexpectedStore {
        fn try_acquire(
            &self,
        ) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError> {
            self.acquire_calls.fetch_add(1, TestAtomicOrdering::SeqCst);
            Err(GatewayComplianceError::Persistence(
                "checkpoint store must not be accessed".into(),
            ))
        }
    }
    #[test]
    fn cross_role_signer_reuse_fails_before_provider_or_store_access() {
        let mut reused_id = config();
        reused_id.trust_policy.gateway_signers[0].signer_id =
            reused_id.trust_policy.catalog_signers[0].signer_id.clone();
        let mut reused_key = config();
        reused_key.trust_policy.gateway_signers[0].public_key =
            reused_key.trust_policy.catalog_signers[0].public_key;
        for (label, config) in [
            ("signer identifier", reused_id),
            ("Ed25519 public key", reused_key),
        ] {
            let transport = QualifiableTransport::expected();
            let store = Arc::new(UnexpectedStore::default());
            let error = GatewayComplianceController::new_with_feed_transport(
                config,
                store.clone(),
                &transport,
            )
            .expect_err("cross-role signer reuse must fail startup");
            assert!(
                matches!(&error, GatewayComplianceError::InvalidPolicy(message)
                    if message.contains("administratively disjoint")),
                "unexpected {label} reuse error: {error}"
            );
            assert_eq!(
                transport
                    .qualification_calls
                    .load(TestAtomicOrdering::SeqCst),
                0,
                "{label} reuse must fail before provider qualification"
            );
            assert_eq!(
                store.acquire_calls.load(TestAtomicOrdering::SeqCst),
                0,
                "{label} reuse must fail before checkpoint access"
            );
        }
    }
    #[test]
    fn feed_transport_qualification_rejects_bad_providers_before_store_access() {
        let cases = [
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.provider_handle = "sorafs.gateway.compliance.other.v1".into();
                    identity
                },
                GatewayComplianceError::FeedTransportSubstituted,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.revision += 1;
                    identity
                },
                GatewayComplianceError::FeedTransportStale,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.policy_digest[0] ^= 0xFF;
                    identity
                },
                GatewayComplianceError::FeedTransportSubstituted,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.test_marked = true;
                    identity
                },
                GatewayComplianceError::FeedTransportTestMarked,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.provider_handle = "sorafs.gateway.compliance.feed-dummy.v1".into();
                    identity
                },
                GatewayComplianceError::FeedTransportTestMarked,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.provider_handle = "sorafs gateway compliance feed".into();
                    identity
                },
                GatewayComplianceError::FeedTransportUnqualified,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.revision = 0;
                    identity
                },
                GatewayComplianceError::FeedTransportStale,
            ),
            (
                {
                    let mut identity = config()
                        .expected_feed_transport_identity()
                        .expect("expected identity");
                    identity.policy_digest = [0; 32];
                    identity
                },
                GatewayComplianceError::FeedTransportUnqualified,
            ),
        ];
        for (identity, expected_error) in cases {
            let transport = QualifiableTransport::new(identity);
            let store = Arc::new(UnexpectedStore::default());
            let error = GatewayComplianceController::new_with_feed_transport(
                config(),
                store.clone(),
                &transport,
            )
            .expect_err("bad provider must fail startup");
            assert_eq!(
                std::mem::discriminant(&error),
                std::mem::discriminant(&expected_error)
            );
            assert_eq!(
                store.acquire_calls.load(TestAtomicOrdering::SeqCst),
                0,
                "provider qualification must precede checkpoint access"
            );
            assert_eq!(
                transport.resolve_calls.load(TestAtomicOrdering::SeqCst),
                0,
                "provider qualification must not perform network access"
            );
        }
    }
    #[test]
    fn feed_transport_probe_failure_is_redacted_before_store_access() {
        let transport = QualifiableTransport::expected();
        transport
            .probe_available
            .store(false, TestAtomicOrdering::SeqCst);
        let store = Arc::new(UnexpectedStore::default());
        let error = GatewayComplianceController::new_with_feed_transport(
            config(),
            store.clone(),
            &transport,
        )
        .expect_err("unavailable provider must fail startup");
        assert!(matches!(
            &error,
            GatewayComplianceError::FeedTransportUnavailable
        ));
        assert_eq!(
            error.to_string(),
            "gateway compliance feed transport is unavailable"
        );
        assert_eq!(store.acquire_calls.load(TestAtomicOrdering::SeqCst), 0);
    }
    #[test]
    fn feed_transport_operation_diagnostics_are_redacted() {
        let error = redact_feed_transport_operation_error(GatewayComplianceError::InvalidFeed(
            "PRIVATE provider credential diagnostic".into(),
        ));
        assert!(matches!(
            &error,
            GatewayComplianceError::FeedTransportOperationFailed
        ));
        assert_eq!(
            error.to_string(),
            "gateway compliance feed transport operation failed"
        );
        assert!(!error.to_string().contains("PRIVATE"));
        assert!(matches!(
            redact_feed_transport_operation_error(GatewayComplianceError::FetchTimeout),
            GatewayComplianceError::FetchTimeout
        ));
    }
    #[test]
    fn external_feed_startup_rejects_missing_config_binding_before_store_access() {
        let transport = QualifiableTransport::expected();
        let store = Arc::new(UnexpectedStore::default());
        let mut unbound = config();
        unbound.feed_transport_provider = None;
        let error = GatewayComplianceController::new_with_feed_transport(
            unbound,
            store.clone(),
            &transport,
        )
        .expect_err("external feed startup requires one exact config binding");
        assert!(matches!(error, GatewayComplianceError::InvalidPolicy(_)));
        assert_eq!(store.acquire_calls.load(TestAtomicOrdering::SeqCst), 0);
        assert_eq!(
            transport
                .qualification_calls
                .load(TestAtomicOrdering::SeqCst),
            0,
            "configuration must fail before runtime-provider access"
        );
        assert_eq!(transport.resolve_calls.load(TestAtomicOrdering::SeqCst), 0);
    }
    #[test]
    fn feed_transport_identity_is_revalidated_before_each_network_use() {
        let transport = QualifiableTransport::expected();
        let controller = GatewayComplianceController::new_with_feed_transport(
            config(),
            Arc::new(MemoryStore::default()),
            &transport,
        )
        .expect("qualified controller");
        assert_eq!(
            transport
                .qualification_calls
                .load(TestAtomicOrdering::SeqCst),
            1
        );
        transport
            .identity
            .lock()
            .expect("transport identity lock")
            .revision += 1;
        assert!(matches!(
            controller.fetch_feed("baseline", &transport),
            Err(GatewayComplianceError::FeedTransportStale)
        ));
        assert_eq!(
            transport
                .qualification_calls
                .load(TestAtomicOrdering::SeqCst),
            2
        );
        assert_eq!(
            transport.resolve_calls.load(TestAtomicOrdering::SeqCst),
            0,
            "stale providers must be rejected before DNS or HTTP"
        );
    }
    #[test]
    fn feed_transport_drift_during_network_io_discards_response_bytes() {
        for (drift_policy, expected_error) in [
            (false, GatewayComplianceError::FeedTransportStale),
            (true, GatewayComplianceError::FeedTransportSubstituted),
        ] {
            let transport = QualifiableTransport::expected();
            let controller = GatewayComplianceController::new_with_feed_transport(
                config(),
                Arc::new(MemoryStore::default()),
                &transport,
            )
            .expect("qualified controller");
            if drift_policy {
                transport
                    .drift_policy_on_fetch
                    .store(true, TestAtomicOrdering::SeqCst);
            } else {
                transport
                    .drift_revision_on_fetch
                    .store(true, TestAtomicOrdering::SeqCst);
            }
            let error = controller
                .fetch_feed("baseline", &transport)
                .expect_err("provider drift must discard fetched bytes");
            assert_eq!(
                std::mem::discriminant(&error),
                std::mem::discriminant(&expected_error)
            );
            assert_eq!(
                transport
                    .qualification_calls
                    .load(TestAtomicOrdering::SeqCst),
                6,
                "startup plus outer, DNS, and fetch operation fences must be checked"
            );
            assert_eq!(
                transport.resolve_calls.load(TestAtomicOrdering::SeqCst),
                1,
                "fetch drift must be rejected before DNS revalidation"
            );
        }
    }
    #[test]
    fn feed_transport_drift_during_dns_discards_addresses_before_http() {
        let transport = QualifiableTransport::expected();
        let controller = GatewayComplianceController::new_with_feed_transport(
            config(),
            Arc::new(MemoryStore::default()),
            &transport,
        )
        .expect("qualified controller");
        transport
            .drift_revision_on_resolve
            .store(true, TestAtomicOrdering::SeqCst);
        assert!(matches!(
            controller.fetch_feed("baseline", &transport),
            Err(GatewayComplianceError::FeedTransportStale)
        ));
        assert_eq!(
            transport
                .qualification_calls
                .load(TestAtomicOrdering::SeqCst),
            4,
            "startup, outer preflight, and DNS pre/post identities must be checked"
        );
        assert_eq!(transport.resolve_calls.load(TestAtomicOrdering::SeqCst), 1);
    }
    #[test]
    fn unbound_controller_cannot_reach_a_feed_transport() {
        let transport = QualifiableTransport::expected();
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller core");
        assert!(matches!(
            controller.fetch_feed("baseline", &transport),
            Err(GatewayComplianceError::FeedTransportNotQualified)
        ));
        assert_eq!(
            transport
                .qualification_calls
                .load(TestAtomicOrdering::SeqCst),
            0
        );
        assert_eq!(transport.resolve_calls.load(TestAtomicOrdering::SeqCst), 0);
    }
    fn mutation_binding(nonce: u8) -> GatewayComplianceMutationBindingV1 {
        assert_ne!(nonce, 0, "test idempotency key must be non-zero");
        GatewayComplianceMutationBindingV1 {
            key_digest: [nonce; 32],
            request_digest: [nonce.wrapping_add(1); 32],
        }
    }
    fn indexed_mutation_binding(index: u64) -> GatewayComplianceMutationBindingV1 {
        let mut key_digest = [0xA1; 32];
        key_digest[..8].copy_from_slice(&index.to_be_bytes());
        let mut request_digest = [0xB2; 32];
        request_digest[..8].copy_from_slice(&index.to_be_bytes());
        GatewayComplianceMutationBindingV1 {
            key_digest,
            request_digest,
        }
    }
    fn subject(byte: u8) -> String {
        hex::encode([byte; 32])
    }
    fn payload(
        sequence: u64,
        predecessor_digest: Option<[u8; 32]>,
    ) -> GatewayComplianceCatalogPayloadV1 {
        GatewayComplianceCatalogPayloadV1 {
            version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
            sequence,
            predecessor_digest,
            policy_digest: trust_policy().canonical_digest().expect("policy digest"),
            generated_at_unix: NOW,
            valid_until_unix: NOW + 3_600,
            source_anchors: vec![GatewayComplianceSourceAnchorV1 {
                feed_id: "baseline".into(),
                feed_digest: [0x91; 32],
                generated_at_unix: NOW,
            }],
            baseline_rules: Vec::new(),
            appeal_overrides: Vec::new(),
            legal_safety_holds: Vec::new(),
            toggles: Vec::new(),
        }
    }
    fn sign_catalog(payload: GatewayComplianceCatalogPayloadV1) -> GatewayComplianceCatalogV1 {
        let payload = payload.normalize().expect("normalize catalog");
        let digest = payload.signing_digest().expect("catalog signing digest");
        let keys = catalog_keys();
        GatewayComplianceCatalogV1 {
            payload,
            approvals: vec![
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-a".into(),
                    signature: keys[0].sign(&digest).to_bytes(),
                },
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-b".into(),
                    signature: keys[1].sign(&digest).to_bytes(),
                },
            ],
        }
    }
    fn acknowledgement(
        gateway_index: usize,
        catalog_digest: [u8; 32],
        accepted: bool,
    ) -> GatewayComplianceAcknowledgementV1 {
        acknowledgement_at(gateway_index, catalog_digest, accepted, NOW + 10)
    }
    fn acknowledgement_at(
        gateway_index: usize,
        catalog_digest: [u8; 32],
        accepted: bool,
        observed_at_unix: u64,
    ) -> GatewayComplianceAcknowledgementV1 {
        let gateway_id = if gateway_index == 0 {
            "gateway-eu"
        } else {
            "gateway-us"
        };
        let payload = GatewayComplianceAcknowledgementPayloadV1 {
            version: GATEWAY_COMPLIANCE_ACK_VERSION_V1,
            gateway_id: gateway_id.into(),
            catalog_digest,
            observed_at_unix,
            accepted,
            rejection_code: (!accepted).then(|| "reload-failed".into()),
        };
        let digest = hash_canonical(
            ACK_SIGNING_DOMAIN_V1,
            &payload,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
        .expect("ack digest");
        GatewayComplianceAcknowledgementV1 {
            payload,
            signature: gateway_keys()[gateway_index].sign(&digest).to_bytes(),
        }
    }
    fn rollback_authorization(
        operation_id: [u8; 32],
        from_catalog_digest: [u8; 32],
        to_catalog_digest: [u8; 32],
        authorized_at_unix: u64,
    ) -> GatewayComplianceRollbackV1 {
        let payload = GatewayComplianceRollbackPayloadV1 {
            version: GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1,
            operation_id,
            from_catalog_digest,
            to_catalog_digest,
            reason_code: "bad-feed".into(),
            authorized_at_unix,
        };
        let digest = hash_canonical(
            ROLLBACK_SIGNING_DOMAIN_V1,
            &payload,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
        .expect("rollback digest");
        let keys = catalog_keys();
        GatewayComplianceRollbackV1 {
            payload,
            approvals: vec![
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-a".into(),
                    signature: keys[0].sign(&digest).to_bytes(),
                },
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-b".into(),
                    signature: keys[1].sign(&digest).to_bytes(),
                },
            ],
        }
    }
    fn promote(
        controller: &GatewayComplianceController,
        catalog: GatewayComplianceCatalogV1,
    ) -> [u8; 32] {
        let sequence = catalog.payload.sequence;
        let offset = sequence
            .checked_sub(1)
            .and_then(|value| value.checked_mul(30))
            .expect("test sequence offset");
        let base_nonce = u8::try_from(sequence.checked_mul(8).expect("test mutation nonce"))
            .expect("test mutation nonce fits");
        let digest = controller
            .stage_catalog(catalog, NOW + offset + 5, mutation_binding(base_nonce))
            .expect("stage catalog")
            .catalog_digest;
        controller
            .acknowledge(
                acknowledgement_at(0, digest, true, NOW + offset + 10),
                NOW + offset + 10,
                mutation_binding(base_nonce + 1),
            )
            .expect("first acknowledgement");
        controller
            .acknowledge(
                acknowledgement_at(1, digest, true, NOW + offset + 10),
                NOW + offset + 10,
                mutation_binding(base_nonce + 2),
            )
            .expect("second acknowledgement");
        controller
            .promote(
                digest,
                sequence,
                NOW + offset + 20,
                mutation_binding(base_nonce + 3),
            )
            .expect("promote catalog")
            .catalog_digest
    }
    #[test]
    fn threshold_promotion_is_durable_and_predecessor_bound() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let first = sign_catalog(payload(1, None));
        let first_digest = controller
            .stage_catalog(first.clone(), NOW + 5, mutation_binding(1))
            .expect("stage first")
            .catalog_digest;
        controller
            .acknowledge(
                acknowledgement(0, first_digest, true),
                NOW + 10,
                mutation_binding(2),
            )
            .expect("ack");
        assert!(matches!(
            controller.promote(first_digest, 1, NOW + 20, mutation_binding(3)),
            Err(GatewayComplianceError::GatewayQuorumNotMet { .. })
        ));
        controller
            .acknowledge(
                acknowledgement(1, first_digest, true),
                NOW + 10,
                mutation_binding(4),
            )
            .expect("ack");
        assert_eq!(
            controller
                .promote(first_digest, 1, NOW + 20, mutation_binding(5))
                .expect("promote")
                .catalog_digest,
            first_digest
        );
        assert_eq!(controller.checkpoint().expect("checkpoint").revision, 4);
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover checkpoint");
        assert_eq!(
            recovered
                .checkpoint()
                .expect("checkpoint")
                .serving
                .expect("serving")
                .payload
                .catalog_digest()
                .expect("digest"),
            first_digest
        );
        let wrong_successor = sign_catalog(payload(2, Some([0xFF; 32])));
        assert!(matches!(
            recovered.stage_catalog(wrong_successor, NOW + 30, mutation_binding(6)),
            Err(GatewayComplianceError::InvalidPredecessor)
        ));
    }
    #[test]
    fn exact_mutation_replays_survive_promotion_expiry_and_restart() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let stage_binding = indexed_mutation_binding(1);
        let first_ack_binding = indexed_mutation_binding(2);
        let second_ack_binding = indexed_mutation_binding(3);
        let promote_binding = indexed_mutation_binding(4);
        let staged = controller
            .stage_catalog(catalog.clone(), NOW + 5, stage_binding)
            .expect("stage");
        let first_ack = acknowledgement(0, staged.catalog_digest, true);
        let second_ack = acknowledgement(1, staged.catalog_digest, true);
        let first_ack_result = controller
            .acknowledge(first_ack.clone(), NOW + 10, first_ack_binding)
            .expect("first acknowledgement");
        controller
            .acknowledge(second_ack, NOW + 10, second_ack_binding)
            .expect("second acknowledgement");
        let promoted = controller
            .promote(staged.catalog_digest, 1, NOW + 20, promote_binding)
            .expect("promote");
        assert_eq!(controller.checkpoint().expect("checkpoint").revision, 4);
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover checkpoint");
        assert_eq!(
            recovered
                .stage_catalog(catalog, NOW + 7_200, stage_binding)
                .expect("expired exact stage replay"),
            staged
        );
        assert_eq!(
            recovered
                .acknowledge(first_ack, NOW + 7_200, first_ack_binding)
                .expect("expired exact acknowledgement replay"),
            first_ack_result
        );
        assert_eq!(
            recovered
                .promote(staged.catalog_digest, 1, NOW + 7_200, promote_binding,)
                .expect("expired exact promotion replay"),
            promoted
        );
        assert_eq!(
            recovered
                .checkpoint()
                .expect("checkpoint after replays")
                .revision,
            4,
            "exact replays must not advance the durable revision"
        );
    }
    #[test]
    fn new_keys_for_identical_stage_and_acknowledgement_commit_distinct_replay_records() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let first = controller
            .stage_catalog(catalog.clone(), NOW + 5, indexed_mutation_binding(1))
            .expect("first stage");
        let second = controller
            .stage_catalog(catalog, NOW + 6, indexed_mutation_binding(2))
            .expect("idempotent stage under a new key");
        assert_eq!(first.catalog_digest, second.catalog_digest);
        let acknowledgement = acknowledgement(0, first.catalog_digest, true);
        controller
            .acknowledge(
                acknowledgement.clone(),
                NOW + 10,
                indexed_mutation_binding(3),
            )
            .expect("first acknowledgement");
        controller
            .acknowledge(acknowledgement, NOW + 11, indexed_mutation_binding(4))
            .expect("idempotent acknowledgement under a new key");
        let checkpoint = controller.checkpoint().expect("checkpoint");
        assert_eq!(checkpoint.acknowledgements.len(), 1);
        assert_eq!(checkpoint.idempotency_records.len(), 4);
        assert_eq!(checkpoint.revision, 4);
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover checkpoint");
        assert_eq!(
            recovered
                .checkpoint()
                .expect("recovered checkpoint")
                .idempotency_records
                .len(),
            4
        );
    }
    #[test]
    fn idempotency_key_substitution_and_cross_action_reuse_fail_closed() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let binding = indexed_mutation_binding(1);
        let staged = controller
            .stage_catalog(catalog.clone(), NOW + 5, binding)
            .expect("stage");
        let changed_request = GatewayComplianceMutationBindingV1 {
            key_digest: binding.key_digest,
            request_digest: [0xEF; 32],
        };
        assert!(matches!(
            controller.stage_catalog(catalog, NOW + 6, changed_request),
            Err(GatewayComplianceError::IdempotencyConflict)
        ));
        assert!(matches!(
            controller.acknowledge(
                acknowledgement(0, staged.catalog_digest, true),
                NOW + 10,
                binding,
            ),
            Err(GatewayComplianceError::IdempotencyConflict)
        ));
        assert_eq!(
            controller
                .checkpoint()
                .expect("checkpoint")
                .idempotency_records
                .len(),
            1
        );
    }
    #[test]
    fn promotion_expectation_failure_does_not_consume_the_operation_key() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let digest = controller
            .stage_catalog(catalog, NOW + 5, indexed_mutation_binding(1))
            .expect("stage")
            .catalog_digest;
        for gateway_index in 0..2 {
            controller
                .acknowledge(
                    acknowledgement(gateway_index, digest, true),
                    NOW + 10,
                    indexed_mutation_binding(2 + gateway_index as u64),
                )
                .expect("acknowledge");
        }
        let promotion_binding = indexed_mutation_binding(4);
        assert!(matches!(
            controller.promote([0xFF; 32], 1, NOW + 20, promotion_binding),
            Err(GatewayComplianceError::PromotionTargetMismatch)
        ));
        assert_eq!(
            controller
                .promote(digest, 1, NOW + 20, promotion_binding)
                .expect("corrected promotion")
                .catalog_digest,
            digest
        );
    }
    #[test]
    fn crash_before_durable_replace_commits_neither_state_nor_replay_binding() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let binding = indexed_mutation_binding(1);
        store.fail_next_store();
        assert!(matches!(
            controller.stage_catalog(catalog.clone(), NOW + 5, binding),
            Err(GatewayComplianceError::Persistence(_))
        ));
        let checkpoint = controller.checkpoint().expect("in-memory checkpoint");
        assert!(checkpoint.candidate.is_none());
        assert!(checkpoint.idempotency_records.is_empty());
        assert_eq!(checkpoint.revision, 0);
        assert!(store.durable_bytes().is_none());
        assert_eq!(
            controller
                .stage_catalog(catalog, NOW + 5, binding)
                .expect("retry after failed store")
                .recorded_at_unix,
            NOW + 5
        );
        assert_eq!(controller.checkpoint().expect("checkpoint").revision, 1);
    }
    #[test]
    fn memory_store_allows_one_controller_lease_until_drop() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("first controller");
        assert!(matches!(
            GatewayComplianceController::new(config(), store.clone()),
            Err(GatewayComplianceError::LeaseHeld)
        ));
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("lease after controller drop");
        assert_eq!(recovered.checkpoint().expect("checkpoint").revision, 0);
    }
    #[test]
    fn stale_generation_fences_controller_without_overwriting_durable_bytes() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        controller
            .stage_catalog(catalog.clone(), NOW + 5, indexed_mutation_binding(1))
            .expect("first durable stage");
        let poisoned = b"out-of-band-checkpoint-replacement".to_vec();
        store.force_store(poisoned.clone());
        assert!(matches!(
            controller.stage_catalog(catalog, NOW + 6, indexed_mutation_binding(2)),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert!(matches!(
            controller.checkpoint(),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert_eq!(store.durable_bytes(), Some(poisoned));
    }
    #[test]
    fn crash_after_durable_replace_recovers_exactly_once_and_fences_old_controller() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let binding = indexed_mutation_binding(1);
        let catalog_digest = catalog.payload.catalog_digest().expect("catalog digest");
        store.fail_after_store();
        assert!(matches!(
            controller.stage_catalog(catalog.clone(), NOW + 5, binding),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert!(matches!(
            controller.checkpoint(),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        let durable = decode_checkpoint(
            &store
                .durable_bytes()
                .expect("replacement reached durable memory store"),
        )
        .expect("durable checkpoint");
        assert_eq!(durable.revision, 1);
        assert_eq!(durable.idempotency_records.len(), 1);
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover durable replacement");
        assert_eq!(
            recovered
                .stage_catalog(catalog, NOW + 7_200, binding)
                .expect("exact replay after indeterminate response"),
            GatewayComplianceMutationResultV1 {
                catalog_digest,
                recorded_at_unix: NOW + 5,
            }
        );
        assert_eq!(recovered.checkpoint().expect("checkpoint").revision, 1);
    }
    #[test]
    fn full_idempotency_registry_replays_known_keys_and_rejects_new_keys() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        let digest = controller
            .stage_catalog(catalog.clone(), NOW + 5, indexed_mutation_binding(1))
            .expect("stage")
            .catalog_digest;
        let mut checkpoint = controller.checkpoint().expect("checkpoint");
        checkpoint.idempotency_records =
            (1..=u64::try_from(MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1)
                .expect("registry bound fits u64"))
                .map(|index| {
                    let binding = indexed_mutation_binding(index);
                    GatewayComplianceIdempotencyRecordV1 {
                        key_digest: binding.key_digest,
                        request_digest: binding.request_digest,
                        operation: GatewayComplianceMutationKindV1::Stage,
                        catalog_digest: digest,
                        recorded_at_unix: NOW + 5,
                    }
                })
                .collect();
        checkpoint.revision = u64::try_from(MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1)
            .expect("registry bound fits u64");
        let bytes = encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)
            .expect("encode full registry");
        drop(controller);
        store.force_store(bytes);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover full registry");
        assert_eq!(
            recovered
                .stage_catalog(catalog.clone(), NOW + 7_200, indexed_mutation_binding(1),)
                .expect("known key replay"),
            GatewayComplianceMutationResultV1 {
                catalog_digest: digest,
                recorded_at_unix: NOW + 5,
            }
        );
        assert!(matches!(
            recovered.stage_catalog(
                catalog,
                NOW + 6,
                indexed_mutation_binding(
                    u64::try_from(MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1)
                        .expect("registry bound fits u64")
                        + 1,
                ),
            ),
            Err(GatewayComplianceError::IdempotencyRegistryFull)
        ));
        assert_eq!(
            recovered
                .checkpoint()
                .expect("checkpoint")
                .idempotency_records
                .len(),
            MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1
        );
    }
    #[test]
    fn checkpoint_rejects_duplicate_idempotency_keys() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        controller
            .stage_catalog(
                sign_catalog(payload(1, None)),
                NOW + 5,
                indexed_mutation_binding(1),
            )
            .expect("stage");
        let mut checkpoint = controller.checkpoint().expect("checkpoint");
        checkpoint
            .idempotency_records
            .push(checkpoint.idempotency_records[0].clone());
        checkpoint.revision = checkpoint
            .revision
            .checked_add(1)
            .expect("test checkpoint revision");
        let bytes = encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)
            .expect("encode poisoned checkpoint");
        drop(controller);
        store.force_store(bytes);
        assert!(matches!(
            GatewayComplianceController::new(config(), store),
            Err(GatewayComplianceError::InvalidCheckpoint(_))
        ));
    }
    #[test]
    fn checkpoint_rejects_revision_rollback_and_truncated_bytes_on_restart() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        controller
            .stage_catalog(
                sign_catalog(payload(1, None)),
                NOW + 5,
                indexed_mutation_binding(1),
            )
            .expect("stage");
        let mut checkpoint = controller.checkpoint().expect("checkpoint");
        assert_eq!(checkpoint.revision, 1);
        checkpoint.revision = 0;
        let rolled_back_revision =
            encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)
                .expect("encode revision rollback");
        drop(controller);
        store.force_store(rolled_back_revision);
        assert!(matches!(
            GatewayComplianceController::new(config(), store.clone()),
            Err(GatewayComplianceError::InvalidCheckpoint(_))
        ));
        store.force_store(vec![0x4E, 0x52, 0x54]);
        assert!(matches!(
            GatewayComplianceController::new(config(), store),
            Err(GatewayComplianceError::InvalidCheckpoint(_))
        ));
    }
    #[test]
    fn signature_substitution_and_duplicate_quorum_fail_closed() {
        let policy = trust_policy();
        let mut catalog = sign_catalog(payload(1, None));
        catalog.payload.valid_until_unix += 1;
        assert!(matches!(
            catalog.verify(&policy, NOW + 1, 300),
            Err(GatewayComplianceError::InvalidSignature { .. })
        ));
        let mut duplicate = sign_catalog(payload(1, None));
        duplicate.approvals[1] = duplicate.approvals[0].clone();
        assert!(matches!(
            duplicate.verify(&policy, NOW + 1, 300),
            Err(GatewayComplianceError::DuplicateSigner(_))
        ));
    }
    #[test]
    fn catalog_rejects_stale_and_future_source_anchors() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let mut stale = payload(1, None);
        stale.source_anchors[0].generated_at_unix = NOW - 3_601;
        assert!(matches!(
            controller.stage_catalog(sign_catalog(stale), NOW + 5, mutation_binding(1)),
            Err(GatewayComplianceError::InvalidCatalog(_))
        ));
        let mut future = payload(1, None);
        future.source_anchors[0].generated_at_unix = NOW + 301;
        assert!(matches!(
            controller.stage_catalog(sign_catalog(future), NOW + 5, mutation_binding(2)),
            Err(GatewayComplianceError::InvalidCatalog(_))
        ));
        let mut excessive_validity = payload(1, None);
        excessive_validity.valid_until_unix = NOW + 7_201;
        assert!(matches!(
            controller.stage_catalog(
                sign_catalog(excessive_validity),
                NOW + 5,
                mutation_binding(3),
            ),
            Err(GatewayComplianceError::InvalidCatalog(_))
        ));
    }
    #[test]
    fn serving_rejects_zero_rolled_back_and_expired_clocks() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        promote(&controller, sign_catalog(payload(1, None)));
        for observed_at_unix in [0, NOW - 301, NOW + 3_600] {
            assert!(matches!(
                controller.evaluate_serving(
                    GatewayComplianceSubjectKindV1::ManifestDigest,
                    &subject(1),
                    observed_at_unix,
                ),
                Err(GatewayComplianceError::CatalogNotFresh)
            ));
        }
    }
    #[test]
    fn promotion_revalidates_acknowledgement_freshness() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let digest = controller
            .stage_catalog(sign_catalog(payload(1, None)), NOW + 5, mutation_binding(1))
            .expect("stage")
            .catalog_digest;
        for gateway_index in 0..2 {
            controller
                .acknowledge(
                    acknowledgement_at(gateway_index, digest, true, NOW + 10),
                    NOW + 10,
                    mutation_binding(2 + gateway_index as u8),
                )
                .expect("acknowledge");
        }
        assert!(matches!(
            controller.promote(digest, 1, NOW + 311, mutation_binding(4)),
            Err(GatewayComplianceError::InvalidAcknowledgement(_))
        ));
        assert!(
            controller
                .checkpoint()
                .expect("checkpoint")
                .serving
                .is_none()
        );
    }
    #[test]
    fn every_mutation_rejects_clock_rollback_before_state_change() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let first_digest = controller
            .stage_catalog(sign_catalog(payload(1, None)), NOW + 5, mutation_binding(1))
            .expect("stage first")
            .catalog_digest;
        for gateway_index in 0..2 {
            controller
                .acknowledge(
                    acknowledgement_at(gateway_index, first_digest, true, NOW + 10),
                    NOW + 10,
                    mutation_binding(2 + gateway_index as u8),
                )
                .expect("acknowledge first");
        }
        controller
            .promote(first_digest, 1, NOW + 100, mutation_binding(4))
            .expect("promote first");
        let mut second = payload(2, Some(first_digest));
        second.generated_at_unix = NOW + 40;
        second.valid_until_unix = NOW + 1_000;
        assert!(matches!(
            controller.stage_catalog(sign_catalog(second), NOW + 45, mutation_binding(5)),
            Err(GatewayComplianceError::MutationTimeInvalid)
        ));
        assert_eq!(
            controller
                .checkpoint()
                .expect("checkpoint")
                .chain_head
                .expect("chain head")
                .payload
                .catalog_digest()
                .expect("digest"),
            first_digest
        );
    }
    #[test]
    fn cid_subjects_require_canonical_lowercase_base32_round_trip() {
        let canonical = "bafyr6iffuws2ljnfuws2ljnfuws2ljnfuws2ljnfuws2ljnfuws2ljnfuu";
        assert_eq!(
            normalize_subject(GatewayComplianceSubjectKindV1::Cid, canonical)
                .expect("canonical CID"),
            canonical
        );
        for malformed in [
            "",
            "b",
            "Bafyr6iffuws2ljnfuws2ljnfuws2ljnfuws2ljnfuws2ljnfuws2ljnfuu",
            "ba0",
            "ba1",
            "ba8",
            "ba9",
            "ba",
            "b=",
        ] {
            assert!(
                normalize_subject(GatewayComplianceSubjectKindV1::Cid, malformed).is_err(),
                "malformed CID unexpectedly admitted: {malformed}"
            );
        }
    }
    #[test]
    fn controller_config_rejects_unbounded_fetch_and_freshness_windows() {
        let mut redirects = config();
        redirects.fetch_limits.max_redirects = 9;
        assert!(matches!(
            redirects.validate(),
            Err(GatewayComplianceError::InvalidPolicy(_))
        ));
        let mut timeout = config();
        timeout.fetch_limits.total_timeout = Duration::from_secs(121);
        assert!(matches!(
            timeout.validate(),
            Err(GatewayComplianceError::InvalidPolicy(_))
        ));
        let mut freshness = config();
        freshness.max_feed_age_secs = 0;
        assert!(matches!(
            freshness.validate(),
            Err(GatewayComplianceError::InvalidPolicy(_))
        ));
        let mut unknown_gateway = config();
        unknown_gateway.gateway_scope = "gateway:gateway-unknown".into();
        assert!(matches!(
            unknown_gateway.validate(),
            Err(GatewayComplianceError::InvalidPolicy(_))
        ));
        let mut malformed_region = config();
        malformed_region.region_scope = "gateway:gateway-eu".into();
        assert!(matches!(
            malformed_region.validate(),
            Err(GatewayComplianceError::InvalidPolicy(_))
        ));
    }
    #[test]
    fn serving_evaluation_fails_closed_without_a_promoted_catalog() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        assert!(matches!(
            controller.evaluate_serving(
                GatewayComplianceSubjectKindV1::ManifestDigest,
                &subject(1),
                NOW
            ),
            Err(GatewayComplianceError::NoServingCatalog)
        ));
    }
    #[test]
    fn allow_all_test_controller_serves_from_a_governed_catalog() {
        let controller = allow_all_gateway_compliance_controller_for_tests();
        let observed_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_secs();
        let decision = controller
            .evaluate_serving(
                GatewayComplianceSubjectKindV1::ManifestDigest,
                &subject(0xA4),
                observed_at_unix,
            )
            .expect("allow-all decision");
        assert_eq!(decision.source, GatewayComplianceDecisionSource::NoMatch);
        assert_eq!(decision.disposition, GatewayComplianceDisposition::Allow);
        assert!(decision.catalog_digest.is_some());
        let checkpoint = controller.checkpoint().expect("test checkpoint");
        let serving = checkpoint.serving.as_ref().expect("serving catalog");
        assert_eq!(decision.catalog_sequence, serving.payload.sequence);
        assert_eq!(
            decision.catalog_valid_until_unix,
            serving.payload.valid_until_unix
        );
    }
    #[test]
    fn serving_precedence_spans_global_region_and_gateway_scopes() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let mut candidate = payload(1, None);
        candidate
            .baseline_rules
            .push(GatewayComplianceBaselineRuleV1 {
                rule_id: "global-baseline".into(),
                scope: "global".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(9),
                source_id: "baseline".into(),
                reason_code: "policy-deny".into(),
                toggle_id: None,
                effective_from_unix: NOW,
                expires_at_unix: Some(NOW + 1_000),
            });
        candidate
            .appeal_overrides
            .push(GatewayComplianceAppealOverrideV1 {
                appeal_id: "regional-appeal".into(),
                scope: "region:eu".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(9),
                decision_digest: [0x69; 32],
                effective_from_unix: NOW,
                expires_at_unix: NOW + 1_000,
            });
        candidate
            .baseline_rules
            .push(GatewayComplianceBaselineRuleV1 {
                rule_id: "global-baseline-held".into(),
                scope: "global".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(10),
                source_id: "baseline".into(),
                reason_code: "policy-deny".into(),
                toggle_id: None,
                effective_from_unix: NOW,
                expires_at_unix: Some(NOW + 1_000),
            });
        candidate
            .appeal_overrides
            .push(GatewayComplianceAppealOverrideV1 {
                appeal_id: "regional-appeal-held".into(),
                scope: "region:eu".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(10),
                decision_digest: [0x6A; 32],
                effective_from_unix: NOW,
                expires_at_unix: NOW + 1_000,
            });
        candidate
            .legal_safety_holds
            .push(GatewayComplianceLegalSafetyHoldV1 {
                hold_id: "gateway-hold".into(),
                scope: "gateway:gateway-eu".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(10),
                authority_reference: "court-order-10".into(),
                effective_from_unix: NOW,
                expires_at_unix: Some(NOW + 1_000),
            });
        promote(&controller, sign_catalog(candidate));
        assert_eq!(
            controller
                .evaluate_serving(
                    GatewayComplianceSubjectKindV1::ManifestDigest,
                    &subject(9),
                    NOW + 30,
                )
                .expect("regional appeal decision")
                .source,
            GatewayComplianceDecisionSource::AcceptedAppeal
        );
        assert_eq!(
            controller
                .evaluate_serving(
                    GatewayComplianceSubjectKindV1::ManifestDigest,
                    &subject(10),
                    NOW + 30,
                )
                .expect("gateway hold decision")
                .source,
            GatewayComplianceDecisionSource::LegalSafetyHold
        );
    }
    #[test]
    fn hold_then_appeal_then_baseline_precedence_is_deterministic() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let mut candidate = payload(1, None);
        for (id, byte) in [
            ("held", 1_u8),
            ("appealed", 2),
            ("baseline", 3),
            ("toggle", 4),
        ] {
            candidate
                .baseline_rules
                .push(GatewayComplianceBaselineRuleV1 {
                    rule_id: format!("rule-{id}"),
                    scope: "global".into(),
                    subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                    subject: subject(byte),
                    source_id: "baseline".into(),
                    reason_code: "policy-deny".into(),
                    toggle_id: (id == "toggle").then(|| "provider-deny".into()),
                    effective_from_unix: NOW,
                    expires_at_unix: Some(NOW + 1_000),
                });
        }
        candidate
            .appeal_overrides
            .push(GatewayComplianceAppealOverrideV1 {
                appeal_id: "appeal-held".into(),
                scope: "global".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(1),
                decision_digest: [0x61; 32],
                effective_from_unix: NOW,
                expires_at_unix: NOW + 1_000,
            });
        candidate
            .appeal_overrides
            .push(GatewayComplianceAppealOverrideV1 {
                appeal_id: "appeal-accepted".into(),
                scope: "global".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(2),
                decision_digest: [0x62; 32],
                effective_from_unix: NOW,
                expires_at_unix: NOW + 1_000,
            });
        candidate
            .legal_safety_holds
            .push(GatewayComplianceLegalSafetyHoldV1 {
                hold_id: "hold-safety".into(),
                scope: "global".into(),
                subject_kind: GatewayComplianceSubjectKindV1::ManifestDigest,
                subject: subject(1),
                authority_reference: "court-order-7".into(),
                effective_from_unix: NOW,
                expires_at_unix: Some(NOW + 1_000),
            });
        candidate.toggles.push(GatewayComplianceToggleV1 {
            toggle_id: "provider-deny".into(),
            scope: "global".into(),
            enabled: false,
            approval_reference: "governance-9".into(),
            effective_from_unix: NOW,
            expires_at_unix: NOW + 1_000,
        });
        promote(&controller, sign_catalog(candidate));
        let evaluate = |byte| {
            controller
                .evaluate(
                    "region:eu",
                    GatewayComplianceSubjectKindV1::ManifestDigest,
                    &subject(byte),
                    NOW + 30,
                )
                .expect("decision")
        };
        assert_eq!(
            evaluate(1).source,
            GatewayComplianceDecisionSource::LegalSafetyHold
        );
        assert_eq!(
            evaluate(2).source,
            GatewayComplianceDecisionSource::AcceptedAppeal
        );
        assert_eq!(
            evaluate(3).source,
            GatewayComplianceDecisionSource::Baseline
        );
        assert_eq!(evaluate(4).source, GatewayComplianceDecisionSource::NoMatch);
    }
    #[test]
    fn threshold_rollback_changes_serving_pointer_but_preserves_chain_head() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let first = sign_catalog(payload(1, None));
        let first_digest = promote(&controller, first);
        let second = sign_catalog(payload(2, Some(first_digest)));
        let second_digest = promote(&controller, second);
        let rollback_payload = GatewayComplianceRollbackPayloadV1 {
            version: GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1,
            operation_id: [0xC1; 32],
            from_catalog_digest: second_digest,
            to_catalog_digest: first_digest,
            reason_code: "bad-feed".into(),
            authorized_at_unix: NOW + 55,
        };
        let digest = hash_canonical(
            ROLLBACK_SIGNING_DOMAIN_V1,
            &rollback_payload,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
        .expect("rollback digest");
        let keys = catalog_keys();
        let rollback = GatewayComplianceRollbackV1 {
            payload: rollback_payload,
            approvals: vec![
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-a".into(),
                    signature: keys[0].sign(&digest).to_bytes(),
                },
                GatewayComplianceCatalogApprovalV1 {
                    version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                    signer_id: "council-b".into(),
                    signature: keys[1].sign(&digest).to_bytes(),
                },
            ],
        };
        let rollback_result = controller
            .rollback(&rollback, NOW + 55, mutation_binding(0xC1))
            .expect("rollback");
        assert_eq!(rollback_result.catalog_digest, first_digest);
        assert_eq!(rollback_result.recorded_at_unix, NOW + 55);
        let checkpoint = controller.checkpoint().expect("checkpoint");
        assert_eq!(
            checkpoint
                .serving
                .expect("serving")
                .payload
                .catalog_digest()
                .expect("digest"),
            first_digest
        );
        assert_eq!(
            checkpoint
                .chain_head
                .expect("chain head")
                .payload
                .catalog_digest()
                .expect("digest"),
            second_digest
        );
        assert!(matches!(
            controller.rollback(
                &rollback,
                NOW + 56,
                GatewayComplianceMutationBindingV1 {
                    key_digest: [0xC1; 32],
                    request_digest: [0xFE; 32],
                },
            ),
            Err(GatewayComplianceError::IdempotencyConflict)
        ));
        assert_eq!(
            controller
                .rollback(&rollback, NOW + 3_599, mutation_binding(0xC1))
                .expect("exact replay"),
            rollback_result
        );
        let revision = controller.checkpoint().expect("checkpoint").revision;
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("recover rollback state");
        assert_eq!(
            recovered
                .rollback(&rollback, NOW + 7_200, mutation_binding(0xC1))
                .expect("exact rollback replay after restart"),
            rollback_result
        );
        assert_eq!(
            recovered
                .checkpoint()
                .expect("recovered checkpoint")
                .revision,
            revision
        );
    }
    #[test]
    fn rollback_rejects_an_expired_last_known_good_catalog() {
        let controller =
            GatewayComplianceController::new(config(), Arc::new(MemoryStore::default()))
                .expect("controller");
        let mut first = payload(1, None);
        first.valid_until_unix = NOW + 100;
        let first_digest = promote(&controller, sign_catalog(first));
        let mut second = payload(2, Some(first_digest));
        second.generated_at_unix = NOW + 30;
        second.valid_until_unix = NOW + 1_000;
        let second_digest = controller
            .stage_catalog(sign_catalog(second), NOW + 35, mutation_binding(12))
            .expect("stage second")
            .catalog_digest;
        for gateway_index in 0..2 {
            controller
                .acknowledge(
                    acknowledgement_at(gateway_index, second_digest, true, NOW + 40),
                    NOW + 40,
                    mutation_binding(13 + gateway_index as u8),
                )
                .expect("acknowledge second");
        }
        controller
            .promote(second_digest, 2, NOW + 50, mutation_binding(15))
            .expect("promote second");
        let rollback = rollback_authorization([0xD1; 32], second_digest, first_digest, NOW + 120);
        assert!(matches!(
            controller.rollback(&rollback, NOW + 120, mutation_binding(0xD1)),
            Err(GatewayComplianceError::CatalogNotFresh)
        ));
    }
    #[test]
    fn checkpoint_rejects_pointer_and_history_lineage_substitution() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        promote(&controller, sign_catalog(payload(1, None)));
        let mut checkpoint = controller.checkpoint().expect("checkpoint");
        checkpoint.history[0].serving_digest = [0xBA; 32];
        let encoded = encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)
            .expect("encode tampered checkpoint");
        drop(controller);
        store.force_store(encoded);
        assert!(matches!(
            GatewayComplianceController::new(config(), store),
            Err(GatewayComplianceError::InvalidCheckpoint(_))
        ));
    }
    #[test]
    fn checkpoint_rejects_terminal_history_without_exact_replay_record() {
        let store = Arc::new(MemoryStore::default());
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        promote(&controller, sign_catalog(payload(1, None)));
        let mut checkpoint = controller.checkpoint().expect("checkpoint");
        checkpoint
            .idempotency_records
            .retain(|record| record.operation != GatewayComplianceMutationKindV1::Promote);
        checkpoint.revision = u64::try_from(checkpoint.idempotency_records.len())
            .expect("test checkpoint record count");
        let encoded = encode_bounded(&checkpoint, MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1)
            .expect("encode tampered checkpoint");
        drop(controller);
        store.force_store(encoded);
        assert!(matches!(
            GatewayComplianceController::new(config(), store),
            Err(GatewayComplianceError::InvalidCheckpoint(_))
        ));
    }
    #[derive(Debug)]
    struct ScriptedTransport {
        resolutions: Mutex<VecDeque<Vec<IpAddr>>>,
        response: GatewayComplianceFetchResponse,
    }
    impl GatewayComplianceFeedTransport for ScriptedTransport {
        fn qualification(
            &self,
        ) -> Result<
            GatewayComplianceFeedTransportIdentityV1,
            GatewayComplianceFeedTransportProbeError,
        > {
            Ok(test_feed_transport_identity())
        }
        fn resolve(
            &self,
            _hostname: &str,
            _timeout: Duration,
        ) -> Result<Vec<IpAddr>, GatewayComplianceError> {
            self.resolutions
                .lock()
                .expect("resolution lock")
                .pop_front()
                .ok_or_else(|| GatewayComplianceError::InvalidFeed("missing DNS script".into()))
        }
        fn fetch(
            &self,
            _request: &GatewayComplianceFetchRequest,
        ) -> Result<GatewayComplianceFetchResponse, GatewayComplianceError> {
            Ok(self.response.clone())
        }
    }
    fn test_feed_transport_identity() -> GatewayComplianceFeedTransportIdentityV1 {
        let policy = feed_policy();
        let pins_by_hostname = policy
            .hosts
            .into_iter()
            .map(|host| (host.hostname, host.accepted_spki_sha256))
            .collect();
        GatewayComplianceFeedTransportIdentityV1 {
            provider_handle: GATEWAY_COMPLIANCE_FEED_TRANSPORT_HANDLE_V1.to_owned(),
            revision: GATEWAY_COMPLIANCE_FEED_TRANSPORT_REVISION_V1,
            policy_digest: gateway_compliance_feed_transport_policy_digest(&pins_by_hostname)
                .expect("test transport policy"),
            test_marked: false,
        }
    }
    fn feed_policy() -> GatewayComplianceFeedPolicy {
        GatewayComplianceFeedPolicy {
            feed_id: "baseline".into(),
            url: "https://feed.example/catalog".into(),
            required: true,
            hosts: vec![GatewayComplianceFeedHostPolicy {
                hostname: "feed.example".into(),
                accepted_spki_sha256: BTreeSet::from([[0x77; 32]]),
            }],
        }
    }
    fn fetch_response(body: Vec<u8>) -> GatewayComplianceFetchResponse {
        GatewayComplianceFetchResponse {
            status: 200,
            redirect_location: None,
            connected_address: "93.184.216.34".parse().expect("public IP"),
            peer_spki_sha256: [0x77; 32],
            content_encoding: GatewayComplianceContentEncoding::Identity,
            body,
            elapsed: Duration::from_millis(20),
        }
    }
    include!("compliance_feed_transport_tests.rs");
    #[derive(Debug)]
    struct DeadlineTransport {
        response: GatewayComplianceFetchResponse,
    }
    impl GatewayComplianceFeedTransport for DeadlineTransport {
        fn qualification(
            &self,
        ) -> Result<
            GatewayComplianceFeedTransportIdentityV1,
            GatewayComplianceFeedTransportProbeError,
        > {
            Ok(test_feed_transport_identity())
        }
        fn resolve(
            &self,
            _hostname: &str,
            _timeout: Duration,
        ) -> Result<Vec<IpAddr>, GatewayComplianceError> {
            std::thread::sleep(Duration::from_millis(8));
            Ok(vec!["93.184.216.34".parse().expect("public IP")])
        }
        fn fetch(
            &self,
            _request: &GatewayComplianceFetchRequest,
        ) -> Result<GatewayComplianceFetchResponse, GatewayComplianceError> {
            Ok(self.response.clone())
        }
    }
    #[test]
    fn feed_fetch_enforces_one_cumulative_deadline() {
        let mut limits = GatewayComplianceFetchLimits::default();
        limits.connect_timeout = Duration::from_millis(10);
        limits.total_timeout = Duration::from_millis(15);
        let mut response = fetch_response(Vec::new());
        response.elapsed = Duration::from_millis(8);
        let transport = DeadlineTransport { response };
        assert!(matches!(
            fetch_feed_bytes(
                &feed_policy(),
                limits,
                &test_feed_transport_identity(),
                &transport,
            ),
            Err(GatewayComplianceError::FetchTimeout)
        ));
    }
    #[test]
    fn file_store_lease_restart_and_exact_replay_are_durable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let path = root.join("checkpoint.to");
        let store =
            Arc::new(FileGatewayComplianceStore::new(path.clone()).expect("file checkpoint store"));
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("first controller");
        assert!(matches!(
            GatewayComplianceController::new(config(), store.clone()),
            Err(GatewayComplianceError::LeaseHeld)
        ));
        let catalog = sign_catalog(payload(1, None));
        let binding = indexed_mutation_binding(1);
        let staged = controller
            .stage_catalog(catalog.clone(), NOW + 5, binding)
            .expect("stage");
        assert_eq!(controller.checkpoint().expect("checkpoint").revision, 1);
        let target_metadata = fs::symlink_metadata(&path).expect("checkpoint metadata");
        assert!(target_metadata.is_file());
        assert_eq!(hard_link_count(&target_metadata), 1);
        assert!(
            fs::read_dir(&root)
                .expect("checkpoint directory")
                .all(|entry| !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-")),
            "same-directory temporary files must not survive a successful replacement"
        );
        drop(controller);
        let recovered =
            GatewayComplianceController::new(config(), store).expect("restart controller");
        assert_eq!(
            recovered
                .stage_catalog(catalog, NOW + 7_200, binding)
                .expect("exact restart replay"),
            staged
        );
        assert_eq!(recovered.checkpoint().expect("checkpoint").revision, 1);
    }
    #[test]
    fn file_store_exact_byte_cas_fences_stale_controller() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let path = root.join("checkpoint.to");
        let store =
            Arc::new(FileGatewayComplianceStore::new(path.clone()).expect("file checkpoint store"));
        let controller = GatewayComplianceController::new(config(), store).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        controller
            .stage_catalog(catalog.clone(), NOW + 5, indexed_mutation_binding(1))
            .expect("first stage");
        let replacement = b"external-exact-byte-replacement";
        fs::write(&path, replacement).expect("replace checkpoint outside controller");
        assert!(matches!(
            controller.stage_catalog(catalog, NOW + 6, indexed_mutation_binding(2)),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert!(matches!(
            controller.checkpoint(),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert_eq!(fs::read(path).expect("durable replacement"), replacement);
    }
    #[test]
    fn file_store_rechecks_generation_immediately_before_persist() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let path = root.join("checkpoint.to");
        let store =
            Arc::new(FileGatewayComplianceStore::new(path.clone()).expect("file checkpoint store"));
        let controller =
            GatewayComplianceController::new(config(), store.clone()).expect("controller");
        let catalog = sign_catalog(payload(1, None));
        controller
            .stage_catalog(catalog.clone(), NOW + 5, indexed_mutation_binding(1))
            .expect("first stage");
        let replacement = b"replacement-during-write-preparation".to_vec();
        store.replace_before_next_persist(replacement.clone());
        assert!(matches!(
            controller.stage_catalog(catalog, NOW + 6, indexed_mutation_binding(2)),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert!(matches!(
            controller.checkpoint(),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert_eq!(
            fs::read(&path).expect("replacement must not be overwritten"),
            replacement
        );
        assert!(
            fs::read_dir(&root)
                .expect("checkpoint directory")
                .all(|entry| !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-")),
            "prepared temporary file must be removed after a generation conflict"
        );
    }
    #[cfg(unix)]
    #[test]
    fn file_store_rejects_hardlinked_checkpoint() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let path = root.join("checkpoint.to");
        let alias = root.join("checkpoint.alias");
        fs::write(&path, b"seed").expect("seed checkpoint");
        fs::hard_link(&path, &alias).expect("hardlink checkpoint");
        let store =
            FileGatewayComplianceStore::new(path.clone()).expect("file checkpoint store config");
        assert!(matches!(
            GatewayComplianceController::new(config(), Arc::new(store)),
            Err(GatewayComplianceError::Persistence(_))
        ));
        assert_eq!(fs::read(path).expect("checkpoint"), b"seed");
        assert_eq!(fs::read(alias).expect("checkpoint alias"), b"seed");
    }
    #[cfg(unix)]
    #[test]
    fn file_store_rejects_group_or_world_writable_parent() {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).expect("unsafe checkpoint directory");
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o777))
            .expect("set unsafe directory permissions");
        assert!(matches!(
            FileGatewayComplianceStore::new(unsafe_parent.join("checkpoint.to")),
            Err(GatewayComplianceError::Persistence(_))
        ));
    }
    #[cfg(unix)]
    #[test]
    fn file_store_rejects_symlink_lease_file() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let path = root.join("checkpoint.to");
        let lock_path = checkpoint_lock_path(&path).expect("checkpoint lock path");
        let target = root.join("lease-target");
        fs::write(&target, b"do-not-lock").expect("seed lease target");
        symlink(&target, &lock_path).expect("create lease symlink");
        let store = FileGatewayComplianceStore::new(path).expect("store config");
        assert!(matches!(
            store.try_acquire(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        assert_eq!(fs::read(target).expect("lease target"), b"do-not-lock");
    }
    #[cfg(unix)]
    #[test]
    fn file_store_rejects_symlink_checkpoint() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(temp.path()).expect("canonical tempdir");
        let target = root.join("real.to");
        fs::write(&target, b"old").expect("seed target");
        let link = root.join("checkpoint.to");
        symlink(&target, &link).expect("create symlink");
        let store = FileGatewayComplianceStore::new(link).expect("store config");
        let lease = store.try_acquire().expect("checkpoint lease");
        assert!(matches!(
            lease.compare_and_store(GatewayComplianceStoreGeneration::Absent, b"replacement"),
            Err(GatewayComplianceError::Persistence(_))
        ));
        assert_eq!(fs::read(target).expect("read target"), b"old");
    }
}
