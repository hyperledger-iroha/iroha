//! Scoreboard builder for the SoraFS multi-source fetch orchestrator.
//!
//! The scoreboard converts manifest metadata, provider adverts, and telemetry
//! snapshots into weighted [`FetchProvider`](crate::multi_fetch::FetchProvider)
//! instances that the orchestrator can schedule deterministically. Each run
//! evaluates capability constraints, honouring range and stream budgets, and
//! applies the weighting formula described in `specs/sorafs_orchestrator_plan.md`.
use crate::{
    CarBuildPlan, ChunkFetchSpec,
    multi_fetch::{CapabilityMismatch, FetchProvider, ProviderMetadata, provider_can_serve_chunk},
};
use norito::json::{Map, Number, Value, to_string_pretty};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
/// Default cap (in milliseconds) applied when normalising latency scores.
const DEFAULT_LATENCY_CAP_MS: u32 = 5_000;
/// Default integer scale used when converting normalised weights into scheduler credits.
const DEFAULT_WEIGHT_SCALE: NonZeroU32 = NonZeroU32::MIN.saturating_add(9_999);
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
    /// Build a telemetry snapshot from a published SoraFS reputation snapshot.
    #[cfg(feature = "manifest")]
    pub fn from_reputation_snapshot(
        snapshot: &sorafs_manifest::ReputationSnapshotV1,
    ) -> Result<Self, sorafs_manifest::ReputationValidationError> {
        snapshot.validate()?;
        let records = snapshot
            .providers
            .iter()
            .map(|provider| {
                let mut telemetry = ProviderTelemetry::new(provider.provider_id.clone());
                telemetry.reputation_score_bps = Some(provider.score_bps);
                telemetry.last_updated_unix = Some(snapshot.generated_at_unix);
                telemetry
            })
            .collect::<Vec<_>>();
        Ok(Self::from_records(records))
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
    /// Published reputation score in basis points (0-10_000).
    pub reputation_score_bps: Option<u16>,
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
            reputation_score_bps: None,
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
        let path = path.as_ref();
        let value = self.to_json_value(metadata)?;
        let mut json = to_string_pretty(&value).map_err(io::Error::other)?;
        json.push('\n');
        write_output_bytes(path, "scoreboard", json.as_bytes())
    }
    fn to_json_value(&self, metadata: Option<Value>) -> io::Result<Value> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| io::Error::other("failed to reserve scoreboard JSON entry inventory"))?;
        for entry in &self.entries {
            let mut map = Map::new();
            map.insert(
                "provider_id".into(),
                Value::String(entry.provider.id().as_str().to_string()),
            );
            map.insert(
                "normalised_weight".into(),
                Value::Number(number_from_f64(entry.normalised_weight)?),
            );
            map.insert(
                "raw_score".into(),
                Value::Number(number_from_f64(entry.raw_score)?),
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
            entries.push(Value::Object(map));
        }
        let mut root = Map::new();
        root.insert("entries".into(), Value::Array(entries));
        if let Some(meta) = metadata {
            root.insert("metadata".into(), meta);
        }
        Ok(Value::Object(root))
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
fn write_output_bytes(path: &Path, label: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = open_output_file(path, label)?;
    file.write_all(bytes)
}
fn open_output_file(path: &Path, label: &str) -> io::Result<fs::File> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options.open(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to open {label} `{}`: {err}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to inspect {label} `{}` after open: {err}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "failed to write {label} `{}`: output must be a regular file",
            path.display()
        )));
    }
    Ok(file)
}
fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!(
                    "failed to create output parent `{}`: {err}",
                    parent.display()
                ),
            )
        })?;
    }
    Ok(())
}
fn validate_output_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a symlink",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a directory",
                    path.display()
                )));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to inspect output `{}`: {err}", path.display()),
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        )));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!(
                            "failed to inspect output parent `{}`: {err}",
                            ancestor.display()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}
