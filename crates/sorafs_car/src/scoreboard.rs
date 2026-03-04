//! Scoreboard builder for the SoraFS multi-source fetch orchestrator.
//!
//! The scoreboard converts manifest metadata, provider adverts, and telemetry
//! snapshots into weighted [`FetchProvider`](crate::multi_fetch::FetchProvider)
//! instances that the orchestrator can schedule deterministically. Each run
//! evaluates capability constraints, honouring range and stream budgets, and
//! applies the weighting formula described in `docs/source/sorafs_orchestrator_plan.md`.

use std::{
    collections::HashMap,
    fs, io,
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use norito::json::{Map, Number, Value, to_string_pretty};

use crate::{
    CarBuildPlan, ChunkFetchSpec,
    multi_fetch::{CapabilityMismatch, FetchProvider, ProviderMetadata, provider_can_serve_chunk},
};

/// Default cap (in milliseconds) applied when normalising latency scores.
const DEFAULT_LATENCY_CAP_MS: u32 = 5_000;
/// Default integer scale used when converting normalised weights into scheduler credits.
const DEFAULT_WEIGHT_SCALE: NonZeroU32 = match NonZeroU32::new(10_000) {
    Some(scale) => scale,
    None => panic!("weight scale must be non-zero"),
};
/// Default grace window for telemetry freshness checks.
const DEFAULT_TELEMETRY_GRACE: Duration = Duration::from_secs(900);

/// Configuration for the scoreboard builder.
#[derive(Debug, Clone)]
pub struct ScoreboardConfig {
    /// Maximum latency (ms) considered when normalising latency scores.
    pub latency_cap_ms: u32,
    /// Weight scale applied to normalised scores (must be > 0).
    pub weight_scale: NonZeroU32,
    /// Grace window for telemetry freshness; stale snapshots beyond the window mark providers ineligible.
    pub telemetry_grace_period: Duration,
    /// Optional on-disk destination for persisted scoreboard artefacts.
    pub persist_path: Option<PathBuf>,
    /// Optional metadata blob to persist alongside the entries.
    pub persist_metadata: Option<Value>,
    /// Unix timestamp (seconds) used when evaluating advert validity.
    pub now_unix_secs: u64,
}

impl Default for ScoreboardConfig {
    fn default() -> Self {
        let now_unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            latency_cap_ms: DEFAULT_LATENCY_CAP_MS,
            weight_scale: DEFAULT_WEIGHT_SCALE,
            telemetry_grace_period: DEFAULT_TELEMETRY_GRACE,
            persist_path: None,
            persist_metadata: None,
            now_unix_secs,
        }
    }
}

/// Eligibility outcome for a provider.
#[derive(Debug, Clone, PartialEq)]
pub enum Eligibility {
    /// Provider satisfied all checks and carries a positive weight.
    Eligible,
    /// Provider failed capability or policy checks.
    Ineligible(IneligibilityReason),
}

/// Reasons a provider was excluded from the scoreboard.
#[derive(Debug, Clone, PartialEq)]
pub enum IneligibilityReason {
    /// Required provider identifier missing from advert metadata.
    MissingProviderId,
    /// Provider capabilities cannot satisfy the manifest requirements.
    Capability(CapabilityMismatch),
    /// Advert refresh deadline exceeded.
    RefreshDeadlineElapsed { refresh_deadline: u64 },
    /// Advert expiry exceeded.
    Expired { expires_at: u64 },
    /// Telemetry snapshot marked the provider as penalised.
    TelemetryPenalty,
    /// Telemetry snapshot is stale beyond the configured grace window.
    TelemetryStale { last_updated: u64 },
}

/// Snapshot of runtime telemetry for providers.
#[derive(Debug, Clone, Default)]
pub struct TelemetrySnapshot {
    providers: HashMap<String, ProviderTelemetry>,
}

impl TelemetrySnapshot {
    /// Construct a telemetry snapshot from an iterator of provider records.
    #[must_use]
    pub fn from_records<I>(records: I) -> Self
    where
        I: IntoIterator<Item = ProviderTelemetry>,
    {
        let mut providers = HashMap::new();
        for record in records {
            providers.insert(record.provider_id.clone(), record);
        }
        Self { providers }
    }

    /// Fetch telemetry for the given provider identifier.
    #[must_use]
    pub fn get(&self, provider_id: &str) -> Option<&ProviderTelemetry> {
        self.providers.get(provider_id)
    }

