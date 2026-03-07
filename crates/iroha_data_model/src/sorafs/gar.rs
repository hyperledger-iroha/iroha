//! Gateway Authorization Record (GAR) policy payloads.
//!
//! These types mirror the structured policy hints embedded in GAR v2 payloads.
//! They allow hosts, gateways, and governance tooling to exchange licensing,
//! moderation, and telemetry directives without relying on ad-hoc JSON maps.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::account::AccountId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

/// Licensing bundle referenced by a GAR payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarLicenseSetV1 {
    /// Human-readable identifier (e.g., `sg-2026-pilot`).
    pub slug: String,
    /// Jurisdiction or regulatory body that issued the license.
    pub jurisdiction: String,
    /// Legal entity that holds the broadcast license.
    pub holder: String,
    /// Optional Unix timestamp (seconds) when the license becomes valid.
    #[norito(default)]
    pub valid_from_unix: Option<u64>,
    /// Optional Unix timestamp (seconds) when the license expires.
    #[norito(default)]
    pub valid_until_unix: Option<u64>,
    /// Optional URI or document reference for auditors.
    #[norito(default)]
    pub reference_uri: Option<String>,
}

/// CDN-facing policy embedded in GAR v2 payloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarCdnPolicyV1 {
    /// Optional TTL override applied by gateways (seconds).
    #[norito(default)]
    pub ttl_override_secs: Option<u64>,
    /// Purge tags that must be present before serving cached content.
    #[norito(default)]
    pub purge_tags: Vec<String>,
    /// Moderation directive slugs that apply to this GAR.
    #[norito(default)]
    pub moderation_slugs: Vec<String>,
    /// Optional request-per-second ceiling enforced at the gateway edge.
    #[norito(default)]
    pub rate_ceiling_rps: Option<u64>,
    /// Regions that are explicitly permitted.
    #[norito(default)]
    pub allow_regions: Vec<String>,
    /// Regions that are explicitly denied.
    #[norito(default)]
    pub deny_regions: Vec<String>,
    /// Whether the GAR is under a legal hold (serving is blocked).
    #[norito(default)]
    pub legal_hold: bool,
}

/// Moderation directive embedded in GAR v2.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarModerationDirectiveV1 {
    /// Unique label for the directive.
    pub slug: String,
    /// Action gateways must apply when the directive matches.
    pub action: GarModerationAction,
    /// Sensitivity classes affected by the directive (optional).
    #[norito(default)]
    pub sensitivity_classes: Vec<String>,
    /// Optional governance notes shown in dashboards/runbooks.
    #[norito(default)]
    pub notes: Option<String>,
}

/// Moderation action enforced by the directive.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default, Hash, PartialOrd, Ord,
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "data"))]
pub enum GarModerationAction {
    /// Allow the request but record the directive metadata.
    #[cfg_attr(feature = "json", norito(rename = "allow"))]
    Allow,
    /// Allow the request with an explicit warning/notice.
    #[cfg_attr(feature = "json", norito(rename = "warn"))]
    Warn,
    /// Quarantine the request until an operator reviews it.
    #[cfg_attr(feature = "json", norito(rename = "quarantine"))]
    Quarantine,
    /// Block the request outright.
    #[default]
    #[cfg_attr(feature = "json", norito(rename = "block"))]
    Block,
}

impl GarModerationAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Quarantine => "quarantine",
            Self::Block => "block",
        }
    }

    #[cfg(feature = "json")]
    fn parse(value: &str) -> Result<Self, norito::json::Error> {
        match value {
            "allow" => Ok(Self::Allow),
            "warn" => Ok(Self::Warn),
            "quarantine" => Ok(Self::Quarantine),
            "block" => Ok(Self::Block),
            other => Err(norito::json::Error::Message(format!(
                "unknown moderation action `{other}`"
            ))),
        }
    }
}

#[cfg(feature = "json")]
mod gar_json_impl {
    use norito::json::{Error, FastJsonWrite, JsonDeserialize, JsonSerialize, Parser};

    use super::GarModerationAction;

    impl FastJsonWrite for GarModerationAction {
        fn write_json(&self, out: &mut String) {
            JsonSerialize::json_serialize(self.as_str(), out);
        }
    }

    impl JsonDeserialize for GarModerationAction {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let value = parser.parse_string()?;
            GarModerationAction::parse(&value)
        }
    }
}

