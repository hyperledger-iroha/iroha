//! CLI utility for comparing Android vs Rust telemetry schema definitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fmt::Write as FmtWrite,
    fs::{File, create_dir_all},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use clap::{Parser, ValueEnum};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Diff Android vs Rust telemetry schemas", long_about = None)]
struct Args {
    /// Path to the Android schema JSON (config mode).
    #[arg(long, conflicts_with = "android_commit")]
    android_config: Option<PathBuf>,

    /// Path to the Rust schema JSON (config mode).
    #[arg(long, conflicts_with = "rust_commit")]
    rust_config: Option<PathBuf>,

    /// Git commit (or ref) to load Android schema from.
    #[arg(long, requires = "rust_commit")]
    android_commit: Option<String>,

    /// Git commit (or ref) to load Rust schema from.
    #[arg(long, requires = "android_commit")]
    rust_commit: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,

    /// Relative path to the Android schema inside the repository (commit mode).
    #[arg(long, default_value = "configs/android_telemetry.json")]
    android_schema_path: PathBuf,

    /// Relative path to the Rust schema inside the repository (commit mode).
    #[arg(long, default_value = "configs/rust_telemetry.json")]
    rust_schema_path: PathBuf,

    /// Optional path to write a trimmed policy summary JSON (for governance bundles).
    #[arg(long)]
    policy_out: Option<PathBuf>,

    /// Optional path to write Prometheus textfile metrics for dashboards.
    #[arg(long)]
    metrics_out: Option<PathBuf>,

    /// Optional path to write a Markdown summary for readiness packets.
    #[arg(long)]
    markdown_out: Option<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Debug, JsonDeserialize)]
struct TelemetrySchema {
    #[norito(default)]
    version: Option<String>,
    #[norito(default)]
    signals: Vec<Signal>,
}

#[derive(Debug, JsonDeserialize)]
struct Signal {
    name: String,
    #[norito(default)]
    canonical_name: Option<String>,
    #[norito(default)]
    channel: Option<String>,
    #[norito(default)]
    parity: Option<String>,
    #[norito(default)]
    status: Option<String>,
    #[norito(default)]
    notes: Option<String>,
    #[norito(default)]
    fields: Vec<Field>,
}

#[derive(Debug, JsonDeserialize)]
struct Field {
    name: String,
    #[norito(default)]
    canonical_name: Option<String>,
    #[norito(default)]
    data_type: Option<String>,
    #[norito(default)]
    representation: Option<String>,
    #[norito(default)]
    parity: Option<String>,
    #[norito(default)]
    status: Option<String>,
    #[norito(default)]
    notes: Option<String>,
}

fn canonical_signal_name(signal: &Signal) -> &str {
    signal.canonical_name.as_deref().unwrap_or(&signal.name)
}

fn canonical_field_name(field: &Field) -> &str {
    field.canonical_name.as_deref().unwrap_or(&field.name)
}

fn find_signal<'a>(schema: &'a TelemetrySchema, name: &str) -> Option<&'a Signal> {
    schema
        .signals
        .iter()
        .find(|signal| canonical_signal_name(signal) == name)
}

fn find_field_in_schema<'a>(
    schema: &'a TelemetrySchema,
    signal_name: &str,
    field_name: &str,
) -> Option<&'a Field> {
    find_signal(schema, signal_name).and_then(|signal| {
        signal
            .fields
            .iter()
            .find(|field| canonical_field_name(field) == field_name)
    })
}

#[derive(Debug, JsonSerialize, Default, Clone, PartialEq, Eq)]
struct Summary {
    signals_compared: usize,
    parity_matches: usize,
    intentional_differences: usize,
    android_only_signals: usize,
    rust_only_signals: usize,
}

