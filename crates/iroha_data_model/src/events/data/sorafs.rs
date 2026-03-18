//! `SoraFS` gateway compliance events exposed via the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Events emitted by the `SoraFS` gateway compliance surface.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum SorafsGatewayEvent {
        /// The gateway rejected a request due to a GAR policy violation.
        GarViolation(SorafsGarViolation),
        /// Torii recorded deal usage telemetry for the provided window.
        DealUsage(SorafsDealUsage),
        /// Torii finalised a deal settlement and published the DAG artefact.
        DealSettlement(SorafsDealSettlement),
        /// The runtime recorded a PDP/PoTR proof-health violation.
        ProofHealth(SorafsProofHealthAlert),
    }

    /// High-level policy classification for a GAR violation.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum SorafsGarPolicy {
        /// Manifest envelope requirements.
        ManifestEnvelope,
        /// Provider identity checks.
        Provider,
        /// Admission registry enforcement.
        Admission,
        /// Governance/compliance denylist decisions.
        Denylist,
        /// Gateway rate limiting.
        RateLimit,
        /// CDN and runtime enforcement derived from GAR policy.
        Cdn,
    }

    /// Detailed policy outcome for a GAR violation.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum SorafsGarPolicyDetail {
        /// Manifest envelope was required but not supplied.
        ManifestEnvelopeMissing,
        /// Provider identifier was not supplied with the request.
        ProviderIdMissing,
        /// Admission registry information was unavailable.
        AdmissionUnavailable,
        /// Provider failed admission checks.
        ProviderNotAdmitted,
        /// Request matched a denylisted provider.
        DenylistedProvider,
        /// Request matched a denylisted manifest digest.
        DenylistedManifestDigest,
        /// Request matched a denylisted content identifier.
        DenylistedCid,
        /// Request matched a denylisted URL.
        DenylistedUrl,
        /// Request matched a denylisted account identifier.
        DenylistedAccountId,
        /// Request matched a denylisted account alias.
        DenylistedAccountAlias,
        /// Request matched a denylisted perceptual family.
        DenylistedPerceptualFamily,
        /// Request exceeded the configured rate limit window.
        RateLimitExceeded,
        /// Request was temporarily banned due to repeated rate limit violations.
        RateLimitBanned,
        /// Request TTL exceeded the GAR-configured override.
        CdnTtlExceeded,
        /// Required purge tag was missing when serving cached content.
        CdnPurgeRequired,
        /// Request failed moderation enforcement.
        CdnModerationBlocked,
        /// Request exceeded the GAR-configured rate ceiling.
        CdnRateCeilingExceeded,
        /// Request originated from a denied region or lacked an allowed region.
        CdnGeofenceDenied,
        /// Request blocked due to an active legal hold.
        CdnLegalHoldActive,
    }

    /// Payload describing a GAR policy violation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct SorafsGarViolation {
        /// Policy that triggered the violation.
        pub policy: SorafsGarPolicy,
        /// Detailed outcome for the violation.
        pub detail: SorafsGarPolicyDetail,
        /// Provider identifier associated with the request, when available.
        pub provider_id: Option<crate::sorafs::capacity::ProviderId>,
        /// Manifest digest associated with the request, when available.
        pub manifest_digest: Option<crate::sorafs::pin_registry::ManifestDigest>,
        /// Content identifier (CID) encoded as base64, when provided.
        pub manifest_cid_b64: Option<String>,
        /// Fingerprint of the client (hex encoded BLAKE3 digest).
        pub client_fingerprint_hex: String,
        /// Remote socket address of the client, when available.
        pub remote_addr: Option<String>,
        /// Jurisdiction associated with the denylist entry, if any.
        pub jurisdiction: Option<String>,
        /// Human-readable reason recorded for the denylist entry, if any.
        pub reason: Option<String>,
        /// Optional alias tied to the denylist entry metadata.
        pub entry_alias: Option<String>,
        /// Issuance timestamp of the denylist entry (seconds since UNIX epoch), when available.
        pub issued_at_unix: Option<u64>,
        /// Expiry timestamp of the denylist entry (seconds since UNIX epoch), when available.
        pub expires_at_unix: Option<u64>,
        /// Base64-encoded content identifier matched by the denylist, when provided.
        pub denylisted_cid_b64: Option<String>,
        /// URL matched by the denylist rule, when provided.
        pub denylisted_url: Option<String>,
        /// Account identifier matched by the denylist rule, when provided.
        pub denylisted_account_id: Option<String>,
        /// Account alias matched by the denylist rule, when provided.
        pub denylisted_account_alias: Option<String>,
        /// Perceptual family identifier matched by the denylist rule.
        pub denylisted_perceptual_family_hex: Option<String>,
        /// Perceptual variant identifier matched by the denylist rule.
        pub denylisted_perceptual_variant_hex: Option<String>,
        /// Canonical perceptual hash recorded for the denylist match.
        pub denylisted_perceptual_hash_hex: Option<String>,
        /// Embedding digest recorded for the denylist match.
        pub denylisted_perceptual_embedding_hex: Option<String>,
        /// Hamming distance observed for the perceptual hash match.
        pub perceptual_hamming_distance: Option<u8>,
        /// Configured Hamming radius for the denylist entry.
        pub perceptual_hamming_radius: Option<u8>,
        /// Suggested retry window for rate limiting (seconds), when provided.
        pub retry_after_seconds: Option<u64>,
        /// Observed region used during CDN policy enforcement.
        #[norito(default)]
        pub region: Option<String>,
        /// Optional host name associated with the request.
        #[norito(default)]
        pub host: Option<String>,
        /// Policy labels or tags attached to the GAR violation.
        #[norito(default)]
        pub policy_labels: Vec<String>,
        /// Observed TTL in seconds when applying CDN TTL overrides.
        #[norito(default)]
        pub observed_ttl_seconds: Option<u64>,
        /// Configured rate ceiling applied during CDN enforcement.
        #[norito(default)]
        pub rate_ceiling_rps: Option<u64>,
        /// Timestamp when the violation occurred (seconds since UNIX epoch).
        pub occurred_at_unix: u64,
    }

    /// Usage telemetry emitted after Torii records a deal usage report.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct SorafsDealUsage {
        /// Deal identifier.
        pub deal_id: crate::sorafs::deal::DealId,
        /// Provider that services the deal.
        pub provider_id: crate::sorafs::prelude::ProviderId,
        /// Client responsible for the deal.
        pub client_id: crate::sorafs::deal::ClientId,
        /// Epoch attributed to the usage sample.
        pub epoch: u64,
        /// Storage GiB-hours recorded for the sample.
        pub storage_gib_hours: u64,
        /// Egress bytes recorded for the sample.
        pub egress_bytes: u64,
        /// Deterministic charge accumulated during the sample.
        pub deterministic_charge_nano: u128,
        /// Micropayment credit generated during the sample.
        pub micropayment_credit_generated_nano: u128,
        /// Micropayment credit applied immediately against the charge.
        pub micropayment_credit_applied_nano: u128,
        /// Micropayment credit carried forward to future windows.
        pub micropayment_credit_carry_nano: u128,
        /// Outstanding balance after applying credit.
        pub outstanding_nano: u128,
        /// Total tickets processed in the sample.
        pub tickets_processed: u64,
        /// Tickets that resulted in a payout.
        pub tickets_won: u64,
        /// Tickets discarded as duplicates.
        pub tickets_duplicate: u64,
    }

    /// Settlement summary emitted after Torii finalises a deal window.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct SorafsDealSettlement {
        /// Deterministic settlement ledger record.
        pub record: crate::sorafs::deal::DealSettlementRecord,
        /// BLAKE3 digest of the encoded governance payload.
        pub governance_encoded_blake3: [u8; 32],
        /// Length of the encoded governance payload in bytes.
        pub governance_encoded_len: u64,
        /// Base64-encoded governance payload as published to the DAG.
        pub governance_encoded_b64: String,
    }

    /// Payload describing a PDP/PoTR proof failure alert emitted by the runtime.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct SorafsProofHealthAlert {
        /// Provider identifier.
        pub provider_id: crate::sorafs::prelude::ProviderId,
        /// Start epoch (inclusive) of the telemetry window that triggered the alert.
        pub window_start_epoch: u64,
        /// End epoch (inclusive) of the telemetry window that triggered the alert.
        pub window_end_epoch: u64,
        /// Number of strikes accumulated prior to forcing the penalty threshold.
        pub prior_strikes: u32,
        /// Strike threshold configured by governance.
        pub strike_threshold: u32,
        /// Number of PDP challenges issued in the window.
        pub pdp_challenges: u32,
        /// Number of PDP failures recorded in the window.
        pub pdp_failures: u32,
        /// Number of `PoTR` windows evaluated in the window.
        pub potr_windows: u32,
        /// Number of `PoTR` breaches recorded in the window.
        pub potr_breaches: u32,
        /// Whether PDP failures triggered the alert.
        pub triggered_by_pdp: bool,
        /// Whether `PoTR` breaches triggered the alert.
        pub triggered_by_potr: bool,
        /// Maximum PDP failures tolerated by policy during each window.
        pub max_pdp_failures: u32,
        /// Maximum `PoTR` breaches tolerated by policy during each window.
        pub max_potr_breaches: u32,
        /// Bond slashing ratio (basis points) configured for penalties.
        pub penalty_bond_bps: u16,
        /// Amount of collateral slashed when enforcing the alert (0 when suppressed).
        pub penalty_applied_nano: u128,
        /// Whether the alert was suppressed due to a cooldown window.
        pub cooldown_active: bool,
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SorafsGarPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SorafsGarPolicyDetail {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SorafsGarViolation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SorafsGatewayEvent {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

#[cfg(feature = "json")]
mod json_support {
    use std::io::Cursor;

    use base64::Engine as _;
    use norito::{
        codec::{Decode, Encode},
        json::{Error, FastJsonWrite, JsonDeserialize, JsonSerialize, Parser},
    };

    use super::{SorafsGarViolation, SorafsGatewayEvent};

    fn encode_base64<T: Encode>(value: &T) -> String {
        let bytes = value.encode();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn decode_from_base64<T: Decode>(encoded: &str) -> Result<T, Error> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .map_err(|err| Error::Message(err.to_string()))?;
        let mut cursor = Cursor::new(bytes.as_slice());
        Decode::decode(&mut cursor).map_err(|err| Error::Message(err.to_string()))
    }

    impl FastJsonWrite for SorafsGarViolation {
        fn write_json(&self, out: &mut String) {
            let encoded = encode_base64(self);
            JsonSerialize::json_serialize(&encoded, out);
        }
    }

    impl JsonDeserialize for SorafsGarViolation {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let encoded = parser.parse_string()?;
            decode_from_base64(&encoded)
        }
    }

    impl FastJsonWrite for SorafsGatewayEvent {
        fn write_json(&self, out: &mut String) {
            let encoded = encode_base64(self);
            JsonSerialize::json_serialize(&encoded, out);
        }
    }

    impl JsonDeserialize for SorafsGatewayEvent {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let encoded = parser.parse_string()?;
            decode_from_base64(&encoded)
        }
    }
}

/// Prelude exports for `SoraFS` gateway events.
pub mod prelude {
    pub use super::{
        SorafsGarPolicy, SorafsGarPolicyDetail, SorafsGarViolation, SorafsGatewayEvent,
        SorafsGatewayEventSet, SorafsProofHealthAlert,
    };
}
