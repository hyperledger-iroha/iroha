//! Shared local multi-fetch harness used by language bindings and tests.
//!
//! This module mirrors the behaviour of the developer tooling surfaces
//! (`sorafsMultiFetchLocal`, FFI bindings) so that parity checks can rely on a
//! single source of truth for converting plan/metadata JSON into orchestrator
//! inputs.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
};

use blake3::hash as blake3_hash;
use norito::json::Value;
use sorafs_chunker::ChunkProfile;
use thiserror::Error;

use crate::{
    CarBuildPlan, CarChunk, ChunkFetchSpec, FilePlan, chunker_registry,
    fetch_plan::{FetchPlanError, chunk_fetch_specs_from_json},
    multi_fetch::{
        self, ChunkResponse, FetchOptions, FetchOutcome, FetchProvider, FetchRequest,
        ProviderScoreContext, ProviderScoreDecision, ScorePolicy,
    },
    scoreboard::{self, Eligibility, ProviderTelemetry, ScoreboardConfig},
};

/// Provider specification accepted by the local fetch harness.
#[derive(Debug, Clone)]
pub struct LocalProviderInput {
    pub name: String,
    pub path: PathBuf,
    pub max_concurrent: Option<u32>,
    pub weight: Option<u32>,
    pub metadata: Option<ProviderMetadataInput>,
}

/// Provider metadata fields mirrored from discovery adverts.
#[derive(Debug, Clone, Default)]
pub struct ProviderMetadataInput {
    pub provider_id: Option<String>,
    pub profile_id: Option<String>,
    pub profile_aliases: Option<Vec<String>>,
    pub availability: Option<String>,
    pub stake_amount: Option<String>,
    pub max_streams: Option<u32>,
    pub refresh_deadline: Option<u64>,
    pub expires_at: Option<u64>,
    pub ttl_secs: Option<u64>,
    pub allow_unknown_capabilities: Option<bool>,
    pub capability_names: Option<Vec<String>>,
    pub rendezvous_topics: Option<Vec<String>>,
    pub notes: Option<String>,
    pub range_capability: Option<RangeCapabilityInput>,
    pub stream_budget: Option<StreamBudgetInput>,
    pub transport_hints: Option<Vec<TransportHintInput>>,
}

/// Range capability description supplied by providers.
#[derive(Debug, Clone)]
pub struct RangeCapabilityInput {
    pub max_chunk_span: u32,
    pub min_granularity: u32,
    pub supports_sparse_offsets: Option<bool>,
    pub requires_alignment: Option<bool>,
    pub supports_merkle_proof: Option<bool>,
}

/// Stream quota information supplied by providers.
#[derive(Debug, Clone)]
pub struct StreamBudgetInput {
    pub max_in_flight: u16,
    pub max_bytes_per_sec: u64,
    pub burst_bytes: Option<u64>,
}

/// Transport hints describing how to reach a provider.
#[derive(Debug, Clone)]
pub struct TransportHintInput {
    pub protocol: String,
    pub protocol_id: u8,
    pub priority: u8,
}

/// Telemetry snapshot entry used for scoreboard weighting.
#[derive(Debug, Clone)]
pub struct TelemetryEntryInput {
    pub provider_id: String,
    pub qos_score: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub failure_rate_ewma: Option<f64>,
    pub token_health: Option<f64>,
    pub staking_weight: Option<f64>,
    pub penalty: Option<bool>,
    pub last_updated_unix: Option<u64>,
}

/// Optional tuning knobs for local multi-fetch execution.
#[derive(Debug, Default, Clone)]
pub struct LocalFetchOptions {
    pub verify_digests: Option<bool>,
    pub verify_lengths: Option<bool>,
    pub retry_budget: Option<u32>,
    pub provider_failure_threshold: Option<u32>,
    pub max_parallel: Option<u32>,
    pub max_peers: Option<u32>,
    pub chunker_handle: Option<String>,
    pub telemetry_region: Option<String>,
    pub telemetry: Vec<TelemetryEntryInput>,
    pub use_scoreboard: Option<bool>,
    pub scoreboard_now_unix_secs: Option<u64>,
    pub deny_providers: Vec<String>,
    pub boost_providers: Vec<(String, i64)>,
    pub return_scoreboard: Option<bool>,
}