#[derive(Debug, JsonSerialize)]
struct IntentionalDifference {
    signal: String,
    field: String,
    difference: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct SignalOnlyEntry {
    signal: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    canonical_name: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    parity: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct FieldMismatch {
    signal: String,
    field: String,
    difference: String,
}

#[derive(Debug)]
struct DiffOutcome {
    summary: Summary,
    intentional_differences: Vec<IntentionalDifference>,
    android_only_signals: Vec<SignalOnlyEntry>,
    rust_only_signals: Vec<SignalOnlyEntry>,
    field_mismatches: Vec<FieldMismatch>,
    policy_violations: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct DiffReport {
    generated_at_utc: String,
    tool_version: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    android_commit: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    rust_commit: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    android_config: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    rust_config: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    android_version: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    rust_version: Option<String>,
    summary: Summary,
    intentional_differences: Vec<IntentionalDifference>,
    android_only_signals: Vec<SignalOnlyEntry>,
    rust_only_signals: Vec<SignalOnlyEntry>,
    field_mismatches: Vec<FieldMismatch>,
    #[norito(skip_serializing_if = "Vec::is_empty")]
    policy_violations: Vec<String>,
    recommendations: Vec<String>,
}

#[derive(Debug, JsonSerialize, PartialEq, Eq)]
struct PolicySummary {
    generated_at_utc: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    android_origin: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    rust_origin: Option<String>,
    summary: Summary,
    #[norito(skip_serializing_if = "Vec::is_empty")]
    policy_violations: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (android_schema, android_origin) = load_schema(
        args.android_config.as_ref(),
        args.android_commit.as_ref(),
        &args.android_schema_path,
    )?;
    policy::validate_android_schema(&android_schema)?;
    let (rust_schema, rust_origin) = load_schema(
        args.rust_config.as_ref(),
        args.rust_commit.as_ref(),
        &args.rust_schema_path,
    )?;
    policy::validate_rust_schema(&rust_schema)?;

    let outcome = diff_schemas(&android_schema, &rust_schema);

    let recommendations =
        build_recommendations(&outcome.intentional_differences, &outcome.field_mismatches);

    let report = DiffReport {
        generated_at_utc: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),
        android_commit: android_origin.commit.clone(),
        rust_commit: rust_origin.commit.clone(),
        android_config: android_origin.config_path(),
        rust_config: rust_origin.config_path(),
        android_version: android_schema.version.clone(),
        rust_version: rust_schema.version.clone(),
        summary: outcome.summary,
        intentional_differences: outcome.intentional_differences,
        android_only_signals: outcome.android_only_signals,
        rust_only_signals: outcome.rust_only_signals,
        field_mismatches: outcome.field_mismatches,
        policy_violations: outcome.policy_violations,
        recommendations,
    };

    match args.format {
        OutputFormat::Json => {
            let mut stdout = io::stdout();
            json::to_writer_pretty(&mut stdout, &report)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }

    if let Some(policy_out) = args.policy_out.as_ref() {
        write_policy_summary(&report, policy_out)?;
    }

    if let Some(metrics_out) = args.metrics_out.as_ref() {
        write_metrics(&report, metrics_out)?;
    }

    if let Some(markdown_out) = args.markdown_out.as_ref() {
        write_markdown_summary(&report, markdown_out)?;
    }

    enforce_policy(&report)
}

#[derive(Clone, Debug)]
struct SchemaOrigin {
    commit: Option<String>,
    config: Option<PathBuf>,
}

impl SchemaOrigin {
    fn config_path(&self) -> Option<String> {
        self.config
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
    }
}

fn load_schema(
    config: Option<&PathBuf>,
    commit: Option<&String>,
    default_path: &Path,
) -> Result<(TelemetrySchema, SchemaOrigin)> {
    match (config, commit) {
        (Some(config_path), None) => {
            let data = std::fs::read_to_string(config_path)
                .with_context(|| format!("reading schema from {}", config_path.display()))?;
            let schema: TelemetrySchema = json::from_str(&data)
                .with_context(|| format!("parsing schema from {}", config_path.display()))?;
            Ok((
                schema,
                SchemaOrigin {
                    commit: None,
                    config: Some(config_path.clone()),
                },
            ))
        }
        (None, Some(commit_sha)) => {
            let data = read_file_from_commit(commit_sha, default_path)?;
            let schema: TelemetrySchema = json::from_str(&data).with_context(|| {
                format!(
                    "parsing schema from {} at commit {}",
                    default_path.display(),
                    commit_sha
                )
            })?;
            Ok((
                schema,
                SchemaOrigin {
                    commit: Some(commit_sha.clone()),
                    config: None,
                },
            ))
        }
        (None, None) => Err(anyhow!(
            "either --{platform}-config or --{platform}-commit must be supplied",
            platform = default_path
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("schema")
        )),
        (Some(_), Some(_)) => Err(anyhow!(
            "cannot use --config and --commit simultaneously for {}",
            default_path.display()
        )),
    }
}

fn read_file_from_commit(commit: &str, path: &Path) -> Result<String> {
    let spec = format!("{commit}:{}", normalize_git_path(path));
    let output = Command::new("git")
        .args(["show", &spec])
        .output()
        .with_context(|| format!("running git show {spec}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).context("schema file is not valid UTF-8")
}

fn normalize_git_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn diff_schemas(android: &TelemetrySchema, rust: &TelemetrySchema) -> DiffOutcome {
    let mut summary = Summary::default();
    let mut intentional = Vec::new();
    let mut android_only = Vec::new();
    let mut rust_only = Vec::new();
    let mut mismatches = Vec::new();

    let mut rust_signals: BTreeMap<String, &Signal> =
        rust.signals.iter().map(|s| (signal_key(s), s)).collect();

    for a_signal in &android.signals {
        let key = signal_key(a_signal);
        if let Some(r_signal) = rust_signals.remove(&key) {
            summary.signals_compared += 1;
            let (mut signal_intentional, mut signal_mismatches) = diff_signal(a_signal, r_signal);
            if signal_intentional.is_empty() && signal_mismatches.is_empty() {
                summary.parity_matches += 1;
            } else {
                summary.intentional_differences += signal_intentional.len();
            }
            intentional.append(&mut signal_intentional);
            mismatches.append(&mut signal_mismatches);
        } else {
            summary.android_only_signals += 1;
            android_only.push(SignalOnlyEntry {
                signal: a_signal.name.clone(),
                canonical_name: a_signal.canonical_name.clone(),
                channel: a_signal.channel.clone(),
                parity: a_signal.parity.clone(),
                status: a_signal.status.clone(),
                notes: a_signal.notes.clone(),
            });
        }
    }

    for (_, r_signal) in rust_signals {
        summary.rust_only_signals += 1;
        rust_only.push(SignalOnlyEntry {
            signal: r_signal.name.clone(),
            canonical_name: r_signal.canonical_name.clone(),
            channel: r_signal.channel.clone(),
            parity: r_signal.parity.clone(),
            status: r_signal.status.clone(),
            notes: r_signal.notes.clone(),
        });
    }

    let mut policy_violations = Vec::new();
    policy_violations.extend(policy::check_android_only_signals(&android_only));
    policy_violations.extend(policy::check_rust_only_signals(&rust_only));
    policy_violations.extend(check_intentional_statuses(&intentional));

    DiffOutcome {
        summary,
        intentional_differences: intentional,
        android_only_signals: android_only,
        rust_only_signals: rust_only,
        field_mismatches: mismatches,
        policy_violations,
    }
}

fn diff_signal<'a>(
    android: &'a Signal,
    rust: &'a Signal,
) -> (Vec<IntentionalDifference>, Vec<FieldMismatch>) {
    let mut intentional = Vec::new();
    let mut mismatches = Vec::new();
    let mut rust_fields: BTreeMap<String, &Field> =
        rust.fields.iter().map(|f| (field_key(f), f)).collect();
    let canonical_signal = canonical_signal_name(android).to_string();

    for a_field in &android.fields {
        let key = field_key(a_field);
        match rust_fields.remove(&key) {
            Some(r_field) => {
                if !field_matches(a_field, r_field) {
                    if let Some(reason) =
                        policy::allow_representation_difference(&canonical_signal, a_field, r_field)
                    {
                        intentional.push(IntentionalDifference {
                            signal: android.name.clone(),
                            field: field_display_name(a_field),
                            difference: describe_difference(Some(a_field), Some(r_field)),
                            status: Some("policy_allowlisted".into()),
                            notes: Some(reason.into()),
                        });
                    } else if is_intentional(a_field) {
                        intentional.push(IntentionalDifference {
                            signal: android.name.clone(),
                            field: field_display_name(a_field),
                            difference: describe_difference(Some(a_field), Some(r_field)),
                            status: a_field.status.clone(),
                            notes: a_field.notes.clone(),
                        });
                    } else {
                        mismatches.push(FieldMismatch {
                            signal: android.name.clone(),
                            field: field_display_name(a_field),
                            difference: describe_difference(Some(a_field), Some(r_field)),
                        });
                    }
                }
            }
            None => {
                if is_intentional(a_field) {
                    intentional.push(IntentionalDifference {
                        signal: android.name.clone(),
                        field: field_display_name(a_field),
                        difference: "field missing from Rust schema".to_string(),
                        status: a_field.status.clone(),
                        notes: a_field.notes.clone(),
                    });
                } else {
                    mismatches.push(FieldMismatch {
                        signal: android.name.clone(),
                        field: field_display_name(a_field),
                        difference: "field missing from Rust schema".to_string(),
                    });
                }
            }
        }
    }

    for (_, r_field) in rust_fields {
        if let Some(reason) = policy::allow_rust_only_field(&canonical_signal, r_field) {
            intentional.push(IntentionalDifference {
                signal: android.name.clone(),
                field: field_display_name(r_field),
                difference: "field missing from Android schema".to_string(),
                status: Some("policy_allowlisted".into()),
                notes: Some(reason.into()),
            });
        } else {
            mismatches.push(FieldMismatch {
                signal: android.name.clone(),
                field: field_display_name(r_field),
                difference: "field missing from Android schema".to_string(),
            });
        }
    }

    (intentional, mismatches)
}

fn field_matches(android: &Field, rust: &Field) -> bool {
    android.data_type == rust.data_type && android.representation == rust.representation
}

fn is_intentional(field: &Field) -> bool {
    matches!(
        field.parity.as_deref(),
        Some("intentional_difference" | "android_only")
    )
}

fn signal_key(signal: &Signal) -> String {
    canonical_signal_name(signal).to_string()
}

fn field_key(field: &Field) -> String {
    canonical_field_name(field).to_string()
}

fn field_display_name(field: &Field) -> String {
    field
        .canonical_name
        .clone()
        .unwrap_or_else(|| field.name.clone())
}

fn describe_difference(android: Option<&Field>, rust: Option<&Field>) -> String {
    let mut parts = Vec::new();
    if let (Some(a), Some(r)) = (android, rust) {
        if a.data_type != r.data_type {
            parts.push(format!(
                "data_type {:?} vs {:?}",
                a.data_type.as_deref().unwrap_or("unknown"),
                r.data_type.as_deref().unwrap_or("unknown")
            ));
        }
        if a.representation != r.representation {
            parts.push(format!(
                "representation {:?} vs {:?}",
                a.representation.as_deref().unwrap_or("n/a"),
                r.representation.as_deref().unwrap_or("n/a")
            ));
        }
        if parts.is_empty() {
            parts.push("field definitions differ".to_string());
        }
    } else if android.is_some() {
        parts.push("field missing from Rust schema".to_string());
    } else if rust.is_some() {
        parts.push("field missing from Android schema".to_string());
    }
    parts.join(", ")
}

fn build_recommendations(
    intentional: &[IntentionalDifference],
    mismatches: &[FieldMismatch],
) -> Vec<String> {
    let mut recs = BTreeSet::new();
    for diff in intentional {
        let status = diff.status.as_deref().unwrap_or_default();
        if status != "accepted" && status != "policy_allowlisted" {
            recs.insert(format!(
                "Review intentional difference for {}.{}",
                diff.signal, diff.field
            ));
        }
    }
    for mismatch in mismatches {
        recs.insert(format!(
            "Investigate mismatch for {}.{} ({})",
            mismatch.signal, mismatch.field, mismatch.difference
        ));
    }
    recs.into_iter().collect()
}

fn check_intentional_statuses(intentional: &[IntentionalDifference]) -> Vec<String> {
    intentional
        .iter()
        .filter_map(|diff| match diff.status.as_deref() {
            Some("accepted" | "policy_allowlisted") => None,
            Some(status) => Some(format!(
                "Intentional difference for {}.{} uses unsupported status `{status}`; set status to `accepted` or `policy_allowlisted`.",
                diff.signal, diff.field
            )),
            None => Some(format!(
                "Intentional difference for {}.{} is missing a status; set status to `accepted` or `policy_allowlisted`.",
                diff.signal, diff.field
            )),
        })
        .collect()
}

fn enforce_policy(report: &DiffReport) -> Result<()> {
    if report.policy_violations.is_empty() {
        return Ok(());
    }
    let summary = report.policy_violations.join("; ");
    Err(anyhow!(
        "policy violations detected ({}): {summary}",
        report.policy_violations.len()
    ))
}

fn build_policy_summary(report: &DiffReport) -> PolicySummary {
    PolicySummary {
        generated_at_utc: report.generated_at_utc.clone(),
        android_origin: origin_string(
            report.android_config.as_deref(),
            report.android_commit.as_deref(),
        ),
        rust_origin: origin_string(report.rust_config.as_deref(), report.rust_commit.as_deref()),
        summary: report.summary.clone(),
        policy_violations: report.policy_violations.clone(),
    }
}

fn origin_string(config: Option<&str>, commit: Option<&str>) -> Option<String> {
    config
        .map(str::to_string)
        .or_else(|| commit.map(str::to_string))
}

fn write_table_section(
    buf: &mut String,
    title: &str,
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
) -> Result<()> {
    writeln!(buf, "\n## {title}\n")?;
    writeln!(buf, "| {} |", headers.join(" | "))?;
    writeln!(
        buf,
        "|{}|",
        headers
            .iter()
            .map(|_| "--------")
            .collect::<Vec<_>>()
            .join("|")
    )?;
    for row in rows {
        writeln!(buf, "| {} |", row.join(" | "))?;
    }
    Ok(())
}

fn write_markdown_header(report: &DiffReport, buf: &mut String) -> Result<()> {
    let android_origin = origin_string(
        report.android_config.as_deref(),
        report.android_commit.as_deref(),
    )
    .unwrap_or_else(|| "unknown".into());
    let rust_origin = origin_string(report.rust_config.as_deref(), report.rust_commit.as_deref())
        .unwrap_or_else(|| "unknown".into());
    writeln!(buf, "# Android vs Rust Telemetry Parity Snapshot")?;
    writeln!(
        buf,
        "\n- Generated: `{}`\n- Tool version: `{}`",
        report.generated_at_utc, report.tool_version
    )?;
    writeln!(buf, "- Android origin: `{android_origin}`")?;
    writeln!(buf, "- Rust origin: `{rust_origin}`")?;
    Ok(())
}

fn write_summary_section(report: &DiffReport, buf: &mut String) -> Result<()> {
    let summary_rows = [
        (
            "Signals compared",
            report.summary.signals_compared.to_string(),
        ),
        ("Parity matches", report.summary.parity_matches.to_string()),
        (
            "Intentional differences",
            report.summary.intentional_differences.to_string(),
        ),
        (
            "Android-only signals",
            report.summary.android_only_signals.to_string(),
        ),
        (
            "Rust-only signals",
            report.summary.rust_only_signals.to_string(),
        ),
        (
            "Field mismatches",
            report.field_mismatches.len().to_string(),
        ),
        (
            "Policy violations",
            report.policy_violations.len().to_string(),
        ),
    ];
    write_table_section(
        buf,
        "Summary",
        &["Metric", "Count"],
        summary_rows
            .into_iter()
            .map(|(label, value)| vec![label.to_string(), value]),
    )
}

fn write_intentional_differences_section(report: &DiffReport, buf: &mut String) -> Result<()> {
    if report.intentional_differences.is_empty() {
        return Ok(());
    }
    write_table_section(
        buf,
        "Intentional differences",
        &["Signal", "Field", "Difference", "Status", "Notes"],
        report.intentional_differences.iter().map(|entry| {
            vec![
                escape_markdown_cell(&entry.signal),
                escape_markdown_cell(&entry.field),
                escape_markdown_cell(&entry.difference),
                escape_markdown_cell(entry.status.as_deref().unwrap_or("—")),
                escape_markdown_cell(entry.notes.as_deref().unwrap_or("—")),
            ]
        }),
    )
}

fn write_signal_only_section(
    buf: &mut String,
    title: &str,
    entries: &[SignalOnlyEntry],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    write_table_section(
        buf,
        title,
        &["Signal", "Channel", "Status", "Notes"],
        entries.iter().map(|entry| {
            vec![
                escape_markdown_cell(&entry.signal),
                escape_markdown_cell(entry.channel.as_deref().unwrap_or("—")),
                escape_markdown_cell(entry.status.as_deref().unwrap_or("—")),
                escape_markdown_cell(entry.notes.as_deref().unwrap_or("—")),
            ]
        }),
    )
}

fn write_field_mismatch_section(report: &DiffReport, buf: &mut String) -> Result<()> {
    if report.field_mismatches.is_empty() {
        return Ok(());
    }
    write_table_section(
        buf,
        "Unexpected field mismatches",
        &["Signal", "Field", "Difference"],
        report.field_mismatches.iter().map(|mismatch| {
            vec![
                escape_markdown_cell(&mismatch.signal),
                escape_markdown_cell(&mismatch.field),
                escape_markdown_cell(&mismatch.difference),
            ]
        }),
    )
}

fn write_policy_violations_section(report: &DiffReport, buf: &mut String) -> Result<()> {
    if report.policy_violations.is_empty() {
        return Ok(());
    }
    writeln!(buf, "\n## Policy violations\n")?;
    for violation in &report.policy_violations {
        writeln!(buf, "- {}", escape_markdown_cell(violation))?;
    }
    Ok(())
}

fn persist_markdown(contents: &str, path: &Path) -> Result<()> {
    let mut file = File::create(path)
        .with_context(|| format!("creating markdown summary at {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("writing markdown summary to {}", path.display()))?;
    file.flush()
        .with_context(|| format!("flushing markdown summary {}", path.display()))?;
    Ok(())
}

fn write_markdown_summary(report: &DiffReport, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("creating markdown summary directory {}", parent.display()))?;
    }
    let mut buf = String::new();
    write_markdown_header(report, &mut buf)?;
    write_summary_section(report, &mut buf)?;
    write_intentional_differences_section(report, &mut buf)?;
    write_signal_only_section(
        &mut buf,
        "Android-only signals",
        &report.android_only_signals,
    )?;
    write_signal_only_section(&mut buf, "Rust-only signals", &report.rust_only_signals)?;
    write_field_mismatch_section(report, &mut buf)?;
    write_policy_violations_section(report, &mut buf)?;
    persist_markdown(&buf, path)
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace(['\n', '\r'], "<br>").replace('|', r"\|")
}

fn write_policy_summary(report: &DiffReport, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("creating policy summary directory {}", parent.display()))?;
    }
    let mut file = File::create(path)
        .with_context(|| format!("creating policy summary at {}", path.display()))?;
    let summary = build_policy_summary(report);
    json::to_writer_pretty(&mut file, &summary)
        .with_context(|| format!("writing policy summary to {}", path.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("finalising policy summary {}", path.display()))?;
    Ok(())
}

fn write_metrics(report: &DiffReport, path: &Path) -> Result<()> {
    let android_origin = origin_label(
        report.android_commit.as_ref(),
        report.android_config.as_ref(),
    );
    let rust_origin = origin_label(report.rust_commit.as_ref(), report.rust_config.as_ref());
    let status = metrics_status(report);
    let run_timestamp = parse_run_timestamp(&report.generated_at_utc)?;

    let origin_labels = [
        ("android_origin", android_origin.as_str()),
        ("rust_origin", rust_origin.as_str()),
    ];
    let summary_metrics = [
        (
            "telemetry_schema_diff_signals_compared",
            "Total signals compared between Android and Rust.",
            metric_value(report.summary.signals_compared),
        ),
        (
            "telemetry_schema_diff_parity_matches",
            "Signals with matching representations between Android and Rust.",
            metric_value(report.summary.parity_matches),
        ),
        (
            "telemetry_schema_diff_intentional_differences",
            "Signals marked as intentional differences.",
            metric_value(report.summary.intentional_differences),
        ),
        (
            "telemetry_schema_diff_android_only_signals",
            "Signals present only in the Android schema.",
            metric_value(report.summary.android_only_signals),
        ),
        (
            "telemetry_schema_diff_rust_only_signals",
            "Signals present only in the Rust schema.",
            metric_value(report.summary.rust_only_signals),
        ),
        (
            "telemetry_schema_diff_field_mismatches",
            "Fields that differ without an intentional annotation.",
            metric_value(report.field_mismatches.len()),
        ),
        (
            "telemetry_schema_diff_policy_violations",
            "Policy violations detected by telemetry-schema-diff.",
            metric_value(report.policy_violations.len()),
        ),
        (
            "telemetry_schema_diff_last_run_timestamp_seconds",
            "Unix timestamp for the schema diff run.",
            run_timestamp,
        ),
    ];
    let mut buf = String::new();
    for (metric, help, value) in summary_metrics {
        append_metric(&mut buf, metric, help, &origin_labels, value)?;
    }
    append_metric(
        &mut buf,
        "telemetry_schema_diff_run_status",
        "telemetry-schema-diff run status (ok/policy_violation/field_mismatch).",
        &[
            ("android_origin", android_origin.as_str()),
            ("rust_origin", rust_origin.as_str()),
            ("status", status),
        ],
        1.0,
    )?;

    let mut file = File::create(path)
        .with_context(|| format!("failed to open metrics output {}", path.display()))?;
    file.write_all(buf.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn metrics_status(report: &DiffReport) -> &'static str {
    if !report.policy_violations.is_empty() {
        "policy_violation"
    } else if !report.field_mismatches.is_empty() {
        "field_mismatch"
    } else {
        "ok"
    }
}

fn parse_run_timestamp(generated_at_utc: &str) -> Result<f64> {
    let timestamp = chrono::DateTime::parse_from_rfc3339(generated_at_utc)
        .with_context(|| format!("parsing generated_at_utc {generated_at_utc} for metrics output"))?
        .timestamp();
    Ok(i64_to_f64(timestamp))
}

fn metric_value(count: usize) -> f64 {
    u64_to_f64(u64::try_from(count).expect("usize must fit into u64 for metrics conversion"))
}

#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn i64_to_f64(value: i64) -> f64 {
    value as f64
}

fn append_metric(
    buf: &mut String,
    metric: &str,
    help: &str,
    labels: &[(&str, &str)],
    value: f64,
) -> Result<()> {
    writeln!(buf, "# HELP {metric} {help}").map_err(|err| anyhow!(err))?;
    writeln!(buf, "# TYPE {metric} gauge").map_err(|err| anyhow!(err))?;
    if labels.is_empty() {
        writeln!(buf, "{metric} {value}").map_err(|err| anyhow!(err))?;
    } else {
        let mut label_parts = Vec::with_capacity(labels.len());
        for (key, value) in labels {
            label_parts.push(format!(r#"{}="{}""#, key, escape_label_value(value)));
        }
        writeln!(buf, "{metric}{{{}}} {value}", label_parts.join(","))
            .map_err(|err| anyhow!(err))?;
    }
    Ok(())
}

fn escape_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn origin_label(commit: Option<&String>, config: Option<&String>) -> String {
    commit
        .cloned()
        .or_else(|| config.cloned())
        .unwrap_or_else(|| "unknown".into())
}

mod policy {
    use std::collections::BTreeSet;

    use anyhow::{Result, anyhow};

    use super::{
        Field, SignalOnlyEntry, TelemetrySchema, canonical_field_name, find_field_in_schema,
        find_signal,
    };

    struct FieldAllowance {
        signal: &'static str,
        field: &'static str,
        reason: &'static str,
        expected_android_representation: Option<&'static str>,
    }

    const ANDROID_ONLY_SIGNALS: &[(&str, &str)] = &[
        (
            "android.telemetry.redaction.override",
            "Override records are Android-only and must be mirrored in the audit log.",
        ),
        (
            "network.context",
            "Android exports a sanitized network context event so OEM/device roaming issues can be triaged without exposing carrier metadata; Rust nodes never record this signal.",
        ),
        (
            "streaming_privacy.redaction_fail_total",
            "Mobile-only counter used to flag client-side redaction guardrail violations; Rust relies on server-side streaming privacy metrics instead.",
        ),
        (
            "telemetry.redaction.salt_version",
            "Android clients publish the active hashing salt metadata so operators can confirm rotations; Rust exporters do not track this state.",
        ),
    ];

    const RUST_ONLY_SIGNALS: &[(&str, &str)] = &[];

    struct AllowedField {
        name: &'static str,
        expected_representation: Option<&'static str>,
    }

    struct AllowedFieldSet {
        signal: &'static str,
        fields: &'static [AllowedField],
        reason: &'static str,
    }

    const RUST_ONLY_FIELDS: &[FieldAllowance] = &[FieldAllowance {
        signal: "hardware.profile",
        field: "hardware_tier",
        reason: "Rust retains the precise hardware tier while Android buckets profiles for privacy.",
        expected_android_representation: None,
    }];

    const REPRESENTATION_ALLOWANCES: &[FieldAllowance] = &[FieldAllowance {
        signal: "attestation.result",
        field: "device_tier",
        reason: "Android buckets attestation device tiers; Rust nodes keep raw metadata.",
        expected_android_representation: Some("bucketed"),
    }];

    const REQUIRED_ANDROID_REPRESENTATIONS: &[(&str, &str, &str)] = &[
        ("torii.http.request", "authority", "blake2b_256"),
        ("torii.http.retry", "authority", "blake2b_256"),
        ("attestation.result", "alias", "blake2b_256"),
    ];

    const REQUIRED_RUST_RAW_FIELDS: &[(&str, &str)] = &[
        ("torii.http.request", "authority"),
        ("torii.http.retry", "authority"),
        ("attestation.result", "alias"),
    ];

    const ANDROID_FIELD_ALLOWLISTS: &[AllowedFieldSet] = &[
        AllowedFieldSet {
            signal: "network.context",
            fields: &[
                AllowedField {
                    name: "network_type",
                    expected_representation: None,
                },
                AllowedField {
                    name: "roaming",
                    expected_representation: None,
                },
            ],
            reason: "Android may only emit coarse network metadata (type + roaming) for privacy; any carrier/identifier fields violate the AND7 redaction policy.",
        },
        AllowedFieldSet {
            signal: "hardware.profile",
            fields: &[AllowedField {
                name: "profile_bucket",
                expected_representation: Some("emulator|consumer|enterprise"),
            }],
            reason: "Android exports only device profile buckets; raw hardware tiers remain Rust-only.",
        },
    ];

    pub fn validate_android_schema(schema: &TelemetrySchema) -> Result<()> {
        ensure_android_only_signals(schema)?;
        ensure_required_android_representations(schema)?;
        ensure_absent_rust_only_fields(schema)?;
        ensure_representation_allowances(schema)?;
        ensure_android_field_allowlists(schema)?;

        Ok(())
    }

    fn ensure_android_only_signals(schema: &TelemetrySchema) -> Result<()> {
        for (signal, reason) in ANDROID_ONLY_SIGNALS {
            let Some(found) = find_signal(schema, signal) else {
                return Err(anyhow!(
                    "Android schema missing required Android-only signal `{signal}`: {reason}"
                ));
            };
            let Some(status) = found.status.as_deref() else {
                return Err(anyhow!(
                    "Android-only signal `{signal}` must declare a status (e.g., accepted) documenting governance approval: {reason}"
                ));
            };
            if status != "accepted" && status != "policy_allowlisted" {
                return Err(anyhow!(
                    "Android-only signal `{signal}` must use `status:\"accepted\"` or `status:\"policy_allowlisted\"` (found `{status}`)"
                ));
            }
        }

        Ok(())
    }

    fn ensure_required_android_representations(schema: &TelemetrySchema) -> Result<()> {
        for (signal, field, expected_repr) in REQUIRED_ANDROID_REPRESENTATIONS {
            let Some(found) = find_field_in_schema(schema, signal, field) else {
                return Err(anyhow!(
                    "Android schema missing required field {signal}.{field}"
                ));
            };
            let representation = found
                .representation
                .as_deref()
                .ok_or_else(|| anyhow!("Android field {signal}.{field} must set representation"))?;
            if representation != *expected_repr {
                return Err(anyhow!(
                    "Android field {signal}.{field} must declare representation `{expected_repr}` (found `{representation}`)"
                ));
            }
        }

        Ok(())
    }

    fn ensure_absent_rust_only_fields(schema: &TelemetrySchema) -> Result<()> {
        for allowance in RUST_ONLY_FIELDS {
            if find_field_in_schema(schema, allowance.signal, allowance.field).is_some() {
                return Err(anyhow!(
                    "Android schema must not expose {}.{}: {}",
                    allowance.signal,
                    allowance.field,
                    allowance.reason
                ));
            }
        }

        Ok(())
    }

    fn ensure_representation_allowances(schema: &TelemetrySchema) -> Result<()> {
        for allowance in REPRESENTATION_ALLOWANCES {
            if let Some(expected) = allowance.expected_android_representation
                && let Some(field) = find_field_in_schema(schema, allowance.signal, allowance.field)
                && field.representation.as_deref() != Some(expected)
            {
                return Err(anyhow!(
                    "Android field {}.{} must declare representation `{expected}`: {}",
                    allowance.signal,
                    allowance.field,
                    allowance.reason
                ));
            }
        }

        Ok(())
    }

    fn ensure_android_field_allowlists(schema: &TelemetrySchema) -> Result<()> {
        for allowlist in ANDROID_FIELD_ALLOWLISTS {
            if let Some(signal) = find_signal(schema, allowlist.signal) {
                let allowed: BTreeSet<&str> =
                    allowlist.fields.iter().map(|field| field.name).collect();
                let mut seen = BTreeSet::new();
                for field in &signal.fields {
                    let canonical = canonical_field_name(field);
                    if !allowed.contains(canonical) {
                        return Err(anyhow!(
                            "Android signal {} must only expose {:?}: {}, found `{}`",
                            allowlist.signal,
                            allowed,
                            allowlist.reason,
                            canonical
                        ));
                    }
                    if let Some(expected) = allowlist
                        .fields
                        .iter()
                        .find(|allowed| allowed.name == canonical)
                        .and_then(|allowed| allowed.expected_representation)
                    {
                        let Some(actual) = field.representation.as_deref() else {
                            return Err(anyhow!(
                                "Android field {}.{} must declare representation `{expected}`: {}",
                                allowlist.signal,
                                canonical,
                                allowlist.reason
                            ));
                        };
                        if actual != expected {
                            return Err(anyhow!(
                                "Android field {}.{} must declare representation `{expected}` (found `{actual}`): {}",
                                allowlist.signal,
                                canonical,
                                allowlist.reason
                            ));
                        }
                    }
                    seen.insert(canonical.to_string());
                }

                for required in allowlist.fields {
                    if !seen.contains(required.name) {
                        return Err(anyhow!(
                            "Android signal {} must include field `{}`: {}",
                            allowlist.signal,
                            required.name,
                            allowlist.reason
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn validate_rust_schema(schema: &TelemetrySchema) -> Result<()> {
        for (signal, field) in REQUIRED_RUST_RAW_FIELDS {
            let Some(found) = find_field_in_schema(schema, signal, field) else {
                return Err(anyhow!(
                    "Rust schema missing required field {signal}.{field}"
                ));
            };
            if found.representation.is_some() {
                return Err(anyhow!(
                    "Rust field {signal}.{field} must remain unredacted (representation must be absent)"
                ));
            }
        }

        Ok(())
    }

    pub fn allow_representation_difference(
        signal: &str,
        android_field: &Field,
        rust_field: &Field,
    ) -> Option<&'static str> {
        if canonical_field_name(android_field) != canonical_field_name(rust_field) {
            return None;
        }
        REPRESENTATION_ALLOWANCES.iter().find_map(|entry| {
            if entry.signal == signal && entry.field == canonical_field_name(android_field) {
                match (
                    entry.expected_android_representation,
                    android_field.representation.as_deref(),
                ) {
                    (Some(expected), Some(actual)) if expected == actual => {
                        if rust_field.representation.is_none() {
                            Some(entry.reason)
                        } else {
                            None
                        }
                    }
                    (Some(_), _) => None,
                    (None, _) => {
                        if rust_field.representation.is_none() {
                            Some(entry.reason)
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        })
    }

    pub fn allow_rust_only_field(signal: &str, field: &Field) -> Option<&'static str> {
        RUST_ONLY_FIELDS
            .iter()
            .find(|entry| entry.signal == signal && entry.field == canonical_field_name(field))
            .map(|entry| entry.reason)
    }

    pub fn check_android_only_signals(entries: &[SignalOnlyEntry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|entry| {
                let canonical = entry.canonical_name.as_deref().unwrap_or(&entry.signal);
                allow_android_only_signal(canonical).is_none().then(|| {
                    format!(
                        "Android-only signal `{canonical}` is not allowlisted; update configs or policy."
                    )
                })
            })
            .collect()
    }

    pub fn check_rust_only_signals(entries: &[SignalOnlyEntry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|entry| {
                let canonical = entry.canonical_name.as_deref().unwrap_or(&entry.signal);
                allow_rust_only_signal(canonical).is_none().then(|| {
                    format!(
                        "Rust-only signal `{canonical}` is not allowlisted; update configs or policy."
                    )
                })
            })
            .collect()
    }

    fn allow_android_only_signal(name: &str) -> Option<&'static str> {
        ANDROID_ONLY_SIGNALS
            .iter()
            .find(|(signal, _)| *signal == name)
            .map(|(_, reason)| *reason)
    }

    fn allow_rust_only_signal(name: &str) -> Option<&'static str> {
        RUST_ONLY_SIGNALS
            .iter()
            .find(|(signal, _)| *signal == name)
            .map(|(_, reason)| *reason)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn schema_from_signals(signals: Vec<Signal>) -> TelemetrySchema {
        TelemetrySchema {
            version: Some("test".to_string()),
            signals,
        }
    }

    fn report_with_policy_violations(policy_violations: Vec<String>) -> DiffReport {
        DiffReport {
            generated_at_utc: "now".into(),
            tool_version: "test".into(),
            android_commit: None,
            rust_commit: None,
            android_config: None,
            rust_config: None,
            android_version: None,
            rust_version: None,
            summary: Summary::default(),
            intentional_differences: Vec::new(),
            android_only_signals: Vec::new(),
            rust_only_signals: Vec::new(),
            field_mismatches: Vec::new(),
            policy_violations,
            recommendations: Vec::new(),
        }
    }

    fn report_with_origins() -> DiffReport {
        DiffReport {
            generated_at_utc: "2026-02-24T00:00:00Z".into(),
            tool_version: "test".into(),
            android_commit: Some("android-commit".into()),
            rust_commit: Some("rust-commit".into()),
            android_config: Some("configs/android_telemetry.json".into()),
            rust_config: None,
            android_version: Some("2026-02-24".into()),
            rust_version: Some("2026-02-24".into()),
            summary: Summary {
                parity_matches: 2,
                ..Summary::default()
            },
            intentional_differences: Vec::new(),
            android_only_signals: Vec::new(),
            rust_only_signals: Vec::new(),
            field_mismatches: Vec::new(),
            policy_violations: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    fn signal(name: &str, fields: Vec<Field>) -> Signal {
        Signal {
            name: name.to_string(),
            canonical_name: None,
            channel: Some("metric".into()),
            parity: None,
            status: None,
            notes: None,
            fields,
        }
    }

    fn field(name: &str, data_type: &str) -> Field {
        Field {
            name: name.to_string(),
            canonical_name: None,
            data_type: Some(data_type.to_string()),
            representation: None,
            parity: None,
            status: None,
            notes: None,
        }
    }

    fn android_torii_signal() -> Signal {
        signal(
            "torii.http.request",
            vec![Field {
                representation: Some("blake2b_256".into()),
                ..field("authority", "string")
            }],
        )
    }

    fn android_torii_retry_signal(representation: Option<&str>) -> Signal {
        signal(
            "torii.http.retry",
            vec![Field {
                representation: representation.map(str::to_string),
                ..field("authority", "string")
            }],
        )
    }

    fn android_attestation_signal() -> Signal {
        signal(
            "attestation.result",
            vec![Field {
                representation: Some("blake2b_256".into()),
                ..field("alias", "string")
            }],
        )
    }

    fn android_only_signal_stubs() -> Vec<Signal> {
        vec![
            Signal {
                name: "android.telemetry.redaction.override".into(),
                canonical_name: None,
                channel: Some("event".into()),
                parity: Some("android_only".into()),
                status: Some("accepted".into()),
                notes: Some("allowlisted for governance parity tests".into()),
                fields: Vec::new(),
            },
            Signal {
                name: "android.telemetry.network_context".into(),
                canonical_name: Some("network.context".into()),
                channel: Some("event".into()),
                parity: Some("intentional_difference".into()),
                status: Some("accepted".into()),
                notes: Some("network context is Android-only for privacy".into()),
                fields: vec![
                    Field {
                        name: "network_type".into(),
                        canonical_name: Some("network_type".into()),
                        data_type: Some("string".into()),
                        representation: None,
                        parity: None,
                        status: None,
                        notes: None,
                    },
                    Field {
                        name: "roaming".into(),
                        canonical_name: Some("roaming".into()),
                        data_type: Some("bool".into()),
                        representation: None,
                        parity: None,
                        status: None,
                        notes: None,
                    },
                ],
            },
            Signal {
                name: "android.telemetry.redaction.failure".into(),
                canonical_name: Some("streaming_privacy.redaction_fail_total".into()),
                channel: Some("counter".into()),
                parity: Some("android_only".into()),
                status: Some("accepted".into()),
                notes: Some("redaction failure gauge required for AND7 drills".into()),
                fields: Vec::new(),
            },
            Signal {
                name: "android.telemetry.redaction.salt_version".into(),
                canonical_name: Some("telemetry.redaction.salt_version".into()),
                channel: Some("gauge".into()),
                parity: Some("android_only".into()),
                status: Some("accepted".into()),
                notes: Some("tracks active hash salt epoch".into()),
                fields: Vec::new(),
            },
            Signal {
                name: "android.telemetry.device_profile".into(),
                canonical_name: Some("hardware.profile".into()),
                channel: Some("metric".into()),
                parity: Some("intentional_difference".into()),
                status: Some("accepted".into()),
                notes: Some("only profile buckets are exported on Android".into()),
                fields: vec![Field {
                    name: "profile_bucket".into(),
                    canonical_name: Some("profile_bucket".into()),
                    data_type: Some("string".into()),
                    representation: Some("emulator|consumer|enterprise".into()),
                    parity: None,
                    status: None,
                    notes: None,
                }],
            },
        ]
    }

    fn android_schema(mut signals: Vec<Signal>) -> TelemetrySchema {
        signals.extend(android_only_signal_stubs());
        schema_from_signals(signals)
    }

    fn android_attestation_signal_with_device_tier(representation: Option<&str>) -> Signal {
        let mut signal = android_attestation_signal();
        signal.fields.push(Field {
            representation: representation.map(str::to_string),
            ..field("device_tier", "string")
        });
        signal
    }

    fn rust_torii_retry_signal(representation: Option<&str>) -> Signal {
        signal(
            "torii.http.retry",
            vec![Field {
                representation: representation.map(str::to_string),
                ..field("authority", "string")
            }],
        )
    }

    fn rust_torii_signal(representation: Option<&str>) -> Signal {
        signal(
            "torii.http.request",
            vec![Field {
                representation: representation.map(str::to_string),
                ..field("authority", "string")
            }],
        )
    }

    #[test]
    fn no_differences_counts_as_match() {
        let android = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "string")],
        )]);
        let rust = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "string")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert_eq!(outcome.summary.signals_compared, 1);
        assert_eq!(outcome.summary.parity_matches, 1);
        assert!(outcome.intentional_differences.is_empty());
        assert!(outcome.android_only_signals.is_empty());
        assert!(outcome.rust_only_signals.is_empty());
        assert!(outcome.field_mismatches.is_empty());
        assert!(outcome.policy_violations.is_empty());
    }

    #[test]
    fn intentional_difference_reported() {
        let android_field = Field {
            parity: Some("intentional_difference".into()),
            status: Some("accepted".into()),
            notes: Some("hashes authority".into()),
            ..field("authority", "string")
        };
        let android = schema_from_signals(vec![signal("torii.http.request", vec![android_field])]);
        let rust = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "bytes")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert_eq!(outcome.summary.intentional_differences, 1);
        assert_eq!(outcome.intentional_differences.len(), 1);
        assert!(outcome.field_mismatches.is_empty());
        assert_eq!(
            outcome.intentional_differences[0].signal,
            "torii.http.request"
        );
    }

    #[test]
    fn intentional_difference_without_status_is_policy_violation() {
        let android_field = Field {
            parity: Some("intentional_difference".into()),
            status: None,
            ..field("authority", "string")
        };
        let android = schema_from_signals(vec![signal("torii.http.request", vec![android_field])]);
        let rust = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "bytes")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert!(
            outcome
                .policy_violations
                .iter()
                .any(|violation| violation.contains("missing a status")),
            "intentional differences without status must fail policy gating"
        );
    }

    #[test]
    fn intentional_difference_with_unapproved_status_is_policy_violation() {
        let android_field = Field {
            parity: Some("intentional_difference".into()),
            status: Some("pending".into()),
            ..field("authority", "string")
        };
        let android = schema_from_signals(vec![signal("torii.http.request", vec![android_field])]);
        let rust = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "bytes")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert!(
            outcome
                .policy_violations
                .iter()
                .any(|violation| violation.contains("unsupported status")),
            "non-accepted status values must fail policy gating"
        );
    }

    #[test]
    fn mismatch_detected_when_not_intentional() {
        let android = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "string")],
        )]);
        let rust = schema_from_signals(vec![signal(
            "torii.http.request",
            vec![field("authority", "bytes")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert!(outcome.intentional_differences.is_empty());
        assert_eq!(outcome.field_mismatches.len(), 1);
        assert!(
            outcome.field_mismatches[0]
                .difference
                .contains("data_type \"string\" vs \"bytes\"")
        );
    }

    #[test]
    fn policy_violation_emitted_for_unallowlisted_android_only_signal() {
        let android = schema_from_signals(vec![signal("rogue.signal", vec![])]);
        let rust = schema_from_signals(Vec::new());

        let outcome = diff_schemas(&android, &rust);
        assert_eq!(outcome.summary.android_only_signals, 1);
        assert!(
            outcome
                .policy_violations
                .iter()
                .any(|violation| violation.contains("rogue.signal")),
            "unallowlisted Android-only signals must surface a policy violation"
        );
    }

    #[test]
    fn allowlisted_android_only_signal_has_no_violation() {
        let android =
            schema_from_signals(vec![signal("android.telemetry.redaction.override", vec![])]);
        let rust = schema_from_signals(Vec::new());

        let outcome = diff_schemas(&android, &rust);
        assert_eq!(outcome.summary.android_only_signals, 1);
        assert!(
            outcome.policy_violations.is_empty(),
            "allowlisted Android-only signals must not produce policy violations"
        );
    }

    #[test]
    fn rust_only_field_is_allowlisted() {
        let android = schema_from_signals(vec![signal("hardware.profile", vec![])]);
        let rust = schema_from_signals(vec![signal(
            "hardware.profile",
            vec![field("hardware_tier", "string")],
        )]);

        let outcome = diff_schemas(&android, &rust);
        assert!(outcome.field_mismatches.is_empty());
        assert_eq!(outcome.intentional_differences.len(), 1);
        assert_eq!(
            outcome.intentional_differences[0].status.as_deref(),
            Some("policy_allowlisted")
        );
    }

    #[test]
    fn android_schema_validation_requires_hashed_authority() {
        let mut invalid = android_schema(vec![
            signal(
                "torii.http.request",
                vec![Field {
                    representation: None,
                    ..field("authority", "string")
                }],
            ),
            android_torii_retry_signal(Some("blake2b_256")),
            signal(
                "attestation.result",
                vec![Field {
                    representation: Some("blake2b_256".into()),
                    ..field("alias", "string")
                }],
            ),
        ]);
        assert!(super::policy::validate_android_schema(&invalid).is_err());

        let mut valid_field = field("authority", "string");
        valid_field.representation = Some("blake2b_256".into());
        invalid.signals[0].fields[0] = valid_field;
        assert!(super::policy::validate_android_schema(&invalid).is_ok());
    }

    #[test]
    fn android_schema_validation_requires_hashed_retry_authority() {
        let mut schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(None),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Android must hash torii.http.retry authority for privacy"
        );

        schema.signals[1].fields[0].representation = Some("blake2b_256".into());
        assert!(
            super::policy::validate_android_schema(&schema).is_ok(),
            "Hashing torii.http.retry authority should satisfy validation"
        );
    }

    #[test]
    fn android_schema_validation_rejects_rust_only_fields() {
        let schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal(),
            signal("hardware.profile", vec![field("hardware_tier", "string")]),
        ]);
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Android schema must reject rust-only hardware tier exposure"
        );
    }

    #[test]
    fn android_schema_validation_requires_bucketed_device_tier() {
        let schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(None),
        ]);
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Missing device_tier buckets should fail validation"
        );

        let schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        assert!(
            super::policy::validate_android_schema(&schema).is_ok(),
            "Bucketed device_tier should satisfy validation"
        );
    }

    #[test]
    fn android_schema_validation_requires_android_only_signal_presence() {
        let mut schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        schema
            .signals
            .retain(|signal| signal.name != "android.telemetry.redaction.salt_version");
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Removing allowlisted Android-only signals must fail validation"
        );
    }

    #[test]
    fn android_schema_validation_requires_android_only_signal_status() {
        let mut schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        let salt_index = schema
            .signals
            .iter()
            .position(|signal| signal.name == "android.telemetry.redaction.salt_version")
            .expect("salt version signal present");
        schema.signals[salt_index].status = None;
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Android-only signals must surface accepted/policy_allowlisted statuses"
        );

        schema.signals[salt_index].status = Some("accepted".into());
        assert!(
            super::policy::validate_android_schema(&schema).is_ok(),
            "Restoring status should satisfy validation"
        );
    }

    #[test]
    fn android_schema_validation_rejects_extra_network_context_fields() {
        let mut schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        let network_idx = schema
            .signals
            .iter()
            .position(|signal| canonical_signal_name(signal) == "network.context")
            .expect("network context signal present");
        schema.signals[network_idx].fields.push(Field {
            name: "carrier_name".into(),
            canonical_name: Some("carrier_name".into()),
            data_type: Some("string".into()),
            representation: None,
            parity: None,
            status: None,
            notes: None,
        });
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Android network.context must stay restricted to the coarse allowlist"
        );
    }

    #[test]
    fn android_schema_validation_requires_profile_bucket_representation() {
        let mut schema = android_schema(vec![
            android_torii_signal(),
            android_torii_retry_signal(Some("blake2b_256")),
            android_attestation_signal_with_device_tier(Some("bucketed")),
        ]);
        let profile_idx = schema
            .signals
            .iter()
            .position(|signal| canonical_signal_name(signal) == "hardware.profile")
            .expect("device profile signal present");
        schema.signals[profile_idx].fields[0].representation = None;
        assert!(
            super::policy::validate_android_schema(&schema).is_err(),
            "Device profile buckets must retain the coarse representation"
        );
    }

    #[test]
    fn rust_schema_validation_rejects_missing_authority() {
        let schema = schema_from_signals(vec![signal(
            "attestation.result",
            vec![field("alias", "string")],
        )]);
        assert!(
            super::policy::validate_rust_schema(&schema).is_err(),
            "Missing torii.http.request authority must fail validation"
        );
    }

    #[test]
    fn rust_schema_validation_rejects_redacted_authority() {
        let schema = schema_from_signals(vec![
            rust_torii_signal(Some("blake2b_256")),
            signal("attestation.result", vec![field("alias", "string")]),
        ]);
        assert!(
            super::policy::validate_rust_schema(&schema).is_err(),
            "Rust schemas must keep Torii authorities unredacted"
        );
    }

    #[test]
    fn rust_schema_validation_rejects_missing_retry_authority() {
        let schema = schema_from_signals(vec![
            rust_torii_signal(None),
            signal("attestation.result", vec![field("alias", "string")]),
        ]);
        assert!(
            super::policy::validate_rust_schema(&schema).is_err(),
            "Missing torii.http.retry authority must fail validation"
        );
    }

    #[test]
    fn rust_schema_validation_rejects_redacted_retry_authority() {
        let schema = schema_from_signals(vec![
            rust_torii_signal(None),
            rust_torii_retry_signal(Some("blake2b_256")),
            signal("attestation.result", vec![field("alias", "string")]),
        ]);
        assert!(
            super::policy::validate_rust_schema(&schema).is_err(),
            "Rust schemas must keep Torii retry authorities unredacted"
        );
    }

    #[test]
    fn rust_schema_validation_accepts_raw_retry_authority() {
        let schema = schema_from_signals(vec![
            rust_torii_signal(None),
            rust_torii_retry_signal(None),
            signal("attestation.result", vec![field("alias", "string")]),
        ]);
        assert!(
            super::policy::validate_rust_schema(&schema).is_ok(),
            "Raw retry authority should satisfy validation"
        );
    }

    #[test]
    fn enforce_policy_accepts_clean_reports() {
        let report = report_with_policy_violations(Vec::new());
        assert!(enforce_policy(&report).is_ok());
    }

    #[test]
    fn enforce_policy_rejects_policy_violations() {
        let report = report_with_policy_violations(vec![
            "Android-only signal `rogue.signal` is not allowlisted; update configs or policy."
                .into(),
        ]);
        let error = enforce_policy(&report).unwrap_err();
        let message = format!("{error}");
        assert!(message.contains("policy violations detected"));
        assert!(message.contains("rogue.signal"));
    }

    #[test]
    fn policy_summary_prefers_config_path_over_commit() {
        let report = report_with_origins();
        let summary = build_policy_summary(&report);
        assert_eq!(
            summary.android_origin.as_deref(),
            report.android_config.as_deref(),
            "Config paths should be preferred over commits for provenance reporting"
        );
        assert_eq!(
            summary.rust_origin.as_deref(),
            report.rust_commit.as_deref(),
            "Rust provenance falls back to commit when no config path is provided"
        );
        assert_eq!(summary.summary.parity_matches, 2);
    }

    #[test]
    fn policy_summary_writer_creates_parent_directories() {
        let mut report = report_with_origins();
        report.policy_violations.push("violation".into());
        let dir = std::env::temp_dir().join(format!(
            "telemetry-schema-summary-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock ok")
                .as_nanos()
        ));
        let path = dir.join("policy_summary.json");

        write_policy_summary(&report, &path).expect("policy summary written");

        let data = std::fs::read_to_string(&path).expect("policy summary readable");
        let parsed: json::Value = json::from_str(&data).expect("policy summary is valid JSON");

        assert_eq!(
            parsed["android_origin"],
            json::Value::String("configs/android_telemetry.json".into())
        );
        assert_eq!(parsed["policy_violations"][0].as_str(), Some("violation"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn markdown_summary_writer_emits_tables() {
        let mut report = report_with_origins();
        report.summary.signals_compared = 4;
        report.summary.intentional_differences = 1;
        report.summary.android_only_signals = 1;
        report.summary.rust_only_signals = 1;
        report.intentional_differences.push(IntentionalDifference {
            signal: "torii.http.request".into(),
            field: "authority".into(),
            difference: "representation mismatch".into(),
            status: Some("policy_allowlisted".into()),
            notes: Some("hashed authority on Android".into()),
        });
        report.android_only_signals.push(SignalOnlyEntry {
            signal: "android.telemetry.redaction.override".into(),
            canonical_name: None,
            channel: Some("event".into()),
            parity: None,
            status: Some("policy_allowlisted".into()),
            notes: Some("Android-only override log".into()),
        });
        report.rust_only_signals.push(SignalOnlyEntry {
            signal: "torii.raw.signal".into(),
            canonical_name: None,
            channel: Some("trace".into()),
            parity: None,
            status: None,
            notes: None,
        });
        report.field_mismatches.push(FieldMismatch {
            signal: "torii.http.retry".into(),
            field: "authority".into(),
            difference: "missing from Android schema".into(),
        });
        report
            .policy_violations
            .push("Example violation captured in summary".into());
        let dir = std::env::temp_dir().join(format!(
            "telemetry-schema-markdown-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock ok")
                .as_nanos()
        ));
        let path = dir.join("summary.md");

        write_markdown_summary(&report, &path).expect("markdown summary written");

        let data = std::fs::read_to_string(&path).expect("markdown summary readable");
        assert!(data.contains("# Android vs Rust Telemetry Parity Snapshot"));
        assert!(
            data.contains("| Intentional differences"),
            "intentional table header present"
        );
        assert!(data.contains("torii.http.request"));
        assert!(data.contains("android.telemetry.redaction.override"));
        assert!(
            data.contains("## Policy violations"),
            "markdown includes policy violation section"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn table_section_renders_headers_and_rows() {
        let mut buf = String::new();
        write_table_section(
            &mut buf,
            "Example",
            &["Column A", "Column B"],
            [vec!["foo".into(), "bar".into()]],
        )
        .expect("table renders");

        assert!(buf.contains("## Example"), "title section present");
        assert!(buf.contains("| Column A | Column B |"), "header rendered");
        assert!(buf.contains("| foo | bar |"), "row rendered");
    }

    #[test]
    fn markdown_cell_escapes_pipes_and_newlines() {
        let escaped = escape_markdown_cell("a|b\nc\rd");
        assert_eq!(escaped, r"a\|b<br>c<br>d");
    }

    #[test]
    fn metrics_file_contains_counts() {
        let mut report = report_with_origins();
        report.summary.signals_compared = 4;
        report.summary.parity_matches = 3;
        report.summary.android_only_signals = 1;
        let path = temp_metrics_path("metrics-ok");
        write_metrics(&report, &path).expect("write metrics");
        let contents = std::fs::read_to_string(&path).expect("read metrics file");
        assert!(
            contents.contains("telemetry_schema_diff_signals_compared"),
            "metrics must include signals compared gauge"
        );
        assert!(
            contents.contains(r#"status="ok""#),
            "status label should be ok when there are no violations",
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn metrics_status_reflects_policy_violation() {
        let mut report = report_with_origins();
        report.policy_violations.push("missing status".into());
        let path = temp_metrics_path("metrics-policy");
        write_metrics(&report, &path).expect("write metrics");
        let contents = std::fs::read_to_string(&path).expect("read metrics file");
        assert!(
            contents.contains(r#"status="policy_violation""#),
            "status label should reflect policy violations"
        );
        assert!(
            contents.contains("telemetry_schema_diff_policy_violations"),
            "metrics should report violation count"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn metrics_status_prioritises_policy_over_field_mismatches() {
        let mut report = report_with_origins();
        report.policy_violations.push("missing status".into());
        report.field_mismatches.push(FieldMismatch {
            signal: "torii.http.request".into(),
            field: "authority".into(),
            difference: "representation drift".into(),
        });
        let path = temp_metrics_path("metrics-policy-priority");
        write_metrics(&report, &path).expect("write metrics");
        let contents = std::fs::read_to_string(&path).expect("read metrics file");
        assert!(
            contents.contains(r#"status="policy_violation""#),
            "run status must report policy violation even when mismatches exist"
        );
        assert!(
            contents.contains(
                r#"telemetry_schema_diff_policy_violations{android_origin="android-commit",rust_origin="rust-commit"} 1"#
            ),
            "policy violation gauge should reflect count"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn metrics_status_reports_field_mismatches_when_policy_clean() {
        let mut report = report_with_origins();
        report.field_mismatches.push(FieldMismatch {
            signal: "torii.http.request".into(),
            field: "authority".into(),
            difference: "data_type mismatch".into(),
        });
        let path = temp_metrics_path("metrics-field-mismatch");
        write_metrics(&report, &path).expect("write metrics");
        let contents = std::fs::read_to_string(&path).expect("read metrics file");
        assert!(
            contents.contains(r#"status="field_mismatch""#),
            "run status should flag field mismatches when policy is clean"
        );
        assert!(
            contents.contains(
                r#"telemetry_schema_diff_field_mismatches{android_origin="android-commit",rust_origin="rust-commit"} 1"#
            ),
            "field mismatch gauge should reflect count"
        );
        let _ = std::fs::remove_file(&path);
    }

    fn temp_metrics_path(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock ok")
            .as_nanos();
        path.push(format!("{prefix}-{nanos}.prom"));
        path
    }
}
