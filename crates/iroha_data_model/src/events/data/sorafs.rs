//! `SoraFS` gateway compliance events exposed via the data event stream.

use iroha_data_model_derive::model;
use iroha_primitives::numeric::Quantity;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use getset::Getters;

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
        /// The runtime recorded a PDP/PoTR proof-health violation.
        ProofHealth(SorafsProofHealthAlert),
        /// Consensus committed a repair-task lifecycle transition.
        RepairLedger(SorafsRepairLedgerEvent),
        /// Consensus committed a moderation-ledger lifecycle transition.
        ModerationLedger(SorafsModerationLedgerEvent),
        /// Consensus committed an authoritative orderbook transition.
        OrderbookLedger(SorafsOrderbookLedgerEvent),
        /// Consensus committed an authoritative reserve-ledger transition.
        ReserveLedger(SorafsReserveLedgerEvent),
        /// Consensus committed a reputation recorder-policy or journal transition.
        ReputationJournal(SorafsReputationJournalEvent),
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
        /// Governed gateway-compliance decisions.
        GatewayCompliance,
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
        /// The governed gateway-compliance catalog denied the request.
        GatewayComplianceDenied,
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
        pub penalty_applied: Quantity,
        /// Whether the alert was suppressed due to a cooldown window.
        pub cooldown_active: bool,
    }

    /// Stable chain-authoritative repair transition category.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
    pub enum SorafsRepairLedgerEventKind {
        /// A source-identity-bound repair report was admitted.
        TaskSubmitted,
        /// A worker acquired an absent or expired lease.
        LeaseClaimed,
        /// The current worker extended its unexpired lease.
        LeaseRenewed,
        /// The task committed its successful terminal outcome.
        Completed,
        /// The task committed its unsuccessful terminal outcome.
        Failed,
        /// The task committed an escalated terminal outcome and slash proposal.
        Escalated,
        /// The provider owner committed the slash appeal.
        Appealed,
    }

    /// Typed event emitted by a finalized repair-ledger mutation.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsRepairLedgerEvent {
        /// Transition category.
        pub kind: SorafsRepairLedgerEventKind,
        /// Canonical ticket identifier.
        pub ticket_id: String,
        /// Immutable task identity.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub task_id: [u8; 32],
        /// Affected provider.
        pub provider_id: crate::sorafs::capacity::ProviderId,
        /// Affected manifest.
        pub manifest_digest: crate::sorafs::pin_registry::ManifestDigest,
        /// Resulting task revision.
        pub revision: u64,
        /// Transaction authority that committed the transition.
        pub authority: crate::account::AccountId,
        /// Committing block timestamp.
        pub occurred_at_unix_ms: u64,
    }

    /// Stable chain-authoritative moderation transition category.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
    pub enum SorafsModerationLedgerEventKind {
        /// A policy revision was activated.
        PolicyActivated,
        /// An appeal intake was admitted.
        AppealSubmitted,
        /// A juror eligibility proof was accepted.
        EligibilityRegistered,
        /// Deterministic sortition completed.
        SortitionFinalized,
        /// Sortition closed without enough eligible jurors.
        SortitionFailed,
        /// A selected juror accepted assignment.
        AssignmentAccepted,
        /// The case entered its commit/reveal lifecycle.
        CaseActivated,
        /// Assignment failover exhausted before a case could activate.
        CaseActivationFailed,
        /// A commitment was accepted.
        CommitAccepted,
        /// A challenge was raised.
        ChallengeRaised,
        /// A challenge was resolved.
        ChallengeResolved,
        /// A reveal was accepted.
        RevealAccepted,
        /// The single terminal outcome was committed.
        CaseFinalized,
    }

    /// Typed event emitted by a finalized moderation-ledger mutation.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsModerationLedgerEvent {
        /// Transition category.
        pub kind: SorafsModerationLedgerEventKind,
        /// Case identifier, absent only for policy activation.
        pub case_id: Option<String>,
        /// Round identifier, absent only for policy activation.
        pub round_id: Option<String>,
        /// Transaction authority that committed the transition.
        pub authority: crate::account::AccountId,
        /// Committing block timestamp.
        pub occurred_at_unix_ms: u64,
    }

    /// Stable chain-authoritative orderbook transition category.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
    pub enum SorafsOrderbookLedgerEventKind {
        /// A policy revision was activated.
        PolicyActivated,
        /// A signed order was admitted.
        OrderAdmitted,
        /// A signed owner or governance cancellation was committed.
        OrderCancelled,
        /// Deterministic matching committed a trade and funded channel.
        TradeMatched,
        /// Maintenance expired an unfilled or partially filled order.
        OrderExpired,
        /// Maintenance retired an ask after its exact admitted provider binding was revoked.
        OrderProviderRevoked,
        /// Maintenance expired an unsettled channel and refunded custody.
        ChannelExpired,
        /// A provider-signed settlement receipt was committed.
        ReceiptRecorded,
    }

    /// Typed event emitted by a finalized authoritative orderbook mutation.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsOrderbookLedgerEvent {
        /// Transition category.
        pub kind: SorafsOrderbookLedgerEventKind,
        /// Affected order, when the transition is order-specific.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::option")
        )]
        pub order_id: Option<[u8; 32]>,
        /// Affected trade, when present.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::option")
        )]
        pub trade_id: Option<[u8; 32]>,
        /// Affected settlement channel, when present.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::option")
        )]
        pub channel_id: Option<[u8; 32]>,
        /// Affected settlement receipt, when present.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::option")
        )]
        pub receipt_id: Option<[u8; 32]>,
        /// Affected provider, when known.
        pub provider_id: Option<crate::sorafs::capacity::ProviderId>,
        /// Resulting authoritative book revision.
        pub book_revision: u64,
        /// Transaction authority that committed the transition.
        pub authority: crate::account::AccountId,
        /// Committing block timestamp.
        pub occurred_at_unix_ms: u64,
    }

    /// Stable chain-authoritative reserve transition category.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
    pub enum SorafsReserveLedgerEventKind {
        /// A policy revision was activated.
        PolicyActivated,
        /// A provider reserve partition was registered.
        ProviderRegistered,
        /// A provider requested a custody movement.
        MovementRequested,
        /// Governance approved and applied a custody movement.
        MovementApproved,
        /// Governance rejected a custody movement.
        MovementRejected,
        /// Deterministic rent was charged.
        RentCharged,
        /// A provider lifecycle projection was advanced.
        LifecycleAdvanced,
        /// Protocol credit was drawn into reserve custody.
        CreditDrawn,
        /// Provider credit debt was repaid.
        CreditRepaid,
        /// A provider lifecycle appeal was submitted.
        AppealSubmitted,
        /// Governance accepted a provider lifecycle appeal.
        AppealAccepted,
        /// Governance rejected a provider lifecycle appeal.
        AppealRejected,
    }

    /// Typed event emitted by a finalized authoritative reserve mutation.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsReserveLedgerEvent {
        /// Transition category.
        pub kind: SorafsReserveLedgerEventKind,
        /// Provider affected by the transition, absent for policy activation.
        pub provider_id: Option<crate::sorafs::capacity::ProviderId>,
        /// Movement or appeal identifier, when the transition has one.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::option")
        )]
        pub operation_id: Option<[u8; 32]>,
        /// Active policy digest used by the transition.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Resulting provider revision, or zero for policy activation.
        pub provider_revision: u64,
        /// Resulting authoritative provider lifecycle stage, absent only for
        /// policy activation.
        pub resulting_lifecycle_stage: Option<crate::sorafs::reserve::ReserveLifecycleStage>,
        /// Transaction authority that committed the transition.
        pub authority: crate::account::AccountId,
        /// Committing block timestamp.
        pub occurred_at_unix_ms: u64,
    }

    /// Typed chain-authoritative reputation-journal transition.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
    pub enum SorafsReputationJournalEvent {
        /// Governance activated one strict predecessor-linked recorder policy.
        PolicyActivated(SorafsReputationJournalPolicyActivatedV1),
        /// Consensus appended one globally sequenced source projection.
        EntryCommitted(SorafsReputationJournalEntryCommittedV1),
    }

    /// Recorder-policy activation emitted by the authoritative journal.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsReputationJournalPolicyActivatedV1 {
        /// Canonical active policy digest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Activated policy revision.
        pub revision: u64,
        /// Governance authority that activated the revision.
        pub authority: crate::account::AccountId,
        /// Exact committing block timestamp.
        pub occurred_at_unix_ms: u64,
    }

    /// Globally sequenced journal entry emitted after native state mutation.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    pub struct SorafsReputationJournalEntryCommittedV1 {
        /// One-based global journal sequence.
        pub sequence: u64,
        /// Content-derived event identity.
        pub event_id: crate::sorafs::reputation::ReputationJournalEventIdV1,
        /// Domain-separated native source identity.
        pub source_id: crate::sorafs::reputation::ReputationJournalSourceIdV1,
        /// Stable source family.
        pub source_kind: crate::sorafs::reputation::ReputationJournalSourceKindV1,
        /// Source-local lifecycle revision.
        pub source_revision: u32,
        /// Provider whose deterministic counters consume the entry.
        pub provider_id: crate::sorafs::capacity::ProviderId,
        /// Active recorder-policy digest bound into the entry.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub policy_digest: [u8; 32],
        /// Exact governed recorder authority.
        pub authority: crate::account::AccountId,
        /// Authenticated source decision or observation time.
        pub source_time_unix_ms: u64,
        /// Authoritative committing-block timestamp.
        pub recorded_at_unix_ms: u64,
    }
}