/// Metrics/telemetry policy surfaced through GAR.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarMetricsPolicyV1 {
    /// Identifier used in dashboards and audit reports.
    pub policy_id: String,
    /// Sampling budget expressed in basis points (0-10_000).
    pub sampling_bps: u16,
    /// Maximum retention window for the captured metrics (seconds).
    pub retention_secs: u64,
    /// Named metrics that the policy allows (e.g., `audience`, `rebuffer`).
    #[norito(default)]
    pub allowed_metrics: Vec<String>,
}

/// Structured policy payload embedded in GAR v2.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarPolicyPayloadV1 {
    /// Licensing bundles attached to the GAR.
    #[norito(default)]
    pub license_sets: Vec<GarLicenseSetV1>,
    /// Moderation directives enforced by the gateway.
    #[norito(default)]
    pub moderation_directives: Vec<GarModerationDirectiveV1>,
    /// CDN-facing policy envelope.
    #[norito(default)]
    pub cdn_policy: Option<GarCdnPolicyV1>,
    /// Optional metrics/telemetry policy contract.
    #[norito(default)]
    pub metrics_policy: Option<GarMetricsPolicyV1>,
    /// Canonical telemetry labels emitted with GAR violations.
    #[norito(default)]
    pub telemetry_labels: Vec<String>,
    /// Digest of the latest Replication Proof Token bundle, when available.
    #[cfg_attr(feature = "json", norito(with = "crate::sorafs::gar::rpt_digest_json"))]
    #[norito(default)]
    pub rpt_digest: Option<[u8; 32]>,
}

#[cfg(feature = "json")]
mod rpt_digest_json {
    use norito::json::{Error, Parser};