/// Scoreboard entry exported by the harness.
#[derive(Debug, Clone)]
pub struct LocalFetchScoreboardEntry {
    pub provider_id: String,
    pub alias: String,
    pub raw_score: f64,
    pub normalized_weight: f64,
    pub eligibility: String,
}

type ScoreboardSummary = (
    Option<Vec<LocalFetchScoreboardEntry>>,
    HashSet<String>,
    HashMap<String, NonZeroU32>,
);

/// Successful local fetch outcome.
#[derive(Debug)]
pub struct LocalFetchResult {
    pub chunk_count: usize,
    pub outcome: FetchOutcome,
    pub scoreboard: Option<Vec<LocalFetchScoreboardEntry>>,
    pub telemetry_region: Option<String>,
}

/// Errors surfaced by the local fetch harness.
#[derive(Debug, Error)]
pub enum LocalFetchError {
    #[error("providers list must contain at least one entry")]
    NoProviders,
    #[error("duplicate provider '{0}'")]
    DuplicateProvider(String),
    #[error("provider payload '{path}' does not exist")]
    ProviderPathMissing { path: PathBuf },
    #[error("provider payload '{path}' is not a regular file")]
    ProviderPathNotFile { path: PathBuf },
    #[error("max_concurrent must be greater than zero when provided")]
    InvalidMaxConcurrent,
    #[error("weight must be greater than zero when provided")]
    InvalidWeight,
    #[error("invalid chunk fetch plan: {0}")]
    InvalidPlan(#[from] FetchPlanError),
    #[error(
        "scoreboard requires metadata for provider '{0}' (provide advert metadata or disable use_scoreboard)"
    )]
    MissingScoreboardMetadata(String),
    #[error("no providers available after applying scoreboard filters")]
    ScoreboardExcludedAll,
    #[error("failed to build scoreboard: {0}")]
    ScoreboardBuild(std::io::Error),
    #[error("multi-fetch failed: {0}")]
    Fetch(String),
    #[error("unknown chunker handle '{0}'")]
    UnknownChunkerHandle(String),
}

/// Execute a local multi-provider fetch using preloaded provider descriptors.
pub fn execute_local_fetch(
    plan_json: &Value,
    providers: Vec<LocalProviderInput>,
    options: LocalFetchOptions,
) -> Result<LocalFetchResult, LocalFetchError> {
    if providers.is_empty() {
        return Err(LocalFetchError::NoProviders);
    }

    let mut processed = process_providers(providers)?;
    let mut path_lookup = build_path_lookup(&processed);

    let specs = chunk_fetch_specs_from_json(plan_json)?;
    if specs.is_empty() {
        return Err(LocalFetchError::Fetch(
            "chunk fetch plan must contain at least one chunk".into(),
        ));
    }

    let chunk_profile = resolve_chunk_profile(options.chunker_handle.as_deref())?;
    let plan = build_plan(&specs, chunk_profile);

    let telemetry_snapshot = telemetry_snapshot(&options.telemetry);
    let telemetry_provided = !options.telemetry.is_empty();
    let scoreboard_requested = options.use_scoreboard.unwrap_or(false) || telemetry_provided;

    let mut alias_by_provider_id = HashMap::new();
    let include_scoreboard = options.return_scoreboard.unwrap_or(scoreboard_requested);
    let (scoreboard_entries, eligible_aliases, weight_by_alias) = if scoreboard_requested {
        let list = ensure_scoreboard_metadata(
            &mut processed,
            &mut path_lookup,
            &mut alias_by_provider_id,
        )?;
        let mut config = ScoreboardConfig::default();
        if let Some(now) = options.scoreboard_now_unix_secs {
            config.now_unix_secs = now;
        }
        let scoreboard = scoreboard::build_scoreboard(&plan, &list, &telemetry_snapshot, &config)
            .map_err(LocalFetchError::ScoreboardBuild)?;
        extract_scoreboard(&scoreboard, &alias_by_provider_id, include_scoreboard)?
    } else {
        (None, HashSet::new(), HashMap::new())
    };

    let fetch_providers = build_fetch_providers(
        processed,
        &eligible_aliases,
        &weight_by_alias,
        scoreboard_requested,
        options.max_peers,
    )?;

    let fetch_options = build_fetch_options(&options);
    let outcome = run_fetch(&plan, fetch_providers, path_lookup, fetch_options)?;

    Ok(LocalFetchResult {
        chunk_count: specs.len(),
        outcome,
        scoreboard: scoreboard_entries,
        telemetry_region: options.telemetry_region.clone(),
    })
}