/// Build a scoreboard for the supplied manifest and provider metadata.
pub fn build_scoreboard(
    plan: &CarBuildPlan,
    providers: &[ProviderMetadata],
    telemetry: &TelemetrySnapshot,
    config: &ScoreboardConfig,
) -> io::Result<Scoreboard> {
    let chunk_specs = plan
        .try_chunk_fetch_specs()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(providers.len()).map_err(|_| {
        io::Error::other(format!(
            "failed to reserve {} scoreboard entries",
            providers.len()
        ))
    })?;
    let mut eligible_indices = Vec::new();
    eligible_indices
        .try_reserve_exact(providers.len())
        .map_err(|_| {
            io::Error::other(format!(
                "failed to reserve {} scoreboard eligibility entries",
                providers.len()
            ))
        })?;
    let mut raw_scores = Vec::new();
    raw_scores.try_reserve_exact(providers.len()).map_err(|_| {
        io::Error::other(format!(
            "failed to reserve {} scoreboard score entries",
            providers.len()
        ))
    })?;
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
                provider = provider.with_weight(NonZeroU32::MIN);
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
    )?;
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
    let reputation_component = telemetry
        .and_then(|t| t.reputation_score_bps)
        .map(|score| f64::from(score.min(10_000)) / 10_000.0)
        .unwrap_or(1.0)
        .clamp(0.05, 1.0);
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
    qos_component
        * latency_component
        * failure_component
        * token_component
        * reputation_component
        * staking_weight
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
) -> io::Result<()> {
    if eligible_indices.is_empty() {
        return Ok(());
    }
    if raw_scores
        .iter()
        .any(|score| !score.is_finite() || *score < 0.0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider raw scores must be finite and non-negative",
        ));
    }
    let total: f64 = raw_scores.iter().copied().sum();
    if !total.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider raw score total is non-finite",
        ));
    }
    if total > f64::EPSILON {
        for (raw, idx) in raw_scores.iter().zip(eligible_indices.iter()) {
            let normalised = (raw / total).clamp(0.0, 1.0);
            let weight = weight_from_normalised(normalised, weight_scale)?;
            let entry = entries.get_mut(*idx).ok_or_else(|| {
                io::Error::other("scoreboard eligible index exceeded entry inventory")
            })?;
            entry.normalised_weight = normalised;
            entry.provider = entry.provider.clone().with_weight(weight);
        }
    } else {
        let eligible_count = u32::try_from(eligible_indices.len())
            .map_err(|_| io::Error::other("eligible provider count exceeds u32"))?;
        let equal_weight = 1.0 / f64::from(eligible_count);
        for idx in eligible_indices {
            let weight = weight_from_normalised(equal_weight, weight_scale)?;
            let entry = entries.get_mut(*idx).ok_or_else(|| {
                io::Error::other("scoreboard eligible index exceeded entry inventory")
            })?;
            entry.normalised_weight = equal_weight;
            entry.provider = entry.provider.clone().with_weight(weight);
        }
    }
    Ok(())
}
fn weight_from_normalised(value: f64, scale: NonZeroU32) -> io::Result<NonZeroU32> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "normalised provider weight must be finite and within 0..=1",
        ));
    }
    let scaled = (value * f64::from(scale.get()))
        .ceil()
        .clamp(1.0, f64::from(u32::MAX));
    NonZeroU32::new(scaled as u32).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "normalised provider weight unexpectedly rounded to zero",
        )
    })
}
fn number_from_f64(value: f64) -> io::Result<Number> {
    Number::from_f64(value).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "scoreboard JSON contains a non-finite number",
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use blake3::Hash;
    use norito::json::Value;
    use sorafs_chunker::ChunkProfile;
    use tempfile::{TempDir, tempdir};
    #[test]
    fn non_finite_scoreboard_numbers_are_rejected() {
        assert!(weight_from_normalised(f64::NAN, DEFAULT_WEIGHT_SCALE).is_err());
        assert!(weight_from_normalised(-0.1, DEFAULT_WEIGHT_SCALE).is_err());
        assert!(weight_from_normalised(1.1, DEFAULT_WEIGHT_SCALE).is_err());
        assert!(number_from_f64(f64::INFINITY).is_err());
        assert_eq!(
            weight_from_normalised(0.5, DEFAULT_WEIGHT_SCALE)
                .expect("finite weight")
                .get(),
            5_000
        );
    }
    use crate::multi_fetch::{RangeCapability, StreamBudget};
    fn canonical_tempdir() -> (TempDir, PathBuf) {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().canonicalize().expect("canonical tempdir");
        (temp, path)
    }
    fn plan_with_chunk(length: u32) -> CarBuildPlan {
        CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: Hash::from([0u8; 32]),
            content_length: u64::from(length),
            chunks: vec![crate::CarChunk {
                offset: 0,
                length,
                digest: [0u8; 32],
            }],
            files: vec![crate::FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 1,
                size: u64::from(length),
            }],
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
                reputation_score_bps: None,
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
                reputation_score_bps: None,
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
        let (_tmp, tmp_path) = canonical_tempdir();
        let scoreboard_path = tmp_path.join("scoreboard.json");
        let config = ScoreboardConfig {
            persist_path: Some(scoreboard_path.clone()),
            now_unix_secs: 1_000,
            ..ScoreboardConfig::default()
        };
        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        assert_eq!(scoreboard.entries().len(), 1);
        let persisted = std::fs::read_to_string(scoreboard_path).expect("read scoreboard");
        let value: Value = norito::json::from_str(&persisted).expect("parse scoreboard json");
        assert!(
            value.get("entries").is_some(),
            "entries missing in persisted json"
        );
    }
    #[test]
    fn scoreboard_persist_creates_nested_parent() {
        let (_tmp, tmp_path) = canonical_tempdir();
        let scoreboard_path = tmp_path.join("nested").join("scoreboard.json");
        let scoreboard = Scoreboard::new(Vec::new());
        scoreboard
            .persist_to_path(&scoreboard_path, None)
            .expect("persist scoreboard");
        let persisted = fs::read_to_string(scoreboard_path).expect("read scoreboard");
        assert!(
            persisted.contains("\"entries\""),
            "scoreboard JSON missing entries: {persisted}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn scoreboard_persist_rejects_symlink_output() {
        let (_tmp, tmp_path) = canonical_tempdir();
        let target_path = tmp_path.join("target.json");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let scoreboard_path = tmp_path.join("scoreboard.json");
        std::os::unix::fs::symlink(&target_path, &scoreboard_path).expect("create symlink");
        let scoreboard = Scoreboard::new(Vec::new());
        let err = scoreboard
            .persist_to_path(&scoreboard_path, None)
            .expect_err("reject symlink output");
        let message = err.to_string();
        assert!(
            message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }
    #[cfg(unix)]
    #[test]
    fn scoreboard_persist_rejects_symlink_parent() {
        let (_tmp, tmp_path) = canonical_tempdir();
        let real_dir = tmp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = tmp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let scoreboard_path = linked_dir.join("scoreboard.json");
        let scoreboard = Scoreboard::new(Vec::new());
        let err = scoreboard
            .persist_to_path(&scoreboard_path, None)
            .expect_err("reject symlink parent");
        let message = err.to_string();
        assert!(
            message.contains("parent") && message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join("scoreboard.json").exists(),
            "symlink parent should not receive output"
        );
    }
    #[test]
    fn reputation_score_reduces_provider_weight_without_exclusion() {
        let plan = plan_with_chunk(1_024);
        let providers = vec![base_metadata("provider-a"), base_metadata("provider-b")];
        let telemetry = TelemetrySnapshot::from_records([
            {
                let mut record = ProviderTelemetry::new("provider-a");
                record.reputation_score_bps = Some(9_000);
                record
            },
            {
                let mut record = ProviderTelemetry::new("provider-b");
                record.reputation_score_bps = Some(1_000);
                record
            },
        ]);
        let config = ScoreboardConfig {
            now_unix_secs: 1_000,
            ..ScoreboardConfig::default()
        };
        let scoreboard =
            build_scoreboard(&plan, &providers, &telemetry, &config).expect("build scoreboard");
        let entries = scoreboard.entries();
        assert!(matches!(entries[0].eligibility, Eligibility::Eligible));
        assert!(matches!(entries[1].eligibility, Eligibility::Eligible));
        assert!(
            entries[0].normalised_weight > entries[1].normalised_weight,
            "higher reputation should receive greater routing weight"
        );
    }
    #[cfg(feature = "manifest")]
    #[test]
    fn telemetry_snapshot_can_be_derived_from_reputation_snapshot() {
        use sorafs_manifest::{
            REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
            ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
            ReputationWeightsV1, build_reputation_snapshot,
        };
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: 9_500,
                pdp_success_bps: 9_500,
                potr_success_bps: 9_500,
                latency_health_bps: 9_500,
                dispute_rate_bps: 0,
                token_violation_rate_bps: 0,
                repair_breach_rate_bps: 0,
            },
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let snapshot = build_reputation_snapshot(
            [0x11; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input],
            None,
        )
        .expect("reputation snapshot");
        let telemetry =
            TelemetrySnapshot::from_reputation_snapshot(&snapshot).expect("telemetry snapshot");
        assert_eq!(
            telemetry
                .get("provider-a")
                .and_then(|record| record.reputation_score_bps),
            Some(snapshot.providers[0].score_bps)
        );
    }
}