impl SorafsModerationLedgerEvent {
    /// Construct a typed finalized moderation-ledger event.
    #[must_use]
    pub fn new(
        kind: SorafsModerationLedgerEventKind,
        case_id: Option<String>,
        round_id: Option<String>,
        authority: crate::account::AccountId,
        occurred_at_unix_ms: u64,
    ) -> Self {
        Self {
            kind,
            case_id,
            round_id,
            authority,
            occurred_at_unix_ms,
        }
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
        codec::Decode,
        json::{Error, FastJsonWrite, JsonDeserialize, Parser},
    };

    use super::{SorafsGarViolation, SorafsGatewayEvent};

    fn decode_from_base64<T: Decode>(encoded: &str) -> Result<T, Error> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .map_err(|err| Error::Message(err.to_string()))?;
        let mut cursor = Cursor::new(bytes.as_slice());
        Decode::decode(&mut cursor).map_err(|err| Error::Message(err.to_string()))
    }

    impl FastJsonWrite for SorafsGarViolation {
        fn write_json(&self, out: &mut String) {
            norito::json::write_bare_norito_base64_json(self, out);
        }

        fn write_json_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            norito::json::write_bare_norito_base64_json_to(self, out)
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
            norito::json::write_bare_norito_base64_json(self, out);
        }

        fn write_json_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            norito::json::write_bare_norito_base64_json_to(self, out)
        }
    }

    impl JsonDeserialize for SorafsGatewayEvent {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let encoded = parser.parse_string()?;
            decode_from_base64(&encoded)
        }
    }

    #[cfg(test)]
    mod tests {
        use base64::Engine as _;

        use super::*;

        fn assert_bare_base64_json<T>(value: &T)
        where
            T: norito::codec::Encode + norito::core::NoritoSerialize + norito::json::JsonSerialize,
        {
            let buffered = base64::engine::general_purpose::STANDARD.encode(value.encode());
            let expected = norito::json::to_json(&buffered).expect("serialize buffered base64");
            let direct = norito::json::to_json(value).expect("serialize direct base64");
            assert_eq!(direct, expected);
            assert_eq!(
                norito::json::to_json_bounded(value, expected.len())
                    .expect("serialize at exact bound"),
                expected
            );
            assert_eq!(
                norito::json::to_json_bounded(value, direct.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            );
        }

        #[test]
        fn gateway_event_families_stream_exact_bare_norito_base64() {
            let violation = SorafsGarViolation {
                policy: super::super::SorafsGarPolicy::RateLimit,
                detail: super::super::SorafsGarPolicyDetail::RateLimitExceeded,
                provider_id: None,
                manifest_digest: None,
                manifest_cid_b64: Some("cid".to_owned()),
                client_fingerprint_hex: "ab".repeat(32),
                remote_addr: Some("127.0.0.1:8080".to_owned()),
                retry_after_seconds: Some(5),
                region: None,
                host: None,
                policy_labels: vec!["limited".to_owned()],
                observed_ttl_seconds: None,
                rate_ceiling_rps: Some(10),
                occurred_at_unix: 42,
            };
            assert_bare_base64_json(&violation);
            assert_bare_base64_json(&SorafsGatewayEvent::GarViolation(violation));
        }
    }
}

/// Prelude exports for `SoraFS` gateway events.
pub mod prelude {
    pub use super::{
        SorafsGarPolicy, SorafsGarPolicyDetail, SorafsGarViolation, SorafsGatewayEvent,
        SorafsGatewayEventSet, SorafsModerationLedgerEvent, SorafsModerationLedgerEventKind,
        SorafsOrderbookLedgerEvent, SorafsOrderbookLedgerEventKind, SorafsProofHealthAlert,
        SorafsRepairLedgerEvent, SorafsRepairLedgerEventKind,
        SorafsReputationJournalEntryCommittedV1, SorafsReputationJournalEvent,
        SorafsReputationJournalPolicyActivatedV1, SorafsReserveLedgerEvent,
        SorafsReserveLedgerEventKind,
    };
}
