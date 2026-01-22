//! Soracles evidence helpers (audit bundle generation).

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    events::data::oracle::FeedEventRecord,
    oracle::{FeedConfigVersion, FeedEventOutcome, FeedId, FeedSlot, OracleRejectionCode},
    prelude::Hash,
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Value},
};

use crate::{Run, RunContext};
use crate::cli_output::print_with_optional_text;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build an audit bundle containing oracle feed events and evidence files.
    Bundle(Bundle),
    /// Show the oracle rejection/error catalog for SDK parity.
    Catalog(Catalog),
    /// Garbage-collect evidence bundles and prune unreferenced artifacts.
    #[command(name = "evidence-gc")]
    Gc(GcArgs),
}

#[derive(Args, Debug)]
pub struct Bundle {
    /// Path to a JSON file containing `FeedEventRecord` values (array or single record).
    #[arg(long, value_name = "PATH")]
    events: PathBuf,
    /// Directory where the bundle (manifest + hashed artefacts) will be written.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
    /// Directory of observation JSON files to include (hashed and copied into the bundle).
    #[arg(long, value_name = "DIR")]
    observations: Option<PathBuf>,
    /// Directory of report JSON files to include.
    #[arg(long, value_name = "DIR")]
    reports: Option<PathBuf>,
    /// Directory of connector response JSON files to include.
    #[arg(long, value_name = "DIR")]
    responses: Option<PathBuf>,
    /// Directory of dispute evidence JSON files to include.
    #[arg(long, value_name = "DIR")]
    disputes: Option<PathBuf>,
    /// Optional telemetry snapshot (JSON) to include in the bundle.
    #[arg(long, value_name = "PATH")]
    telemetry: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct Catalog {
    /// Output format (`json` for machine consumption, `markdown` for docs/runbooks).
    ///
    /// Ignored when `--output-format json` is used.
    #[arg(long, value_enum, default_value_t = CatalogFormat::Json)]
    format: CatalogFormat,
}

/// Prune expired soracles evidence bundles and unreferenced artifacts.
#[derive(Args, Debug)]
pub struct GcArgs {
    /// Root directory containing soracles evidence bundles (each with `bundle.json`).
    #[arg(long, value_name = "DIR", default_value = "artifacts/soracles")]
    root: PathBuf,
    /// Retention period in days; bundles older than this are removed.
    #[arg(long, value_name = "DAYS", default_value_t = 180)]
    retention_days: u64,
    /// Retention period for bundles containing dispute evidence (defaults to a longer window).
    #[arg(long, value_name = "DAYS", default_value_t = 365)]
    dispute_retention_days: u64,
    /// Emit a GC summary report to this path (defaults to `<root>/gc_report.json`).
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,
    /// Remove artifact files that are not referenced by `bundle.json`.
    #[arg(long)]
    prune_unreferenced: bool,
    /// Perform a dry run and only report what would be removed.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CatalogFormat {
    Json,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum EvidenceKind {
    Observation,
    Report,
    Response,
    Dispute,
    Telemetry,
}

impl EvidenceKind {
    fn as_label(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Report => "report",
            Self::Response => "response",
            Self::Dispute => "dispute",
            Self::Telemetry => "telemetry",
        }
    }
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct EvidenceEntry {
    pub kind: String,
    pub hash: Hash,
    pub sources: Vec<String>,
    pub bundled_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct FeedEventDigest {
    pub feed_id: FeedId,
    pub feed_config_version: FeedConfigVersion,
    pub slot: FeedSlot,
    pub outcome: FeedEventOutcome,
    pub evidence_hashes: Vec<Hash>,
    pub missing_evidence_hashes: Vec<Hash>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct BundleSummary {
    #[cfg_attr(feature = "json", norito(default))]
    pub generated_at_unix: u64,
    pub artifact_root: String,
    pub feed_events: Vec<FeedEventDigest>,
    pub evidence: Vec<EvidenceEntry>,
    pub coverage: BundleCoverage,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BundleOutput {
    manifest_path: String,
    summary: BundleSummary,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct BundleCoverage {
    pub total_feed_events: usize,
    pub events_with_evidence: usize,
    pub evidence_entries: usize,
    pub evidence_by_kind: BTreeMap<String, usize>,
    pub missing_hashes_total: usize,
    pub missing_hashes_by_feed: BTreeMap<String, usize>,
}

/// Garbage-collection summary for evidence bundles.
#[derive(Debug, Default, JsonSerialize)]
pub struct GcReport {
    pub generated_at_unix: u64,
    pub retention_days: u64,
    pub dispute_retention_days: u64,
    pub dry_run: bool,
    pub retained_bundles: usize,
    pub removed_bundles: Vec<PrunedBundle>,
    pub pruned_files: Vec<PrunedFile>,
    pub skipped_bundles: Vec<SkippedBundle>,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
struct GcOutput {
    report_path: String,
    report: GcReport,
}

#[derive(Debug, JsonSerialize)]
pub struct PrunedBundle {
    pub path: String,
    pub reason: String,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
pub struct PrunedFile {
    pub path: String,
    pub bundle: String,
    pub bytes_freed: u64,
}

#[derive(Debug, JsonSerialize)]
pub struct SkippedBundle {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct RejectionCatalog {
    pub version: u8,
    pub observation_errors: Vec<CatalogEntry>,
    pub aggregation_errors: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct CatalogEntry {
    pub code: String,
    pub meaning: String,
}

impl CatalogEntry {
    fn new(code: impl Into<String>, meaning: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            meaning: meaning.into(),
        }
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Bundle(cmd) => cmd.run(context),
            Command::Catalog(cmd) => cmd.run(context),
            Command::Gc(cmd) => cmd.run(context),
        }
    }
}

impl Run for Bundle {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle_root = normalize_dir(&self.output)?;
        let summary = build_bundle(&self, &bundle_root)?;
        let manifest_path = bundle_root.join("bundle.json");
        let rendered = json::to_json_pretty(&summary)
            .map_err(|err| eyre!("failed to render bundle manifest: {err}"))?;
        fs::create_dir_all(&bundle_root)
            .wrap_err_with(|| format!("failed to create {}", bundle_root.display()))?;
        fs::write(&manifest_path, rendered)
            .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))?;

        let text = format!(
            "soracles bundle wrote manifest={} (artifacts={} entries, feed_events={}, missing_hashes={})",
            manifest_path.display(),
            summary.evidence.len(),
            summary.feed_events.len(),
            summary.coverage.missing_hashes_total
        );
        let output = BundleOutput {
            manifest_path: manifest_path.display().to_string(),
            summary,
        };
        print_with_optional_text(context, Some(text), &output)
    }
}

impl Run for Catalog {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let catalog = build_rejection_catalog();
        match context.output_format() {
            crate::CliOutputFormat::Json => context.print_data(&catalog),
            crate::CliOutputFormat::Text => match self.format {
                CatalogFormat::Json => context.print_data(&catalog),
                CatalogFormat::Markdown => context.println(render_catalog_markdown(&catalog)),
            },
        }
    }
}

impl Run for GcArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let now = SystemTime::now();
        let report_path = self
            .report
            .clone()
            .unwrap_or_else(|| self.root.join("gc_report.json"));
        let report = garbage_collect(
            &self.root,
            self.retention_days,
            self.dispute_retention_days,
            self.prune_unreferenced,
            self.dry_run,
            now,
        )?;
        let rendered = json::to_json_pretty(&report)
            .map_err(|err| eyre!("failed to render GC report: {err}"))?;
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&report_path, rendered)
            .wrap_err_with(|| format!("failed to write {}", report_path.display()))?;

        let verb = if self.dry_run { "scanned" } else { "pruned" };
        let text = format!(
            "soracles evidence-gc {verb}: removed_bundles={} pruned_files={} bytes_freed={} retained={} report={}",
            report.removed_bundles.len(),
            report.pruned_files.len(),
            report.bytes_freed,
            report.retained_bundles,
            report_path.display()
        );
        let output = GcOutput {
            report_path: report_path.display().to_string(),
            report,
        };
        print_with_optional_text(context, Some(text), &output)
    }
}

fn build_bundle(options: &Bundle, bundle_root: &Path) -> Result<BundleSummary> {
    let artifacts_root = bundle_root.join("artifacts");
    fs::create_dir_all(&artifacts_root).wrap_err_with(|| {
        format!(
            "failed to create artifact directory {}",
            artifacts_root.display()
        )
    })?;

    let feed_events = load_feed_events(&options.events)?;
    let mut evidence = BTreeMap::<Hash, EvidenceEntry>::new();

    if let Some(path) = &options.observations {
        ingest_path(
            path,
            EvidenceKind::Observation,
            &artifacts_root,
            &mut evidence,
        )?;
    }
    if let Some(path) = &options.reports {
        ingest_path(path, EvidenceKind::Report, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.responses {
        ingest_path(path, EvidenceKind::Response, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.disputes {
        ingest_path(path, EvidenceKind::Dispute, &artifacts_root, &mut evidence)?;
    }
    if let Some(path) = &options.telemetry {
        ingest_path(
            path,
            EvidenceKind::Telemetry,
            &artifacts_root,
            &mut evidence,
        )?;
    }

    for entry in evidence.values_mut() {
        entry.sources.sort();
        entry.sources.dedup();
    }

    let feed_events = feed_events
        .into_iter()
        .map(|record| digest_event(record, &evidence))
        .collect::<Vec<_>>();
    let coverage = compute_coverage(&feed_events, &evidence);
    let evidence_entries: Vec<EvidenceEntry> = evidence.into_values().collect();

    write_artifact_index(&artifacts_root, &evidence_entries)?;

    let summary = BundleSummary {
        generated_at_unix: now_unix(SystemTime::now())?,
        artifact_root: "artifacts".to_string(),
        feed_events,
        evidence: evidence_entries,
        coverage,
    };

    Ok(summary)
}

fn compute_coverage(
    feed_events: &[FeedEventDigest],
    evidence: &BTreeMap<Hash, EvidenceEntry>,
) -> BundleCoverage {
    let mut evidence_by_kind = BTreeMap::new();
    for entry in evidence.values() {
        *evidence_by_kind.entry(entry.kind.clone()).or_insert(0) += 1usize;
    }

    let mut events_with_evidence = 0usize;
    let mut missing_hashes_total = 0usize;
    let mut missing_hashes_by_feed = BTreeMap::new();
    for event in feed_events {
        if !event.evidence_hashes.is_empty() {
            events_with_evidence += 1;
        }
        let missing = event.missing_evidence_hashes.len();
        if missing > 0 {
            missing_hashes_total += missing;
            let key = event.feed_id.to_string();
            *missing_hashes_by_feed.entry(key).or_insert(0) += missing;
        }
    }

    BundleCoverage {
        total_feed_events: feed_events.len(),
        events_with_evidence,
        evidence_entries: evidence.len(),
        evidence_by_kind,
        missing_hashes_total,
        missing_hashes_by_feed,
    }
}

fn write_artifact_index(root: &Path, evidence: &[EvidenceEntry]) -> Result<()> {
    let index_path = root.join("index.json");
    let rendered = json::to_json_pretty(&evidence.to_vec())
        .map_err(|err| eyre!("failed to render artifact index: {err}"))?;
    fs::write(&index_path, rendered)
        .wrap_err_with(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

fn build_rejection_catalog() -> RejectionCatalog {
    let observation_errors = vec![
        CatalogEntry::new(
            "ResourceUnavailable",
            "Upstream resource unavailable or transiently failing.",
        ),
        CatalogEntry::new(
            "AuthFailed",
            "Connector authentication or credentials failed.",
        ),
        CatalogEntry::new("Timeout", "Connector timed out before returning data."),
        CatalogEntry::new(
            "Missing",
            "Connector returned no payload or could not parse upstream data.",
        ),
        CatalogEntry::new(
            "Other(<u16>)",
            "Connector-specific error code recorded in `ObservationErrorCode::Other`.",
        ),
    ];

    let aggregation_errors = OracleRejectionCode::all()
        .iter()
        .map(|code| CatalogEntry::new(code.as_code(), code.description()))
        .collect();

    RejectionCatalog {
        version: 1,
        observation_errors,
        aggregation_errors,
    }
}

fn render_catalog_markdown(catalog: &RejectionCatalog) -> String {
    fn render_section(title: &str, entries: &[CatalogEntry], out: &mut String) {
        out.push_str("### ");
        out.push_str(title);
        out.push_str("\n\n| code | meaning |\n| --- | --- |\n");
        for entry in entries {
            out.push_str("| ");
            out.push_str(&entry.code);
            out.push_str(" | ");
            out.push_str(&entry.meaning);
            out.push_str(" |\n");
        }
        out.push('\n');
    }

    let mut rendered = String::new();
    let _ = writeln!(&mut rendered, "Catalog version {}", catalog.version);
    rendered.push('\n');
    render_section(
        "Observation errors",
        &catalog.observation_errors,
        &mut rendered,
    );
    render_section(
        "Aggregation errors",
        &catalog.aggregation_errors,
        &mut rendered,
    );
    rendered
}

fn digest_event(
    record: FeedEventRecord,
    evidence: &BTreeMap<Hash, EvidenceEntry>,
) -> FeedEventDigest {
    let missing = record
        .evidence_hashes
        .iter()
        .filter(|hash| !evidence.contains_key(hash))
        .copied()
        .collect::<Vec<_>>();

    FeedEventDigest {
        feed_id: record.event.feed_id,
        feed_config_version: record.event.feed_config_version,
        slot: record.event.slot,
        outcome: record.event.outcome,
        evidence_hashes: record.evidence_hashes,
        missing_evidence_hashes: missing,
    }
}

fn load_feed_events(path: &Path) -> Result<Vec<FeedEventRecord>> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read feed events from {}", path.display()))?;
    if let Ok(list) = json::from_slice::<Vec<FeedEventRecord>>(&bytes) {
        Ok(list)
    } else {
        let value: Value =
            json::from_slice(&bytes).wrap_err("failed to parse feed event JSON payload")?;
        if let Value::Array(values) = value {
            let mut events = Vec::new();
            for entry in values {
                let record =
                    json::from_value::<FeedEventRecord>(entry).map_err(|err| {
                        eyre!("failed to parse feed event from array entry: {err}")
                    })?;
                events.push(record);
            }
            Ok(events)
        } else {
            let record: FeedEventRecord =
                json::from_value(value).wrap_err("failed to parse feed event record")?;
            Ok(vec![record])
        }
    }
}

fn ingest_path(
    path: &Path,
    kind: EvidenceKind,
    artifact_root: &Path,
    evidence: &mut BTreeMap<Hash, EvidenceEntry>,
) -> Result<()> {
    let metadata = path
        .metadata()
        .wrap_err_with(|| format!("failed to stat {}", path.display()))?;
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).wrap_err_with(|| format!("failed to read {}", path.display()))?
        {
            let entry = entry?;
            ingest_path(&entry.path(), kind, artifact_root, evidence)?;
        }
        return Ok(());
    }

    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read evidence file {}", path.display()))?;
    let hash = Hash::new(&bytes);
    let bundled_name = bundled_name(&hash, path);
    let bundled_path = artifact_root.join(&bundled_name);

    if !bundled_path.exists() {
        fs::write(&bundled_path, &bytes).wrap_err_with(|| {
            format!(
                "failed to copy evidence {} -> {}",
                path.display(),
                bundled_path.display()
            )
        })?;
    }

    let source = path.display().to_string();
    let entry = evidence.entry(hash).or_insert_with(|| EvidenceEntry {
        kind: kind.as_label().to_string(),
        hash,
        sources: Vec::new(),
        bundled_path: format!("artifacts/{bundled_name}"),
        size_bytes: bytes.len() as u64,
    });

    if !entry.sources.contains(&source) {
        entry.sources.push(source);
    }

    Ok(())
}

fn bundled_name(hash: &Hash, source: &Path) -> String {
    let digest = hex::encode_upper(hash.as_ref());
    match source.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{digest}.{ext}"),
        _ => digest,
    }
}

fn normalize_dir(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .wrap_err("failed to get current directory")?
            .join(path)
    };
    Ok(absolute)
}

struct GcContext {
    retention: Option<Duration>,
    dispute_retention: Option<Duration>,
    retention_days: u64,
    dispute_retention_days: u64,
    prune_unreferenced: bool,
    dry_run: bool,
    now: SystemTime,
}

fn garbage_collect(
    root: &Path,
    retention_days: u64,
    dispute_retention_days: u64,
    prune_unreferenced: bool,
    dry_run: bool,
    now: SystemTime,
) -> Result<GcReport> {
    let mut report = GcReport {
        generated_at_unix: now_unix(now)?,
        retention_days,
        dispute_retention_days,
        dry_run,
        ..GcReport::default()
    };

    if !root.exists() {
        return Ok(report);
    }

    let ctx = GcContext {
        retention: retention_window(retention_days),
        dispute_retention: retention_window(dispute_retention_days),
        retention_days,
        dispute_retention_days,
        prune_unreferenced,
        dry_run,
        now,
    };

    for entry in
        fs::read_dir(root).wrap_err_with(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let bundle_root = entry.path();
        if !bundle_root.is_dir() {
            continue;
        }
        process_bundle_root(&bundle_root, &ctx, &mut report)?;
    }

    Ok(report)
}

fn process_bundle_root(bundle_root: &Path, ctx: &GcContext, report: &mut GcReport) -> Result<()> {
    let manifest_path = bundle_root.join("bundle.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let manifest_meta = match manifest_path.metadata() {
        Ok(meta) => meta,
        Err(err) => {
            report.skipped_bundles.push(SkippedBundle {
                path: bundle_root.display().to_string(),
                reason: format!("failed to stat bundle.json: {err}"),
            });
            return Ok(());
        }
    };

    let manifest_bytes = fs::read(&manifest_path)
        .wrap_err_with(|| format!("failed to read {}", manifest_path.display()))?;
    let summary: BundleSummary = match json::from_slice(&manifest_bytes) {
        Ok(summary) => summary,
        Err(err) => {
            report.skipped_bundles.push(SkippedBundle {
                path: bundle_root.display().to_string(),
                reason: format!("failed to parse manifest: {err}"),
            });
            return Ok(());
        }
    };

    let age = compute_bundle_age(&summary, &manifest_meta, ctx.now);
    let has_dispute = summary
        .evidence
        .iter()
        .any(|entry| entry.kind == EvidenceKind::Dispute.as_label());
    let window = if has_dispute {
        ctx.dispute_retention.as_ref()
    } else {
        ctx.retention.as_ref()
    };

    let expired = match (window, age) {
        (None, _) => true,
        (Some(limit), Some(duration)) => duration >= *limit,
        (Some(_), None) => false,
    };

    let bundle_label = bundle_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let artifacts_root = bundle_root.join(&summary.artifact_root);

    if expired {
        let reason_days = if has_dispute {
            ctx.dispute_retention_days
        } else {
            ctx.retention_days
        };
        let bytes_freed = dir_size(bundle_root)?;
        report.bytes_freed = report.bytes_freed.saturating_add(bytes_freed);
        report.removed_bundles.push(PrunedBundle {
            path: bundle_root.display().to_string(),
            reason: format!("older than {reason_days} days"),
            bytes_freed,
        });
        if !ctx.dry_run {
            fs::remove_dir_all(bundle_root).wrap_err_with(|| {
                format!(
                    "failed to remove expired bundle directory {}",
                    bundle_root.display()
                )
            })?;
        }
        return Ok(());
    }

    report.retained_bundles = report.retained_bundles.saturating_add(1);

    if ctx.prune_unreferenced && artifacts_root.exists() {
        let referenced = referenced_artifacts(bundle_root, &summary);
        prune_unreferenced_files(
            &artifacts_root,
            &referenced,
            &bundle_label,
            ctx.dry_run,
            report,
        )?;
    }

    Ok(())
}

fn compute_bundle_age(
    summary: &BundleSummary,
    manifest_meta: &fs::Metadata,
    now: SystemTime,
) -> Option<Duration> {
    let generated = if summary.generated_at_unix > 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(summary.generated_at_unix))?
    } else {
        manifest_meta.modified().ok()?
    };
    now.duration_since(generated).ok()
}