    /// Iterate over all telemetry records.
    pub fn iter(&self) -> impl Iterator<Item = &ProviderTelemetry> {
        self.providers.values()
    }
}

/// Per-provider telemetry inputs used by the scoreboard.
#[derive(Debug, Clone)]
pub struct ProviderTelemetry {
    /// Provider identifier (matches advert metadata).
    pub provider_id: String,
    /// Quality-of-service score (0-100).
    pub qos_score: Option<f64>,
    /// Latency P95 in milliseconds.
    pub latency_p95_ms: Option<f64>,
    /// EWMA of failure rate (0-1).
    pub failure_rate_ewma: Option<f64>,
    /// Token health score (0-1).
    pub token_health: Option<f64>,
    /// Staking weight multiplier.
    pub staking_weight: Option<f64>,
    /// Whether telemetry flagged the provider with a penalty.
    pub penalty: bool,
    /// Unix timestamp of the telemetry snapshot (seconds).
    pub last_updated_unix: Option<u64>,
}

impl ProviderTelemetry {
    /// Create a telemetry record with the supplied identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            provider_id: id.into(),
            qos_score: None,
            latency_p95_ms: None,
            failure_rate_ewma: None,
            token_health: None,
            staking_weight: None,
            penalty: false,
            last_updated_unix: None,
        }
    }
}

/// Scoreboard entry describing a provider outcome.
#[derive(Debug, Clone)]
pub struct ScoreboardEntry {
    /// Normalised weight (0-1) assigned by the scoreboard.
    pub normalised_weight: f64,
    /// Raw score before normalisation.
    pub raw_score: f64,
    /// Provider instance with metadata and assigned weight.
    pub provider: FetchProvider,
    /// Eligibility outcome for the provider.
    pub eligibility: Eligibility,
}

impl ScoreboardEntry {
    fn new(provider: FetchProvider, eligibility: Eligibility) -> Self {
        Self {
            normalised_weight: 0.0,
            raw_score: 0.0,
            provider,
            eligibility,
        }
    }
}

/// Scoreboard artefact emitted by the builder.
#[derive(Debug, Clone)]
pub struct Scoreboard {
    entries: Vec<ScoreboardEntry>,
}

impl Scoreboard {
    fn new(entries: Vec<ScoreboardEntry>) -> Self {
        Self { entries }
    }

    /// Returns the scoreboard entries.
    #[must_use]
    pub fn entries(&self) -> &[ScoreboardEntry] {
        &self.entries
    }

    /// Consumes the scoreboard returning providers that are eligible.
    #[must_use]
    pub fn into_providers(self) -> Vec<FetchProvider> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry.eligibility {
                Eligibility::Eligible => Some(entry.provider),
                Eligibility::Ineligible(_) => None,
            })
            .collect()
    }

    /// Persist the scoreboard as a Norito JSON document at `path`.
    pub fn persist_to_path(
        &self,
        path: impl AsRef<Path>,
        metadata: Option<Value>,
    ) -> io::Result<()> {
        let value = self.to_json_value(metadata);
        let json = to_string_pretty(&value).map_err(io::Error::other)?;
        if let Some(parent) = path.as_ref().parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{json}\n"))
    }

    fn to_json_value(&self, metadata: Option<Value>) -> Value {
        let entries: Vec<Value> = self
            .entries
            .iter()
            .map(|entry| {
                let mut map = Map::new();
                map.insert(
                    "provider_id".into(),
                    Value::String(entry.provider.id().as_str().to_string()),
                );
                map.insert(
                    "normalised_weight".into(),
                    Value::Number(number_from_f64(entry.normalised_weight)),
                );
                map.insert(
                    "raw_score".into(),
                    Value::Number(number_from_f64(entry.raw_score)),
                );
                map.insert(
                    "eligibility".into(),
                    match &entry.eligibility {
                        Eligibility::Eligible => Value::String("eligible".into()),
                        Eligibility::Ineligible(reason) => {
                            let mut reason_map = Map::new();
                            reason_map.insert("status".into(), Value::String("ineligible".into()));
                            reason_map.insert("reason".into(), Value::String(reason.to_string()));
                            Value::Object(reason_map)
                        }
                    },
                );
                Value::Object(map)
            })
            .collect();
        let mut root = Map::new();
        root.insert("entries".into(), Value::Array(entries));
        if let Some(meta) = metadata {
            root.insert("metadata".into(), meta);
        }
        Value::Object(root)
    }
}

