//! Canonical signed gateway-compliance trust, catalogs, acknowledgements and feed documents.
//!
//! This module owns the bounded V1 frames, normalization, signature domains and pure validation.
//! Runtime promotion, replay journals, feed I/O and serving decisions belong to the controller.
use blake3::Hasher;
use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use std::{cmp::Ordering, collections::BTreeSet, net::IpAddr, num::NonZeroU16};
use thiserror::Error;
use url::Url;
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
/// Maximum catalog entries across all entry families.
pub const MAX_GATEWAY_COMPLIANCE_ENTRIES_V1: usize = 65_536;
/// Maximum trusted governance or gateway signers.
pub const MAX_GATEWAY_COMPLIANCE_SIGNERS_V1: usize = 128;
/// Maximum catalog/feed bytes before decoding.
pub const MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1: usize = 16 * 1024 * 1024;
/// Default local timestamp tolerance shared by SDK preflight and controller configuration.
/// Controllers still enforce their explicitly configured tolerance independently.
pub const DEFAULT_GATEWAY_COMPLIANCE_MAX_CLOCK_SKEW_SECS: u64 = 5 * 60;
const CATALOG_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-catalog-v1";
const ACK_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-ack-v1";
const ROLLBACK_SIGNING_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-rollback-v1";
const TRUST_POLICY_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-trust-policy-v1";
const CATALOG_DIGEST_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-catalog-digest-v1";
const FEED_DIGEST_DOMAIN_V1: &[u8] = b"sorafs-gateway-compliance-feed-v1";
const FEED_TRANSPORT_POLICY_DOMAIN_V1: &[u8] =
    b"sorafs-gateway-compliance-feed-transport-policy-v1";