// --- internal helpers ----------------------------------------------------

struct ProcessedProvider {
    name: String,
    path: PathBuf,
    max_concurrent: NonZeroUsize,
    weight: Option<NonZeroU32>,
    metadata: Option<ProviderMetadataInput>,
}

fn process_providers(
    providers: Vec<LocalProviderInput>,
) -> Result<Vec<ProcessedProvider>, LocalFetchError> {
    let mut seen = HashSet::new();
    let mut processed = Vec::with_capacity(providers.len());
    for provider in providers {
        let name = provider.name.clone();
        if !seen.insert(name.clone()) {
            return Err(LocalFetchError::DuplicateProvider(name));
        }
        let path = provider.path;
        if !path.exists() {
            return Err(LocalFetchError::ProviderPathMissing { path });
        }
        if !is_regular_file(&path) {
            return Err(LocalFetchError::ProviderPathNotFile { path });
        }
        let max_concurrent = provider
            .max_concurrent
            .map(|value| NonZeroUsize::new(value as usize))
            .unwrap_or_else(|| NonZeroUsize::new(2))
            .ok_or(LocalFetchError::InvalidMaxConcurrent)?;
        let weight = match provider.weight {
            Some(value) => NonZeroU32::new(value)
                .ok_or(LocalFetchError::InvalidWeight)
                .map(Some)?,
            None => None,
        };
        processed.push(ProcessedProvider {
            name,
            path,
            max_concurrent,
            weight,
            metadata: provider.metadata,
        });
    }
    Ok(processed)
}

fn is_regular_file(path: &Path) -> bool {
    path.is_file()
}

fn build_path_lookup(providers: &[ProcessedProvider]) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    for provider in providers {
        map.insert(provider.name.clone(), provider.path.clone());
    }
    map
}

fn resolve_chunk_profile(handle: Option<&str>) -> Result<ChunkProfile, LocalFetchError> {
    if let Some(handle) = handle {
        if let Some(entry) = chunker_registry::lookup_by_handle(handle) {
            return Ok(entry.profile);
        }
        return Err(LocalFetchError::UnknownChunkerHandle(handle.to_string()));
    }
    Ok(ChunkProfile::DEFAULT)
}

fn build_plan(specs: &[ChunkFetchSpec], chunk_profile: ChunkProfile) -> CarBuildPlan {
    let content_length = specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .unwrap_or(0);
    CarBuildPlan {
        chunk_profile,
        payload_digest: blake3_hash(&[]),
        content_length,
        chunks: specs
            .iter()
            .map(|spec| CarChunk {
                offset: spec.offset,
                length: spec.length,
                digest: spec.digest,
                taikai_segment_hint: spec.taikai_segment_hint.clone(),
            })
            .collect(),
        files: vec![FilePlan {
            path: vec!["payload.bin".to_string()],
            first_chunk: 0,
            chunk_count: specs.len(),
            size: content_length,
        }],
    }
}

fn telemetry_snapshot(entries: &[TelemetryEntryInput]) -> scoreboard::TelemetrySnapshot {
    let records = entries
        .iter()
        .map(|entry| {
            let mut telemetry = ProviderTelemetry::new(entry.provider_id.clone());
            telemetry.qos_score = entry.qos_score;
            telemetry.latency_p95_ms = entry.latency_p95_ms;
            telemetry.failure_rate_ewma = entry.failure_rate_ewma;
            telemetry.token_health = entry.token_health;
            telemetry.staking_weight = entry.staking_weight;
            telemetry.penalty = entry.penalty.unwrap_or(false);
            telemetry.last_updated_unix = entry.last_updated_unix;
            telemetry
        })
        .collect::<Vec<_>>();
    scoreboard::TelemetrySnapshot::from_records(records)
}