impl std::fmt::Display for IneligibilityReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProviderId => write!(f, "missing provider identifier"),
            Self::Capability(reason) => write!(f, "{reason}"),
            Self::RefreshDeadlineElapsed { refresh_deadline } => {
                write!(f, "refresh deadline {refresh_deadline} has elapsed")
            }
            Self::Expired { expires_at } => write!(f, "advert expired at {expires_at}"),
            Self::TelemetryPenalty => write!(f, "telemetry penalty active"),
            Self::TelemetryStale { last_updated } => {
                write!(f, "telemetry stale (last updated at {last_updated})")
            }
        }
    }
}

/// Build a scoreboard for the supplied manifest and provider metadata.
pub fn build_scoreboard(
    plan: &CarBuildPlan,
    providers: &[ProviderMetadata],
    telemetry: &TelemetrySnapshot,
    config: &ScoreboardConfig,
) -> io::Result<Scoreboard> {
    let chunk_specs = plan.chunk_fetch_specs();
    let mut entries = Vec::with_capacity(providers.len());
    let mut eligible_indices = Vec::new();
    let mut raw_scores = Vec::new();

    for metadata in providers {
        let mut provider = match derive_provider(metadata) {
            Ok(provider) => provider,
            Err(reason) => {
                entries.push(ScoreboardEntry::new(
                    FetchProvider::new("<unknown>"),
                    Eligibility::Ineligible(reason),
                ));
                continue;
            }
        };

        let eligibility = match evaluate_eligibility(
            &provider,
            metadata,
            &chunk_specs,
            telemetry.get(provider.id().as_str()),
            config,
        ) {
            Ok(score_inputs) => {
                let raw = compute_raw_score(score_inputs);
                raw_scores.push(raw);
                eligible_indices.push(entries.len());
                provider = provider.with_weight(NonZeroU32::new(1).expect("constant non-zero"));
                let mut entry = ScoreboardEntry::new(provider, Eligibility::Eligible);
                entry.raw_score = raw;
                entries.push(entry);
                continue;
            }
            Err(reason) => Eligibility::Ineligible(reason),
        };

        entries.push(ScoreboardEntry::new(provider, eligibility));
    }

    normalise_weights(
        &mut entries,
        &eligible_indices,
        &raw_scores,
        config.weight_scale,
    );

    let scoreboard = Scoreboard::new(entries);
    if let Some(path) = &config.persist_path {
        scoreboard.persist_to_path(path, config.persist_metadata.clone())?;
    }
    Ok(scoreboard)
}

fn derive_provider(metadata: &ProviderMetadata) -> Result<FetchProvider, IneligibilityReason> {
    let identifier = metadata
        .provider_id
        .as_ref()
        .or_else(|| metadata.profile_aliases.first())
        .cloned()
        .ok_or(IneligibilityReason::MissingProviderId)?;

    let mut provider = FetchProvider::new(identifier);
    provider = provider.with_metadata(metadata.clone());

    if let Some(non_zero) = metadata
        .stream_budget
        .as_ref()
        .and_then(|budget| NonZeroUsize::new(usize::from(budget.max_in_flight.max(1))))
    {
        provider = provider.with_max_concurrent_chunks(non_zero);
    } else if let Some(non_zero) = metadata
        .max_streams
        .and_then(|max_streams| NonZeroUsize::new(max_streams.max(1).into()))
    {
        provider = provider.with_max_concurrent_chunks(non_zero);
    }

    Ok(provider)
}

struct ScoreInputs<'a> {
    metadata: &'a ProviderMetadata,
    telemetry: Option<&'a ProviderTelemetry>,
    config: &'a ScoreboardConfig,
}