fn retention_window(days: u64) -> Option<Duration> {
    if days == 0 {
        None
    } else {
        Some(Duration::from_secs(days.saturating_mul(86_400)))
    }
}

fn referenced_artifacts(bundle_root: &Path, summary: &BundleSummary) -> HashSet<PathBuf> {
    summary
        .evidence
        .iter()
        .map(|entry| bundle_root.join(&entry.bundled_path))
        .collect()
}

fn prune_unreferenced_files(
    artifact_root: &Path,
    referenced: &HashSet<PathBuf>,
    bundle_label: &str,
    dry_run: bool,
    report: &mut GcReport,
) -> Result<()> {
    let mut stack = vec![artifact_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).wrap_err_with(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() || referenced.contains(&path) {
                continue;
            }

            let bytes_freed = path.metadata().map(|m| m.len()).unwrap_or(0);
            report.pruned_files.push(PrunedFile {
                path: path.display().to_string(),
                bundle: bundle_label.to_string(),
                bytes_freed,
            });
            report.bytes_freed = report.bytes_freed.saturating_add(bytes_freed);
            if !dry_run {
                fs::remove_file(&path).wrap_err_with(|| {
                    format!("failed to remove unreferenced artifact {}", path.display())
                })?;
            }
        }
    }

    Ok(())
}