fn ensure_scoreboard_metadata(
    providers: &mut [ProcessedProvider],
    path_lookup: &mut HashMap<String, PathBuf>,
    alias_by_provider_id: &mut HashMap<String, String>,
) -> Result<Vec<crate::multi_fetch::ProviderMetadata>, LocalFetchError> {
    let mut list = Vec::with_capacity(providers.len());
    for provider in providers.iter_mut() {
        let metadata_input = provider
            .metadata
            .clone()
            .ok_or_else(|| LocalFetchError::MissingScoreboardMetadata(provider.name.clone()))?;
        let metadata = metadata_input.into_provider_metadata(&provider.name)?;
        if let Some(provider_id) = metadata.provider_id.as_ref() {
            alias_by_provider_id.insert(provider_id.clone(), provider.name.clone());
            if !path_lookup.contains_key(provider_id) {
                path_lookup.insert(provider_id.clone(), provider.path.clone());
            }
        }
        provider.metadata = Some(ProviderMetadataInput::from_metadata(&metadata));
        list.push(metadata);
    }
    Ok(list)
}

fn extract_scoreboard(
    scoreboard: &scoreboard::Scoreboard,
    alias_by_provider_id: &HashMap<String, String>,
    include_scoreboard: bool,
) -> Result<ScoreboardSummary, LocalFetchError> {
    let include = include_scoreboard;
    let mut eligible_aliases = HashSet::new();
    let mut weight_by_alias = HashMap::new();
    let mut entries = Vec::new();
    for entry in scoreboard.entries() {
        let provider_id = entry.provider.id().as_str().to_string();
        let alias = alias_by_provider_id
            .get(&provider_id)
            .cloned()
            .unwrap_or_else(|| provider_id.clone());
        match &entry.eligibility {
            Eligibility::Eligible => {
                eligible_aliases.insert(alias.clone());
                weight_by_alias.insert(alias.clone(), entry.provider.weight());
            }
            Eligibility::Ineligible(_) => {}
        }
        if include {
            let eligibility = match &entry.eligibility {
                Eligibility::Eligible => "eligible".to_string(),
                Eligibility::Ineligible(reason) => reason.to_string(),
            };
            entries.push(LocalFetchScoreboardEntry {
                provider_id,
                alias,
                raw_score: entry.raw_score,
                normalized_weight: entry.normalised_weight,
                eligibility,
            });
        }
    }
    if include {
        Ok((Some(entries), eligible_aliases, weight_by_alias))
    } else {
        Ok((None, eligible_aliases, weight_by_alias))
    }
}

fn build_fetch_providers(
    providers: Vec<ProcessedProvider>,
    eligible_aliases: &HashSet<String>,
    weight_by_alias: &HashMap<String, NonZeroU32>,
    scoreboard_requested: bool,
    max_peers: Option<u32>,
) -> Result<Vec<FetchProvider>, LocalFetchError> {
    let mut list = Vec::new();
    for provider in providers {
        if scoreboard_requested && !eligible_aliases.contains(&provider.name) {
            continue;
        }
        let mut fetch_provider = FetchProvider::new(provider.name.clone())
            .with_max_concurrent_chunks(provider.max_concurrent);
        if let Some(weight) = weight_by_alias
            .get(&provider.name)
            .copied()
            .or(provider.weight)
        {
            fetch_provider = fetch_provider.with_weight(weight);
        }
        if let Some(metadata_input) = provider.metadata {
            let metadata = metadata_input.into_provider_metadata(&provider.name)?;
            fetch_provider = fetch_provider.with_metadata(metadata);
        }
        list.push(fetch_provider);
    }

    if list.is_empty() {
        return Err(LocalFetchError::ScoreboardExcludedAll);
    }

    if let Some(limit) = max_peers {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        if list.len() > limit {
            list.truncate(limit.max(1));
        }
    }

    Ok(list)
}