fn evaluate_eligibility<'a>(
    provider: &'a FetchProvider,
    metadata: &'a ProviderMetadata,
    chunk_specs: &'a [ChunkFetchSpec],
    telemetry: Option<&'a ProviderTelemetry>,
    config: &'a ScoreboardConfig,
) -> Result<ScoreInputs<'a>, IneligibilityReason> {
    if let Some(deadline) = metadata.refresh_deadline
        && deadline <= config.now_unix_secs
    {
        return Err(IneligibilityReason::RefreshDeadlineElapsed {
            refresh_deadline: deadline,
        });
    }
    if let Some(expires_at) = metadata.expires_at
        && expires_at <= config.now_unix_secs
    {
        return Err(IneligibilityReason::Expired { expires_at });
    }

    if let Some(record) = telemetry {
        if record.penalty {
            return Err(IneligibilityReason::TelemetryPenalty);
        }
        let grace_secs = config.telemetry_grace_period.as_secs();
        if let Some(last) = record.last_updated_unix
            && config.now_unix_secs > last
            && config.now_unix_secs - last > grace_secs
        {
            return Err(IneligibilityReason::TelemetryStale { last_updated: last });
        }
    }

    for spec in chunk_specs {
        provider_can_serve_chunk(provider, spec).map_err(IneligibilityReason::Capability)?;
    }

    Ok(ScoreInputs {
        metadata,
        telemetry,
        config,
    })
}

fn compute_raw_score(inputs: ScoreInputs<'_>) -> f64 {
    let ScoreInputs {
        metadata,
        telemetry,
        config,
    } = inputs;
    let qos = telemetry
        .and_then(|t| t.qos_score)
        .unwrap_or(100.0)
        .clamp(0.0, 100.0)
        / 100.0;
    let latency = telemetry
        .and_then(|t| t.latency_p95_ms)
        .unwrap_or(0.0)
        .max(0.0);
    let failure = telemetry
        .and_then(|t| t.failure_rate_ewma)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let token_health = telemetry
        .and_then(|t| t.token_health)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let qos_component = qos.clamp(0.1, 1.0);
    let latency_cap = f64::from(config.latency_cap_ms.max(1));
    let latency_component = (1.0 - (latency / latency_cap)).clamp(0.1, 1.0);
    let failure_component = (1.0 - failure).clamp(0.0, 1.0);
    let token_component = if token_health >= 0.8 {
        1.0
    } else {
        token_health.clamp(0.0, 1.0)
    };

    let staking_weight = telemetry
        .and_then(|t| t.staking_weight)
        .or_else(|| parse_stake_amount(metadata))
        .unwrap_or(1.0)
        .clamp(0.5, 3.0);

    qos_component * latency_component * failure_component * token_component * staking_weight
}

fn parse_stake_amount(metadata: &ProviderMetadata) -> Option<f64> {
    metadata
        .stake_amount
        .as_ref()
        .and_then(|amount| amount.parse::<f64>().ok())
        .map(|value| value.max(0.0))
}

fn normalise_weights(
    entries: &mut [ScoreboardEntry],
    eligible_indices: &[usize],
    raw_scores: &[f64],
    weight_scale: NonZeroU32,
) {
    if eligible_indices.is_empty() {
        return;
    }

    let total: f64 = raw_scores.iter().copied().sum();
    if total > f64::EPSILON {
        for (raw, idx) in raw_scores.iter().zip(eligible_indices.iter()) {
            let normalised = (raw / total).clamp(0.0, 1.0);
            let weight = weight_from_normalised(normalised, weight_scale);
            let entry = &mut entries[*idx];
            entry.normalised_weight = normalised;
            entry.provider = entry.provider.clone().with_weight(weight);
        }
    } else {
        let equal_weight = 1.0 / f64::from(eligible_indices.len() as u32);
        for idx in eligible_indices {
            let weight = weight_from_normalised(equal_weight, weight_scale);
            let entry = &mut entries[*idx];
            entry.normalised_weight = equal_weight;
            entry.provider = entry.provider.clone().with_weight(weight);
        }
    }
}

fn weight_from_normalised(value: f64, scale: NonZeroU32) -> NonZeroU32 {
    let scaled = (value * f64::from(scale.get())).ceil().max(1.0);
    NonZeroU32::new(scaled as u32).unwrap_or_else(|| NonZeroU32::new(1).expect("non-zero"))
}

fn number_from_f64(value: f64) -> Number {
    Number::from_f64(value).unwrap_or_else(|| Number::from(0_u64))
}

#[cfg(test)]
mod tests {
    use blake3::Hash;
    use norito::json::Value;
    use sorafs_chunker::ChunkProfile;

    use super::*;
    use crate::multi_fetch::{RangeCapability, StreamBudget};