    #[allow(clippy::ref_option)]
    pub fn serialize(value: &Option<[u8; 32]>, out: &mut String) {
        crate::json_helpers::fixed_bytes::option::serialize(value, out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Option<[u8; 32]>, Error> {
        crate::json_helpers::fixed_bytes::option::deserialize(parser)
    }
}

/// Gateway enforcement actions recorded for audit/compliance (SNNet-15G1).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "data"))]
pub enum GarEnforcementActionV1 {
    /// Purge the gateway cache or static zone immediately.
    #[cfg_attr(feature = "json", norito(rename = "purge_static_zone"))]
    PurgeStaticZone,
    /// Temporarily bypass the cache and serve content directly from origin.
    #[cfg_attr(feature = "json", norito(rename = "cache_bypass"))]
    CacheBypass,
    /// Override cache TTL according to GAR policy.
    #[cfg_attr(feature = "json", norito(rename = "ttl_override"))]
    TtlOverride,
    /// Apply or tighten rate limiting for the name or namespace.
    #[cfg_attr(feature = "json", norito(rename = "rate_limit_override"))]
    RateLimitOverride,
    /// Enforce a geofence or regional block.
    #[cfg_attr(feature = "json", norito(rename = "geo_fence"))]
    GeoFence,
    /// Place the asset or route under a legal/guardian freeze.
    #[cfg_attr(feature = "json", norito(rename = "legal_hold"))]
    LegalHold,
    /// Apply GAR-linked moderation directive (warn/quarantine/block).
    #[cfg_attr(feature = "json", norito(rename = "moderation"))]
    Moderation,
    /// Emit an operator-only audit notice without changing live routing.
    #[cfg_attr(feature = "json", norito(rename = "audit_notice"))]
    #[default]
    AuditNotice,
    /// Custom action recorded with a caller-specified slug.
    #[cfg_attr(feature = "json", norito(rename = "custom"))]
    Custom(String),
}

/// Deterministic receipt recorded whenever a GAR policy action is enforced.
///
/// These receipts allow the SNNet-15G1 compliance tooling to export audit-ready
/// evidence bundles that link a GAR, canonical host, operator, and enforcement
/// reason to the policy digest that triggered the action.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GarEnforcementReceiptV1 {
    /// Unique identifier (e.g., ULID) that callers can correlate with logs.
    pub receipt_id: [u8; 16],
    /// Human-readable `GAR` name (`SoraDNS` label).
    pub gar_name: String,
    /// Canonical host that was affected by the enforcement action.
    pub canonical_host: String,
    /// Type of enforcement action taken.
    pub action: GarEnforcementActionV1,
    /// Unix timestamp (seconds) when the action triggered.
    pub triggered_at_unix: u64,
    /// Optional timestamp when the enforcement expires.
    #[norito(default)]
    pub expires_at_unix: Option<u64>,
    /// Optional policy version label (e.g., release tag or manifest slug).
    #[norito(default)]
    pub policy_version: Option<String>,
    /// Optional digest of the exact policy blob that triggered the action.
    #[norito(default)]
    pub policy_digest: Option<[u8; 32]>,
    /// Operator account that executed the enforcement.
    pub operator: AccountId,
    /// Human-readable reason recorded in dashboards/runbooks.
    pub reason: String,
    /// Optional free-form notes for auditors.
    #[norito(default)]
    pub notes: Option<String>,
    /// Evidence URIs (logs, dashboards, CAR manifests) referenced by the receipt.
    #[norito(default)]
    pub evidence_uris: Vec<String>,
    /// Additional machine-readable labels (guardian ticket, incident slug, etc.).
    #[norito(default)]
    pub labels: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_round_trip_via_norito_codec() {
        let receipt = GarEnforcementReceiptV1 {
            receipt_id: *b"0123456789abcdef",
            gar_name: "docs.sora".to_string(),
            canonical_host: "docs.gateway.sora.net".to_string(),
            action: GarEnforcementActionV1::GeoFence,
            triggered_at_unix: 1_747_483_200,
            expires_at_unix: Some(1_747_569_600),
            policy_version: Some("2026-q2".to_string()),
            policy_digest: Some([0xAB; 32]),
            operator: AccountId::parse_encoded(
                "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("account id"),
            reason: "Guardian freeze window".to_string(),
            notes: Some("Escalated during SNNet-15 drill".to_string()),
            evidence_uris: vec![
                "sora://gar/receipts/docs/0123".to_string(),
                "https://ops.sora.net/incidents/SN15-0001".to_string(),
            ],
            labels: vec!["guardian-freeze".to_string(), "sn15-drill".to_string()],
        };
        let bytes = norito::to_bytes(&receipt).expect("encode receipt");
        let decoded: GarEnforcementReceiptV1 =
            norito::decode_from_bytes(&bytes).expect("decode receipt");
        assert_eq!(receipt, decoded);
    }

    #[cfg(feature = "json")]
    #[test]
    fn receipt_round_trip_via_json() {
        let receipt = GarEnforcementReceiptV1 {
            receipt_id: *b"fedcba9876543210",
            gar_name: "taikai.sora".to_string(),
            canonical_host: "taikai.cdn.sora.net".to_string(),
            action: GarEnforcementActionV1::Custom("purge-l7".to_string()),
            triggered_at_unix: 1_747_000_000,
            expires_at_unix: None,
            policy_version: None,
            policy_digest: None,
            operator: AccountId::parse_encoded(
                "6cmzPVPX8gQ65j3aqa3YHGbjs9CfKCxx1zJP4n2P7stQ4CwadqpmGED",
            )
            .expect("account id")
            .into_account_id(),
            reason: "Taikai stream purge drill".to_string(),
            notes: None,
            evidence_uris: vec!["sora://gar/receipts/taikai/purge-l7".to_string()],
            labels: vec!["taikai".to_string(), "purge".to_string()],
        };
        let json_bytes = norito::json::to_vec(&receipt).expect("encode json");
        let decoded: GarEnforcementReceiptV1 =
            norito::json::from_slice(&json_bytes).expect("decode json");
        assert_eq!(receipt, decoded);
    }

    #[cfg(feature = "json")]
    #[test]
    fn cdn_policy_round_trip_via_json() {
        let payload = GarPolicyPayloadV1 {
            license_sets: vec![],
            moderation_directives: vec![],
            cdn_policy: Some(GarCdnPolicyV1 {
                ttl_override_secs: Some(60),
                purge_tags: vec!["hotfix".to_string()],
                moderation_slugs: vec!["blocklist".to_string()],
                rate_ceiling_rps: Some(10),
                allow_regions: vec!["EU".to_string()],
                deny_regions: vec!["US".to_string()],
                legal_hold: true,
            }),
            metrics_policy: None,
            telemetry_labels: vec!["cdn".to_string()],
            rpt_digest: None,
        };

        let encoded = norito::json::to_vec(&payload).expect("encode policy");
        let decoded: GarPolicyPayloadV1 =
            norito::json::from_slice(&encoded).expect("decode policy");
        assert_eq!(payload, decoded);
    }
}
