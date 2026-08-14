//! Deterministic fixtures for multi-peer SoraFS chunk-store scenarios.
//!
//! This module builds on the existing chunk-store utilities to provide
//! repeatable inputs for orchestrator tests. The fixtures expose a canonical
//! payload, the derived [`CarBuildPlan`], provider metadata, and telemetry
//! snapshots so integration tests can exercise end-to-end fetch flows without
//! standing up real storage nodes.
use sorafs_chunker::{ChunkProfile, fixtures::FixtureProfile};
use crate::{
    CarBuildPlan, CarPlanError,
    multi_fetch::{ProviderMetadata, RangeCapability, StreamBudget, TransportHint},
    scoreboard::{ProviderTelemetry, TelemetrySnapshot},
};
const MAX_FIXTURE_PROVIDERS: usize = 1_024;
/// Errors returned while constructing deterministic multi-peer fixtures.
#[derive(Debug, thiserror::Error)]
pub enum MultiPeerFixtureError {
    /// At least one provider is required.
    #[error("fixture provider count must be greater than zero")]
    NoProviders,
    /// Provider inventory exceeded the bounded fixture ceiling.
    #[error("fixture provider count {count} exceeds maximum {maximum}")]
    TooManyProviders { count: usize, maximum: usize },
    /// Canonical CAR planning failed.
    #[error(transparent)]
    Plan(#[from] CarPlanError),
    /// A bounded fixture allocation could not be reserved.
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
}
/// Unix timestamp (seconds) used across fixture artefacts.
const FIXTURE_NOW_UNIX_SECS: u64 = 1_725_000_000;
/// Deterministic multi-peer fixture derived from the canonical chunk-store vectors.
#[derive(Debug, Clone)]
pub struct MultiPeerFixture {
    plan: CarBuildPlan,
    payload: Vec<u8>,
    provider_payloads: Vec<Vec<u8>>,
    providers: Vec<ProviderMetadata>,
    telemetry: TelemetrySnapshot,
    max_chunk_length: u32,
}
impl MultiPeerFixture {
    /// Builds a fixture populated with `provider_count` mock providers.
    ///
    /// Each provider exposes the same canonical payload but carries unique
    /// identifiers and telemetry so schedulers can differentiate between them.
    /// The resulting metadata satisfies capability checks for the default SF1
    /// chunk profile.
    pub fn with_providers(provider_count: usize) -> Result<Self, MultiPeerFixtureError> {
        if provider_count == 0 {
            return Err(MultiPeerFixtureError::NoProviders);
        }
        if provider_count > MAX_FIXTURE_PROVIDERS {
            return Err(MultiPeerFixtureError::TooManyProviders {
                count: provider_count,
                maximum: MAX_FIXTURE_PROVIDERS,
            });
        }
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let payload = vectors.input.clone();
        let plan = CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT)?;
        let max_chunk_length = plan
            .chunks
            .iter()
            .map(|chunk| chunk.length)
            .max()
            .unwrap_or(0);
        let mut provider_payloads = Vec::new();
        try_reserve_fixture(
            &mut provider_payloads,
            provider_count,
            "fixture provider payloads",
        )?;
        for _ in 0..provider_count {
            let mut replica = Vec::new();
            try_reserve_fixture(&mut replica, payload.len(), "fixture payload replica")?;
            replica.extend_from_slice(&payload);
            provider_payloads.push(replica);
        }
        let providers = build_provider_metadata(provider_count, max_chunk_length)?;
        let telemetry = build_telemetry(&providers)?;
        Ok(Self {
            plan,
            payload,
            provider_payloads,
            providers,
            telemetry,
            max_chunk_length,
        })
    }
    /// Returns the canonical payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Returns the chunk fetch plan derived from the payload.
    #[must_use]
    pub fn plan(&self) -> &CarBuildPlan {
        &self.plan
    }
    /// Returns the metadata records describing each mock provider.
    #[must_use]
    pub fn providers(&self) -> &[ProviderMetadata] {
        &self.providers
    }
    /// Returns the telemetry snapshot aligned with the provider metadata.
    #[must_use]
    pub fn telemetry(&self) -> &TelemetrySnapshot {
        &self.telemetry
    }
    /// Returns per-provider payload replicas.
    #[must_use]
    pub fn provider_payloads(&self) -> &[Vec<u8>] {
        &self.provider_payloads
    }
    /// Returns the longest chunk length present in the plan.
    #[must_use]
    pub fn max_chunk_length(&self) -> u32 {
        self.max_chunk_length
    }
    /// Returns the canonical timestamp used when evaluating advert freshness.
    #[must_use]
    pub fn now_unix_secs(&self) -> u64 {
        FIXTURE_NOW_UNIX_SECS
    }
}
fn try_reserve_fixture<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), MultiPeerFixtureError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| MultiPeerFixtureError::AllocationFailed {
            context,
            requested: additional,
        })
}
fn build_provider_metadata(
    count: usize,
    max_chunk_length: u32,
) -> Result<Vec<ProviderMetadata>, MultiPeerFixtureError> {
    let mut providers = Vec::new();
    try_reserve_fixture(&mut providers, count, "fixture provider metadata")?;
    for idx in 0..count {
        let mut metadata = ProviderMetadata::new();
        metadata.provider_id = Some(format!("fixture-provider-{idx}"));
        metadata.profile_aliases = vec![format!("fixture-peer-{idx}")];
        metadata.availability = Some("hot".into());
        metadata.capability_names = vec![
            "chunk-range".into(),
            "sorafs-fetch".into(),
            "sorafs-fixture".into(),
        ];
        metadata.allow_unknown_capabilities = false;
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: max_chunk_length,
            min_granularity: 1,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        });
        metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 1,
            max_bytes_per_sec: u64::from(max_chunk_length).saturating_mul(8),
            burst_bytes: Some(u64::from(max_chunk_length).saturating_mul(4)),
        });
        metadata.refresh_deadline = Some(FIXTURE_NOW_UNIX_SECS + 600);
        metadata.expires_at = Some(FIXTURE_NOW_UNIX_SECS + 86_400);
        metadata.ttl_secs = Some(86_400);
        metadata.notes = Some("Deterministic fixture provider".into());
        metadata.transport_hints = vec![TransportHint {
            protocol: "fixture".into(),
            protocol_id: 0,
            priority: 0,
        }];
        providers.push(metadata);
    }
    Ok(providers)
}
fn build_telemetry(
    providers: &[ProviderMetadata],
) -> Result<TelemetrySnapshot, MultiPeerFixtureError> {
    let mut records = Vec::new();
    try_reserve_fixture(&mut records, providers.len(), "fixture telemetry")?;
    for (idx, metadata) in providers.iter().enumerate() {
        if let Some(provider_id) = metadata.provider_id.as_ref() {
            let mut telemetry = ProviderTelemetry::new(provider_id.clone());
            telemetry.qos_score = Some(95.0 - (idx as f64));
            telemetry.latency_p95_ms = Some(120.0 + (idx as f64 * 15.0));
            telemetry.failure_rate_ewma = Some(0.02 * (idx as f64));
            telemetry.token_health = Some(0.9);
            telemetry.staking_weight = Some(1.0);
            telemetry.reputation_score_bps = Some(10_000);
            telemetry.last_updated_unix = Some(FIXTURE_NOW_UNIX_SECS - 120);
            records.push(telemetry);
        }
    }
    Ok(TelemetrySnapshot::from_records(records))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_provider_count_is_bounded_without_panicking() {
        assert!(matches!(
            MultiPeerFixture::with_providers(0),
            Err(MultiPeerFixtureError::NoProviders)
        ));
        assert!(matches!(
            MultiPeerFixture::with_providers(MAX_FIXTURE_PROVIDERS + 1),
            Err(MultiPeerFixtureError::TooManyProviders { .. })
        ));
    }
    #[test]
    fn fixture_builds_valid_bounded_provider_inventory() {
        let fixture = MultiPeerFixture::with_providers(2).expect("build fixture");
        assert_eq!(fixture.providers().len(), 2);
        assert_eq!(fixture.provider_payloads().len(), 2);
        assert_eq!(fixture.payload(), fixture.provider_payloads()[0]);
    }
}