fn dir_size(root: &Path) -> Result<u64> {
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).wrap_err_with(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(meta) = path.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn now_unix(now: SystemTime) -> Result<u64> {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|err| eyre!("system clock before unix epoch: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::oracle::{
        FeedEvent, FeedSuccess, ObservationBody, ObservationValue, ReportEntry,
    };
    use iroha_crypto::HashOf;
    use iroha_i18n::{Bundle as I18nBundle, Language, Localizer};
    use std::fmt::Display;
    use tempfile::TempDir;

    struct TestContext {
        output_format: crate::CliOutputFormat,
        printed: Vec<String>,
        config: iroha::config::Config,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(output_format: crate::CliOutputFormat) -> Self {
            Self {
                output_format,
                printed: Vec::new(),
                config: crate::fallback_config(),
                i18n: Localizer::new(I18nBundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.config
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> crate::CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> eyre::Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let rendered = norito::json::to_json_pretty(data)?;
            self.printed.push(rendered);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> eyre::Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn builds_bundle_and_marks_missing_hashes() {
        let tmp = TempDir::new().expect("tmpdir");
        let events_path = tmp.path().join("events.json");
        let artifacts_root = tmp.path().join("evidence");
        let missing_hash = Hash::new(b"missing");
        let observed_payload = br#"{"dummy":"payload"}"#;

        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let observed = Hash::new(observed_payload);
        let observation_hash = HashOf::<ObservationBody>::from_untyped_unchecked(Hash::new(b"req"));

        let event = FeedEventRecord {
            event: FeedEvent {
                feed_id: feed_id.clone(),
                feed_config_version: FeedConfigVersion(1),
                slot: 42,
                outcome: FeedEventOutcome::Success(FeedSuccess {
                    value: ObservationValue::new(1_000, 2),
                    entries: vec![ReportEntry {
                        oracle_id: "34mSYnLrmfrui7Ba2h9RbAPY1hHa7ZCvLRLUSYBujVoUYk1eeBFAZPChUmyGTH47EtrQxAFVA"
                            .parse()
                            .expect("oracle id"),
                        observation_hash,
                        value: ObservationValue::new(1_000, 2),
                        outlier: false,
                    }],
                }),
            },
            evidence_hashes: vec![observed, missing_hash],
        };

        let rendered = json::to_json_pretty(&event).expect("serialize feed event");
        fs::write(&events_path, rendered).expect("write events");

        let observation_dir = tmp.path().join("observations");
        fs::create_dir(&observation_dir).expect("mkdir observations");
        let observation_path = observation_dir.join("obs.json");
        fs::write(&observation_path, observed_payload).expect("write observation");

        let options = Bundle {
            events: events_path.clone(),
            output: artifacts_root.clone(),
            observations: Some(observation_dir),
            reports: None,
            responses: None,
            disputes: None,
            telemetry: None,
        };

        let bundle_root = normalize_dir(&options.output).expect("normalize output");
        let summary = build_bundle(&options, &bundle_root).expect("build bundle");
        assert_eq!(summary.feed_events.len(), 1);
        assert_eq!(summary.evidence.len(), 1);
        assert_eq!(
            summary.feed_events[0].missing_evidence_hashes,
            vec![missing_hash]
        );
        assert_eq!(summary.coverage.total_feed_events, 1);
        assert_eq!(summary.coverage.events_with_evidence, 1);
        assert_eq!(summary.coverage.evidence_entries, 1);
        assert_eq!(summary.coverage.missing_hashes_total, 1);
        assert_eq!(
            summary
                .coverage
                .missing_hashes_by_feed
                .get(feed_id.as_str()),
            Some(&1)
        );
        assert_eq!(
            summary
                .coverage
                .evidence_by_kind
                .get("observation")
                .copied(),
            Some(1)
        );
        let bundled = artifacts_root.join("bundle.json");
        assert!(!bundled.exists(), "summary writing handled by run");
        let copied = artifacts_root.join("artifacts");
        assert!(copied.exists(), "artifact directory created");
        assert!(
            fs::read_dir(&copied).expect("read dir").next().is_some(),
            "artifact file written"
        );
        let index_path = copied.join("index.json");
        let index_bytes = fs::read(&index_path).expect("index exists");
        let index_value: Value = json::from_slice(&index_bytes).expect("index parses");
        match index_value {
            Value::Array(entries) => assert_eq!(entries.len(), 1, "index entry count"),
            other => panic!("unexpected index format: {other:?}"),
        }
    }

    #[test]
    fn catalog_markdown_lists_codes() {
        let catalog = build_rejection_catalog();
        let rendered = render_catalog_markdown(&catalog);
        assert!(
            rendered.contains("Observation errors"),
            "missing observation header"
        );
        assert!(
            rendered.contains("ResourceUnavailable"),
            "expected ResourceUnavailable entry"
        );
        assert!(
            rendered.contains("Aggregation errors"),
            "missing aggregation header"
        );
        assert!(
            rendered.contains("oracle_model_feed_version_mismatch"),
            "expected model code entry"
        );
        assert!(
            rendered.contains("oracle_agg_no_inliers"),
            "expected aggregation code entry"
        );
        assert!(
            rendered.contains("Catalog version"),
            "expected catalog version header"
        );
    }

    #[test]
    fn catalog_run_ignores_markdown_when_output_is_json() {
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        Catalog {
            format: CatalogFormat::Markdown,
        }
        .run(&mut ctx)
        .expect("run catalog");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert!(obj.contains_key("observation_errors"));
    }

    #[test]
    fn bundle_run_emits_json_output() {
        let tmp = TempDir::new().expect("tmpdir");
        let events_path = tmp.path().join("events.json");
        let artifacts_root = tmp.path().join("bundle");
        let observed_payload = br#"{"dummy":"payload"}"#;
        let feed_id: FeedId = "xor_usd".parse().expect("feed id");
        let observed = Hash::new(observed_payload);
        let observation_hash = HashOf::<ObservationBody>::from_untyped_unchecked(Hash::new(b"req"));
        let event = FeedEventRecord {
            event: FeedEvent {
                feed_id,
                feed_config_version: FeedConfigVersion(1),
                slot: 42,
                outcome: FeedEventOutcome::Success(FeedSuccess {
                    value: ObservationValue::new(1_000, 2),
                    entries: vec![ReportEntry {
                        oracle_id: "34mSYnLrmfrui7Ba2h9RbAPY1hHa7ZCvLRLUSYBujVoUYk1eeBFAZPChUmyGTH47EtrQxAFVA"
                            .parse()
                            .expect("oracle id"),
                        observation_hash,
                        value: ObservationValue::new(1_000, 2),
                        outlier: false,
                    }],
                }),
            },
            evidence_hashes: vec![observed],
        };
        let rendered = json::to_json_pretty(&event).expect("serialize feed event");
        fs::write(&events_path, rendered).expect("write events");
        let observation_dir = tmp.path().join("observations");
        fs::create_dir(&observation_dir).expect("mkdir observations");
        let observation_path = observation_dir.join("obs.json");
        fs::write(&observation_path, observed_payload).expect("write observation");

        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        Bundle {
            events: events_path,
            output: artifacts_root,
            observations: Some(observation_dir),
            reports: None,
            responses: None,
            disputes: None,
            telemetry: None,
        }
        .run(&mut ctx)
        .expect("bundle run");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert!(obj.contains_key("manifest_path"));
        assert!(obj.contains_key("summary"));
    }

    #[test]
    fn gc_run_emits_json_output() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = tmp.path().join("soracles");
        fs::create_dir(&root).expect("mkdir root");
        let report_path = root.join("gc_report.json");
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        GcArgs {
            root,
            retention_days: 1,
            dispute_retention_days: 1,
            report: Some(report_path.clone()),
            prune_unreferenced: false,
            dry_run: true,
        }
        .run(&mut ctx)
        .expect("gc run");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert_eq!(
            obj.get("report_path").and_then(Value::as_str),
            Some(report_path.to_string_lossy().as_ref())
        );
        assert!(obj.get("report").is_some());
    }

    #[test]
    fn gc_removes_expired_bundles_and_keeps_fresh() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let old_time = now
            .checked_sub(Duration::from_secs(172_800))
            .expect("time travel");
        let old_secs = now_unix(old_time).expect("unix time");
        let fresh_secs = now_unix(now).expect("unix time");

        let expired = write_test_bundle(
            tmp.path(),
            "expired",
            old_secs,
            "observation",
            &[("artifacts/old.json", b"old")],
        );
        let fresh = write_test_bundle(
            tmp.path(),
            "fresh",
            fresh_secs,
            "observation",
            &[("artifacts/fresh.json", b"fresh")],
        );

        let report = garbage_collect(tmp.path(), 1, 365, false, false, now).expect("gc succeeds");
        assert_eq!(report.removed_bundles.len(), 1);
        assert!(!expired.exists(), "expired bundle should be removed");
        assert!(fresh.exists(), "fresh bundle should remain");
    }

    #[test]
    fn gc_prunes_unreferenced_files_in_dry_run() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let bundle = write_test_bundle(
            tmp.path(),
            "keep",
            now_unix(now).expect("unix time"),
            "observation",
            &[("artifacts/ref.bin", b"ref")],
        );
        let orphan = bundle.join("artifacts/orphan.bin");
        fs::create_dir_all(orphan.parent().expect("parent")).expect("mkdir orphan parent");
        fs::write(&orphan, b"orphaned").expect("write orphan");

        let report = garbage_collect(tmp.path(), 365, 365, true, true, now)
            .expect("gc succeeds with dry-run");
        assert_eq!(
            report.pruned_files.len(),
            1,
            "unreferenced file should be reported"
        );
        assert!(orphan.exists(), "dry-run must not delete files");
        assert_eq!(report.retained_bundles, 1);
    }

    #[test]
    fn gc_respects_longer_dispute_retention() {
        let tmp = TempDir::new().expect("tmpdir");
        let now = SystemTime::now();
        let old_time = now
            .checked_sub(Duration::from_secs(200 * 86_400))
            .expect("time travel");
        let old_secs = now_unix(old_time).expect("unix time");

        let dispute_bundle = write_test_bundle(
            tmp.path(),
            "dispute",
            old_secs,
            "dispute",
            &[("artifacts/dispute.json", b"dispute")],
        );
        assert!(
            dispute_bundle.exists(),
            "bundle should be laid out for gc test"
        );

        let report = garbage_collect(tmp.path(), 180, 365, false, false, now)
            .expect("gc succeeds with dispute retention");
        assert_eq!(
            report.removed_bundles.len(),
            0,
            "dispute bundle should remain under longer window"
        );
        assert!(dispute_bundle.exists(), "dispute evidence not pruned early");
    }

    fn write_test_bundle(
        root: &Path,
        name: &str,
        generated_at_unix: u64,
        kind: &str,
        artifacts: &[(&str, &[u8])],
    ) -> PathBuf {
        let bundle_root = root.join(name);
        let artifact_dir = bundle_root.join("artifacts");
        fs::create_dir_all(&artifact_dir).expect("create artifact dir");

        let mut evidence_entries = Vec::new();
        for &(relative_path, contents) in artifacts {
            let full_path = bundle_root.join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(&full_path, contents).expect("write artifact");
            let hash = Hash::new(contents);
            evidence_entries.push(EvidenceEntry {
                kind: kind.to_string(),
                hash,
                sources: vec![],
                bundled_path: relative_path.to_string(),
                size_bytes: contents.len() as u64,
            });
        }

        let mut evidence_by_kind = BTreeMap::new();
        evidence_by_kind.insert(kind.to_string(), evidence_entries.len());
        let summary = BundleSummary {
            generated_at_unix,
            artifact_root: "artifacts".to_string(),
            feed_events: Vec::new(),
            evidence: evidence_entries,
            coverage: BundleCoverage {
                total_feed_events: 0,
                events_with_evidence: 0,
                evidence_entries: artifacts.len(),
                evidence_by_kind,
                missing_hashes_total: 0,
                missing_hashes_by_feed: BTreeMap::new(),
            },
        };
        let manifest = json::to_json_pretty(&summary).expect("render bundle");
        fs::create_dir_all(&bundle_root).expect("create bundle root");
        fs::write(bundle_root.join("bundle.json"), manifest).expect("write bundle manifest");
        bundle_root
    }
}
