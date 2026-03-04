use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher;
use iroha_data_model::ministry::{ReviewPanelSummaryV1, TransparencyReleaseV1};
use norito::{
    json::{self, JsonDeserialize, JsonSerialize, Value},
    to_bytes,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use serde_json::Value as SerdeJsonValue;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::ministry_panel::{self, SynthesizeOptions as PanelSynthesizeOptions};

pub enum Command {
    Ingest(Box<IngestOptions>),
    Build(Box<BuildOptions>),
    Sanitize(Box<SanitizeOptions>),
    Anchor(Box<AnchorOptions>),
    VolunteerValidate(Box<VolunteerValidateOptions>),
}

#[derive(Clone)]
pub struct IngestOptions {
    pub quarter: String,
    pub ledger_path: PathBuf,
    pub appeals_path: PathBuf,
    pub denylist_path: PathBuf,
    pub treasury_path: PathBuf,
    pub volunteer_path: Option<PathBuf>,
    pub red_team_reports: Vec<PathBuf>,
    pub panel_summary: Option<PanelSummaryRequest>,
    pub output_path: PathBuf,
}

#[derive(Clone)]
pub struct PanelSummaryRequest {
    pub proposal_path: PathBuf,
    pub volunteer_path: PathBuf,
    pub ai_manifest_path: PathBuf,
    pub panel_round_id: String,
    pub output_path: PathBuf,
    pub language_override: Option<String>,
    pub generated_at_unix_ms: Option<u64>,
}

#[derive(Clone)]
pub struct BuildOptions {
    pub ingest_path: PathBuf,
    pub metrics_output: PathBuf,
    pub manifest_output: PathBuf,
    pub note: Option<String>,
}

#[derive(Clone)]
pub struct SanitizeOptions {
    pub ingest_path: PathBuf,
    pub output_path: PathBuf,
    pub report_path: PathBuf,
    pub epsilon_counts: f64,
    pub epsilon_accuracy: f64,
    pub delta: f64,
    pub suppress_threshold: u64,
    pub min_accuracy_samples: u64,
    pub seed: Option<u64>,
}

#[derive(Clone)]
pub struct AnchorOptions {
    pub action_path: PathBuf,
    pub governance_dir: PathBuf,
}

#[derive(Clone)]
pub struct VolunteerValidateOptions {
    pub inputs: Vec<PathBuf>,
    pub json_output: Option<PathBuf>,
}

const APPEAL_SLA_HOURS: f64 = 72.0;

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct QuarterIngestSnapshot {
    quarter: String,
    generated_at: String,
    ai_metrics: BTreeMap<String, AiMetricSummary>,
    appeals: AppealsSummary,
    denylist: DenylistSummary,
    treasury: TreasurySummary,
    volunteer: Option<VolunteerSummary>,
    #[norito(default)]
    red_team: Vec<RedTeamScenario>,
    review_panel_summary: Option<ReviewPanelSummaryV1>,
    source_checksums: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct RedTeamScenario {
    drill_id: String,
    #[serde(default)]
    date_window: Option<String>,
    #[serde(default)]
    scenario_class: Option<String>,
    #[serde(default)]
    operators: Vec<String>,
    #[serde(default)]
    dashboards_sha: Option<String>,
    #[serde(default)]
    evidence_path: Option<String>,
    #[serde(default)]
    sorafs_cid: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct AiMetricSummary {
    total_samples: u64,
    false_positives: u64,
    false_negatives: u64,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct AppealsSummary {
    total: u64,
    resolved: u64,
    reopened: u64,
    sla_breaches: u64,
    avg_resolution_hours: f64,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct DenylistSummary {
    additions: u64,
    removals: u64,
    emergency_actions: u64,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct TreasurySummary {
    total_deposits_xor: i64,
    total_payouts_xor: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSerialize, JsonDeserialize, Clone)]
struct VolunteerSummary {
    total_briefs: u64,
    languages: BTreeMap<String, u64>,
    fact_rows: u64,
    fact_rows_with_citation: u64,
    disclosures_missing: u64,
    off_topic_rejections: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct QuarterDashboard {
    quarter: String,
    generated_at: String,
    ai_accuracy: BTreeMap<String, AiAccuracyRow>,
    appeals: AppealDashboard,
    denylist: DenylistDashboard,
    treasury: TreasuryDashboard,
    volunteer: Option<VolunteerSummary>,
    #[serde(default)]
    red_team: Vec<RedTeamScenario>,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct AiAccuracyRow {
    total_samples: u64,
    false_positive_rate: f64,
    false_negative_rate: f64,
    accuracy: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct AppealDashboard {
    total: u64,
    resolved: u64,
    avg_resolution_hours: f64,
    sla_breach_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct DenylistDashboard {
    additions: u64,
    removals: u64,
    emergency_actions: u64,
    net_delta: i64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct TreasuryDashboard {
    total_deposits_xor: String,
    total_payouts_xor: String,
    net_flow_xor: String,
}

#[derive(Debug, JsonSerialize)]
struct TransparencyManifest {
    version: u32,
    quarter: String,
    generated_at: String,
    ingest_checksum: String,
    metrics_checksum: String,
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizedMetrics {
    quarter: String,
    generated_at: String,
    metadata: SanitizerMetadata,
    appeals: SanitizedAppeals,
    denylist: SanitizedDenylist,
    treasury: TreasurySummary,
    volunteer: Option<SanitizedVolunteer>,
    ai_policies: BTreeMap<String, SanitizedPolicy>,
    #[serde(default)]
    red_team: Vec<RedTeamScenario>,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizerMetadata {
    epsilon_counts: f64,
    epsilon_accuracy: f64,
    delta: f64,
    suppress_threshold: u64,
    min_accuracy_samples: u64,
    seed_commitment: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizedAppeals {
    total: u64,
    resolved: u64,
    reopened: u64,
    sla_breaches: u64,
    avg_resolution_hours: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizedDenylist {
    additions: u64,
    removals: u64,
    emergency_actions: u64,
    net_delta: i64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizedVolunteer {
    total_briefs: u64,
    languages: BTreeMap<String, u64>,
    fact_rows: u64,
    fact_rows_with_citation: u64,
    disclosures_missing: u64,
    off_topic_rejections: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSerialize, JsonDeserialize)]
struct SanitizedPolicy {
    total_samples: u64,
    false_positive_rate: f64,
    false_negative_rate: f64,
    accuracy: f64,
    suppressed: bool,
}

#[derive(Debug, JsonSerialize)]
struct DpReport {
    quarter: String,
    generated_at: String,
    epsilon_counts: f64,
    epsilon_accuracy: f64,
    delta: f64,
    suppress_threshold: u64,
    min_accuracy_samples: u64,
    seed_commitment: String,
    count_buckets: Vec<CountNoiseRecord>,
    accuracy_buckets: Vec<AccuracyNoiseRecord>,
    suppressed_buckets: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct CountNoiseRecord {
    bucket: String,
    raw: f64,
    noise: f64,
    sanitized: f64,
    suppressed: bool,
}

#[derive(Debug, JsonSerialize)]
struct AccuracyNoiseRecord {
    policy: String,
    raw_samples: u64,
    raw_false_positive: u64,
    raw_false_negative: u64,
    noise_samples: f64,
    noise_false_positive: f64,
    noise_false_negative: f64,
    sanitized_samples: f64,
    sanitized_false_positive: f64,
    sanitized_false_negative: f64,
    suppressed: bool,
}

pub fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Ingest(options) => run_ingest(*options),
        Command::Build(options) => run_build(*options),
        Command::Sanitize(options) => run_sanitize(*options),
        Command::Anchor(options) => run_anchor(*options),
        Command::VolunteerValidate(options) => run_volunteer_validate(*options),
    }
}

fn parse_red_team_report(path: &Path) -> Result<RedTeamScenario, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let drill_id = capture_report_value(&lines, "Drill ID:")
        .ok_or_else(|| format!("{}: missing `Drill ID:` line", path.display()))?;
    let date_window = capture_report_value(&lines, "Date & window:");
    let scenario_class = capture_report_value(&lines, "Scenario class:");
    let operators_raw = capture_report_value(&lines, "Operators:");
    let dashboards_sha = capture_report_value(&lines, "Dashboards frozen from commit:");
    let evidence_path = capture_report_value(&lines, "Evidence bundle:");
    let sorafs_cid = capture_report_value(&lines, "SoraFS CID (optional):");

    let operators = operators_raw
        .map(|text| {
            text.split([',', '|'])
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(|entry| entry.trim_matches('`').to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(RedTeamScenario {
        drill_id,
        date_window,
        scenario_class,
        operators,
        dashboards_sha,
        evidence_path,
        sorafs_cid,
    })
}

fn capture_report_value(lines: &[&str], label: &str) -> Option<String> {
    let needle = format!("**{label}**");
    lines.iter().find_map(|line| {
        let trimmed = line.trim();
        let pos = trimmed.find(&needle)?;
        let rest = trimmed[pos + needle.len()..].trim_start_matches(':').trim();
        let cleaned = rest.trim_matches('`').trim();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.to_string())
        }
    })
}

fn run_ingest(options: IngestOptions) -> Result<(), Box<dyn Error>> {
    let ledger_entries = load_array(&options.ledger_path)?;
    let appeals_entries = load_array(&options.appeals_path)?;
    let denylist_entries = load_array(&options.denylist_path)?;
    let treasury_entries = load_array(&options.treasury_path)?;
    let volunteer_entries = match &options.volunteer_path {
        Some(path) => Some(load_array(path)?),
        None => None,
    };
    let mut red_team_scenarios: Vec<RedTeamScenario> = Vec::new();

    let mut source_checksums = collect_checksums(&[
        ("ledger", &options.ledger_path),
        ("appeals", &options.appeals_path),
        ("denylist", &options.denylist_path),
        ("treasury", &options.treasury_path),
    ])?;
    if let Some(path) = &options.volunteer_path {
        let sum = file_checksum(path)?;
        source_checksums.insert("volunteer".to_string(), sum);
    }
    for report in &options.red_team_reports {
        let scenario = parse_red_team_report(report)?;
        if red_team_scenarios
            .iter()
            .any(|existing| existing.drill_id == scenario.drill_id)
        {
            return Err(format!(
                "duplicate drill id `{}` in red-team reports (second: {})",
                scenario.drill_id,
                report.display()
            )
            .into());
        }
        red_team_scenarios.push(scenario);
        let checksum_key = report
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("red_team:{name}"))
            .unwrap_or_else(|| format!("red_team:{}", report.display()));
        let sum = file_checksum(report)?;
        source_checksums.insert(checksum_key, sum);
    }
    let review_panel_summary = if let Some(panel) = &options.panel_summary {
        let synth_options = PanelSynthesizeOptions {
            proposal_path: panel.proposal_path.clone(),
            volunteer_path: panel.volunteer_path.clone(),
            ai_manifest_path: panel.ai_manifest_path.clone(),
            output_path: panel.output_path.clone(),
            panel_round_id: panel.panel_round_id.clone(),
            language_override: panel.language_override.clone(),
            generated_at_unix_ms: panel.generated_at_unix_ms,
        };
        let summary = ministry_panel::synthesize_summary(&synth_options)?;
        write_json_file(&panel.output_path, &summary)?;
        let sum = file_checksum(&panel.output_path)?;
        source_checksums.insert("review_panel_summary".to_string(), sum);
        Some(summary)
    } else {
        None
    };

    let snapshot = QuarterIngestSnapshot {
        quarter: options.quarter,
        generated_at: now_rfc3339(),
        ai_metrics: summarize_ai_metrics(&ledger_entries),
        appeals: summarize_appeals(&appeals_entries),
        denylist: summarize_denylist(&denylist_entries),
        treasury: summarize_treasury(&treasury_entries),
        volunteer: volunteer_entries
            .as_deref()
            .map(summarize_volunteer)
            .filter(|summary| summary.total_briefs > 0 || summary.off_topic_rejections > 0),
        red_team: red_team_scenarios,
        review_panel_summary,
        source_checksums,
    };

    write_json_file(&options.output_path, &snapshot)?;
    println!(
        "wrote ministry transparency ingest snapshot to {}",
        options.output_path.display()
    );
    Ok(())
}

fn run_build(options: BuildOptions) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(&options.ingest_path)?;
    let ingest: QuarterIngestSnapshot = json::from_slice(&bytes)?;

    let dashboard = QuarterDashboard {
        quarter: ingest.quarter.clone(),
        generated_at: now_rfc3339(),
        ai_accuracy: build_ai_accuracy(&ingest.ai_metrics),
        appeals: build_appeal_dashboard(&ingest.appeals),
        denylist: build_denylist_dashboard(&ingest.denylist),
        treasury: build_treasury_dashboard(&ingest.treasury),
        volunteer: ingest.volunteer.clone(),
        red_team: ingest.red_team.clone(),
    };

    write_json_file(&options.metrics_output, &dashboard)?;

    let manifest = TransparencyManifest {
        version: 1,
        quarter: ingest.quarter,
        generated_at: now_rfc3339(),
        ingest_checksum: file_checksum(&options.ingest_path)?,
        metrics_checksum: file_checksum(&options.metrics_output)?,
        note: options.note,
    };
    write_json_file(&options.manifest_output, &manifest)?;

    println!(
        "wrote ministry transparency metrics to {} and manifest to {}",
        options.metrics_output.display(),
        options.manifest_output.display()
    );
    Ok(())
}

fn run_sanitize(options: SanitizeOptions) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(&options.ingest_path)?;
    let ingest: QuarterIngestSnapshot = json::from_slice(&bytes)?;

    let seed = options.seed.unwrap_or_else(rand::random::<u64>);
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let seed_commitment = {
        let mut buf = seed.to_be_bytes().to_vec();
        buf.extend_from_slice(ingest.quarter.as_bytes());
        blake3::hash(&buf).to_hex().to_string()
    };

    let mut count_records = Vec::new();
    let mut accuracy_records = Vec::new();
    let mut suppressed = BTreeSet::new();

    let appeals_total = sanitize_count(
        "appeals.total",
        ingest.appeals.total,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let appeals_resolved = sanitize_count(
        "appeals.resolved",
        ingest.appeals.resolved,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let appeals_reopened = sanitize_count(
        "appeals.reopened",
        ingest.appeals.reopened,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let appeals_sla_breaches = sanitize_count(
        "appeals.sla_breaches",
        ingest.appeals.sla_breaches,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let raw_resolution_hours = ingest.appeals.avg_resolution_hours * ingest.appeals.resolved as f64;
    let resolution_noise = laplace_noise(options.epsilon_counts, &mut rng);
    let noisy_resolution_hours = (raw_resolution_hours + resolution_noise).max(0.0);
    let appeals_avg_resolution_hours = if appeals_resolved > 0 {
        noisy_resolution_hours / appeals_resolved as f64
    } else {
        0.0
    };
    count_records.push(CountNoiseRecord {
        bucket: "appeals.avg_resolution_hours".to_string(),
        raw: raw_resolution_hours,
        noise: resolution_noise,
        sanitized: appeals_avg_resolution_hours,
        suppressed: false,
    });

    let denylist_additions = sanitize_count(
        "denylist.additions",
        ingest.denylist.additions,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let denylist_removals = sanitize_count(
        "denylist.removals",
        ingest.denylist.removals,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );
    let denylist_emergency = sanitize_count(
        "denylist.emergency_actions",
        ingest.denylist.emergency_actions,
        options.epsilon_counts,
        options.suppress_threshold,
        &mut rng,
        &mut count_records,
        &mut suppressed,
    );

    let sanitized_volunteer = ingest
        .volunteer
        .as_ref()
        .map(|vol| {
            sanitize_volunteer_summary(vol, &options, &mut rng, &mut count_records, &mut suppressed)
        })
        .filter(|vol| {
            vol.total_briefs > 0 || vol.off_topic_rejections > 0 || !vol.languages.is_empty()
        });

    let (ai_policies, policy_records) =
        sanitize_ai_policies(&ingest.ai_metrics, &options, &mut rng, &mut suppressed);
    accuracy_records.extend(policy_records);

    let sanitized = SanitizedMetrics {
        quarter: ingest.quarter.clone(),
        generated_at: now_rfc3339(),
        metadata: SanitizerMetadata {
            epsilon_counts: options.epsilon_counts,
            epsilon_accuracy: options.epsilon_accuracy,
            delta: options.delta,
            suppress_threshold: options.suppress_threshold,
            min_accuracy_samples: options.min_accuracy_samples,
            seed_commitment: seed_commitment.clone(),
        },
        appeals: SanitizedAppeals {
            total: appeals_total,
            resolved: appeals_resolved,
            reopened: appeals_reopened,
            sla_breaches: appeals_sla_breaches,
            avg_resolution_hours: appeals_avg_resolution_hours,
        },
        denylist: SanitizedDenylist {
            additions: denylist_additions,
            removals: denylist_removals,
            emergency_actions: denylist_emergency,
            net_delta: denylist_additions as i64 - denylist_removals as i64,
        },
        treasury: ingest.treasury.clone(),
        volunteer: sanitized_volunteer,
        ai_policies,
        red_team: ingest.red_team.clone(),
    };

    let report = DpReport {
        quarter: ingest.quarter,
        generated_at: sanitized.generated_at.clone(),
        epsilon_counts: options.epsilon_counts,
        epsilon_accuracy: options.epsilon_accuracy,
        delta: options.delta,
        suppress_threshold: options.suppress_threshold,
        min_accuracy_samples: options.min_accuracy_samples,
        seed_commitment,
        count_buckets: count_records,
        accuracy_buckets: accuracy_records,
        suppressed_buckets: suppressed.into_iter().collect(),
    };

    write_json_file(&options.output_path, &sanitized)?;
    write_json_file(&options.report_path, &report)?;

    println!(
        "wrote sanitized transparency metrics to {} and DP report to {}",
        options.output_path.display(),
        options.report_path.display()
    );
    Ok(())
}

fn run_anchor(options: AnchorOptions) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&options.action_path)?;
    let json_value: SerdeJsonValue = serde_json::from_slice(&raw)?;
    let payload: TransparencyReleaseActionPayload = serde_json::from_value(json_value.clone())?;

    if payload.action != "TransparencyReleaseV1" {
        return Err(format!(
            "unsupported action `{}` in {} (expected TransparencyReleaseV1)",
            payload.action,
            options.action_path.display()
        )
        .into());
    }

    if payload.version != 1 {
        return Err(format!(
            "unsupported TransparencyReleaseV1 version {} in {} (expected 1)",
            payload.version,
            options.action_path.display()
        )
        .into());
    }

    let generated_at = OffsetDateTime::parse(&payload.generated_at, &Rfc3339)
        .map_err(|err| format!("invalid generated_at timestamp: {err}"))?;
    let generated_at_ms = generated_at.unix_timestamp_nanos() / 1_000_000;
    if generated_at_ms < 0 {
        return Err(
            "generated_at timestamp must be greater than or equal to the Unix epoch".into(),
        );
    }
    let generated_at_ms = generated_at_ms as u64;

    let manifest_digest = decode_fixed_hex::<32>(
        &payload.manifest_digest_blake2b_256,
        "manifest_digest_blake2b_256",
    )?;
    let sorafs_cid = decode_hex_vec(&payload.sorafs_cid_hex, "sorafs_cid_hex")?;
    let dashboards_git_sha = match payload.dashboards_git_sha.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            Some(decode_hex_vec(value, "dashboards_git_sha")?)
        }
        _ => None,
    };

    let release = TransparencyReleaseV1 {
        quarter: payload.quarter.clone(),
        generated_at_unix_ms: generated_at_ms,
        manifest_digest_blake2b_256: manifest_digest,
        sorafs_root_cid: sorafs_cid,
        dashboards_git_sha,
        note: payload.note.clone(),
    };

    let encoded = to_bytes(&release)?;
    let quarter_dir = options
        .governance_dir
        .join("ministry")
        .join("releases")
        .join(&payload.quarter);
    fs::create_dir_all(&quarter_dir)?;
    let quarter_slug = payload.quarter.replace('/', "-");
    let base_name = format!("{}_{}", quarter_slug, generated_at_ms);
    let norito_path = quarter_dir.join(format!("{base_name}.to"));
    fs::write(&norito_path, &encoded)?;

    let mut json_text = serde_json::to_string_pretty(&json_value)?;
    json_text.push('\n');
    let json_path = quarter_dir.join(format!("{base_name}.json"));
    fs::write(&json_path, json_text)?;

    println!(
        "anchored TransparencyReleaseV1 for {} at {}",
        payload.quarter,
        norito_path.display()
    );

    Ok(())
}

fn run_volunteer_validate(options: VolunteerValidateOptions) -> Result<(), Box<dyn Error>> {
    if options.inputs.is_empty() {
        return Err(
            "ministry-transparency volunteer-validate requires at least one --input <path>".into(),
        );
    }

    let mut total_entries = 0usize;
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut entries_with_errors = 0usize;
    let mut entry_summaries: Vec<VolunteerValidationEntrySummary> = Vec::new();

    for path in &options.inputs {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read volunteer brief `{}`: {err}", path.display()))?;
        let value: Value = json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse volunteer brief JSON `{}`: {err}",
                path.display()
            )
        })?;
        let entries = extract_volunteer_entries(&value).map_err(|err| {
            format!(
                "invalid volunteer brief payload `{}`: {err}",
                path.display()
            )
        })?;

        if entries.is_empty() {
            println!("{}: 1 error(s), 0 warning(s)", path.display());
            println!("  error: no volunteer briefs were found in this file");
            total_errors += 1;
            continue;
        }

        for (idx, entry) in entries.iter().enumerate() {
            let base = if entries.len() == 1 {
                "brief".to_string()
            } else {
                format!("brief[{idx}]")
            };
            let report = validate_volunteer_entry(entry, &base);
            total_entries += 1;
            total_errors += report.errors.len();
            total_warnings += report.warnings.len();
            if !report.errors.is_empty() {
                entries_with_errors += 1;
            }
            let suffix = if entries.len() == 1 {
                String::new()
            } else {
                format!("#{idx}", idx = idx + 1)
            };
            println!(
                "{}{}: {} error(s), {} warning(s)",
                path.display(),
                suffix,
                report.errors.len(),
                report.warnings.len()
            );
            for err in &report.errors {
                println!("  error: {err}");
            }
            for warn in &report.warnings {
                println!("  warning: {warn}");
            }
            entry_summaries.push(VolunteerValidationEntrySummary {
                input_path: path.display().to_string(),
                entry_index: idx,
                errors: report.errors.clone(),
                warnings: report.warnings.clone(),
                metadata: report.metadata.clone(),
            });
        }
    }

    if total_entries == 0 && total_errors == 0 {
        return Err("no volunteer briefs were validated; check the provided --input paths".into());
    }

    if let Some(output_path) = &options.json_output {
        let timestamp = OffsetDateTime::now_utc();
        let nanos = timestamp.unix_timestamp_nanos();
        let generated_at_unix_ms = if nanos >= 0 {
            (nanos as u128 / 1_000_000) as u64
        } else {
            0
        };
        let report = VolunteerValidationJsonReport {
            generated_at_unix_ms,
            total_entries,
            entries_with_errors,
            total_errors,
            total_warnings,
            inputs: options
                .inputs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            entries: entry_summaries.clone(),
        };
        write_json_file(output_path, &report)?;
    }

    println!(
        "volunteer validation summary: {total_entries} brief(s), {total_errors} error(s), {total_warnings} warning(s)"
    );

    if total_errors > 0 {
        Err("volunteer validation reported errors".into())
    } else {
        Ok(())
    }
}

fn extract_volunteer_entries(value: &Value) -> Result<Vec<&Value>, String> {
    if let Some(array) = value.as_array() {
        Ok(array.iter().collect())
    } else if value.as_object().is_some() {
        Ok(vec![value])
    } else {
        Err("payload must be a JSON object or an array of objects".into())
    }
}

#[derive(Debug, Default, Clone, JsonSerialize)]
struct VolunteerEntryMetadata {
    brief_id: Option<String>,
    proposal_id: Option<String>,
    stance: Option<String>,
    language: Option<String>,
    submitted_at: Option<String>,
    moderation_off_topic: Option<bool>,
}

#[derive(Default)]
struct VolunteerValidationReport {
    errors: Vec<String>,
    warnings: Vec<String>,
    metadata: VolunteerEntryMetadata,
}

#[derive(Debug, Clone, JsonSerialize)]
struct VolunteerValidationEntrySummary {
    input_path: String,
    entry_index: usize,
    errors: Vec<String>,
    warnings: Vec<String>,
    metadata: VolunteerEntryMetadata,
}

#[derive(Debug, JsonSerialize)]
struct VolunteerValidationJsonReport {
    generated_at_unix_ms: u64,
    total_entries: usize,
    entries_with_errors: usize,
    total_errors: usize,
    total_warnings: usize,
    inputs: Vec<String>,
    entries: Vec<VolunteerValidationEntrySummary>,
}

const STANCE_VALUES: &[&str] = &["support", "oppose", "context"];
const FACT_STATUS_VALUES: &[&str] = &["corroborated", "disputed", "context-only"];
const IMPACT_VALUES: &[&str] = &["governance", "technical", "compliance", "community"];
const DISCLOSURE_TYPES: &[&str] = &["financial", "employment", "governance", "family", "other"];
const MODERATION_TAGS: &[&str] = &[
    "duplicate",
    "needs-translation",
    "needs-follow-up",
    "spam",
    "astroturf",
    "policy-escalation",
];

fn validate_volunteer_entry(entry: &Value, base: &str) -> VolunteerValidationReport {
    let mut report = VolunteerValidationReport::default();

    if let Some(submitted_at) =
        require_nonempty_string_field(entry, "submitted_at", base, &mut report.errors)
    {
        if OffsetDateTime::parse(&submitted_at, &Rfc3339).is_err() {
            report
                .errors
                .push(format!("{}.submitted_at must be RFC3339 formatted", base));
        }
        report.metadata.submitted_at = Some(submitted_at);
    }

    if let Some(stance) = require_nonempty_string_field(entry, "stance", base, &mut report.errors) {
        if !is_allowed(&stance, STANCE_VALUES) {
            report.errors.push(format!(
                "{}.stance must be one of {}",
                base,
                STANCE_VALUES.join("/")
            ));
        }
        report.metadata.stance = Some(stance);
    }

    if let Some(brief_id) =
        require_nonempty_string_field(entry, "brief_id", base, &mut report.errors)
    {
        report.metadata.brief_id = Some(brief_id);
    }
    if let Some(proposal_id) =
        require_nonempty_string_field(entry, "proposal_id", base, &mut report.errors)
    {
        report.metadata.proposal_id = Some(proposal_id);
    }
    if let Some(language) =
        require_nonempty_string_field(entry, "language", base, &mut report.errors)
    {
        report.metadata.language = Some(language);
    }

    if let Some(summary) = require_object_field(entry, "summary", base, &mut report.errors) {
        let summary_base = field_path(base, "summary");
        if require_nonempty_string_field(summary, "abstract", &summary_base, &mut report.errors)
            .is_some_and(|abstract_text| abstract_text.chars().count() > 2_000)
        {
            report.warnings.push(format!(
                "{}.abstract exceeds the 2 000 character guidance",
                summary_base
            ));
        }
        require_nonempty_string_field(summary, "title", &summary_base, &mut report.errors);
        require_nonempty_string_field(
            summary,
            "requested_action",
            &summary_base,
            &mut report.errors,
        );
    }

    let mut author_certified_no_conflicts = false;
    if let Some(author) = require_object_field(entry, "author", base, &mut report.errors) {
        let author_base = field_path(base, "author");
        require_nonempty_string_field(author, "name", &author_base, &mut report.errors);
        require_nonempty_string_field(author, "contact", &author_base, &mut report.errors);
        if let Some(value) = author.get("no_conflicts_certified") {
            match value.as_bool() {
                Some(flag) => author_certified_no_conflicts = flag,
                None => report.errors.push(format!(
                    "{}.no_conflicts_certified must be a boolean",
                    author_base
                )),
            }
        }
    }

    if let Some(facts) = require_array_field(entry, "fact_table", base, &mut report.errors) {
        if facts.is_empty() {
            report.errors.push(format!(
                "{}.fact_table must contain at least one entry",
                base
            ));
        }
        let mut claim_ids = HashSet::new();
        for (idx, fact) in facts.iter().enumerate() {
            let fact_base = format!("{}.fact_table[{idx}]", base);
            if fact.as_object().is_none() {
                report
                    .errors
                    .push(format!("{fact_base} must be a JSON object"));
                continue;
            }
            if let Some(claim_id) =
                require_nonempty_string_field(fact, "claim_id", &fact_base, &mut report.errors)
                    .filter(|claim_id| !claim_ids.insert(claim_id.clone()))
            {
                report.errors.push(format!(
                    "{fact_base}.claim_id duplicates a previous claim ({})",
                    claim_id
                ));
            }
            require_nonempty_string_field(fact, "claim", &fact_base, &mut report.errors);
            if let Some(status) =
                require_nonempty_string_field(fact, "status", &fact_base, &mut report.errors)
                    .filter(|status| !is_allowed(status, FACT_STATUS_VALUES))
            {
                report.errors.push(format!(
                    "{fact_base}.status `{status}` must be one of {}",
                    FACT_STATUS_VALUES.join("/")
                ));
            }
            if let Some(impacts) =
                require_array_field(fact, "impact", &fact_base, &mut report.errors)
            {
                if impacts.is_empty() {
                    report.errors.push(format!(
                        "{fact_base}.impact must include at least one value"
                    ));
                }
                for impact in impacts {
                    match impact.as_str() {
                        Some(value) if is_allowed(value, IMPACT_VALUES) => {}
                        Some(value) => report.errors.push(format!(
                            "{fact_base}.impact contains unsupported value `{value}`"
                        )),
                        None => report
                            .errors
                            .push(format!("{fact_base}.impact entries must be strings")),
                    }
                }
            }
            if let Some(citations) =
                require_array_field(fact, "citations", &fact_base, &mut report.errors)
            {
                if citations.is_empty() {
                    report.warnings.push(format!(
                        "{fact_base}.citations is empty; include at least one citation or explain the omission"
                    ));
                }
                for citation in citations {
                    match citation.as_str() {
                        Some(text) if !text.trim().is_empty() => {}
                        _ => report.errors.push(format!(
                            "{fact_base}.citations entries must be non-empty strings"
                        )),
                    }
                }
            }
            if let Some(digest_value) = fact.get("evidence_digest") {
                match digest_value.as_str() {
                    Some(text) if is_hex_digest(text) => {}
                    Some(_) => report.warnings.push(format!(
                        "{fact_base}.evidence_digest should be a 64-character hex string"
                    )),
                    None => report.errors.push(format!(
                        "{fact_base}.evidence_digest must be a string when provided"
                    )),
                }
            }
        }
    }

    let mut disclosures_present = false;
    if let Some(disclosures_value) = entry.get("disclosures") {
        match disclosures_value.as_array() {
            Some(rows) => {
                if rows.is_empty() {
                    report.errors.push(format!(
                        "{}.disclosures must be omitted or contain at least one entry",
                        base
                    ));
                }
                for (idx, disclosure) in rows.iter().enumerate() {
                    let disclosure_base = format!("{}.disclosures[{idx}]", base);
                    if disclosure.as_object().is_none() {
                        report
                            .errors
                            .push(format!("{disclosure_base} must be a JSON object"));
                        continue;
                    }
                    disclosures_present = true;
                    if let Some(kind) = require_nonempty_string_field(
                        disclosure,
                        "type",
                        &disclosure_base,
                        &mut report.errors,
                    )
                    .filter(|kind| !is_allowed(kind, DISCLOSURE_TYPES))
                    {
                        report.errors.push(format!(
                            "{disclosure_base}.type `{kind}` must be one of {}",
                            DISCLOSURE_TYPES.join("/")
                        ));
                    }
                    require_nonempty_string_field(
                        disclosure,
                        "entity",
                        &disclosure_base,
                        &mut report.errors,
                    );
                    require_nonempty_string_field(
                        disclosure,
                        "relationship",
                        &disclosure_base,
                        &mut report.errors,
                    );
                    require_nonempty_string_field(
                        disclosure,
                        "details",
                        &disclosure_base,
                        &mut report.errors,
                    );
                }
            }
            None => report.errors.push(format!(
                "{}.disclosures must be an array when provided",
                base
            )),
        }
    }

    if !disclosures_present && !author_certified_no_conflicts {
        report.errors.push(format!(
            "{} must include a disclosures array or set author.no_conflicts_certified=true",
            base
        ));
    }

    if let Some(moderation) = optional_object_field(entry, "moderation", base, &mut report.errors) {
        let moderation_base = field_path(base, "moderation");
        if let Some(value) = moderation.get("off_topic") {
            match value.as_bool() {
                Some(flag) => {
                    report.metadata.moderation_off_topic = Some(flag);
                }
                None => {
                    report
                        .errors
                        .push(format!("{moderation_base}.off_topic must be a boolean"));
                }
            }
        }
        if let Some(tags_value) = moderation.get("tags") {
            match tags_value.as_array() {
                Some(tags) => {
                    for tag in tags {
                        match tag.as_str() {
                            Some(text) if is_allowed(text, MODERATION_TAGS) => {}
                            Some(text) => report.errors.push(format!(
                                "{moderation_base}.tags contains unsupported value `{text}`"
                            )),
                            None => report
                                .errors
                                .push(format!("{moderation_base}.tags entries must be strings")),
                        }
                    }
                }
                None => report
                    .errors
                    .push(format!("{moderation_base}.tags must be an array")),
            }
        }
        if let Some(notes_value) = moderation.get("notes") {
            match notes_value.as_str() {
                Some(text) if text.len() > 512 => report
                    .warnings
                    .push(format!("{moderation_base}.notes exceeds 512 characters")),
                Some(_) => {}
                None => report
                    .errors
                    .push(format!("{moderation_base}.notes must be a string")),
            }
        }
    }

    report
}

fn require_nonempty_string_field(
    value: &Value,
    field: &str,
    base: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    let path = field_path(base, field);
    match value.get(field) {
        Some(entry) => match entry.as_str().map(str::trim) {
            Some(text) if !text.is_empty() => Some(text.to_owned()),
            _ => {
                errors.push(format!("{path} must be a non-empty string"));
                None
            }
        },
        None => {
            errors.push(format!("{path} is required"));
            None
        }
    }
}

fn require_object_field<'a>(
    value: &'a Value,
    field: &str,
    base: &str,
    errors: &mut Vec<String>,
) -> Option<&'a Value> {
    let path = field_path(base, field);
    match value.get(field) {
        Some(entry) if entry.as_object().is_some() => Some(entry),
        Some(_) => {
            errors.push(format!("{path} must be a JSON object"));
            None
        }
        None => {
            errors.push(format!("{path} is required"));
            None
        }
    }
}

fn optional_object_field<'a>(
    value: &'a Value,
    field: &str,
    base: &str,
    errors: &mut Vec<String>,
) -> Option<&'a Value> {
    let path = field_path(base, field);
    match value.get(field) {
        Some(entry) if entry.as_object().is_some() => Some(entry),
        Some(_) => {
            errors.push(format!("{path} must be a JSON object"));
            None
        }
        None => None,
    }
}

fn require_array_field<'a>(
    value: &'a Value,
    field: &str,
    base: &str,
    errors: &mut Vec<String>,
) -> Option<&'a [Value]> {
    let path = field_path(base, field);
    match value.get(field) {
        Some(entry) => match entry.as_array() {
            Some(array) => Some(array),
            None => {
                errors.push(format!("{path} must be an array"));
                None
            }
        },
        None => {
            errors.push(format!("{path} is required"));
            None
        }
    }
}

fn field_path(base: &str, field: &str) -> String {
    if base.is_empty() {
        field.to_string()
    } else {
        format!("{base}.{field}")
    }
}

fn is_allowed(value: &str, allowed: &[&str]) -> bool {
    allowed
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[derive(Debug, Deserialize)]
struct TransparencyReleaseActionPayload {
    action: String,
    version: u64,
    quarter: String,
    generated_at: String,
    manifest_digest_blake2b_256: String,
    sorafs_cid_hex: String,
    dashboards_git_sha: Option<String>,
    note: Option<String>,
}

fn decode_hex_vec(input: &str, label: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let trimmed = input.trim();
    let normalised = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes = hex::decode(normalised).map_err(|err| format!("invalid {label}: {err}"))?;
    Ok(bytes)
}

fn decode_fixed_hex<const N: usize>(input: &str, label: &str) -> Result<[u8; N], Box<dyn Error>> {
    let bytes = decode_hex_vec(input, label)?;
    if bytes.len() != N {
        return Err(format!("{label} must be {N} bytes, got {}", bytes.len()).into());
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn sanitize_volunteer_summary(
    summary: &VolunteerSummary,
    options: &SanitizeOptions,
    rng: &mut ChaCha20Rng,
    records: &mut Vec<CountNoiseRecord>,
    suppressed: &mut BTreeSet<String>,
) -> SanitizedVolunteer {
    let total = sanitize_count(
        "volunteer.total",
        summary.total_briefs,
        options.epsilon_counts,
        options.suppress_threshold,
        rng,
        records,
        suppressed,
    );
    let mut languages = BTreeMap::new();
    for (language, count) in &summary.languages {
        let bucket = format!("volunteer.language.{language}");
        let sanitized = sanitize_count(
            &bucket,
            *count,
            options.epsilon_counts,
            options.suppress_threshold,
            rng,
            records,
            suppressed,
        );
        if sanitized > 0 {
            languages.insert(language.clone(), sanitized);
        }
    }
    let fact_rows = sanitize_count(
        "volunteer.fact_rows",
        summary.fact_rows,
        options.epsilon_counts,
        options.suppress_threshold,
        rng,
        records,
        suppressed,
    );
    let fact_rows_with_citation = sanitize_count(
        "volunteer.fact_rows_with_citation",
        summary.fact_rows_with_citation,
        options.epsilon_counts,
        options.suppress_threshold,
        rng,
        records,
        suppressed,
    );
    let disclosures_missing = sanitize_count(
        "volunteer.disclosures_missing",
        summary.disclosures_missing,
        options.epsilon_counts,
        options.suppress_threshold,
        rng,
        records,
        suppressed,
    );
    let off_topic_rejections = sanitize_count(
        "volunteer.off_topic_rejections",
        summary.off_topic_rejections,
        options.epsilon_counts,
        options.suppress_threshold,
        rng,
        records,
        suppressed,
    );
    SanitizedVolunteer {
        total_briefs: total,
        languages,
        fact_rows,
        fact_rows_with_citation,
        disclosures_missing,
        off_topic_rejections,
    }
}

fn sanitize_ai_policies(
    metrics: &BTreeMap<String, AiMetricSummary>,
    options: &SanitizeOptions,
    rng: &mut ChaCha20Rng,
    suppressed: &mut BTreeSet<String>,
) -> (BTreeMap<String, SanitizedPolicy>, Vec<AccuracyNoiseRecord>) {
    let mut map = BTreeMap::new();
    let mut records = Vec::new();
    for (policy, summary) in metrics {
        if summary.total_samples < options.min_accuracy_samples {
            suppressed.insert(format!("ai_policy:{policy}"));
            records.push(AccuracyNoiseRecord {
                policy: policy.clone(),
                raw_samples: summary.total_samples,
                raw_false_positive: summary.false_positives,
                raw_false_negative: summary.false_negatives,
                noise_samples: 0.0,
                noise_false_positive: 0.0,
                noise_false_negative: 0.0,
                sanitized_samples: 0.0,
                sanitized_false_positive: 0.0,
                sanitized_false_negative: 0.0,
                suppressed: true,
            });
            continue;
        }

        let noise_samples = gaussian_noise(options.epsilon_accuracy, options.delta, rng);
        let noisy_samples =
            (summary.total_samples as f64 + noise_samples).clamp(0.0, u64::MAX as f64);
        let sanitized_samples = noisy_samples.round().max(1.0);
        let noise_fp = gaussian_noise(options.epsilon_accuracy, options.delta, rng);
        let noise_fn = gaussian_noise(options.epsilon_accuracy, options.delta, rng);

        let mut sanitized_fp =
            (summary.false_positives as f64 + noise_fp).clamp(0.0, sanitized_samples);
        let mut sanitized_fn =
            (summary.false_negatives as f64 + noise_fn).clamp(0.0, sanitized_samples);

        if sanitized_fp + sanitized_fn > sanitized_samples {
            let scale = sanitized_samples / (sanitized_fp + sanitized_fn);
            sanitized_fp *= scale;
            sanitized_fn *= scale;
        }

        let fp_rate = (sanitized_fp / sanitized_samples).clamp(0.0, 1.0);
        let fn_rate = (sanitized_fn / sanitized_samples).clamp(0.0, 1.0);
        let accuracy = (1.0 - fp_rate - fn_rate).clamp(0.0, 1.0);

        map.insert(
            policy.clone(),
            SanitizedPolicy {
                total_samples: sanitized_samples as u64,
                false_positive_rate: fp_rate,
                false_negative_rate: fn_rate,
                accuracy,
                suppressed: false,
            },
        );

        records.push(AccuracyNoiseRecord {
            policy: policy.clone(),
            raw_samples: summary.total_samples,
            raw_false_positive: summary.false_positives,
            raw_false_negative: summary.false_negatives,
            noise_samples,
            noise_false_positive: noise_fp,
            noise_false_negative: noise_fn,
            sanitized_samples,
            sanitized_false_positive: sanitized_fp,
            sanitized_false_negative: sanitized_fn,
            suppressed: false,
        });
    }
    (map, records)
}

fn sanitize_count(
    bucket: &str,
    raw: u64,
    epsilon: f64,
    suppress_threshold: u64,
    rng: &mut ChaCha20Rng,
    records: &mut Vec<CountNoiseRecord>,
    suppressed: &mut BTreeSet<String>,
) -> u64 {
    let noise = laplace_noise(epsilon, rng);
    let noisy = (raw as f64 + noise).clamp(0.0, u64::MAX as f64);
    let rounded = noisy.round();
    let mut sanitized = rounded as u64;
    let is_suppressed = sanitized < suppress_threshold;
    if is_suppressed {
        suppressed.insert(bucket.to_string());
        sanitized = 0;
    }
    records.push(CountNoiseRecord {
        bucket: bucket.to_string(),
        raw: raw as f64,
        noise,
        sanitized: sanitized as f64,
        suppressed: is_suppressed,
    });
    sanitized
}

fn laplace_noise(epsilon: f64, rng: &mut ChaCha20Rng) -> f64 {
    if epsilon <= 0.0 {
        return 0.0;
    }
    let beta = (1.0 / epsilon).max(f64::EPSILON);
    let u: f64 = rng.random_range(-0.5..0.5);
    let sign = if u >= 0.0 { 1.0 } else { -1.0 };
    let inner = (1.0 - 2.0 * u.abs()).max(f64::MIN_POSITIVE);
    -beta * sign * inner.ln()
}

fn gaussian_noise(epsilon: f64, delta: f64, rng: &mut ChaCha20Rng) -> f64 {
    if epsilon <= 0.0 || delta <= 0.0 {
        return 0.0;
    }
    let sigma = (2.0 * (1.25 / delta).ln()).sqrt() / epsilon;
    let dist = Normal::new(0.0, sigma.max(f64::EPSILON)).unwrap();
    dist.sample(rng)
}

fn load_array(path: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let raw = fs::read(path)?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = json::from_slice(&raw)?;
    let array = value
        .as_array()
        .ok_or_else(|| format!("expected JSON array in {}", path.display()))?;
    Ok(array.clone())
}

fn summarize_ai_metrics(entries: &[Value]) -> BTreeMap<String, AiMetricSummary> {
    let mut map: BTreeMap<String, AiMetricSummary> = BTreeMap::new();
    for entry in entries {
        let Some(policy) = entry.get("policy").and_then(Value::as_str) else {
            continue;
        };
        let samples = entry.get("samples").and_then(Value::as_u64).unwrap_or(0);
        let false_pos = entry
            .get("false_positive")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let false_neg = entry
            .get("false_negative")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let stats = map.entry(policy.to_string()).or_default();
        stats.total_samples = stats.total_samples.saturating_add(samples);
        stats.false_positives = stats.false_positives.saturating_add(false_pos);
        stats.false_negatives = stats.false_negatives.saturating_add(false_neg);
    }
    map
}

fn summarize_appeals(entries: &[Value]) -> AppealsSummary {
    if entries.is_empty() {
        return AppealsSummary::default();
    }
    let mut summary = AppealsSummary {
        total: entries.len() as u64,
        ..AppealsSummary::default()
    };
    let mut total_resolution_hours = 0.0f64;
    for entry in entries {
        if matches!(
            entry.get("status").and_then(Value::as_str),
            Some("approved") | Some("rejected")
        ) {
            summary.resolved += 1;
        }
        if entry
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status.eq_ignore_ascii_case("reopened"))
        {
            summary.reopened += 1;
        }
        if let Some(hours) = extract_resolution_hours(entry) {
            total_resolution_hours += hours;
            if hours > APPEAL_SLA_HOURS {
                summary.sla_breaches += 1;
            }
        }
    }
    if summary.resolved > 0 {
        summary.avg_resolution_hours = total_resolution_hours / summary.resolved as f64;
    }
    summary
}

fn summarize_denylist(entries: &[Value]) -> DenylistSummary {
    let mut summary = DenylistSummary::default();
    for entry in entries {
        match entry.get("action").and_then(Value::as_str) {
            Some(action) if action.eq_ignore_ascii_case("add") => summary.additions += 1,
            Some(action) if action.eq_ignore_ascii_case("remove") => summary.removals += 1,
            _ => {}
        }
        if entry
            .get("emergency")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            summary.emergency_actions += 1;
        }
    }
    summary
}

fn summarize_treasury(entries: &[Value]) -> TreasurySummary {
    let mut summary = TreasurySummary::default();
    for entry in entries {
        let Some(kind) = entry.get("kind").and_then(Value::as_str) else {
            continue;
        };
        let Some(amount) = extract_amount(entry.get("amount")) else {
            continue;
        };
        if kind.eq_ignore_ascii_case("deposit") {
            summary.total_deposits_xor = summary.total_deposits_xor.saturating_add(amount);
        } else if kind.eq_ignore_ascii_case("payout") {
            summary.total_payouts_xor = summary.total_payouts_xor.saturating_add(amount);
        }
    }
    summary
}

fn summarize_volunteer(entries: &[Value]) -> VolunteerSummary {
    let mut summary = VolunteerSummary::default();
    for entry in entries {
        if is_off_topic(entry) {
            summary.off_topic_rejections = summary.off_topic_rejections.saturating_add(1);
            continue;
        }
        summary.total_briefs += 1;
        if let Some(language) = entry.get("language").and_then(Value::as_str) {
            *summary
                .languages
                .entry(language.to_lowercase())
                .or_insert(0) += 1;
        }
        if !has_conflict_disclosure(entry) {
            summary.disclosures_missing = summary.disclosures_missing.saturating_add(1);
        }
        if let Some(facts) = entry.get("fact_table").and_then(Value::as_array) {
            for fact in facts {
                summary.fact_rows = summary.fact_rows.saturating_add(1);
                if fact_has_citation(fact) {
                    summary.fact_rows_with_citation =
                        summary.fact_rows_with_citation.saturating_add(1);
                }
            }
        }
    }
    summary
}

fn is_off_topic(entry: &Value) -> bool {
    entry
        .get("moderation")
        .and_then(|m| m.get("off_topic"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn has_conflict_disclosure(entry: &Value) -> bool {
    if entry
        .get("author")
        .and_then(|author| author.get("no_conflicts_certified"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    entry
        .get("disclosures")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter().any(|row| {
                has_nonempty_text(row.get("type"))
                    || has_nonempty_text(row.get("entity"))
                    || has_nonempty_text(row.get("details"))
            })
        })
        .unwrap_or(false)
}

fn fact_has_citation(fact: &Value) -> bool {
    fact.get("citations")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().any(|row| has_nonempty_text(Some(row))))
        .unwrap_or(false)
}

fn has_nonempty_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

fn build_ai_accuracy(
    summaries: &BTreeMap<String, AiMetricSummary>,
) -> BTreeMap<String, AiAccuracyRow> {
    let mut rows = BTreeMap::new();
    for (policy, summary) in summaries {
        let total = summary.total_samples.max(1);
        let fp_rate = summary.false_positives as f64 / total as f64;
        let fn_rate = summary.false_negatives as f64 / total as f64;
        rows.insert(
            policy.clone(),
            AiAccuracyRow {
                total_samples: summary.total_samples,
                false_positive_rate: fp_rate,
                false_negative_rate: fn_rate,
                accuracy: (1.0 - fp_rate - fn_rate).clamp(0.0, 1.0),
            },
        );
    }
    rows
}

fn build_appeal_dashboard(summary: &AppealsSummary) -> AppealDashboard {
    AppealDashboard {
        total: summary.total,
        resolved: summary.resolved,
        avg_resolution_hours: summary.avg_resolution_hours,
        sla_breach_rate: if summary.resolved == 0 {
            0.0
        } else {
            summary.sla_breaches as f64 / summary.resolved as f64
        },
    }
}

fn build_denylist_dashboard(summary: &DenylistSummary) -> DenylistDashboard {
    DenylistDashboard {
        additions: summary.additions,
        removals: summary.removals,
        emergency_actions: summary.emergency_actions,
        net_delta: summary.additions as i64 - summary.removals as i64,
    }
}

fn build_treasury_dashboard(summary: &TreasurySummary) -> TreasuryDashboard {
    let net = summary.total_deposits_xor - summary.total_payouts_xor;
    TreasuryDashboard {
        total_deposits_xor: summary.total_deposits_xor.to_string(),
        total_payouts_xor: summary.total_payouts_xor.to_string(),
        net_flow_xor: net.to_string(),
    }
}

fn extract_resolution_hours(entry: &Value) -> Option<f64> {
    if let Some(explicit) = entry.get("resolution_hours").and_then(Value::as_f64) {
        return Some(explicit);
    }
    let submitted_ms = entry.get("submitted_ms").and_then(Value::as_u64)?;
    let resolved_ms = entry.get("resolved_ms").and_then(Value::as_u64)?;
    resolved_ms
        .checked_sub(submitted_ms)
        .map(|ms| ms as f64 / 3_600_000.0)
}

fn extract_amount(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(val) = value.as_i64() {
        return Some(val);
    }
    if let Some(val) = value.as_u64() {
        return Some(val.min(i64::MAX as u64) as i64);
    }
    if let Some(text) = value.as_str() {
        let cleaned = text.replace('_', "");
        if let Ok(parsed) = cleaned.parse::<i128>() {
            let clamped = parsed.clamp(i64::MIN as i128, i64::MAX as i128);
            return Some(clamped as i64);
        }
    }
    None
}

fn collect_checksums(
    entries: &[(&str, &PathBuf)],
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut map = BTreeMap::new();
    for (label, path) in entries {
        let sum = file_checksum(path)?;
        map.insert(label.to_string(), sum);
    }
    Ok(map)
}

fn file_checksum(path: &Path) -> Result<String, Box<dyn Error>> {
    let data = fs::read(path)?;
    let mut hasher = Hasher::new();
    hasher.update(&data);
    Ok(hasher.finalize().to_hex().to_string())
}

fn write_json_file<T: ?Sized + JsonSerialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_value = json::to_value(value)?;
    let mut text = json::to_string_pretty(&json_value)?;
    text.push('\n');
    fs::write(path, text)?;
    Ok(())
}

fn now_rfc3339() -> String {
    let now = OffsetDateTime::now_utc();
    now.format(&Rfc3339)
        .unwrap_or_else(|_| now.unix_timestamp().to_string())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, PublicKey, Signature, SignatureOf};
    use iroha_data_model::{
        ministry::{
            AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
            AgendaProposalAction, AgendaProposalSubmitter, AgendaProposalSummary,
            AgendaProposalTarget, AgendaProposalV1,
        },
        sorafs::{
            moderation::{
                ModerationModelFingerprintV1, ModerationReproBodyV1, ModerationReproManifestV1,
                ModerationReproSignatureV1, ModerationSeedMaterialV1, ModerationThresholdsV1,
            },
            prelude::MODERATION_REPRO_MANIFEST_VERSION_V1,
        },
    };
    use norito::decode_from_bytes;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    #[test]
    fn volunteer_validator_accepts_template() {
        let raw = include_str!("../../docs/examples/ministry/volunteer_brief_template.json");
        let value: Value = norito::json::from_str(raw).expect("parse volunteer template");
        let report = super::validate_volunteer_entry(&value, "brief");
        assert!(
            report.errors.is_empty(),
            "expected no errors, got {:?}",
            report.errors
        );
    }

    #[test]
    fn volunteer_validator_flags_missing_fields() {
        let bad = norito::json!({
            "brief_id": "",
            "proposal_id": "BLACKLIST-1",
            "language": "en",
            "stance": "invalid",
            "submitted_at": "nope",
            "author": {
                "name": "",
                "contact": "",
                "no_conflicts_certified": false
            },
            "summary": {
                "title": "",
                "abstract": "",
                "requested_action": ""
            },
            "fact_table": [
                {
                    "claim_id": "VB-1",
                    "claim": "",
                    "status": "bad",
                    "impact": [],
                    "citations": ""
                }
            ],
            "moderation": {
                "off_topic": "true",
                "tags": ["unknown-tag"],
                "notes": 10
            }
        });

        let report = super::validate_volunteer_entry(&bad, "brief");
        assert!(
            report
                .errors
                .iter()
                .any(|err| err.contains("stance must be one of")),
            "expected stance error, got {:?}",
            report.errors
        );
        assert!(
            report
                .errors
                .iter()
                .any(|err| err.contains("fact_table[0].impact")),
            "expected impact error, got {:?}",
            report.errors
        );
        assert!(
            report
                .errors
                .iter()
                .any(|err| err.contains("disclosures array or set author.no_conflicts")),
            "expected disclosure error, got {:?}",
            report.errors
        );
    }

    #[test]
    fn volunteer_validator_emits_json_report() {
        let tmp = TempDir::new().expect("tempdir");
        let input_path = tmp.path().join("briefs.json");
        let json_output = tmp.path().join("report.json");
        let payload = norito::json!([
            {
                "submitted_at": "2026-02-01T10:00:00Z",
                "stance": "support",
                "brief_id": "VB-OK",
                "proposal_id": "BLACKLIST-1",
                "language": "en",
                "summary": {
                    "title": "Summary",
                    "abstract": "All good",
                    "requested_action": "Adopt"
                },
                "author": {
                    "name": "Citizen",
                    "contact": "citizen@sora.org",
                    "no_conflicts_certified": true
                },
                "fact_table": [
                    {
                        "claim_id": "VB-OK-F1",
                        "claim": "Restitution paid.",
                        "status": "corroborated",
                        "impact": ["governance"],
                        "citations": ["torii://case/123"]
                    }
                ],
                "disclosures": [
                    {
                        "type": "other",
                        "entity": "None",
                        "relationship": "None",
                        "details": "None"
                    }
                ],
                "moderation": {
                    "off_topic": false,
                    "tags": []
                }
            },
            {
                "submitted_at": "2026-02-02T11:00:00Z",
                "stance": "oppose",
                "brief_id": "VB-BAD",
                "proposal_id": "BLACKLIST-2",
                "language": "ja",
                "summary": {
                    "title": "Needs work",
                    "abstract": "Missing facts",
                    "requested_action": "Reject"
                },
                "author": {
                    "name": "Reviewer",
                    "contact": "reviewer@sora.org",
                    "no_conflicts_certified": true
                },
                "fact_table": [],
                "disclosures": [],
                "moderation": {
                    "off_topic": true,
                    "tags": ["needs-follow-up"]
                }
            }
        ]);
        write_json(&input_path, &payload);

        let result = super::run_volunteer_validate(super::VolunteerValidateOptions {
            inputs: vec![input_path.clone()],
            json_output: Some(json_output.clone()),
        });
        assert!(result.is_err(), "expected errors due to invalid brief");

        let report_bytes = fs::read(&json_output).expect("report exists");
        let report_value: Value = json::from_slice(&report_bytes).expect("json report");
        assert_eq!(
            report_value.get("total_entries").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            report_value
                .get("entries_with_errors")
                .and_then(Value::as_u64),
            Some(1)
        );
        let entries = report_value
            .get("entries")
            .and_then(Value::as_array)
            .expect("entries array");
        assert_eq!(entries.len(), 2);
        let ok = entries
            .iter()
            .find(|entry| {
                entry
                    .get("metadata")
                    .and_then(|m| m.get("brief_id"))
                    .and_then(Value::as_str)
                    == Some("VB-OK")
            })
            .expect("ok entry");
        assert_eq!(
            ok.get("metadata")
                .and_then(|m| m.get("proposal_id"))
                .and_then(Value::as_str),
            Some("BLACKLIST-1")
        );
        assert_eq!(
            ok.get("warnings")
                .and_then(Value::as_array)
                .map(|w| w.len()),
            Some(0)
        );
        let bad = entries
            .iter()
            .find(|entry| {
                entry
                    .get("metadata")
                    .and_then(|m| m.get("brief_id"))
                    .and_then(Value::as_str)
                    == Some("VB-BAD")
            })
            .expect("bad entry");
        assert_eq!(
            bad.get("metadata")
                .and_then(|m| m.get("language"))
                .and_then(Value::as_str),
            Some("ja")
        );
        assert_eq!(
            bad.get("metadata")
                .and_then(|m| m.get("moderation_off_topic"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            bad.get("errors")
                .and_then(Value::as_array)
                .map(|errs| !errs.is_empty())
                .unwrap_or(false),
            "expected error entries for invalid brief"
        );
    }

    #[test]
    fn volunteer_summary_enforces_template_requirements() {
        let entries = norito::json!([
            {
                "language": "en",
                "fact_table": [
                    {
                        "claim": "Restitution completed.",
                        "citations": ["torii://case/TR-100"]
                    },
                    {
                        "claim": "Cooling window open.",
                        "citations": []
                    }
                ],
                "disclosures": [
                    {"type": "employment", "entity": "Sora Commons", "details": "Contractor"}
                ],
                "moderation": {"off_topic": false}
            },
            {
                "language": "ja",
                "fact_table": [
                    {
                        "claim": "Citizen hotline logged zero calls.",
                        "citations": ["https://example.org/hotline"]
                    }
                ],
                "author": {"no_conflicts_certified": true}
            },
            {
                "language": "fr",
                "moderation": {"off_topic": true},
                "fact_table": [
                    {
                        "claim": "Off-topic entry should be ignored.",
                        "citations": ["https://spam.invalid"]
                    }
                ]
            },
            {
                "language": "en",
                "fact_table": [
                    {
                        "claim": "One more fact with citation.",
                        "citations": ["cid://abc123"]
                    }
                ]
            }
        ]);

        let summary = super::summarize_volunteer(entries.as_array().expect("array"));
        assert_eq!(summary.total_briefs, 3);
        assert_eq!(summary.off_topic_rejections, 1);
        assert_eq!(summary.fact_rows, 4);
        assert_eq!(summary.fact_rows_with_citation, 3);
        assert_eq!(summary.disclosures_missing, 1);
        assert_eq!(summary.languages.get("en"), Some(&2));
        assert_eq!(summary.languages.get("ja"), Some(&1));
        assert!(!summary.languages.contains_key("fr"));
    }

    #[test]
    fn parse_red_team_report_extracts_metadata() {
        let report = NamedTempFile::new().expect("temp report");
        let body = r#"
- **Drill ID:** 20261012-operation-test
- **Date & window:** 2026-10-12 10:00Z – 12:00Z
- **Scenario class:** smuggling
- **Operators:** Alice Example, Bob Beta
- **Dashboards frozen from commit:** deadbeefcafebabe1234
- **Evidence bundle:** artifacts/ministry/red-team/2026-10/operation-test/
- **SoraFS CID (optional):** bafybeicid123
"#;
        std::fs::write(report.path(), body).expect("write report");

        let scenario = super::parse_red_team_report(report.path()).expect("parse report");
        assert_eq!(scenario.drill_id, "20261012-operation-test");
        assert_eq!(
            scenario.date_window.as_deref(),
            Some("2026-10-12 10:00Z – 12:00Z")
        );
        assert_eq!(scenario.scenario_class.as_deref(), Some("smuggling"));
        assert_eq!(scenario.operators, ["Alice Example", "Bob Beta"]);
        assert_eq!(
            scenario.dashboards_sha.as_deref(),
            Some("deadbeefcafebabe1234")
        );
        assert_eq!(
            scenario.evidence_path.as_deref(),
            Some("artifacts/ministry/red-team/2026-10/operation-test/")
        );
        assert_eq!(scenario.sorafs_cid.as_deref(), Some("bafybeicid123"));
    }

    #[test]
    fn summarizes_and_builds_outputs() {
        let tmp = TempDir::new().expect("temp dir");
        let ledger_path = tmp.path().join("ledger.json");
        let appeals_path = tmp.path().join("appeals.json");
        let denylist_path = tmp.path().join("denylist.json");
        let treasury_path = tmp.path().join("treasury.json");
        let volunteer_path = tmp.path().join("volunteer.json");
        let ingest_path = tmp.path().join("ingest.json");
        let metrics_path = tmp.path().join("metrics.json");
        let manifest_path = tmp.path().join("manifest.json");

        write_json(
            &ledger_path,
            &norito::json!([
                {"policy": "hashes", "samples": 120, "false_positive": 2, "false_negative": 1},
                {"policy": "hashes", "samples": 80, "false_positive": 1, "false_negative": 0},
                {"policy": "evidence", "samples": 50, "false_positive": 0, "false_negative": 2}
            ]),
        );
        write_json(
            &appeals_path,
            &norito::json!([
                {"status": "approved", "submitted_ms": 0, "resolved_ms": 36_000_000},
                {"status": "reopened", "submitted_ms": 0, "resolved_ms": 200_000_000},
                {"status": "rejected"}
            ]),
        );
        write_json(
            &denylist_path,
            &norito::json!([
                {"action": "add"},
                {"action": "remove"},
                {"action": "add", "emergency": true}
            ]),
        );
        write_json(
            &treasury_path,
            &norito::json!([
                {"kind": "deposit", "amount": 1000},
                {"kind": "payout", "amount": 250},
                {"kind": "deposit", "amount": 500}
            ]),
        );
        write_json(
            &volunteer_path,
            &norito::json!([
                {"language": "en"},
                {"language": "ja"}
            ]),
        );

        run(Command::Ingest(Box::new(IngestOptions {
            quarter: "2026-Q3".to_string(),
            ledger_path: ledger_path.clone(),
            appeals_path: appeals_path.clone(),
            denylist_path: denylist_path.clone(),
            treasury_path: treasury_path.clone(),
            volunteer_path: Some(volunteer_path.clone()),
            red_team_reports: Vec::new(),
            panel_summary: None,
            output_path: ingest_path.clone(),
        })))
        .expect("ingest runs");

        assert!(ingest_path.exists());

        run(Command::Build(Box::new(BuildOptions {
            ingest_path: ingest_path.clone(),
            metrics_output: metrics_path.clone(),
            manifest_output: manifest_path.clone(),
            note: Some("test-release".to_string()),
        })))
        .expect("build runs");

        let dashboard: Value =
            json::from_slice(&fs::read(&metrics_path).expect("metrics read")).expect("json");
        assert_eq!(
            dashboard.get("quarter").and_then(Value::as_str),
            Some("2026-Q3")
        );
        assert!(manifest_path.exists());
    }

    #[test]
    fn ingest_generates_review_panel_summary_when_requested() {
        let tmp = TempDir::new().expect("tempdir");
        let ingest_path = tmp.path().join("ingest.json");
        let panel_summary_path = tmp.path().join("panel_summary.json");
        let ledger_path = tmp.path().join("ledger.json");
        let appeals_path = tmp.path().join("appeals.json");
        let denylist_path = tmp.path().join("denylist.json");
        let treasury_path = tmp.path().join("treasury.json");
        let volunteer_path = tmp.path().join("volunteer.json");
        let proposal_path = tmp.path().join("proposal.json");
        let manifest_path = tmp.path().join("manifest.json");

        write_json(
            &ledger_path,
            &norito::json!([
                {"policy": "hashes", "samples": 10, "false_positive": 1, "false_negative": 0}
            ]),
        );
        write_json(
            &appeals_path,
            &norito::json!([{"status": "approved", "submitted_ms": 0, "resolved_ms": 1000}]),
        );
        write_json(&denylist_path, &norito::json!([{"action": "add"}]));
        write_json(
            &treasury_path,
            &norito::json!([{"kind": "deposit", "amount": 100}]),
        );

        let volunteer_payload = norito::json!([
            {
                "brief_id": "VB-support-01",
                "proposal_id": "AC-2026-050",
                "language": "en",
                "stance": "support",
                "summary": {"title": "Support entry"},
                "fact_table": [
                    {
                        "claim_id": "VB-S1",
                        "claim": "Support fact",
                        "status": "corroborated",
                        "impact": ["governance"],
                        "citations": ["https://evidence.example.org/support"]
                    }
                ]
            },
            {
                "brief_id": "VB-oppose-01",
                "proposal_id": "AC-2026-050",
                "language": "en",
                "stance": "oppose",
                "summary": {"title": "Oppose entry"},
                "fact_table": [
                    {
                        "claim_id": "VB-O1",
                        "claim": "Oppose fact",
                        "status": "disputed",
                        "impact": ["community"],
                        "citations": ["https://evidence.example.org/oppose"]
                    }
                ]
            }
        ]);
        write_json(&volunteer_path, &volunteer_payload);

        write_json_file(&proposal_path, &sample_proposal()).expect("write proposal");
        write_json_file(&manifest_path, &sample_ai_manifest()).expect("write manifest");

        run(Command::Ingest(Box::new(IngestOptions {
            quarter: "2026-Q4".to_string(),
            ledger_path: ledger_path.clone(),
            appeals_path: appeals_path.clone(),
            denylist_path: denylist_path.clone(),
            treasury_path: treasury_path.clone(),
            volunteer_path: Some(volunteer_path.clone()),
            red_team_reports: Vec::new(),
            panel_summary: Some(PanelSummaryRequest {
                proposal_path: proposal_path.clone(),
                volunteer_path: volunteer_path.clone(),
                ai_manifest_path: manifest_path.clone(),
                panel_round_id: "RP-2026-07".into(),
                output_path: panel_summary_path.clone(),
                language_override: Some("en".into()),
                generated_at_unix_ms: Some(1_780_034_567_890),
            }),
            output_path: ingest_path.clone(),
        })))
        .expect("ingest runs");

        let snapshot_bytes = fs::read(&ingest_path).expect("ingest read");
        let snapshot: QuarterIngestSnapshot = json::from_slice(&snapshot_bytes).expect("snapshot");
        let summary = snapshot
            .review_panel_summary
            .expect("review panel summary present");
        assert_eq!(summary.panel_round_id, "RP-2026-07");
        assert_eq!(summary.highlights.len(), 2);

        let summary_file: ReviewPanelSummaryV1 =
            json::from_slice(&fs::read(&panel_summary_path).expect("panel summary read"))
                .expect("panel summary decode");
        assert_eq!(summary_file.overview.title, summary.overview.title);
    }

    #[test]
    fn sanitizes_metrics_with_deterministic_seed() {
        let tmp = TempDir::new().expect("temp dir");
        let ledger_path = tmp.path().join("ledger.json");
        let appeals_path = tmp.path().join("appeals.json");
        let denylist_path = tmp.path().join("denylist.json");
        let treasury_path = tmp.path().join("treasury.json");
        let ingest_path = tmp.path().join("ingest.json");
        let sanitized_path = tmp.path().join("sanitized.json");
        let report_path = tmp.path().join("dp_report.json");

        write_json(
            &ledger_path,
            &norito::json!([
                {"policy": "hashes", "samples": 200, "false_positive": 4, "false_negative": 3},
                {"policy": "evidence", "samples": 120, "false_positive": 2, "false_negative": 1}
            ]),
        );
        write_json(
            &appeals_path,
            &norito::json!([
                {"status": "approved", "submitted_ms": 0, "resolved_ms": 36_000_000},
                {"status": "reopened", "submitted_ms": 0, "resolved_ms": 200_000_000}
            ]),
        );
        write_json(
            &denylist_path,
            &norito::json!([
                {"action": "add"},
                {"action": "remove"},
                {"action": "add"}
            ]),
        );
        write_json(
            &treasury_path,
            &norito::json!([
                {"kind": "deposit", "amount": 1000},
                {"kind": "payout", "amount": 200}
            ]),
        );

        run(Command::Ingest(Box::new(IngestOptions {
            quarter: "2026-Q3".to_string(),
            ledger_path: ledger_path.clone(),
            appeals_path: appeals_path.clone(),
            denylist_path: denylist_path.clone(),
            treasury_path: treasury_path.clone(),
            volunteer_path: None,
            red_team_reports: Vec::new(),
            panel_summary: None,
            output_path: ingest_path.clone(),
        })))
        .expect("ingest runs");

        run(Command::Sanitize(Box::new(SanitizeOptions {
            ingest_path: ingest_path.clone(),
            output_path: sanitized_path.clone(),
            report_path: report_path.clone(),
            epsilon_counts: 0.75,
            epsilon_accuracy: 0.5,
            delta: 1e-6,
            suppress_threshold: 5,
            min_accuracy_samples: 50,
            seed: Some(7),
        })))
        .expect("sanitize runs");

        let sanitized: Value =
            json::from_slice(&fs::read(&sanitized_path).expect("sanitized read")).expect("json");
        assert_eq!(
            sanitized.get("quarter").and_then(Value::as_str),
            Some("2026-Q3")
        );
        assert!(sanitized.get("metadata").is_some());

        let report: Value =
            json::from_slice(&fs::read(&report_path).expect("report read")).expect("json");
        assert!(
            report
                .get("suppressed_buckets")
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn anchor_command_writes_release_artifacts() {
        let tmp = TempDir::new().expect("tempdir");
        let governance_dir = tmp.path().join("governance");
        let action_path = tmp.path().join("release.json");
        let action = serde_json::json!({
            "action": "TransparencyReleaseV1",
            "version": 1,
            "quarter": "2026-Q3",
            "generated_at": "2026-10-15T00:00:00Z",
            "manifest_digest_blake2b_256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sorafs_cid_hex": "0123456789abcdef",
            "dashboards_git_sha": "00112233445566778899aabbccddeeff00112233",
            "note": "test release",
            "manifest_path": "artifacts/ministry/transparency/2026-Q3/transparency_manifest.json",
            "checksums_path": "artifacts/ministry/transparency/2026-Q3/checksums.sha256"
        });
        fs::write(
            &action_path,
            format!("{}\n", serde_json::to_string_pretty(&action).expect("json")),
        )
        .expect("write action");

        run_anchor(AnchorOptions {
            action_path: action_path.clone(),
            governance_dir: governance_dir.clone(),
        })
        .expect("anchor runs");

        let release_dir = governance_dir.join("ministry/releases/2026-Q3");
        let mut artifacts = fs::read_dir(&release_dir)
            .expect("release dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        artifacts.sort();
        assert_eq!(artifacts.len(), 2, "expected .to and .json artefacts");
        let norito_path = artifacts
            .iter()
            .find(|path| path.extension().map(|ext| ext == "to").unwrap_or(false))
            .expect("norito artefact path present");
        let json_path = artifacts
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact path present");

        let encoded = fs::read(norito_path).expect("read norito payload");
        let decoded: TransparencyReleaseV1 =
            decode_from_bytes(&encoded).expect("decode release payload");
        assert_eq!(decoded.quarter, "2026-Q3");
        assert_eq!(decoded.note.as_deref(), Some("test release"));
        assert_eq!(
            decoded.sorafs_root_cid,
            vec![1, 35, 69, 103, 137, 171, 205, 239]
        );

        let summary: SerdeJsonValue =
            serde_json::from_slice(&fs::read(json_path).expect("read json")).expect("json");
        assert_eq!(
            summary.get("action").and_then(SerdeJsonValue::as_str),
            Some("TransparencyReleaseV1")
        );
    }

    fn write_json(path: &Path, value: &Value) {
        fs::write(
            path,
            format!("{}\n", json::to_string_pretty(value).expect("json")),
        )
        .expect("write");
    }

    fn sample_proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-050".into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Remove malicious hash family".into(),
                motivation: "Evidence collected across gateways.".into(),
                expected_impact: "Removes abused payload.".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "malicious-entry".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a0d0c8a".into(),
                reason: "Flagged by moderators".into(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::Url,
                uri: "https://evidence.example.org/ac-2026-050".into(),
                digest_blake3_hex: None,
                description: Some("Primary evidence bundle".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Citizen 7".into(),
                contact: "citizen7@example.org".into(),
                organization: None,
                pgp_fingerprint: None,
            },
            duplicates: vec![],
        }
    }

    fn sample_ai_manifest() -> ModerationReproManifestV1 {
        ModerationReproManifestV1 {
            body: ModerationReproBodyV1 {
                schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
                manifest_id: [0x11; 16],
                manifest_digest: [0x22; 32],
                runner_hash: [0x33; 32],
                runtime_version: "sorafs-ai-runner 0.5.0".into(),
                issued_at_unix: 1_780_000_000,
                seed_material: ModerationSeedMaterialV1 {
                    domain_tag: "ai-runner".into(),
                    seed_version: 1,
                    run_nonce: [0x44; 32],
                },
                thresholds: ModerationThresholdsV1 {
                    quarantine: 7800,
                    escalate: 3200,
                },
                models: vec![ModerationModelFingerprintV1 {
                    model_id: [0x55; 16],
                    artifact_digest: [0x66; 32],
                    weights_digest: [0x77; 32],
                    opset: 17,
                    weight: Some(5_000),
                }],
                notes: None,
            },
            signatures: vec![ModerationReproSignatureV1 {
                role: "council".into(),
                public_key: PublicKey::from_hex(
                    Algorithm::Ed25519,
                    "87fdcacf58b891947600b0c37795cadb5a2ae6de612338fda9489ab21ce427ba",
                )
                .expect("public key"),
                signature: SignatureOf::from_signature(
                    Signature::from_hex(hex::encode([0xAA; 64])).expect("signature"),
                ),
            }],
        }
    }
}