fn build_fetch_options(options: &LocalFetchOptions) -> FetchOptions {
    let mut fetch_options = FetchOptions::default();
    if let Some(flag) = options.verify_digests {
        fetch_options.verify_digests = flag;
    }
    if let Some(flag) = options.verify_lengths {
        fetch_options.verify_lengths = flag;
    }
    if let Some(limit) = options.retry_budget {
        fetch_options.per_chunk_retry_limit = Some(limit.max(1) as usize);
    }
    if let Some(threshold) = options.provider_failure_threshold {
        fetch_options.provider_failure_threshold = threshold as usize;
    }
    if let Some(limit) = options.max_parallel {
        fetch_options.global_parallel_limit = Some(limit.max(1) as usize);
    }
    if !options.deny_providers.is_empty() || !options.boost_providers.is_empty() {
        let policy = LocalScorePolicy::new(
            options.deny_providers.clone(),
            options.boost_providers.clone(),
        );
        fetch_options.score_policy = Some(Arc::new(policy));
    }
    fetch_options
}

fn run_fetch(
    plan: &CarBuildPlan,
    providers: Vec<FetchProvider>,
    path_lookup: HashMap<String, PathBuf>,
    fetch_options: FetchOptions,
) -> Result<FetchOutcome, LocalFetchError> {
    let path_map = Arc::new(path_lookup);
    let fetcher = move |request: FetchRequest| {
        let map = Arc::clone(&path_map);
        async move {
            let provider_name = request.provider.id().as_str().to_string();
            let path = map.get(&provider_name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("unknown provider '{provider_name}'"),
                )
            })?;
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(request.spec.offset))?;
            let mut buf = vec![0u8; request.spec.length as usize];
            file.read_exact(&mut buf)?;
            Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(buf))
        }
    };

    match futures::executor::block_on(multi_fetch::fetch_plan_parallel(
        plan,
        providers,
        fetcher,
        fetch_options,
    )) {
        Ok(outcome) => Ok(outcome),
        Err(err) => Err(LocalFetchError::Fetch(err.to_string())),
    }
}

struct LocalScorePolicy {
    deny: HashSet<String>,
    boosts: HashMap<String, i64>,
}

impl LocalScorePolicy {
    fn new(deny: Vec<String>, boosts: Vec<(String, i64)>) -> Self {
        let deny_set = deny.into_iter().collect();
        let boost_map = boosts.into_iter().collect();
        Self {
            deny: deny_set,
            boosts: boost_map,
        }
    }
}

impl ScorePolicy for LocalScorePolicy {
    fn score(&self, ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision {
        let provider = ctx.provider.id().as_str();
        if self.deny.contains(provider) {
            return ProviderScoreDecision {
                priority_delta: 0,
                allow: false,
            };
        }
        let delta = self.boosts.get(provider).copied().unwrap_or(0);
        ProviderScoreDecision {
            priority_delta: delta,
            allow: true,
        }
    }
}

impl ProviderMetadataInput {
    pub fn into_provider_metadata(
        self,
        alias: &str,
    ) -> Result<crate::multi_fetch::ProviderMetadata, LocalFetchError> {
        let mut metadata = crate::multi_fetch::ProviderMetadata::new();
        metadata.provider_id = Some(self.provider_id.unwrap_or_else(|| alias.to_string()));
        metadata.profile_id = self.profile_id;
        if let Some(aliases) = self.profile_aliases {
            metadata.profile_aliases = aliases;
        }
        metadata.availability = self.availability;
        metadata.stake_amount = self.stake_amount;
        if let Some(value) = self.max_streams {
            let converted = u16::try_from(value).map_err(|_| {
                LocalFetchError::Fetch(format!("max_streams {value} exceeds u16 range"))
            })?;
            metadata.max_streams = Some(converted);
        }
        metadata.refresh_deadline = self.refresh_deadline;
        metadata.expires_at = self.expires_at;
        metadata.ttl_secs = self.ttl_secs;
        metadata.allow_unknown_capabilities = self.allow_unknown_capabilities.unwrap_or(false);
        if let Some(names) = self.capability_names {
            metadata.capability_names = names;
        }
        if let Some(topics) = self.rendezvous_topics {
            metadata.rendezvous_topics = topics;
        }
        if let Some(notes) = self.notes {
            metadata.notes = Some(notes);
        }
        if let Some(range) = self.range_capability {
            metadata.range_capability = Some(range.into());
        }
        if let Some(budget) = self.stream_budget {
            metadata.stream_budget = Some(budget.into());
        }
        if let Some(hints) = self.transport_hints {
            metadata.transport_hints = hints.into_iter().map(Into::into).collect();
        }
        Ok(metadata)
    }