/// One strong Ed25519 identity authorized by policy.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceTrustedSignerV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceTrustPolicyV1")]
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
    pub fn validate(&self) -> Result<(), GatewayComplianceProtocolError> {
        if self.policy_id.iter().all(|byte| *byte == 0) {
            return Err(GatewayComplianceProtocolError::InvalidPolicy(
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
    pub fn canonical_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceSubjectKindV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceBaselineRuleV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceAppealOverrideV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceLegalSafetyHoldV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceToggleV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceSourceAnchorV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceCatalogPayloadV1")]
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
    pub fn normalize(mut self) -> Result<Self, GatewayComplianceProtocolError> {
        for anchor in &mut self.source_anchors {
            anchor.feed_id = normalize_token(&anchor.feed_id, "feed_id")?;
            if anchor.feed_digest.iter().all(|byte| *byte == 0) || anchor.generated_at_unix == 0 {
                return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
    pub fn validate(&self) -> Result<(), GatewayComplianceProtocolError> {
        if self.version != GATEWAY_COMPLIANCE_CATALOG_VERSION_V1 {
            return Err(GatewayComplianceProtocolError::InvalidCatalog(format!(
                "unsupported catalog version {}",
                self.version
            )));
        }
        if self.sequence == 0 {
            return Err(GatewayComplianceProtocolError::InvalidCatalog(
                "catalog sequence must be non-zero".into(),
            ));
        }
        if self.policy_digest.iter().all(|byte| *byte == 0) {
            return Err(GatewayComplianceProtocolError::InvalidCatalog(
                "policy digest must not be all zeroes".into(),
            ));
        }
        if self.generated_at_unix == 0 || self.valid_until_unix <= self.generated_at_unix {
            return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
            return Err(GatewayComplianceProtocolError::ResourceLimit {
                resource: "catalog entries",
                found: entry_count,
                maximum: MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
            });
        }
        let normalized = self.clone().normalize()?;
        if normalized != *self {
            return Err(GatewayComplianceProtocolError::NonCanonical(
                "catalog inventories or fields".into(),
            ));
        }
        encode_bounded(self, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1)?;
        Ok(())
    }
    /// Return the exact domain-separated catalog identifier.
    pub fn catalog_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        self.validate()?;
        hash_canonical(
            CATALOG_DIGEST_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
    /// Return the digest governance signers approve.
    pub fn signing_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        self.validate()?;
        hash_canonical(
            CATALOG_SIGNING_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
}
/// One governance approval on a catalog.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceCatalogApprovalV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceCatalogV1")]
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
    ) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        policy.validate()?;
        self.payload.validate()?;
        if self.payload.policy_digest != policy.canonical_digest()? {
            return Err(GatewayComplianceProtocolError::PolicyDigestMismatch);
        }
        validate_catalog_freshness(&self.payload, observed_at_unix, max_clock_skew_secs)?;
        if self.approvals.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
            return Err(GatewayComplianceProtocolError::ResourceLimit {
                resource: "catalog approvals",
                found: self.approvals.len(),
                maximum: MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
            });
        }
        if self.approvals.len() < usize::from(policy.catalog_threshold) {
            return Err(GatewayComplianceProtocolError::QuorumNotMet {
                found: self.approvals.len(),
                required: policy.catalog_threshold,
            });
        }
        let digest = self.payload.signing_digest()?;
        let mut previous: Option<&str> = None;
        for approval in &self.approvals {
            if approval.version != GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1 {
                return Err(GatewayComplianceProtocolError::InvalidSignature {
                    signer_id: approval.signer_id.clone(),
                    reason: "unsupported approval version".into(),
                });
            }
            validate_token(&approval.signer_id, "signer_id")?;
            if let Some(previous) = previous {
                match previous.cmp(approval.signer_id.as_str()) {
                    Ordering::Equal => {
                        return Err(GatewayComplianceProtocolError::DuplicateSigner(
                            approval.signer_id.clone(),
                        ));
                    }
                    Ordering::Greater => {
                        return Err(GatewayComplianceProtocolError::NonCanonical(
                            "catalog approval order".into(),
                        ));
                    }
                    Ordering::Less => {}
                }
            }
            let trusted = policy.catalog_signer(&approval.signer_id).ok_or_else(|| {
                GatewayComplianceProtocolError::UntrustedSigner(approval.signer_id.clone())
            })?;
            if contains_sorted(
                &policy.revoked_catalog_signer_ids,
                approval.signer_id.as_str(),
            ) {
                return Err(GatewayComplianceProtocolError::RevokedSigner(
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
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::gateway_compliance::GatewayComplianceAcknowledgementPayloadV1"
)]
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
impl GatewayComplianceAcknowledgementPayloadV1 {
    /// Validate context-free canonical fields before signing or verification.
    fn validate(&self) -> Result<(), GatewayComplianceProtocolError> {
        if self.version != GATEWAY_COMPLIANCE_ACK_VERSION_V1 {
            return Err(GatewayComplianceProtocolError::InvalidAcknowledgement(
                "unsupported acknowledgement version".into(),
            ));
        }
        validate_token(&self.gateway_id, "gateway_id")?;
        if self.observed_at_unix == 0 {
            return Err(GatewayComplianceProtocolError::InvalidAcknowledgement(
                "acknowledgement timestamp is invalid".into(),
            ));
        }
        match (self.accepted, self.rejection_code.as_ref()) {
            (true, None) => Ok(()),
            (false, Some(code)) => validate_token(code, "rejection_code"),
            _ => Err(GatewayComplianceProtocolError::InvalidAcknowledgement(
                "accepted acknowledgements omit rejection_code; rejected acknowledgements require it"
                    .into(),
            )),
        }
    }

    /// Return the exact domain-separated digest a regional gateway signs.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical payload fields or an unencodable bounded payload.
    /// This does not authenticate the gateway, authorize the catalog, or establish
    /// freshness relative to a controller's clock; verification owns those checks.
    pub fn signing_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        self.validate()?;
        hash_canonical(
            ACK_SIGNING_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
}
/// Signed regional gateway acknowledgement.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceAcknowledgementV1")]
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
    /// Verify the exact catalog binding, current observation interval and trusted gateway signature.
    pub fn verify(
        &self,
        policy: &GatewayComplianceTrustPolicyV1,
        expected_catalog_digest: [u8; 32],
        observed_at_unix: u64,
        max_clock_skew_secs: u64,
    ) -> Result<(), GatewayComplianceProtocolError> {
        policy.validate()?;
        let digest = self.payload.signing_digest()?;
        if self.payload.catalog_digest != expected_catalog_digest {
            return Err(GatewayComplianceProtocolError::InvalidAcknowledgement(
                "catalog digest mismatch".into(),
            ));
        }
        if self.payload.observed_at_unix > observed_at_unix.saturating_add(max_clock_skew_secs)
            || self
                .payload
                .observed_at_unix
                .saturating_add(max_clock_skew_secs)
                < observed_at_unix
        {
            return Err(GatewayComplianceProtocolError::InvalidAcknowledgement(
                "acknowledgement timestamp is invalid".into(),
            ));
        }
        let trusted = policy
            .gateway_signer(&self.payload.gateway_id)
            .ok_or_else(|| {
                GatewayComplianceProtocolError::UntrustedSigner(self.payload.gateway_id.clone())
            })?;
        if contains_sorted(
            &policy.revoked_gateway_signer_ids,
            self.payload.gateway_id.as_str(),
        ) {
            return Err(GatewayComplianceProtocolError::RevokedSigner(
                self.payload.gateway_id.clone(),
            ));
        }
        verify_ed25519(
            &trusted.public_key,
            &self.signature,
            &digest,
            &self.payload.gateway_id,
        )
    }
}
/// Unsigned rollback command.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceRollbackPayloadV1")]
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
impl GatewayComplianceRollbackPayloadV1 {
    /// Return the canonical rollback signing digest after context-free field validation.
    ///
    /// This does not establish signer quorum, freshness, replay state or the controller's targets.
    pub fn signing_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        if self.version != GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1
            || self.operation_id.iter().all(|byte| *byte == 0)
            || self.from_catalog_digest.iter().all(|byte| *byte == 0)
            || self.to_catalog_digest.iter().all(|byte| *byte == 0)
            || self.from_catalog_digest == self.to_catalog_digest
            || self.authorized_at_unix == 0
        {
            return Err(GatewayComplianceProtocolError::InvalidRollback(
                "rollback payload is malformed or stale".into(),
            ));
        }
        validate_token(&self.reason_code, "reason_code")?;
        hash_canonical(
            ROLLBACK_SIGNING_DOMAIN_V1,
            self,
            MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
        )
    }
}
/// Threshold-approved rollback authorization.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceRollbackV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::gateway_compliance::GatewayComplianceFeedDocumentV1")]
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
    pub fn normalize(mut self) -> Result<Self, GatewayComplianceProtocolError> {
        if self.version != GATEWAY_COMPLIANCE_FEED_VERSION_V1 {
            return Err(GatewayComplianceProtocolError::InvalidFeed(
                "unsupported feed version".into(),
            ));
        }
        self.feed_id = normalize_token(&self.feed_id, "feed_id")?;
        if self.generated_at_unix == 0 {
            return Err(GatewayComplianceProtocolError::InvalidFeed(
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
            return Err(GatewayComplianceProtocolError::ResourceLimit {
                resource: "feed entries",
                found: entry_count,
                maximum: MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
            });
        }
        encode_bounded(&self, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1)?;
        Ok(self)
    }
    /// Return a digest of the canonical normalized feed.
    pub fn canonical_digest(&self) -> Result<[u8; 32], GatewayComplianceProtocolError> {
        let normalized = self.clone().normalize()?;
        if normalized != *self {
            return Err(GatewayComplianceProtocolError::NonCanonical(
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
#[derive(Debug, NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::gateway_compliance::GatewayComplianceFeedTransportPolicyDigestV1"
)]
struct GatewayComplianceFeedTransportPolicyDigestV1 {
    version: u8,
    hosts: Vec<GatewayComplianceFeedTransportHostDigestV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::gateway_compliance::GatewayComplianceFeedTransportHostDigestV1"
)]
#[derive(Debug, NoritoSerialize)]
struct GatewayComplianceFeedTransportHostDigestV1 {
    hostname: String,
    accepted_spki_sha256: Vec<[u8; 32]>,
}
/// Compute the V1 domain-separated digest of a canonical hostname/SPKI policy.
///
/// Runtime adapter implementations use this helper to attest the exact non-secret trust inventory
/// they enforce. Hostnames and pin sets are traversed in `BTreeMap`/`BTreeSet` order, so every
/// platform produces the same digest. An empty inventory explicitly denies every external
/// hostname; it does not authorize a catalog or bypass governed promotion.
///
/// # Errors
///
/// Returns an error when a hostname is noncanonical or an existing host has an empty
/// or all-zero pin set.
pub fn gateway_compliance_feed_transport_policy_digest(
    pins_by_hostname: &std::collections::BTreeMap<String, BTreeSet<[u8; 32]>>,
) -> Result<[u8; 32], GatewayComplianceProtocolError> {
    let mut hosts = Vec::with_capacity(pins_by_hostname.len());
    for (hostname, pins) in pins_by_hostname {
        validate_dns_hostname(hostname)?;
        if pins.is_empty() || pins.iter().any(|pin| pin.iter().all(|byte| *byte == 0)) {
            return Err(GatewayComplianceProtocolError::InvalidPolicy(
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
fn validate_signer_inventory(
    signers: &[GatewayComplianceTrustedSignerV1],
    revoked: &[String],
    threshold: u16,
    label: &'static str,
) -> Result<(), GatewayComplianceProtocolError> {
    if signers.is_empty() || signers.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
        return Err(GatewayComplianceProtocolError::InvalidPolicy(format!(
            "{label} signer count is invalid"
        )));
    }
    let mut previous: Option<&str> = None;
    let mut keys = BTreeSet::new();
    for signer in signers {
        validate_token(&signer.signer_id, "signer_id")?;
        if previous.is_some_and(|value| value >= signer.signer_id.as_str()) {
            return Err(GatewayComplianceProtocolError::NonCanonical(format!(
                "{label} signer order"
            )));
        }
        let verifying_key = VerifyingKey::from_bytes(&signer.public_key).map_err(|error| {
            GatewayComplianceProtocolError::InvalidPolicy(format!(
                "{label} signer `{}` has invalid Ed25519 key: {error}",
                signer.signer_id
            ))
        })?;
        if verifying_key.is_weak() {
            return Err(GatewayComplianceProtocolError::InvalidPolicy(format!(
                "{label} signer `{}` uses a weak Ed25519 key",
                signer.signer_id
            )));
        }
        if !keys.insert(signer.public_key) {
            return Err(GatewayComplianceProtocolError::InvalidPolicy(format!(
                "{label} signer public keys must be unique"
            )));
        }
        previous = Some(&signer.signer_id);
    }
    let mut previous_revoked: Option<&str> = None;
    for signer_id in revoked {
        validate_token(signer_id, "revoked signer_id")?;
        if previous_revoked.is_some_and(|value| value >= signer_id.as_str()) {
            return Err(GatewayComplianceProtocolError::NonCanonical(format!(
                "{label} revocation order"
            )));
        }
        if find_signer(signers, signer_id).is_none() {
            return Err(GatewayComplianceProtocolError::InvalidPolicy(format!(
                "{label} revocation names an unknown signer"
            )));
        }
        previous_revoked = Some(signer_id);
    }
    let active = signers.len().saturating_sub(revoked.len());
    let threshold = usize::from(
        NonZeroU16::new(threshold)
            .ok_or_else(|| {
                GatewayComplianceProtocolError::InvalidPolicy(format!(
                    "{label} threshold must be non-zero"
                ))
            })?
            .get(),
    );
    if threshold > active {
        return Err(GatewayComplianceProtocolError::InvalidPolicy(format!(
            "{label} threshold exceeds active signer count"
        )));
    }
    Ok(())
}
fn validate_disjoint_signer_roles(
    catalog_signers: &[GatewayComplianceTrustedSignerV1],
    gateway_signers: &[GatewayComplianceTrustedSignerV1],
) -> Result<(), GatewayComplianceProtocolError> {
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
        return Err(GatewayComplianceProtocolError::InvalidPolicy(
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
) -> Result<(), GatewayComplianceProtocolError> {
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
) -> Result<(), GatewayComplianceProtocolError> {
    appeal.appeal_id = normalize_token(&appeal.appeal_id, "appeal_id")?;
    appeal.scope = normalize_scope(&appeal.scope)?;
    appeal.subject = normalize_subject(appeal.subject_kind, &appeal.subject)?;
    if appeal.decision_digest.iter().all(|byte| *byte == 0) {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
            "accepted appeal decision digest must not be all zeroes".into(),
        ));
    }
    validate_interval(appeal.effective_from_unix, Some(appeal.expires_at_unix))
}
fn normalize_hold(
    hold: &mut GatewayComplianceLegalSafetyHoldV1,
) -> Result<(), GatewayComplianceProtocolError> {
    hold.hold_id = normalize_token(&hold.hold_id, "hold_id")?;
    hold.scope = normalize_scope(&hold.scope)?;
    hold.subject = normalize_subject(hold.subject_kind, &hold.subject)?;
    hold.authority_reference = normalize_token(&hold.authority_reference, "authority_reference")?;
    validate_interval(hold.effective_from_unix, hold.expires_at_unix)
}
fn normalize_toggle(
    toggle: &mut GatewayComplianceToggleV1,
) -> Result<(), GatewayComplianceProtocolError> {
    toggle.toggle_id = normalize_token(&toggle.toggle_id, "toggle_id")?;
    toggle.scope = normalize_scope(&toggle.scope)?;
    toggle.approval_reference = normalize_token(&toggle.approval_reference, "approval_reference")?;
    validate_interval(toggle.effective_from_unix, Some(toggle.expires_at_unix))
}
fn validate_interval(
    effective_from_unix: u64,
    expires_at_unix: Option<u64>,
) -> Result<(), GatewayComplianceProtocolError> {
    if effective_from_unix == 0
        || expires_at_unix.is_some_and(|expiry| expiry <= effective_from_unix)
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
            "compliance entry validity interval is invalid".into(),
        ));
    }
    Ok(())
}
fn normalize_token(
    value: &str,
    field: &'static str,
) -> Result<String, GatewayComplianceProtocolError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 128
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'-' | b'_' | b':')
        })
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(format!(
            "{field} is not a canonical bounded token"
        )));
    }
    Ok(normalized)
}
/// Require an already canonical bounded protocol token.
pub fn validate_token(
    value: &str,
    field: &'static str,
) -> Result<(), GatewayComplianceProtocolError> {
    if normalize_token(value, field)? != value {
        return Err(GatewayComplianceProtocolError::NonCanonical(field.into()));
    }
    Ok(())
}
/// Normalize one global, region or gateway protocol scope.
pub fn normalize_scope(value: &str) -> Result<String, GatewayComplianceProtocolError> {
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
        Err(GatewayComplianceProtocolError::InvalidCatalog(
            "scope must be global, region:<id>, or gateway:<id>".into(),
        ))
    }
}
/// Normalize one bounded subject using its canonical subject family.
pub fn normalize_subject(
    kind: GatewayComplianceSubjectKindV1,
    value: &str,
) -> Result<String, GatewayComplianceProtocolError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 2_048
        || !trimmed.is_ascii()
        || trimmed.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
fn normalize_cid(value: &str) -> Result<String, GatewayComplianceProtocolError> {
    let encoded = value.strip_prefix('b').ok_or_else(|| {
        GatewayComplianceProtocolError::InvalidCatalog(
            "CID subjects must use the lowercase base32 multibase prefix".into(),
        )
    })?;
    if encoded.is_empty()
        || encoded
            .bytes()
            .any(|byte| !matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
            "CID subjects must use canonical lowercase base32 without padding".into(),
        ));
    }
    let decoded = decode_base32_lower(encoded)?;
    if encode_base32_lower(&decoded) != encoded {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
            "CID subject is not a canonical base32 round-trip".into(),
        ));
    }
    Ok(value.to_owned())
}
fn decode_base32_lower(value: &str) -> Result<Vec<u8>, GatewayComplianceProtocolError> {
    let mut output = Vec::with_capacity(value.len().saturating_mul(5) / 8);
    let mut accumulator = 0_u16;
    let mut bits = 0_u8;
    for byte in value.bytes() {
        let digit = match byte {
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => {
                return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
fn normalize_hex(
    value: &str,
    expected_length: usize,
) -> Result<String, GatewayComplianceProtocolError> {
    let normalized = value.to_ascii_lowercase();
    if normalized.len() != expected_length
        || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(format!(
            "hex subject must contain exactly {expected_length} digits"
        )));
    }
    Ok(normalized)
}
fn normalize_subject_url(value: &str) -> Result<String, GatewayComplianceProtocolError> {
    let parsed = Url::parse(value).map_err(|error| {
        GatewayComplianceProtocolError::InvalidCatalog(format!("invalid URL subject: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
            "URL subject contains forbidden components".into(),
        ));
    }
    Ok(parsed.to_string())
}
fn reject_duplicate_keys<T, F>(
    values: &[T],
    key: F,
    field: &'static str,
) -> Result<(), GatewayComplianceProtocolError>
where
    F: Fn(&T) -> &str,
{
    if values
        .windows(2)
        .any(|window| key(&window[0]) == key(&window[1]))
    {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(format!(
            "{field} contains duplicate identities"
        )));
    }
    Ok(())
}
fn reject_duplicate_toggle_scope(
    toggles: &[GatewayComplianceToggleV1],
) -> Result<(), GatewayComplianceProtocolError> {
    if toggles.windows(2).any(|window| {
        window[0].toggle_id == window[1].toggle_id && window[0].scope == window[1].scope
    }) {
        return Err(GatewayComplianceProtocolError::InvalidCatalog(
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
) -> Result<(), GatewayComplianceProtocolError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|error| {
        GatewayComplianceProtocolError::InvalidSignature {
            signer_id: signer_id.to_owned(),
            reason: error.to_string(),
        }
    })?;
    let signature = Ed25519Signature::from_bytes(signature);
    key.verify_strict(message, &signature).map_err(|error| {
        GatewayComplianceProtocolError::InvalidSignature {
            signer_id: signer_id.to_owned(),
            reason: error.to_string(),
        }
    })
}
/// Verify the initial or exact successor catalog predecessor and sequence.
pub fn validate_catalog_transition(
    previous: Option<&GatewayComplianceCatalogV1>,
    next: &GatewayComplianceCatalogV1,
) -> Result<(), GatewayComplianceProtocolError> {
    match previous {
        None => {
            if next.payload.sequence != 1 || next.payload.predecessor_digest.is_some() {
                return Err(GatewayComplianceProtocolError::InvalidPredecessor);
            }
        }
        Some(previous) => {
            let expected_sequence = previous
                .payload
                .sequence
                .checked_add(1)
                .ok_or(GatewayComplianceProtocolError::SequenceOverflow)?;
            if next.payload.sequence != expected_sequence
                || next.payload.predecessor_digest != Some(previous.payload.catalog_digest()?)
            {
                return Err(GatewayComplianceProtocolError::InvalidPredecessor);
            }
        }
    }
    Ok(())
}
/// Verify catalog creation and exclusive expiry against the supplied observation.
pub fn validate_catalog_freshness(
    payload: &GatewayComplianceCatalogPayloadV1,
    observed_at_unix: u64,
    max_clock_skew_secs: u64,
) -> Result<(), GatewayComplianceProtocolError> {
    if observed_at_unix == 0
        || payload.generated_at_unix > observed_at_unix.saturating_add(max_clock_skew_secs)
        || observed_at_unix >= payload.valid_until_unix
    {
        return Err(GatewayComplianceProtocolError::CatalogNotFresh);
    }
    Ok(())
}
/// Verify the bounded signed rollback authorization against the supplied trust policy and clock.
pub fn verify_rollback(
    authorization: &GatewayComplianceRollbackV1,
    policy: &GatewayComplianceTrustPolicyV1,
    observed_at_unix: u64,
    max_clock_skew_secs: u64,
) -> Result<(), GatewayComplianceProtocolError> {
    policy.validate()?;
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
        return Err(GatewayComplianceProtocolError::InvalidRollback(
            "rollback payload is malformed or stale".into(),
        ));
    }
    validate_token(&payload.reason_code, "reason_code")?;
    if authorization.approvals.len() < usize::from(policy.catalog_threshold) {
        return Err(GatewayComplianceProtocolError::QuorumNotMet {
            found: authorization.approvals.len(),
            required: policy.catalog_threshold,
        });
    }
    if authorization.approvals.len() > MAX_GATEWAY_COMPLIANCE_SIGNERS_V1 {
        return Err(GatewayComplianceProtocolError::ResourceLimit {
            resource: "rollback approvals",
            found: authorization.approvals.len(),
            maximum: MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
        });
    }
    let digest = payload.signing_digest()?;
    let mut previous: Option<&str> = None;
    for approval in &authorization.approvals {
        if approval.version != GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1 {
            return Err(GatewayComplianceProtocolError::InvalidRollback(
                "rollback approval version is unsupported".into(),
            ));
        }
        if previous.is_some_and(|value| value >= approval.signer_id.as_str()) {
            return Err(GatewayComplianceProtocolError::NonCanonical(
                "rollback approval order".into(),
            ));
        }
        let trusted = policy.catalog_signer(&approval.signer_id).ok_or_else(|| {
            GatewayComplianceProtocolError::UntrustedSigner(approval.signer_id.clone())
        })?;
        if contains_sorted(
            &policy.revoked_catalog_signer_ids,
            approval.signer_id.as_str(),
        ) {
            return Err(GatewayComplianceProtocolError::RevokedSigner(
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
/// Require a canonical public DNS hostname for a configured feed trust inventory.
pub fn validate_dns_hostname(host: &str) -> Result<(), GatewayComplianceProtocolError> {
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
        return Err(GatewayComplianceProtocolError::UnsafeUrl(
            "host is not a canonical public DNS name".into(),
        ));
    }
    Ok(())
}
fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
    maximum: usize,
) -> Result<[u8; 32], GatewayComplianceProtocolError> {
    let bytes = encode_bounded(value, maximum)?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| GatewayComplianceProtocolError::Encoding("payload length overflow".into()))?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&length.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
/// Encode a protocol value or its controller checkpoint with the canonical framed codec and byte cap.
pub fn encode_bounded<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, GatewayComplianceProtocolError> {
    // Bound and serialize the same canonical layout, independent of an enclosing decoder.
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    match norito::core::to_bytes_bounded(value, maximum) {
        Ok(bytes) => Ok(bytes),
        Err(norito::core::BoundedEncodeError::FrameTooLarge { encoded_bytes, .. }) => {
            Err(GatewayComplianceProtocolError::ResourceLimit {
                resource: "canonical encoded bytes",
                found: encoded_bytes,
                maximum,
            })
        }
        Err(error) => Err(GatewayComplianceProtocolError::Encoding(error.to_string())),
    }
}
/// Pure signed-compliance protocol validation or bounded canonical encoding failure.
#[derive(Debug, Error)]
pub enum GatewayComplianceProtocolError {
    /// Catalog timestamp is stale or too far in the future.
    #[error("gateway compliance catalog is stale or future-dated")]
    CatalogNotFresh,
    /// Signature identity is repeated.
    #[error("duplicate compliance signer `{0}`")]
    DuplicateSigner(String),
    /// Norito encoding failed.
    #[error("gateway compliance encoding failed: {0}")]
    Encoding(String),
    /// Gateway acknowledgement is malformed.
    #[error("invalid gateway compliance acknowledgement: {0}")]
    InvalidAcknowledgement(String),
    /// Catalog shape or semantics are malformed.
    #[error("invalid gateway compliance catalog: {0}")]
    InvalidCatalog(String),
    /// Feed shape or response is malformed.
    #[error("invalid gateway compliance feed: {0}")]
    InvalidFeed(String),
    /// Trust or controller policy is malformed.
    #[error("invalid gateway compliance policy: {0}")]
    InvalidPolicy(String),
    /// Initial or successor linkage is invalid.
    #[error("gateway compliance catalog predecessor or sequence is invalid")]
    InvalidPredecessor,
    /// Rollback authorization is malformed.
    #[error("invalid gateway compliance rollback: {0}")]
    InvalidRollback(String),
    /// Signature verification failed.
    #[error("invalid compliance signature from `{signer_id}`: {reason}")]
    InvalidSignature {
        /// Signer identity.
        signer_id: String,
        /// Verification failure.
        reason: String,
    },
    /// A canonical inventory or string was not normalized.
    #[error("non-canonical gateway compliance value: {0}")]
    NonCanonical(String),
    /// Catalog policy digest differs from resolved config.
    #[error("gateway compliance policy digest mismatch")]
    PolicyDigestMismatch,
    /// Catalog approval quorum is incomplete.
    #[error("gateway compliance quorum not met: found {found}, required {required}")]
    QuorumNotMet {
        /// Valid approval count.
        found: usize,
        /// Required approval count.
        required: u16,
    },
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
    /// Signer is explicitly revoked.
    #[error("revoked compliance signer `{0}`")]
    RevokedSigner(String),
    /// Sequence arithmetic overflowed.
    #[error("gateway compliance catalog sequence overflow")]
    SequenceOverflow,
    /// URL violates HTTPS/allowlist rules.
    #[error("unsafe gateway compliance URL: {0}")]
    UnsafeUrl(String),
    /// Signer is not in the resolved trust policy.
    #[error("untrusted compliance signer `{0}`")]
    UntrustedSigner(String),
}

#[cfg(test)]
mod tests;

#[cfg(test)]
include!("gateway_compliance/captured_owner_identity_tests.rs");
