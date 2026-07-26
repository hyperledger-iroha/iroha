//! SNNet-15H1 — pen-test and bug bounty rollout helper.
//! Generates a repeatable packet (overview, triage checklist, remediation
//! template, JSON summary) so SRE/Security can contract testers and prove
//! scope/SLA compliance for the gateway CDN.

use std::{
    fmt::Write as FmtWrite,
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug)]
pub struct BugBountyOptions {
    pub config_path: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct BugBountyOutcome {
    pub overview_path: PathBuf,
    pub triage_checklist_path: PathBuf,
    pub remediation_template_path: PathBuf,
    pub summary_path: PathBuf,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct BugBountyConfig {
    program: String,
    slug: Option<String>,
    owners: Vec<String>,
    contact: Contact,
    partners: Vec<Partner>,
    scope: Vec<ScopeEntry>,
    severity_sla: SeveritySla,
    triage: TriagePolicy,
    reward_table: Vec<RewardBand>,
    reporting: Reporting,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct Contact {
    intake_email: String,
    pagerduty: Option<String>,
    tracker: String,
    disclosure_url: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct Partner {
    name: String,
    contact: String,
    contract_id: String,
    renewal: String,
    coverage: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct ScopeEntry {
    area: String,
    cadence: String,
    targets: Vec<String>,
    exclusions: Vec<String>,
    notes: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, PartialEq)]
pub struct SeverityWindow {
    pub ack_hours: u64,
    pub contain_hours: u64,
    pub fix_days: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, PartialEq)]
pub struct SeveritySla {
    pub critical: SeverityWindow,
    pub high: SeverityWindow,
    pub medium: SeverityWindow,
    pub low: SeverityWindow,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, PartialEq)]
pub struct RewardBand {
    pub severity: String,
    pub min: u64,
    pub max: u64,
    pub currency: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct TriagePolicy {
    intake_channels: Vec<String>,
    playbook: Vec<String>,
    duplication_policy: String,
    evidence_requirements: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct Reporting {
    disclosure_window_days: u64,
    remediation_template_version: String,
    sla_dashboard: String,
    audit_bucket: String,
}

#[derive(Clone, Debug, JsonSerialize)]
struct ContactSummary {
    intake_email: String,
    pagerduty: Option<String>,
    tracker: String,
    disclosure_url: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct PartnerSummary {
    name: String,
    contract_id: String,
    contact: String,
    coverage: Vec<String>,
    renewal: String,
}

#[derive(Clone, Debug, JsonSerialize)]
struct ScopeSummary {
    area: String,
    cadence: String,
    targets: Vec<String>,
    exclusions: Vec<String>,
    notes: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct TriageSummary {
    intake_channels: Vec<String>,
    duplication_policy: String,
    evidence_requirements: Vec<String>,
    playbook: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct ReportingSummary {
    disclosure_window_days: u64,
    remediation_template_version: String,
    sla_dashboard: String,
    audit_bucket: String,
}

#[derive(Clone, Debug, JsonSerialize)]
struct OutputSummary {
    overview: String,
    triage_checklist: String,
    remediation_template: String,
}

#[derive(Clone, Debug, JsonSerialize)]
struct BugBountySummary {
    program: String,
    slug: String,
    owners: Vec<String>,
    contact: ContactSummary,
    partners: Vec<PartnerSummary>,
    scope: Vec<ScopeSummary>,
    severity_sla: SeveritySla,
    reward_table: Vec<RewardBand>,
    triage: TriageSummary,
    reporting: ReportingSummary,
    outputs: OutputSummary,
    generated_at_rfc3339: String,
}

/// Generate the bug bounty / pen-test kit from the provided configuration.
pub fn generate_bug_bounty_pack(options: BugBountyOptions) -> Result<BugBountyOutcome> {
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create bug bounty output directory {}",
            options.output_dir.display()
        )
    })?;

    let raw = fs::read(&options.config_path).wrap_err_with(|| {
        format!(
            "failed to read bug bounty config {}",
            options.config_path.display()
        )
    })?;
    let config: BugBountyConfig =
        json::from_slice(&raw).wrap_err("failed to parse bug bounty config as Norito JSON")?;

    let slug = sanitize_slug(config.slug.as_deref().unwrap_or(&config.program));
    let normalized_scope = normalize_scope(&config.scope)?;
    validate_required_scope(&normalized_scope)?;
    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    let overview_path = options.output_dir.join("bug_bounty_overview.md");
    let triage_path = options.output_dir.join("triage_checklist.md");
    let remediation_path = options.output_dir.join("remediation_template.md");
    let summary_path = options.output_dir.join("bug_bounty_summary.json");

    fs::write(
        &overview_path,
        render_overview_markdown(&config, &normalized_scope, &slug, &generated_at),
    )
    .wrap_err_with(|| format!("failed to write {}", overview_path.display()))?;
    fs::write(
        &triage_path,
        render_triage_checklist(&config, &normalized_scope),
    )
    .wrap_err_with(|| format!("failed to write {}", triage_path.display()))?;
    fs::write(
        &remediation_path,
        render_remediation_template(&config.reporting),
    )
    .wrap_err_with(|| format!("failed to write {}", remediation_path.display()))?;

    let summary = build_summary(
        &config,
        &normalized_scope,
        &slug,
        &generated_at,
        &overview_path,
        &triage_path,
        &remediation_path,
        &options.output_dir,
    );
    let summary_file = fs::File::create(&summary_path)
        .wrap_err_with(|| format!("failed to create {}", summary_path.display()))?;
    json::to_writer_pretty(summary_file, &summary)
        .wrap_err_with(|| format!("failed to write {}", summary_path.display()))?;

    Ok(BugBountyOutcome {
        overview_path,
        triage_checklist_path: triage_path,
        remediation_template_path: remediation_path,
        summary_path,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_summary(
    config: &BugBountyConfig,
    scope: &[ScopeSummary],
    slug: &str,
    generated_at: &str,
    overview_path: &Path,
    triage_path: &Path,
    remediation_path: &Path,
    output_dir: &Path,
) -> BugBountySummary {
    BugBountySummary {
        program: config.program.clone(),
        slug: slug.to_string(),
        owners: config.owners.clone(),
        contact: ContactSummary {
            intake_email: config.contact.intake_email.clone(),
            pagerduty: config.contact.pagerduty.clone(),
            tracker: config.contact.tracker.clone(),
            disclosure_url: config.contact.disclosure_url.clone(),
        },
        partners: config
            .partners
            .iter()
            .map(|partner| PartnerSummary {
                name: partner.name.clone(),
                contract_id: partner.contract_id.clone(),
                contact: partner.contact.clone(),
                coverage: partner
                    .coverage
                    .iter()
                    .map(|entry| normalize_area(entry))
                    .collect(),
                renewal: partner.renewal.clone(),
            })
            .collect(),
        scope: scope.to_vec(),
        severity_sla: config.severity_sla.clone(),
        reward_table: config.reward_table.clone(),
        triage: TriageSummary {
            intake_channels: config.triage.intake_channels.clone(),
            duplication_policy: config.triage.duplication_policy.clone(),
            evidence_requirements: config.triage.evidence_requirements.clone(),
            playbook: config.triage.playbook.clone(),
        },
        reporting: ReportingSummary {
            disclosure_window_days: config.reporting.disclosure_window_days,
            remediation_template_version: config.reporting.remediation_template_version.clone(),
            sla_dashboard: config.reporting.sla_dashboard.clone(),
            audit_bucket: config.reporting.audit_bucket.clone(),
        },
        outputs: OutputSummary {
            overview: summarize_path(overview_path, output_dir),
            triage_checklist: summarize_path(triage_path, output_dir),
            remediation_template: summarize_path(remediation_path, output_dir),
        },
        generated_at_rfc3339: generated_at.to_string(),
    }
}

fn normalize_scope(entries: &[ScopeEntry]) -> Result<Vec<ScopeSummary>> {
    entries
        .iter()
        .map(|entry| {
            if entry.targets.is_empty() {
                return Err(eyre::eyre!(
                    "scope `{}` must list at least one target",
                    entry.area
                ));
            }
            if entry.cadence.trim().is_empty() {
                return Err(eyre::eyre!(
                    "scope `{}` must include a cadence description",
                    entry.area
                ));
            }
            Ok(ScopeSummary {
                area: normalize_area(&entry.area),
                cadence: entry.cadence.clone(),
                targets: entry.targets.clone(),
                exclusions: entry.exclusions.clone(),
                notes: entry.notes.clone(),
            })
        })
        .collect()
}

fn validate_required_scope(scope: &[ScopeSummary]) -> Result<()> {
    let required = ["edge", "control-plane", "billing"];
    for entry in required {
        if !scope.iter().any(|scope| scope.area == entry) {
            return Err(eyre::eyre!(
                "missing `{entry}` coverage in bug bounty scope"
            ));
        }
    }
    Ok(())
}

fn sanitize_slug(value: &str) -> String {
    let mut cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    while cleaned.starts_with('-') || cleaned.starts_with('_') {
        cleaned.remove(0);
    }
    while cleaned.ends_with('-') || cleaned.ends_with('_') {
        cleaned.pop();
    }
    if cleaned.is_empty() {
        "bug-bounty".to_string()
    } else {
        cleaned
    }
}

fn normalize_area(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '_'], "-")
}

fn summarize_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn render_overview_markdown(
    config: &BugBountyConfig,
    scope: &[ScopeSummary],
    slug: &str,
    generated_at: &str,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# {} - Pen-test & Bug Bounty Packet ({})",
        config.program, slug
    );
    let _ = writeln!(out, "*Generated: {}*", generated_at);
    let _ = writeln!(out);
    let _ = writeln!(out, "Owners: {}", config.owners.join(", "));
    let _ = writeln!(
        out,
        "Intake: {} | Tracker: {}",
        config.contact.intake_email, config.contact.tracker
    );
    if let Some(pager) = &config.contact.pagerduty {
        let _ = writeln!(out, "On-call escalation: {}", pager);
    }
    if let Some(disclosure) = &config.contact.disclosure_url {
        let _ = writeln!(out, "Public policy: {}", disclosure);
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## External Partners");
    for partner in &config.partners {
        let coverage = partner
            .coverage
            .iter()
            .map(|entry| normalize_area(entry))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "- {} (contract {}, coverage: {}, renewal {} - contact {})",
            partner.name, partner.contract_id, coverage, partner.renewal, partner.contact
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Scope & Cadence");
    for entry in scope {
        let _ = writeln!(
            out,
            "- **{}** - cadence: {}, targets: {}",
            entry.area,
            entry.cadence,
            entry.targets.join(", ")
        );
        if !entry.exclusions.is_empty() {
            let _ = writeln!(out, "  - Exclusions: {}", entry.exclusions.join(", "));
        }
        if let Some(notes) = &entry.notes {
            let _ = writeln!(out, "  - Notes: {}", notes);
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Severity SLA (ack / contain / fix)");
    for (label, window) in [
        ("Critical", &config.severity_sla.critical),
        ("High", &config.severity_sla.high),
        ("Medium", &config.severity_sla.medium),
        ("Low", &config.severity_sla.low),
    ] {
        let _ = writeln!(
            out,
            "- {}: {}h / {}h / {}d",
            label, window.ack_hours, window.contain_hours, window.fix_days
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Reward bands");
    for band in &config.reward_table {
        let _ = writeln!(
            out,
            "- {}: {} {} - {} {}",
            band.severity, band.min, band.currency, band.max, band.currency
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Reporting & disclosure");
    let _ = writeln!(
        out,
        "- Disclosure window: {} days",
        config.reporting.disclosure_window_days
    );
    let _ = writeln!(out, "- SLO dashboard: {}", config.reporting.sla_dashboard);
    let _ = writeln!(out, "- Audit bucket: {}", config.reporting.audit_bucket);
    out
}

fn render_triage_checklist(config: &BugBountyConfig, scope: &[ScopeSummary]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Triage checklist");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Intake channels: {}.",
        config.triage.intake_channels.join(", ")
    );
    let _ = writeln!(
        out,
        "Duplication policy: {}.",
        config.triage.duplication_policy
    );
    let _ = writeln!(out, "Evidence requirements:");
    for requirement in &config.triage.evidence_requirements {
        let _ = writeln!(out, "- {}", requirement);
    }
    let slowest_ack = config
        .severity_sla
        .critical
        .ack_hours
        .max(config.severity_sla.high.ack_hours)
        .max(config.severity_sla.medium.ack_hours)
        .max(config.severity_sla.low.ack_hours);
    let _ = writeln!(
        out,
        "Acknowledge via intake within {}h and record the run id in the tracker.",
        slowest_ack
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "Playbook:");
    for (idx, step) in config.triage.playbook.iter().enumerate() {
        let _ = writeln!(out, "{}. {}", idx + 1, step);
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "Coverage checkpoints:");
    for entry in scope {
        let _ = writeln!(
            out,
            "- {}: validate targets {} and note exclusions: {}.",
            entry.area,
            entry.targets.join(", "),
            if entry.exclusions.is_empty() {
                "(none)".to_string()
            } else {
                entry.exclusions.join(", ")
            }
        );
    }
    out
}

fn render_remediation_template(reporting: &Reporting) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Remediation report (template {})",
        reporting.remediation_template_version
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "- Finding id:");
    let _ = writeln!(out, "- Severity / CVSS:");
    let _ = writeln!(out, "- Impacted asset(s):");
    let _ = writeln!(out, "- Discovery source (pen-test / bug bounty):");
    let _ = writeln!(out, "- Proof of concept:");
    let _ = writeln!(out, "- Blast radius + affected caches/PoPs:");
    let _ = writeln!(out, "- Containment action and timestamp:");
    let _ = writeln!(out, "- Fix applied (build/idempotency notes):");
    let _ = writeln!(out, "- Regression tests / monitors:");
    let _ = writeln!(
        out,
        "- Disclosure window alignment ({} days):",
        reporting.disclosure_window_days
    );
    let _ = writeln!(
        out,
        "- Attachments (logs, evidence bundle, dashboard screenshots):"
    );
    let _ = writeln!(out, "- Approvals (Security, SRE, Release):");
    out
}

#[cfg(test)]
mod tests {
    use norito::{json, json::Value};
    use tempfile::tempdir;

    use super::*;

    fn sample_config() -> BugBountyConfig {
        BugBountyConfig {
            program: "Gateway Bug Bounty".to_string(),
            slug: Some("snnet-15h1".to_string()),
            owners: vec!["Security".to_string(), "SRE".to_string()],
            contact: Contact {
                intake_email: "security@example.org".to_string(),
                pagerduty: Some("pagerduty://security".to_string()),
                tracker: "https://tracker".to_string(),
                disclosure_url: Some("https://policy".to_string()),
            },
            partners: vec![Partner {
                name: "Vector Labs".to_string(),
                contact: "vector@example.org".to_string(),
                contract_id: "VEC-2027-01".to_string(),
                renewal: "2027-04-01".to_string(),
                coverage: vec!["edge".to_string(), "billing".to_string()],
            }],
            scope: vec![
                ScopeEntry {
                    area: "edge".to_string(),
                    cadence: "quarterly".to_string(),
                    targets: vec!["gateway".to_string()],
                    exclusions: vec![],
                    notes: Some("cover TLS/ECH".to_string()),
                },
                ScopeEntry {
                    area: "control-plane".to_string(),
                    cadence: "quarterly".to_string(),
                    targets: vec!["api".to_string()],
                    exclusions: vec!["beta".to_string()],
                    notes: None,
                },
                ScopeEntry {
                    area: "billing".to_string(),
                    cadence: "semiannual".to_string(),
                    targets: vec!["rating".to_string()],
                    exclusions: vec![],
                    notes: None,
                },
            ],
            severity_sla: SeveritySla {
                critical: SeverityWindow {
                    ack_hours: 4,
                    contain_hours: 24,
                    fix_days: 7,
                },
                high: SeverityWindow {
                    ack_hours: 8,
                    contain_hours: 48,
                    fix_days: 14,
                },
                medium: SeverityWindow {
                    ack_hours: 24,
                    contain_hours: 120,
                    fix_days: 30,
                },
                low: SeverityWindow {
                    ack_hours: 48,
                    contain_hours: 240,
                    fix_days: 45,
                },
            },
            triage: TriagePolicy {
                intake_channels: vec!["security@example.org".to_string()],
                playbook: vec![
                    "Acknowledge reporter".to_string(),
                    "Assign severity".to_string(),
                ],
                duplication_policy: "Match cache version + manifest id".to_string(),
                evidence_requirements: vec!["PoC".to_string(), "logs".to_string()],
            },
            reward_table: vec![RewardBand {
                severity: "critical".to_string(),
                min: 3000,
                max: 6000,
                currency: "USD".to_string(),
            }],
            reporting: Reporting {
                disclosure_window_days: 90,
                remediation_template_version: "v1".to_string(),
                sla_dashboard: "dashboards/grafana/soranet_bug_bounty.json".to_string(),
                audit_bucket: "s3://artifacts".to_string(),
            },
        }
    }

    #[test]
    fn slug_sanitizer_removes_bad_chars() {
        assert_eq!(
            sanitize_slug("  Gateway & Bug Bounty!!"),
            "gateway---bug-bounty"
        );
        assert_eq!(sanitize_slug(""), "bug-bounty");
    }

    #[test]
    fn overview_mentions_scope_and_sla() {
        let cfg = sample_config();
        let normalized = normalize_scope(&cfg.scope).expect("normalize scope");
        let overview =
            render_overview_markdown(&cfg, &normalized, "snnet-15h1", "2026-11-20T00:00:00Z");
        assert!(
            overview.contains("edge") && overview.contains("billing"),
            "scope areas missing"
        );
        assert!(overview.contains("4h / 24h / 7d"), "SLA not rendered");
    }

    #[test]
    fn generates_pack() {
        let cfg = sample_config();
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("config.json");
        let config_file = fs::File::create(&config_path).expect("create config file");
        json::to_writer_pretty(config_file, &cfg).expect("write config");

        let output_dir = temp.path().join("out");
        let outcome = generate_bug_bounty_pack(BugBountyOptions {
            config_path,
            output_dir: output_dir.clone(),
        })
        .expect("generate pack");

        for path in [
            outcome.overview_path,
            outcome.triage_checklist_path,
            outcome.remediation_template_path,
            outcome.summary_path,
        ] {
            assert!(path.exists(), "{} missing", path.display());
        }
        let summary_bytes =
            fs::read(output_dir.join("bug_bounty_summary.json")).expect("summary exists");
        let summary: Value = json::from_slice(&summary_bytes).expect("parse summary");
        assert_eq!(
            summary["slug"],
            norito::json!("snnet-15h1"),
            "slug mismatch"
        );
        assert_eq!(
            summary["scope"].as_array().map(|v| v.len()),
            Some(3),
            "scope length wrong"
        );
    }
}