    pub fn from_metadata(metadata: &crate::multi_fetch::ProviderMetadata) -> Self {
        Self {
            provider_id: metadata.provider_id.clone(),
            profile_id: metadata.profile_id.clone(),
            profile_aliases: Some(metadata.profile_aliases.clone()),
            availability: metadata.availability.clone(),
            stake_amount: metadata.stake_amount.clone(),
            max_streams: metadata.max_streams.map(u32::from),
            refresh_deadline: metadata.refresh_deadline,
            expires_at: metadata.expires_at,
            ttl_secs: metadata.ttl_secs,
            allow_unknown_capabilities: Some(metadata.allow_unknown_capabilities),
            capability_names: Some(metadata.capability_names.clone()),
            rendezvous_topics: Some(metadata.rendezvous_topics.clone()),
            notes: metadata.notes.clone(),
            range_capability: metadata.range_capability.as_ref().map(Into::into),
            stream_budget: metadata.stream_budget.as_ref().map(Into::into),
            transport_hints: Some(metadata.transport_hints.iter().map(Into::into).collect()),
        }
    }
}

impl From<RangeCapabilityInput> for crate::multi_fetch::RangeCapability {
    fn from(value: RangeCapabilityInput) -> Self {
        Self {
            max_chunk_span: value.max_chunk_span,
            min_granularity: value.min_granularity,
            supports_sparse_offsets: value.supports_sparse_offsets.unwrap_or(true),
            requires_alignment: value.requires_alignment.unwrap_or(false),
            supports_merkle_proof: value.supports_merkle_proof.unwrap_or(true),
        }
    }
}

impl From<&crate::multi_fetch::RangeCapability> for RangeCapabilityInput {
    fn from(value: &crate::multi_fetch::RangeCapability) -> Self {
        Self {
            max_chunk_span: value.max_chunk_span,
            min_granularity: value.min_granularity,
            supports_sparse_offsets: Some(value.supports_sparse_offsets),
            requires_alignment: Some(value.requires_alignment),
            supports_merkle_proof: Some(value.supports_merkle_proof),
        }
    }
}

impl From<StreamBudgetInput> for crate::multi_fetch::StreamBudget {
    fn from(value: StreamBudgetInput) -> Self {
        Self {
            max_in_flight: value.max_in_flight,
            max_bytes_per_sec: value.max_bytes_per_sec,
            burst_bytes: value.burst_bytes,
        }
    }
}

impl From<&crate::multi_fetch::StreamBudget> for StreamBudgetInput {
    fn from(value: &crate::multi_fetch::StreamBudget) -> Self {
        Self {
            max_in_flight: value.max_in_flight,
            max_bytes_per_sec: value.max_bytes_per_sec,
            burst_bytes: value.burst_bytes,
        }
    }
}

impl From<TransportHintInput> for crate::multi_fetch::TransportHint {
    fn from(value: TransportHintInput) -> Self {
        Self {
            protocol: value.protocol,
            protocol_id: value.protocol_id,
            priority: value.priority,
        }
    }
}

impl From<&crate::multi_fetch::TransportHint> for TransportHintInput {
    fn from(value: &crate::multi_fetch::TransportHint) -> Self {
        Self {
            protocol: value.protocol.clone(),
            protocol_id: value.protocol_id,
            priority: value.priority,
        }
    }
}
