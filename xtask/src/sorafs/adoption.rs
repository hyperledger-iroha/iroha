//! Multi-provider adoption evidence, scoreboard comparison and telemetry burn-in gates.
use norito::json::{self, Value};
use serde::Serialize;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone)]
pub struct AdoptionCheckOptions {
    pub scoreboard_paths: Vec<PathBuf>,
    pub summary_paths: Vec<PathBuf>,
    pub min_eligible_providers: usize,
    pub require_positive_weight: bool,
    pub allow_single_source_fallback: bool,
    pub require_telemetry_source: bool,
    pub allow_implicit_metadata: bool,
    pub require_direct_only: bool,
    pub require_telemetry_region: bool,
}
#[derive(Debug, Serialize)]
pub struct AdoptionCheckReport {
    pub scoreboard_reports: Vec<ScoreboardReport>,
    pub total_evaluated: usize,
    pub min_providers_required: usize,
    pub single_source_override_used: bool,
    pub implicit_metadata_override_used: bool,
}
#[derive(Debug, Serialize)]
pub struct ScoreboardReport {
    pub scoreboard_path: String,
    pub summary_path: String,
    pub provider_count: usize,
    pub eligible_providers: usize,
    pub active_providers: usize,
    pub min_required: usize,
    pub weight_sum: f64,
    pub summary_provider_count: u64,
    pub summary_gateway_provider_count: u64,
    pub summary_provider_mix: String,
    pub summary_transport_policy: String,
    pub summary_transport_policy_override: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_transport_policy_override_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ScoreboardMetadata>,
}
#[derive(Debug, Serialize, Default)]
pub struct ScoreboardMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestrator_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_scoreboard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_implicit_metadata: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_provider_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_mix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parallel: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_peers: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_failure_threshold: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assume_now: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_provided: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy_override: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy_override_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy_override: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy_override_label: Option<String>,
}
#[derive(Clone)]
pub struct ScoreboardDiffOptions {
    pub previous_scoreboard: PathBuf,
    pub current_scoreboard: PathBuf,
    pub threshold_percent: f64,
}
#[derive(Debug, Serialize)]
pub struct ScoreboardDiffReport {
    pub previous_scoreboard: String,
    pub current_scoreboard: String,
    pub threshold_percent: f64,
    pub total_providers: usize,
    pub changed_providers: Vec<ProviderWeightDelta>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProviderWeightDelta {
    pub provider_id: String,
    pub previous_weight: f64,
    pub current_weight: f64,
    pub delta: f64,
    pub delta_percent: f64,
    pub was_added: bool,
    pub was_removed: bool,
    pub exceeds_threshold: bool,
}
#[derive(Clone)]
pub struct BurnInCheckOptions {
    pub log_paths: Vec<PathBuf>,
    pub required_window_days: u64,
    pub min_pq_ratio: f64,
    pub max_no_provider_errors: u64,
    pub max_brownout_ratio: f64,
    pub min_fetches: u64,
}
#[derive(Debug, Serialize)]
pub struct BurnInSummary {
    pub sources: Vec<String>,
    pub coverage_days: f64,
    pub first_timestamp: String,
    pub last_timestamp: String,
    pub fetches_total: u64,
    pub successful_fetches: u64,
    pub brownout_fetches: u64,
    pub brownout_ratio: f64,
    pub stall_event_ratio: f64,
    pub stall_events: u64,
    pub stall_chunks: u64,
    pub stall_chunk_ratio: f64,
    pub pq_ratio_min: f64,
    pub pq_ratio_avg: f64,
    pub failure_reasons: BTreeMap<String, u64>,
}
pub fn run_adoption_check(
    options: AdoptionCheckOptions,
) -> Result<AdoptionCheckReport, Box<dyn Error>> {
    let scoreboard_paths = if options.scoreboard_paths.is_empty() {
        vec![default_adoption_scoreboard_path()]
    } else {
        options.scoreboard_paths.clone()
    };
    if scoreboard_paths.is_empty() {
        return Err("no scoreboards supplied for adoption check".into());
    }
    let summary_paths = if options.summary_paths.is_empty() {
        scoreboard_paths
            .iter()
            .map(|path| default_summary_path_for_scoreboard(path.as_path()))
            .collect()
    } else if options.summary_paths.len() == scoreboard_paths.len() {
        options.summary_paths.clone()
    } else {
        return Err(format!(
            "provided {} --summary path(s) but {scoreboard_count} scoreboard(s); counts must match",
            options.summary_paths.len(),
            scoreboard_count = scoreboard_paths.len()
        )
        .into());
    };
    let mut evaluated = 0usize;
    let mut single_source_override_used = false;
    let mut implicit_metadata_override_used = false;
    let mut reports = Vec::new();
    for (path, summary_path) in scoreboard_paths.iter().zip(summary_paths.iter()) {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read scoreboard `{}`: {err}", path.display()))?;
        let value: Value = json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse scoreboard JSON `{}`: {err}",
                path.display()
            )
        })?;
        let root = value
            .as_object()
            .ok_or_else(|| format!("scoreboard `{}` must be a JSON object", path.display()))?;
        let metadata = match root.get("metadata") {
            Some(value) => parse_scoreboard_metadata(value, path)?,
            None => {
                return Err(format!(
                    "scoreboard `{}` missing `metadata` object; rerun the capture with --use-scoreboard so adoption evidence records provider totals, mix labels, and transport overrides",
                    path.display()
                )
                .into());
            }
        };
        let meta = metadata.as_ref().ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata block is null or empty; rerun the capture with --use-scoreboard so provider totals and mix labels are recorded",
                path.display()
            )
        })?;
        let mut expected_gateway_manifest: Option<(String, String)> = None;
        let scoreboard_telemetry_label = non_empty_trimmed(meta.telemetry_source.as_deref());
        let direct = meta.provider_count.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `provider_count`; rerun the capture with the latest CLI/SDK so adoption evidence records provider totals",
                path.display()
            )
        })?;
        let gateway = meta.gateway_provider_count.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `gateway_provider_count`; rerun the capture with the latest CLI/SDK so adoption evidence records provider totals",
                path.display()
            )
        })?;
        let provider_mix = meta.provider_mix.as_deref().ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `provider_mix`; rerun the capture with the latest CLI/SDK so adoption evidence records `provider_count`, `gateway_provider_count`, and the derived mix label",
                path.display()
            )
        })?;
        let expected_provider_mix = provider_mix_label_from_counts(direct, gateway);
        if provider_mix != expected_provider_mix {
            return Err(format!(
                "scoreboard `{}` metadata.provider_mix=`{provider_mix}` does not match the derived `{expected_provider_mix}` label for provider_count={} and gateway_provider_count={}; regenerate the capture or fix the metadata",
                path.display(),
                direct,
                gateway
            )
            .into());
        }
        if meta.use_scoreboard == Some(false) {
            return Err(format!(
                "scoreboard `{}` metadata.use_scoreboard=false; multi-source adoption requires scoreboard mode",
                path.display()
            )
            .into());
        }
        let stray_gateway_manifest_metadata = meta.gateway_manifest_provided == Some(true)
            || non_empty_trimmed(meta.gateway_manifest_id.as_deref()).is_some()
            || non_empty_trimmed(meta.gateway_manifest_cid.as_deref()).is_some();
        if gateway == 0 && stray_gateway_manifest_metadata {
            return Err(format!(
                "scoreboard `{}` metadata includes gateway manifest fields but records no gateway providers; rerun without gateway manifest flags or capture gateway fetch evidence",
                path.display()
            )
            .into());
        }
        if meta.allow_implicit_metadata == Some(true) {
            if options.allow_implicit_metadata {
                implicit_metadata_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata.allow_implicit_metadata=true; rerun the capture with live provider adverts or pass --allow-implicit-metadata when the baked-in capability metadata is intentionally used",
                    path.display()
                )
                .into());
            }
        }
        if let Some(label) = metadata_direct_transport_label(meta) {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata transport policy `{label}` indicates a direct-only fallback; rerun with --allow-single-source when downgrades are intentional",
                    path.display()
                )
                .into());
            }
        }
        if let Some((field, value)) = metadata_single_source_hint(meta) {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata {field}={value} indicates single-source fallback; rerun with --allow-single-source when downgrades are intentional",
                    path.display()
                )
                .into());
            }
        }
        if gateway > 0 {
            if meta.gateway_manifest_provided != Some(true) {
                return Err(format!(
                    "scoreboard `{}` recorded {gateway} gateway provider(s) but metadata.gateway_manifest_provided=false; capture evidence with --manifest-envelope so gateway fetches include the signed manifest",
                    path.display()
                )
                .into());
            }
            let manifest_id = non_empty_trimmed(meta.gateway_manifest_id.as_deref()).ok_or_else(|| {
                format!(
                    "scoreboard `{}` recorded gateway providers but metadata.gateway_manifest_id is missing; rerun the capture with --manifest-envelope so manifest digests are persisted",
                    path.display()
                )
            })?;
            let manifest_cid = non_empty_trimmed(meta.gateway_manifest_cid.as_deref()).ok_or_else(|| {
                format!(
                    "scoreboard `{}` recorded gateway providers but metadata.gateway_manifest_cid is missing; rerun the capture with --manifest-envelope so manifest digests are persisted",
                    path.display()
                )
            })?;
            expected_gateway_manifest = Some((manifest_id, manifest_cid));
        }
        let entries = root
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("scoreboard `{}` missing `entries` array", path.display()))?;
        let mut eligible = 0usize;
        let mut zero_weight: Vec<String> = Vec::new();
        let mut weight_sum = 0.0;
        let mut scoreboard_providers: HashSet<String> = HashSet::new();
        for (index, entry) in entries.iter().enumerate() {
            let provider = entry
                .get("provider_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` entry {index} missing `provider_id` string",
                        path.display()
                    )
                })?;
            scoreboard_providers.insert(provider.to_string());
            if scoreboard_entry_is_eligible(entry) {
                eligible += 1;
                let weight = entry
                    .get("normalised_weight")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                weight_sum += weight;
                if options.require_positive_weight && weight <= 0.0 {
                    zero_weight.push(provider.to_string());
                }
                continue;
            }
        }
        if eligible < options.min_eligible_providers {
            if options.allow_single_source_fallback && eligible == 1 {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` has {eligible} eligible provider(s); require at least {}",
                    path.display(),
                    options.min_eligible_providers
                )
                .into());
            }
        }
        if !zero_weight.is_empty() {
            return Err(format!(
                "scoreboard `{}` reported zero normalised weight for providers: {}",
                path.display(),
                zero_weight.join(", ")
            )
            .into());
        }
        const WEIGHT_SUM_TOLERANCE: f64 = 1e-3;
        if (weight_sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
            return Err(format!(
                "scoreboard `{}` normalised weights sum to {:.6}; expected 1.0 (±{WEIGHT_SUM_TOLERANCE})",
                path.display(),
                weight_sum
            )
            .into());
        }
        let declared_total = direct + gateway;
        if declared_total > 0
            && declared_total != u64::try_from(scoreboard_providers.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "scoreboard `{}` metadata reports {} provider(s) (local + gateway) but the scoreboard lists {}; regenerate the capture so counts stay in sync",
                path.display(),
                declared_total,
                scoreboard_providers.len()
            )
            .into());
        }
        let summary_bytes = fs::read(summary_path)
            .map_err(|err| format!("failed to read summary `{}`: {err}", summary_path.display()))?;
        let summary_value: Value = json::from_slice(&summary_bytes).map_err(|err| {
            format!(
                "failed to parse summary JSON `{}`: {err}",
                summary_path.display()
            )
        })?;
        let stats = collect_summary_stats(&summary_value, summary_path)?;
        if options.require_telemetry_source {
            let scoreboard_label = scoreboard_telemetry_label
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` metadata missing `telemetry_source`; rerun the capture with --telemetry-json so adoption evidence records the OTLP source (or omit --require-telemetry-source)",
                        path.display()
                    )
                })?;
            let summary_label = stats
                .telemetry_source
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "summary `{}` missing `telemetry_source`; rerun the capture so scoreboard `{}` and summary reference the same OTLP source",
                        summary_path.display(),
                        path.display()
                    )
                })?;
            if summary_label != scoreboard_label {
                return Err(format!(
                    "summary `{}` telemetry_source=`{summary_label}` does not match scoreboard `{}` metadata `{scoreboard_label}`; rerun the capture with matching telemetry labels",
                    summary_path.display(),
                    path.display()
                )
                .into());
            }
        } else if let Some(summary_label) = stats.telemetry_source.as_deref() {
            match scoreboard_telemetry_label.as_deref() {
                Some(scoreboard_label) if summary_label != scoreboard_label => {
                    return Err(format!(
                        "summary `{}` telemetry_source=`{summary_label}` does not match scoreboard `{}` metadata `{scoreboard_label}`; rerun the capture with matching telemetry labels",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
                Some(_) => {}
                None => {
                    return Err(format!(
                        "summary `{}` records telemetry_source=`{summary_label}` but scoreboard `{}` metadata.telemetry_source is missing; rerun the capture so both artefacts include the same OTLP label",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
            }
        }
        let scoreboard_region = meta
            .telemetry_region
            .as_deref()
            .and_then(|value| non_empty_trimmed(Some(value)));
        if options.require_telemetry_region && scoreboard_region.is_none() {
            return Err(format!(
                "scoreboard `{}` metadata missing `telemetry_region`; rerun the capture with --telemetry-region=<label> (or omit --require-telemetry-region)",
                path.display()
            )
            .into());
        }
        if let Some(region) = scoreboard_region.as_deref() {
            match stats.telemetry_region.as_deref() {
                Some(summary_region) if summary_region == region => {}
                Some(summary_region) => {
                    return Err(format!(
                        "summary `{}` telemetry_region=`{summary_region}` does not match scoreboard `{}` metadata `{region}`; rerun the capture so the region labels stay in sync",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
                None => {
                    return Err(format!(
                        "summary `{}` is missing `telemetry_region` but scoreboard `{}` metadata records `{region}`; rerun the capture so both artefacts advertise the region label",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
            }
        } else if stats.telemetry_region.is_some() {
            return Err(format!(
                "summary `{}` records telemetry_region but scoreboard `{}` metadata.telemetry_region is missing; rerun the capture so both artefacts include the same region label",
                summary_path.display(),
                path.display()
            )
            .into());
        }
        if options.require_telemetry_region && stats.telemetry_region.is_none() {
            return Err(format!(
                "summary `{}` missing `telemetry_region`; rerun the capture with --telemetry-region=<label> so scoreboard `{}` and summary align (or omit --require-telemetry-region)",
                summary_path.display(),
                path.display()
            )
            .into());
        }
        let mut summary_override_label: Option<&str> = None;
        if stats.transport_policy_override {
            let override_label = stats.transport_policy_override_label.as_deref().ok_or_else(
                || {
                    format!(
                        "summary `{}` sets transport_policy_override=true but transport_policy_override_label is missing; rerun the capture with the latest CLI/SDK so override evidence is preserved",
                        summary_path.display()
                    )
                },
            )?;
            if override_label != stats.transport_policy {
                return Err(format!(
                    "summary `{}` transport_policy_override_label=`{}` does not match transport_policy=`{}`; rerun the capture so override evidence stays consistent",
                    summary_path.display(),
                    override_label,
                    stats.transport_policy
                )
                .into());
            }
            summary_override_label = Some(override_label);
        }
        let scoreboard_transport_policy =
            meta.transport_policy.as_deref().ok_or_else(|| {
                format!(
                    "scoreboard `{}` metadata missing `transport_policy`; rerun the capture with --use-scoreboard from the latest CLI/SDK so policy evidence is persisted",
                    path.display()
                )
            })?;
        if scoreboard_transport_policy != stats.transport_policy {
            return Err(format!(
                "scoreboard `{}` metadata.transport_policy=`{}` does not match summary transport_policy=`{}`; rerun the capture so policy evidence stays consistent",
                path.display(),
                scoreboard_transport_policy,
                stats.transport_policy
            )
            .into());
        }
        let scoreboard_override = meta.transport_policy_override.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `transport_policy_override`; rerun the capture with the latest CLI/SDK so override evidence is preserved",
                path.display()
            )
        })?;
        if scoreboard_override != stats.transport_policy_override {
            return Err(format!(
                "scoreboard `{}` metadata.transport_policy_override={} does not match summary transport_policy_override={}; rerun the capture so policy evidence stays consistent",
                path.display(),
                scoreboard_override,
                stats.transport_policy_override
            )
            .into());
        }
        if scoreboard_override {
            let scoreboard_label = meta
                .transport_policy_override_label
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` metadata.transport_policy_override_label missing; rerun the capture so override evidence stays consistent",
                        path.display()
                    )
                })?;
            let summary_label = summary_override_label.ok_or_else(|| {
                format!(
                    "summary `{}` missing transport_policy_override_label despite override flag; rerun the capture with the latest CLI/SDK",
                    summary_path.display()
                )
            })?;
            if scoreboard_label != summary_label {
                return Err(format!(
                    "scoreboard `{}` metadata.transport_policy_override_label=`{}` does not match summary override label `{}`; rerun the capture so override evidence stays consistent",
                    path.display(),
                    scoreboard_label,
                    summary_label
                )
                .into());
            }
        }
        let transport_is_direct_only = is_direct_only_label(&stats.transport_policy);
        if options.require_direct_only && !transport_is_direct_only {
            return Err(format!(
                "summary `{}` transport_policy=`{}` but --require-direct-only was supplied; rerun the capture with --transport-policy=direct-only (or omit the flag when running the standard SoraNet-first posture)",
                summary_path.display(),
                stats.transport_policy
            )
            .into());
        }
        if transport_is_direct_only {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "summary `{}` transport_policy=`{}` indicates a direct-only fallback; rerun with --allow-single-source when downgrades are intentional",
                    summary_path.display(),
                    stats.transport_policy
                )
                .into());
            }
        }
        if direct != stats.provider_count {
            return Err(format!(
                "summary `{}` recorded provider_count={} but scoreboard metadata reported {}; rerun the capture so counts stay in sync",
                summary_path.display(),
                stats.provider_count,
                direct
            )
            .into());
        }
        if gateway != stats.gateway_provider_count {
            return Err(format!(
                "summary `{}` recorded gateway_provider_count={} but scoreboard metadata reported {}; rerun the capture so counts stay in sync",
                summary_path.display(),
                stats.gateway_provider_count,
                gateway
            )
            .into());
        }
        if provider_mix != stats.provider_mix {
            return Err(format!(
                "summary `{}` recorded provider_mix=`{}` but scoreboard metadata reported `{provider_mix}`; rerun the capture so mix labels remain consistent",
                summary_path.display(),
                stats.provider_mix
            )
            .into());
        }
        if let Some((expected_manifest_id, expected_manifest_cid)) =
            expected_gateway_manifest.as_ref()
        {
            let summary_manifest_id = non_empty_trimmed(stats.manifest_id.as_deref()).ok_or_else(|| {
                format!(
                    "summary `{}` missing `manifest_id` while gateway providers were captured; rerun the capture with the latest CLI/SDK so manifest digests are included",
                    summary_path.display()
                )
            })?;
            let summary_manifest_cid = non_empty_trimmed(stats.manifest_cid.as_deref()).ok_or_else(|| {
                format!(
                    "summary `{}` missing `manifest_cid` while gateway providers were captured; rerun the capture with the latest CLI/SDK so manifest digests are included",
                    summary_path.display()
                )
            })?;
            if &summary_manifest_id != expected_manifest_id {
                return Err(format!(
                    "summary `{}` manifest_id=`{}` does not match scoreboard metadata `{}`; ensure the same manifest envelope is referenced in both artefacts",
                    summary_path.display(),
                    summary_manifest_id,
                    expected_manifest_id
                )
                .into());
            }
            if &summary_manifest_cid != expected_manifest_cid {
                return Err(format!(
                    "summary `{}` manifest_cid=`{}` does not match scoreboard metadata `{}`; ensure the same manifest envelope is referenced in both artefacts",
                    summary_path.display(),
                    summary_manifest_cid,
                    expected_manifest_cid
                )
                .into());
            }
            if !stats.gateway_manifest_provided {
                return Err(format!(
                    "summary `{}` recorded gateway providers but gateway_manifest_provided=false; capture evidence with --gateway-manifest-envelope so manifest presence is reflected in the adoption artefacts",
                    summary_path.display()
                )
                .into());
            }
        } else if stats.gateway_manifest_provided {
            return Err(format!(
                "summary `{}` sets gateway_manifest_provided=true without gateway metadata; rerun the capture without gateway manifest flags or include gateway provider evidence",
                summary_path.display()
            )
            .into());
        }
        let active = stats.active_providers.len();
        if active < options.min_eligible_providers {
            if options.allow_single_source_fallback && active == 1 {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "summary `{}` recorded {active} provider(s) with successful chunk fetches; require at least {}",
                    summary_path.display(),
                    options.min_eligible_providers
                )
                .into());
            }
        }
        let mut referenced_providers = stats.chunk_receipt_providers.clone();
        referenced_providers.extend(stats.provider_report_providers.iter().cloned());
        let mut missing: Vec<String> = referenced_providers
            .into_iter()
            .filter(|provider| !scoreboard_providers.contains(provider))
            .collect();
        if !missing.is_empty() {
            missing.sort();
            return Err(format!(
                "summary `{}` references provider(s) not present in scoreboard `{}`: {}",
                summary_path.display(),
                path.display(),
                missing.join(", ")
            )
            .into());
        }
        evaluated += 1;
        reports.push(ScoreboardReport {
            scoreboard_path: path.display().to_string(),
            summary_path: summary_path.display().to_string(),
            provider_count: scoreboard_providers.len(),
            eligible_providers: eligible,
            active_providers: active,
            min_required: options.min_eligible_providers,
            weight_sum,
            summary_provider_count: stats.provider_count,
            summary_gateway_provider_count: stats.gateway_provider_count,
            summary_provider_mix: stats.provider_mix.clone(),
            summary_transport_policy: stats.transport_policy.clone(),
            summary_transport_policy_override: stats.transport_policy_override,
            summary_transport_policy_override_label: stats.transport_policy_override_label.clone(),
            metadata,
        });
    }
    if evaluated == 0 {
        return Err("no scoreboards evaluated; provide --scoreboard <path>".into());
    }
    Ok(AdoptionCheckReport {
        scoreboard_reports: reports,
        total_evaluated: evaluated,
        min_providers_required: options.min_eligible_providers,
        single_source_override_used,
        implicit_metadata_override_used,
    })
}
fn default_adoption_scoreboard_path() -> PathBuf {
    crate::workspace_root()
        .join("artifacts")
        .join("sorafs_orchestrator")
        .join("latest")
        .join("scoreboard.json")
}
const WEIGHT_EPSILON: f64 = 1e-9;
pub fn run_scoreboard_diff(
    options: ScoreboardDiffOptions,
) -> Result<ScoreboardDiffReport, Box<dyn Error>> {
    if options.threshold_percent < 0.0 {
        return Err("threshold-percent must be non-negative".into());
    }
    let threshold_fraction = (options.threshold_percent / 100.0).abs();
    let previous = load_scoreboard_weights(&options.previous_scoreboard)?;
    let current = load_scoreboard_weights(&options.current_scoreboard)?;
    let mut providers: BTreeSet<String> = BTreeSet::new();
    providers.extend(previous.keys().cloned());
    providers.extend(current.keys().cloned());
    let mut deltas: Vec<ProviderWeightDelta> = Vec::new();
    for provider in &providers {
        let previous_weight = *previous.get(provider).unwrap_or(&0.0);
        let current_weight = *current.get(provider).unwrap_or(&0.0);
        if approx_equal(previous_weight, current_weight) {
            continue;
        }
        let delta = current_weight - previous_weight;
        deltas.push(ProviderWeightDelta {
            provider_id: provider.clone(),
            previous_weight,
            current_weight,
            delta,
            delta_percent: delta * 100.0,
            was_added: approx_equal(previous_weight, 0.0) && !approx_equal(current_weight, 0.0),
            was_removed: approx_equal(current_weight, 0.0) && !approx_equal(previous_weight, 0.0),
            exceeds_threshold: delta.abs() > threshold_fraction + WEIGHT_EPSILON,
        });
    }
    deltas.sort_by(|left, right| {
        right
            .delta
            .abs()
            .partial_cmp(&left.delta.abs())
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.provider_id.cmp(&right.provider_id))
    });
    Ok(ScoreboardDiffReport {
        previous_scoreboard: options.previous_scoreboard.display().to_string(),
        current_scoreboard: options.current_scoreboard.display().to_string(),
        threshold_percent: options.threshold_percent,
        total_providers: providers.len(),
        changed_providers: deltas,
    })
}
pub fn print_scoreboard_diff(report: &ScoreboardDiffReport) {
    println!("sorafs scoreboard diff:");
    println!("  previous: {}", report.previous_scoreboard);
    println!("  current : {}", report.current_scoreboard);
    if report.changed_providers.is_empty() {
        println!(
            "  no eligible provider weight changes detected across {} provider(s)",
            report.total_providers
        );
        return;
    }
    let exceed_count = report
        .changed_providers
        .iter()
        .filter(|entry| entry.exceeds_threshold)
        .count();
    println!(
        "  {} provider(s) changed; {} exceed the {:.2}% threshold ({} compared)",
        report.changed_providers.len(),
        exceed_count,
        report.threshold_percent,
        report.total_providers
    );
    let displayed = report.changed_providers.len().min(10);
    println!("  Provider weight deltas (showing top {displayed} by |Δ|):");
    println!(
        "  {:<24} {:>11} {:>11} {:>11} {:>12}",
        "Provider", "Previous", "Current", "Δ", "Flags"
    );
    for entry in report.changed_providers.iter().take(displayed) {
        let mut flags: Vec<&str> = Vec::new();
        if entry.was_added {
            flags.push("added");
        }
        if entry.was_removed {
            flags.push("removed");
        }
        if entry.exceeds_threshold {
            flags.push("exceeds");
        }
        let flag_text = if flags.is_empty() {
            "-".to_string()
        } else {
            flags.join("|")
        };
        println!(
            "  {:<24} {:>11.6} {:>11.6} {:>11.6} {:>12}",
            entry.provider_id, entry.previous_weight, entry.current_weight, entry.delta, flag_text
        );
    }
}
fn load_scoreboard_weights(path: &Path) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read scoreboard `{}`: {err}", path.display()))?;
    let value: Value = json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse scoreboard JSON `{}`: {err}",
            path.display()
        )
    })?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("scoreboard `{}` missing `entries` array", path.display()))?;
    let mut weights = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if !scoreboard_entry_is_eligible(entry) {
            continue;
        }
        let provider = entry
            .get("provider_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "scoreboard `{}` entry {index} missing `provider_id` string",
                    path.display()
                )
            })?;
        let weight = entry
            .get("normalised_weight")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                format!(
                    "scoreboard `{}` entry {index} missing `normalised_weight` number",
                    path.display()
                )
            })?;
        weights.insert(provider.to_string(), weight);
    }
    if weights.is_empty() {
        return Err(format!(
            "scoreboard `{}` has no eligible providers to compare",
            path.display()
        )
        .into());
    }
    Ok(weights)
}
fn approx_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= WEIGHT_EPSILON
}
fn default_summary_path_for_scoreboard(scoreboard_path: &Path) -> PathBuf {
    scoreboard_path
        .parent()
        .map(|parent| parent.join("summary.json"))
        .unwrap_or_else(|| PathBuf::from("summary.json"))
}
fn scoreboard_entry_is_eligible(entry: &Value) -> bool {
    match entry.get("eligibility") {
        Some(Value::String(label)) => label == "eligible",
        Some(Value::Object(obj)) => obj.get("status").and_then(Value::as_str) == Some("eligible"),
        _ => false,
    }
}
struct SummaryStats {
    active_providers: HashSet<String>,
    chunk_receipt_providers: HashSet<String>,
    provider_report_providers: HashSet<String>,
    manifest_id: Option<String>,
    manifest_cid: Option<String>,
    gateway_manifest_provided: bool,
    provider_count: u64,
    gateway_provider_count: u64,
    provider_mix: String,
    transport_policy: String,
    transport_policy_override: bool,
    transport_policy_override_label: Option<String>,
    telemetry_source: Option<String>,
    telemetry_region: Option<String>,
}
fn collect_summary_stats(summary: &Value, path: &Path) -> Result<SummaryStats, String> {
    let chunk_count = summary
        .get("chunk_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("summary `{}` missing `chunk_count` number", path.display()))?;
    let manifest_id = summary
        .get("manifest_id")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let manifest_cid = summary
        .get("manifest_cid")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let receipts = summary
        .get("chunk_receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `chunk_receipts` array",
                path.display()
            )
        })?;
    if receipts.len() != chunk_count as usize {
        return Err(format!(
            "summary `{}` reports chunk_count={} but chunk_receipts has {} entries",
            path.display(),
            chunk_count,
            receipts.len()
        ));
    }
    let mut receipt_providers = HashSet::new();
    for (idx, receipt) in receipts.iter().enumerate() {
        let provider = receipt
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "summary `{}` chunk_receipts[{idx}] missing `provider` string",
                    path.display()
                )
            })?;
        receipt_providers.insert(provider.to_string());
    }
    let reports = summary
        .get("provider_reports")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_reports` array",
                path.display()
            )
        })?;
    let mut provider_report_providers = HashSet::new();
    let mut provider_success_sum = 0u64;
    let mut active_providers = HashSet::new();
    for (idx, report) in reports.iter().enumerate() {
        let provider = report
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "summary `{}` provider_reports[{idx}] missing `provider` string",
                    path.display()
                )
            })?;
        provider_report_providers.insert(provider.to_string());
        let successes = report
            .get("successes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "summary `{}` provider_reports[{idx}] missing `successes` number",
                    path.display()
                )
            })?;
        provider_success_sum = provider_success_sum.checked_add(successes).ok_or_else(|| {
            format!(
                "summary `{}` provider success count overflowed u64",
                path.display()
            )
        })?;
        if successes > 0 {
            active_providers.insert(provider.to_string());
        }
    }
    let provider_success_total = summary
        .get("provider_success_total")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_success_total` number",
                path.display()
            )
        })?;
    if provider_success_total != provider_success_sum {
        return Err(format!(
            "summary `{}` reports provider_success_total={} but provider_reports sum to {}",
            path.display(),
            provider_success_total,
            provider_success_sum
        ));
    }
    if provider_success_total != chunk_count {
        return Err(format!(
            "summary `{}` reports provider_success_total={} but chunk_count={}",
            path.display(),
            provider_success_total,
            chunk_count
        ));
    }
    let gateway_manifest_provided = summary
        .get("gateway_manifest_provided")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `gateway_manifest_provided` boolean",
                path.display()
            )
        })?;
    let provider_count = summary
        .get("provider_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_count` number",
                path.display()
            )
        })?;
    let gateway_provider_count = summary
        .get("gateway_provider_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `gateway_provider_count` number",
                path.display()
            )
        })?;
    let provider_mix = summary
        .get("provider_mix")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("summary `{}` missing `provider_mix` string", path.display()))?;
    let expected_mix = provider_mix_label_from_counts(provider_count, gateway_provider_count);
    if provider_mix != expected_mix {
        return Err(format!(
            "summary `{}` provider_mix=`{provider_mix}` does not match derived `{expected_mix}` for provider_count={} gateway_provider_count={}",
            path.display(),
            provider_count,
            gateway_provider_count
        ));
    }
    let transport_policy = summary
        .get("transport_policy")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `transport_policy` string",
                path.display()
            )
        })?;
    require_canonical_transport_policy(
        transport_policy,
        &format!("summary `{}` transport_policy", path.display()),
    )?;
    let transport_policy = transport_policy.to_string();
    let transport_policy_override = summary
        .get("transport_policy_override")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `transport_policy_override` boolean",
                path.display()
            )
        })?;
    let transport_policy_override_label = summary
        .get("transport_policy_override_label")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    if let Some(label) = transport_policy_override_label.as_deref() {
        require_canonical_transport_policy(
            label,
            &format!(
                "summary `{}` transport_policy_override_label",
                path.display()
            ),
        )?;
    }
    let telemetry_source = summary
        .get("telemetry_source")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let telemetry_region = summary
        .get("telemetry_region")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(SummaryStats {
        active_providers,
        chunk_receipt_providers: receipt_providers,
        provider_report_providers,
        manifest_id,
        manifest_cid,
        gateway_manifest_provided,
        provider_count,
        gateway_provider_count,
        provider_mix: provider_mix.to_string(),
        transport_policy,
        transport_policy_override,
        transport_policy_override_label,
        telemetry_source,
        telemetry_region,
    })
}
fn parse_scoreboard_metadata(
    value: &Value,
    path: &Path,
) -> Result<Option<ScoreboardMetadata>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let obj = value.as_object().ok_or_else(|| {
        format!(
            "scoreboard `{}` metadata must be a JSON object",
            path.display()
        )
    })?;
    let metadata = ScoreboardMetadata {
        orchestrator_version: value_as_string(obj.get("version")),
        use_scoreboard: value_as_bool(obj.get("use_scoreboard")),
        allow_implicit_metadata: value_as_bool(obj.get("allow_implicit_metadata")),
        provider_count: value_as_u64(obj.get("provider_count")),
        gateway_provider_count: value_as_u64(obj.get("gateway_provider_count")),
        provider_mix: value_as_string(obj.get("provider_mix")),
        max_parallel: value_as_u64(obj.get("max_parallel")),
        max_peers: value_as_u64(obj.get("max_peers")),
        retry_budget: value_as_u64(obj.get("retry_budget")),
        provider_failure_threshold: value_as_u64(obj.get("provider_failure_threshold")),
        assume_now: value_as_u64(obj.get("assume_now")),
        telemetry_source: value_as_string(obj.get("telemetry_source")),
        telemetry_region: value_as_string(obj.get("telemetry_region")),
        gateway_manifest_id: value_as_string(obj.get("gateway_manifest_id")),
        gateway_manifest_cid: value_as_string(obj.get("gateway_manifest_cid")),
        gateway_manifest_provided: value_as_bool(obj.get("gateway_manifest_provided")),
        transport_policy: value_as_string(obj.get("transport_policy")),
        transport_policy_override: value_as_bool(obj.get("transport_policy_override")),
        transport_policy_override_label: value_as_string(
            obj.get("transport_policy_override_label"),
        ),
        anonymity_policy: value_as_string(obj.get("anonymity_policy")),
        anonymity_policy_override: value_as_bool(obj.get("anonymity_policy_override")),
        anonymity_policy_override_label: value_as_string(
            obj.get("anonymity_policy_override_label"),
        ),
    };
    if let Some(label) = metadata.transport_policy.as_deref() {
        require_canonical_transport_policy(
            label,
            &format!("scoreboard `{}` metadata.transport_policy", path.display()),
        )?;
    }
    if let Some(label) = metadata.transport_policy_override_label.as_deref() {
        require_canonical_transport_policy(
            label,
            &format!(
                "scoreboard `{}` metadata.transport_policy_override_label",
                path.display()
            ),
        )?;
    }
    Ok(Some(metadata))
}
fn value_as_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|v| match v {
        Value::Number(num) => num
            .as_u64()
            .or_else(|| num.as_i64().and_then(|signed| u64::try_from(signed).ok())),
        _ => None,
    })
}
fn value_as_bool(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}
fn value_as_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(|s| s.to_string())
}
fn non_empty_trimmed(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
fn metadata_direct_transport_label(metadata: &ScoreboardMetadata) -> Option<&str> {
    metadata
        .transport_policy
        .as_deref()
        .filter(|label| is_direct_only_label(label))
        .or_else(|| {
            metadata
                .transport_policy_override_label
                .as_deref()
                .filter(|label| is_direct_only_label(label))
        })
}
fn is_direct_only_label(label: &str) -> bool {
    label == "direct-only"
}
fn require_canonical_transport_policy(label: &str, context: &str) -> Result<(), String> {
    match label {
        "soranet-first" | "soranet-strict" | "direct-only" => Ok(()),
        _ => Err(format!(
            "{context} must be exactly soranet-first|soranet-strict|direct-only, got `{label}`"
        )),
    }
}
fn provider_mix_label_from_counts(direct: u64, gateway: u64) -> &'static str {
    match (direct > 0, gateway > 0) {
        (true, true) => "mixed",
        (true, false) => "direct-only",
        (false, true) => "gateway-only",
        (false, false) => "none",
    }
}
fn metadata_single_source_hint(metadata: &ScoreboardMetadata) -> Option<(&'static str, u64)> {
    if let Some(value) = metadata.max_parallel
        && value <= 1
    {
        return Some(("max_parallel", value));
    }
    if let Some(value) = metadata.max_peers
        && value <= 1
    {
        return Some(("max_peers", value));
    }
    let direct = metadata.provider_count.unwrap_or(0);
    let gateway = metadata.gateway_provider_count.unwrap_or(0);
    if direct == 0 && gateway == 0 {
        return None;
    }
    let total = direct.saturating_add(gateway);
    if total <= 1 {
        if direct > 0 && gateway == 0 {
            return Some(("provider_count", direct));
        }
        if gateway > 0 && direct == 0 {
            return Some(("gateway_provider_count", gateway));
        }
        return Some(("total_provider_count", total));
    }
    None
}
pub fn run_burn_in_check(options: BurnInCheckOptions) -> Result<BurnInSummary, Box<dyn Error>> {
    if options.log_paths.is_empty() {
        return Err("sorafs-burn-in-check requires at least one --log <path>".into());
    }
    let mut accumulator = BurnInAccumulator::default();
    for path in &options.log_paths {
        let contents = fs::read_to_string(path)?;
        for line in contents.lines() {
            if let Some(event) = parse_telemetry_line(line) {
                accumulator.observe(event);
            }
        }
    }
    let summary = accumulator.finalize(&options)?;
    Ok(summary)
}
struct ParsedTelemetryLine {
    timestamp: OffsetDateTime,
    target: String,
    fields: HashMap<String, String>,
}
fn parse_telemetry_line(line: &str) -> Option<ParsedTelemetryLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let mut parts = trimmed.splitn(3, ' ');
    let timestamp_raw = parts.next()?;
    let target_raw = parts.next()?;
    let rest = parts.next().unwrap_or_default();
    if !target_raw.starts_with("telemetry::") {
        return None;
    }
    let timestamp = OffsetDateTime::parse(timestamp_raw, &Rfc3339).ok()?;
    let target = target_raw.trim_start_matches("telemetry::").to_string();
    let mut fields = HashMap::new();
    for token in tokenize_field_pairs(rest) {
        if let Some((key, value)) = token.split_once('=') {
            let mut value = value.trim().to_string();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            fields.insert(key.trim().to_string(), value);
        }
    }
    Some(ParsedTelemetryLine {
        timestamp,
        target,
        fields,
    })
}
fn tokenize_field_pairs(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
struct BurnInAccumulator {
    first_timestamp: Option<OffsetDateTime>,
    last_timestamp: Option<OffsetDateTime>,
    fetches_total: u64,
    successful_fetches: u64,
    brownout_fetches: u64,
    stall_events: u64,
    stall_chunks: u64,
    pq_ratio_min: f64,
    pq_ratio_sum: f64,
    pq_ratio_samples: u64,
    failure_reasons: HashMap<String, u64>,
}
impl Default for BurnInAccumulator {
    fn default() -> Self {
        Self {
            first_timestamp: None,
            last_timestamp: None,
            fetches_total: 0,
            successful_fetches: 0,
            brownout_fetches: 0,
            stall_events: 0,
            stall_chunks: 0,
            pq_ratio_min: f64::INFINITY,
            pq_ratio_sum: 0.0,
            pq_ratio_samples: 0,
            failure_reasons: HashMap::new(),
        }
    }
}
impl BurnInAccumulator {
    fn observe(&mut self, event: ParsedTelemetryLine) {
        self.update_span(event.timestamp);
        match event.target.as_str() {
            "sorafs.fetch.lifecycle" => self.observe_lifecycle(&event.fields),
            "sorafs.fetch.error" => {
                let reason = event
                    .fields
                    .get("reason")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                *self.failure_reasons.entry(reason).or_insert(0) += 1;
            }
            "sorafs.fetch.stall" => {
                self.stall_events += 1;
            }
            _ => {}
        }
    }
    fn update_span(&mut self, timestamp: OffsetDateTime) {
        match self.first_timestamp {
            Some(existing) if timestamp < existing => self.first_timestamp = Some(timestamp),
            None => self.first_timestamp = Some(timestamp),
            _ => {}
        }
        match self.last_timestamp {
            Some(existing) if timestamp > existing => self.last_timestamp = Some(timestamp),
            None => self.last_timestamp = Some(timestamp),
            _ => {}
        }
    }
    fn observe_lifecycle(&mut self, fields: &HashMap<String, String>) {
        if let Some("complete") = fields.get("event").map(String::as_str) {
            self.fetches_total += 1;
            if fields.get("status").map(String::as_str) == Some("success") {
                self.successful_fetches += 1;
            }
            if fields.get("anonymity_outcome").map(String::as_str) == Some("brownout") {
                self.brownout_fetches += 1;
            }
            if let Some(value) = fields
                .get("stall_count")
                .and_then(|raw| raw.parse::<u64>().ok())
            {
                self.stall_chunks += value;
            }
            if let Some(value) = fields
                .get("anonymity_ratio")
                .and_then(|raw| raw.parse::<f64>().ok())
                && !value.is_nan()
            {
                if self.pq_ratio_min.is_infinite() || value < self.pq_ratio_min {
                    self.pq_ratio_min = value;
                }
                self.pq_ratio_sum += value;
                self.pq_ratio_samples += 1;
            }
        }
    }
    fn finalize(self, options: &BurnInCheckOptions) -> Result<BurnInSummary, Box<dyn Error>> {
        let first = self
            .first_timestamp
            .ok_or("no telemetry lifecycle events were parsed")?;
        let last = self
            .last_timestamp
            .ok_or("no telemetry lifecycle events were parsed")?;
        if last < first {
            return Err("telemetry timestamps are out of order".into());
        }
        let span = last - first;
        let required_days = i64::try_from(options.required_window_days)
            .map_err(|_| "required_window_days exceeds supported range")?;
        let required_window = TimeDuration::days(required_days);
        if span < required_window {
            let coverage_days = span.as_seconds_f64() / 86_400.0;
            return Err(format!(
                "telemetry window {:.2} days shorter than required {} days",
                coverage_days, options.required_window_days
            )
            .into());
        }
        if self.fetches_total == 0 {
            return Err("telemetry log did not include any completed fetches".into());
        }
        if options.min_fetches > 0 && self.fetches_total < options.min_fetches {
            return Err(format!(
                "telemetry log captured {} fetch(es); require at least {} during the burn-in window",
                self.fetches_total, options.min_fetches
            )
            .into());
        }
        if self.pq_ratio_samples == 0 {
            return Err("no anonymity_ratio fields were found in lifecycle telemetry".into());
        }
        let min_ratio = if self.pq_ratio_min.is_infinite() {
            0.0
        } else {
            self.pq_ratio_min
        };
        if min_ratio + f64::EPSILON < options.min_pq_ratio {
            return Err(format!(
                "minimum anonymity_ratio {:.3} fell below the required {:.3}",
                min_ratio, options.min_pq_ratio
            )
            .into());
        }
        let no_provider_errors = self
            .failure_reasons
            .get("no_healthy_providers")
            .copied()
            .unwrap_or(0);
        if no_provider_errors > options.max_no_provider_errors {
            return Err(format!(
                "encountered {no_provider_errors} no_healthy_providers errors (max allowed: {})",
                options.max_no_provider_errors
            )
            .into());
        }
        let coverage_days = span.as_seconds_f64() / 86_400.0;
        let brownout_ratio = self.brownout_fetches as f64 / self.fetches_total as f64;
        if brownout_ratio > options.max_brownout_ratio + f64::EPSILON {
            return Err(format!(
                "brownout ratio {:.3}% exceeded allowed {:.3}%",
                brownout_ratio * 100.0,
                options.max_brownout_ratio * 100.0
            )
            .into());
        }
        let stall_event_ratio = self.stall_events as f64 / self.fetches_total as f64;
        let stall_chunk_ratio = self.stall_chunks as f64 / self.fetches_total as f64;
        let avg_ratio = self.pq_ratio_sum / self.pq_ratio_samples as f64;
        let failure_reasons = self.failure_reasons.into_iter().collect::<BTreeMap<_, _>>();
        let summary = BurnInSummary {
            sources: options
                .log_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            coverage_days,
            first_timestamp: first.format(&Rfc3339)?,
            last_timestamp: last.format(&Rfc3339)?,
            fetches_total: self.fetches_total,
            successful_fetches: self.successful_fetches,
            brownout_fetches: self.brownout_fetches,
            brownout_ratio,
            stall_event_ratio,
            stall_events: self.stall_events,
            stall_chunks: self.stall_chunks,
            stall_chunk_ratio,
            pq_ratio_min: min_ratio,
            pq_ratio_avg: avg_ratio,
            failure_reasons,
        };
        Ok(summary)
    }
}

#[cfg(test)]
mod tests;