    fn plan_with_chunk(length: u32) -> CarBuildPlan {
        CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: Hash::from([0u8; 32]),
            content_length: u64::from(length),
            chunks: vec![crate::CarChunk {
                offset: 0,
                length,
                digest: [0u8; 32],
                taikai_segment_hint: None,
            }],
            files: Vec::new(),
        }
    }

    fn base_metadata(id: &str) -> ProviderMetadata {
        ProviderMetadata {
            provider_id: Some(id.to_string()),
            range_capability: Some(RangeCapability {
                max_chunk_span: 2_048,
                min_granularity: 1,
                supports_sparse_offsets: true,
                requires_alignment: false,
                supports_merkle_proof: true,
            }),
            stream_budget: Some(StreamBudget {
                max_in_flight: 4,
                max_bytes_per_sec: 10 * 1024 * 1024,
                burst_bytes: Some(2_048),
            }),
            ..ProviderMetadata::default()
        }
    }

    #[test]
    fn provider_within_capabilities_is_eligible() {
        let plan = plan_with_chunk(1_024);
        let providers = vec![base_metadata("provider-a"), base_metadata("provider-b")];
        let telemetry = TelemetrySnapshot::from_records([
            ProviderTelemetry {
                provider_id: "provider-a".into(),
                qos_score: Some(90.0),
                latency_p95_ms: Some(120.0),
                failure_rate_ewma: Some(0.05),
                token_health: Some(0.95),
                staking_weight: Some(1.2),
                penalty: false,
                last_updated_unix: Some(1_000),
            },
            ProviderTelemetry {
                provider_id: "provider-b".into(),
                qos_score: Some(70.0),
                latency_p95_ms: Some(800.0),
                failure_rate_ewma: Some(0.2),
                token_health: Some(0.7),
                staking_weight: Some(0.8),
                penalty: false,
                last_updated_unix: Some(1_000),
            },
        ]);

        let config = ScoreboardConfig {
            now_unix_secs: 1_100,
            ..ScoreboardConfig::default()
        };

        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        assert_eq!(scoreboard.entries().len(), 2);

        let eligible: Vec<_> = scoreboard
            .entries()
            .iter()
            .filter(|entry| matches!(entry.eligibility, Eligibility::Eligible))
            .collect();
        assert_eq!(eligible.len(), 2);
        let weights: Vec<f64> = eligible
            .iter()
            .map(|entry| entry.normalised_weight)
            .collect();
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn provider_exceeding_chunk_span_is_ineligible() {
        let plan = plan_with_chunk(8_192);
        let providers = vec![base_metadata("provider-a")];
        let telemetry = TelemetrySnapshot::default();
        let config = ScoreboardConfig::default();

        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        let entry = &scoreboard.entries()[0];
        assert!(matches!(
            entry.eligibility,
            Eligibility::Ineligible(IneligibilityReason::Capability(_))
        ));
    }

    #[test]
    fn provider_with_penalty_is_ineligible() {
        let plan = plan_with_chunk(1_024);
        let providers = vec![base_metadata("provider-a")];
        let telemetry = TelemetrySnapshot::from_records([ProviderTelemetry {
            provider_id: "provider-a".into(),
            penalty: true,
            ..ProviderTelemetry::new("provider-a")
        }]);
        let config = ScoreboardConfig {
            now_unix_secs: 10,
            ..ScoreboardConfig::default()
        };

        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        let entry = &scoreboard.entries()[0];
        assert!(matches!(
            entry.eligibility,
            Eligibility::Ineligible(IneligibilityReason::TelemetryPenalty)
        ));
    }

    #[test]
    fn scoreboard_persist_writes_json() {
        let plan = plan_with_chunk(1_024);
        let providers = vec![base_metadata("provider-a")];
        let telemetry = TelemetrySnapshot::default();
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = ScoreboardConfig {
            persist_path: Some(tmp.path().join("scoreboard.json")),
            now_unix_secs: 1_000,
            ..ScoreboardConfig::default()
        };

        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        assert_eq!(scoreboard.entries().len(), 1);

        let persisted =
            std::fs::read_to_string(tmp.path().join("scoreboard.json")).expect("read scoreboard");
        let value: Value = norito::json::from_str(&persisted).expect("parse scoreboard json");
        assert!(
            value.get("entries").is_some(),
            "entries missing in persisted json"
        );
    }
}
