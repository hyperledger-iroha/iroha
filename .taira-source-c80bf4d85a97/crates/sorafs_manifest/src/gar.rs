//! Gateway Authorization Record policy payload types.
//!
//! When the optional `iroha_data_model_bridge` feature is enabled we re-export
//! the canonical types from `iroha_data_model`. Otherwise we ship lightweight
//! local definitions so dependants (e.g. `iroha_js_host`) can parse GAR payloads
//! without pulling the entire data model crate, avoiding cyclic dependencies.

#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "iroha_data_model_bridge")]
pub use iroha_data_model::sorafs::gar::{
    GarCdnPolicyV1, GarLicenseSetV1, GarMetricsPolicyV1, GarModerationAction,
    GarModerationDirectiveV1, GarPolicyPayloadV1,
};

#[cfg(not(feature = "iroha_data_model_bridge"))]
mod local {
    /// Licensing bundle referenced by a GAR payload.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct GarLicenseSetV1 {
        /// Human-readable identifier (e.g., `sg-2026-pilot`).
        pub slug: String,
        /// Jurisdiction or regulatory body that issued the licence.
        pub jurisdiction: String,
        /// Legal entity that holds the broadcast licence.
        pub holder: String,
        /// Optional Unix timestamp (seconds) when the licence becomes valid.
        pub valid_from_unix: Option<u64>,
        /// Optional Unix timestamp (seconds) when the licence expires.
        pub valid_until_unix: Option<u64>,
        /// Optional URI or document reference for auditors.
        pub reference_uri: Option<String>,
    }

    /// CDN-facing policy embedded in GAR v2 payloads.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct GarCdnPolicyV1 {
        /// Optional TTL override applied by gateways (seconds).
        pub ttl_override_secs: Option<u64>,
        /// Purge tags that must be present before serving cached content.
        pub purge_tags: Vec<String>,
        /// Moderation directive slugs that apply to this GAR.
        pub moderation_slugs: Vec<String>,
        /// Optional request-per-second ceiling enforced at the gateway edge.
        pub rate_ceiling_rps: Option<u64>,
        /// Regions that are explicitly permitted.
        pub allow_regions: Vec<String>,
        /// Regions that are explicitly denied.
        pub deny_regions: Vec<String>,
        /// Whether the GAR is under a legal hold (serving is blocked).
        pub legal_hold: bool,
    }

    /// Moderation directive embedded in GAR v2.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct GarModerationDirectiveV1 {
        /// Unique label for the directive.
        pub slug: String,
        /// Action gateways must apply when the directive matches.
        pub action: GarModerationAction,
        /// Sensitivity classes affected by the directive (optional).
        pub sensitivity_classes: Vec<String>,
        /// Optional governance notes shown in dashboards/runbooks.
        pub notes: Option<String>,
    }

    /// Moderation action enforced by the directive.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum GarModerationAction {
        /// Allow the request but record the directive metadata.
        Allow,
        /// Allow the request with an explicit warning/notice.
        Warn,
        /// Quarantine the request until an operator reviews it.
        Quarantine,
        /// Block the request outright.
        #[default]
        Block,
    }

    impl GarModerationAction {
        /// Returns the canonical string representation used in GAR payloads.
        #[must_use]
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Allow => "allow",
                Self::Warn => "warn",
                Self::Quarantine => "quarantine",
                Self::Block => "block",
            }
        }
    }

    /// Metrics/telemetry policy surfaced through GAR.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct GarMetricsPolicyV1 {
        /// Identifier used in dashboards and audit reports.
        pub policy_id: String,
        /// Sampling budget expressed in basis points (0-10_000).
        pub sampling_bps: u16,
        /// Maximum retention window for the captured metrics (seconds).
        pub retention_secs: u64,
        /// Named metrics that the policy allows (e.g., `audience`, `rebuffer`).
        pub allowed_metrics: Vec<String>,
    }

    /// Structured policy payload embedded in GAR v2.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    pub struct GarPolicyPayloadV1 {
        /// Licensing bundles attached to the GAR.
        pub license_sets: Vec<GarLicenseSetV1>,
        /// Moderation directives enforced by the gateway.
        pub moderation_directives: Vec<GarModerationDirectiveV1>,
        /// CDN-facing policy envelope.
        pub cdn_policy: Option<GarCdnPolicyV1>,
        /// Optional metrics/telemetry policy contract.
        pub metrics_policy: Option<GarMetricsPolicyV1>,
        /// Canonical telemetry labels emitted with GAR violations.
        pub telemetry_labels: Vec<String>,
        /// Digest of the latest Replication Proof Token bundle, when available.
        pub rpt_digest: Option<[u8; 32]>,
    }
}

#[cfg(not(feature = "iroha_data_model_bridge"))]
pub use local::{
    GarCdnPolicyV1, GarLicenseSetV1, GarMetricsPolicyV1, GarModerationAction,
    GarModerationDirectiveV1, GarPolicyPayloadV1,
};
