//! GAR controller bundle generator (SNNet-15G).
//!
//! Reads a GAR controller config, emits NATS dispatch events, and records GAR
//! enforcement receipts for each PoP so compliance/export pipelines can ship a
//! single evidence bundle.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use eyre::{Result, WrapErr, eyre};
use hex::encode as hex_encode;
use iroha_data_model::{
    account::AccountId,
    sorafs::gar::{GarEnforcementActionV1, GarEnforcementReceiptV1, GarPolicyPayloadV1},
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

/// CLI options accepted by the controller generator.
#[derive(Debug, Clone)]
pub struct GarControllerOptions {
    pub config: PathBuf,
    pub output_dir: PathBuf,
    pub markdown_out: Option<PathBuf>,
    pub now_unix: Option<u64>,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct GarControllerConfig {
    base_subject: String,
    operator: String,
    #[norito(default)]
    default_expires_at_unix: Option<u64>,
    pops: Vec<PopSpec>,
    policies: Vec<GarPolicySpec>,
}

#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
struct PopSpec {
    label: String,
    #[norito(default)]
    nats_subject: Option<String>,
    #[norito(default)]
    audit_uri: Option<String>,
}

#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
struct GarPolicySpec {
    gar_name: String,
    canonical_host: String,
    policy: GarPolicyPayloadV1,
    #[norito(default)]
    policy_version: Option<String>,
    #[norito(default)]
    cache_version: Option<String>,
    #[norito(default)]
    evidence_uris: Vec<String>,
    #[norito(default)]
    labels: Vec<String>,
    #[norito(default)]
    expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct GarControllerSummary {
    pub base_subject: String,
    pub output_dir: String,
    pub events_path: String,
    pub receipts_dir: String,
    pub markdown_report: Option<String>,
    pub reconciliation_path: String,
    pub metrics_path: String,
    pub audit_log_path: String,
    pub pops: Vec<PopDispatchSummary>,
    pub policies: Vec<PolicyDispatchSummary>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct PopDispatchSummary {
    pub label: String,
    pub subject: String,
    pub receipts: usize,
    pub audit_uri: Option<String>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct PolicyDispatchSummary {
    pub gar_name: String,
    pub canonical_host: String,
    pub policy_version: Option<String>,
    pub cache_version: Option<String>,
    pub action: String,
    pub reason: String,
    pub policy_digest: String,
    pub expires_at_unix: Option<u64>,
    pub labels: Vec<String>,
    pub evidence_uris: Vec<String>,
    pub pops: Vec<String>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct DispatchPayload {
    gar_name: String,
    canonical_host: String,
    policy_version: Option<String>,
    cache_version: Option<String>,
    policy_digest: String,
    emitted_at_unix: u64,
    policy: GarPolicyPayloadV1,
    labels: Vec<String>,
    pop_label: String,
}

#[derive(Debug, Clone, JsonSerialize)]
struct NatsEvent {
    subject: String,
    payload: DispatchPayload,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct ReconciliationReport {
    generated_at_unix: u64,
    pop_coverage: Vec<PopCoverage>,
    policy_coverage: Vec<PolicyCoverage>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct PopCoverage {
    label: String,
    receipts: usize,
    missing_policies: Vec<String>,
    audit_uri: Option<String>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct PolicyCoverage {
    gar_name: String,
    canonical_host: String,
    action: String,
    pop_count: usize,
    missing_pops: Vec<String>,
    expires_at_unix: Option<u64>,
    policy_version: Option<String>,
    cache_version: Option<String>,
    labels: Vec<String>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct AuditEntry {
    gar_name: String,
    canonical_host: String,
    pop_label: String,
    action: String,
    reason: String,
    subject: String,
    receipt_json: String,
    expires_at_unix: Option<u64>,
    evidence_uris: Vec<String>,
    labels: Vec<String>,
    emitted_at_unix: u64,
}

/// Generate dispatch events and receipts for the supplied config, returning a summary.
pub fn run_gar_controller(options: GarControllerOptions) -> Result<GarControllerSummary> {
    let GarControllerOptions {
        config,
        output_dir,
        markdown_out,
        now_unix,
    } = options;

    let reader = File::open(&config)
        .wrap_err_with(|| format!("failed to open controller config {}", config.display()))?;
    let parsed: GarControllerConfig = json::from_reader(reader)
        .wrap_err_with(|| format!("failed to parse controller config at {}", config.display()))?;
    let operator: AccountId = AccountId::parse_encoded(&parsed.operator)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| eyre!("invalid operator account `{}`: {err}", parsed.operator))?;

    let now_unix = now_unix.unwrap_or_else(current_unix);
    fs::create_dir_all(&output_dir).wrap_err_with(|| {
        format!(
            "failed to create GAR controller output dir {}",
            output_dir.display()
        )
    })?;
    let receipts_dir = output_dir.join("gar_receipts");
    fs::create_dir_all(&receipts_dir).wrap_err_with(|| {
        format!(
            "failed to create GAR receipt dir {}",
            receipts_dir.display()
        )
    })?;
    let audit_log_path = output_dir.join("gar_audit_log.jsonl");
    let reconciliation_path = output_dir.join("gar_reconciliation_report.json");
    let metrics_path = output_dir.join("gar_metrics.prom");
    let events_path = output_dir.join("gar_events.jsonl");
    let mut events_file = File::create(&events_path).wrap_err_with(|| {
        format!(
            "failed to create NATS event spool {}",
            events_path.display()
        )
    })?;

    let mut pop_summaries = Vec::new();
    let mut policy_summaries = Vec::new();
    let mut audit_entries = Vec::new();

    if parsed.pops.is_empty() {
        return Err(eyre!("controller config must list at least one pop target"));
    }

    for policy in parsed.policies.iter() {
        let digest = policy_digest(&policy.policy)?;
        let policy_digest_hex = hex_encode(digest);
        let (action, reason) = derive_primary_action(policy);
        let effective_expiry = policy.expires_at_unix.or(parsed.default_expires_at_unix);
        let mut pop_labels = Vec::new();

        for pop in parsed.pops.iter() {
            let subject = pop
                .nats_subject
                .clone()
                .unwrap_or_else(|| format!("{}.{}", parsed.base_subject, sanitize(&pop.label)));
            let payload = DispatchPayload {
                gar_name: policy.gar_name.clone(),
                canonical_host: policy.canonical_host.clone(),
                policy_version: policy.policy_version.clone(),
                cache_version: policy.cache_version.clone(),
                policy_digest: policy_digest_hex.clone(),
                emitted_at_unix: now_unix,
                policy: policy.policy.clone(),
                labels: policy.labels.clone(),
                pop_label: pop.label.clone(),
            };
            let event = NatsEvent {
                subject: subject.clone(),
                payload,
            };
            let event_value =
                json::to_value(&event).wrap_err("failed to encode NATS event payload")?;
            json::to_writer(&mut events_file, &event_value)
                .wrap_err("failed to serialize NATS event payload")?;
            writeln!(&mut events_file).wrap_err("failed to write NATS event spool")?;

            let receipt = build_receipt(
                policy,
                &digest,
                &action,
                &reason,
                pop,
                &operator,
                now_unix,
                parsed.default_expires_at_unix,
            )?;
            let (receipt_json_path, _norito_path) =
                write_receipt(&receipts_dir, &policy.gar_name, &pop.label, &receipt)?;
            pop_labels.push(pop.label.clone());

            audit_entries.push(AuditEntry {
                gar_name: policy.gar_name.clone(),
                canonical_host: policy.canonical_host.clone(),
                pop_label: pop.label.clone(),
                action: action_label(&action).to_string(),
                reason: reason.clone(),
                subject: subject.clone(),
                receipt_json: receipt_json_path.display().to_string(),
                expires_at_unix: receipt.expires_at_unix,
                evidence_uris: receipt.evidence_uris.clone(),
                labels: receipt.labels.clone(),
                emitted_at_unix: now_unix,
            });

            update_pop_summary(&mut pop_summaries, pop, &subject);
        }

        policy_summaries.push(PolicyDispatchSummary {
            gar_name: policy.gar_name.clone(),
            canonical_host: policy.canonical_host.clone(),
            policy_version: policy.policy_version.clone(),
            cache_version: policy.cache_version.clone(),
            action: action_label(&action).to_string(),
            reason: reason.clone(),
            policy_digest: policy_digest_hex,
            expires_at_unix: effective_expiry,
            labels: policy.labels.clone(),
            evidence_uris: policy.evidence_uris.clone(),
            pops: pop_labels,
        });
    }

    write_audit_log(&audit_log_path, &audit_entries)?;
    let reconciliation_report =
        build_reconciliation_report(&parsed.pops, &policy_summaries, now_unix);
    let reconciliation_file = File::create(&reconciliation_path).wrap_err_with(|| {
        format!(
            "failed to create GAR reconciliation report {}",
            reconciliation_path.display()
        )
    })?;
    json::to_writer_pretty(reconciliation_file, &reconciliation_report).wrap_err_with(|| {
        format!(
            "failed to write GAR reconciliation report {}",
            reconciliation_path.display()
        )
    })?;

    fs::write(
        &metrics_path,
        render_metrics(&policy_summaries, &reconciliation_report),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write GAR metrics snapshot {}",
            metrics_path.display()
        )
    })?;

    let markdown_path = markdown_out.unwrap_or_else(|| output_dir.join("gar_controller_report.md"));
    let summary = GarControllerSummary {
        base_subject: parsed.base_subject.clone(),
        output_dir: output_dir.display().to_string(),
        events_path: events_path.display().to_string(),
        receipts_dir: receipts_dir.display().to_string(),
        markdown_report: Some(markdown_path.display().to_string()),
        reconciliation_path: reconciliation_path.display().to_string(),
        metrics_path: metrics_path.display().to_string(),
        audit_log_path: audit_log_path.display().to_string(),
        pops: pop_summaries,
        policies: policy_summaries,
    };

    let summary_path = output_dir.join("gar_controller_summary.json");
    let summary_file = File::create(&summary_path).wrap_err_with(|| {
        format!(
            "failed to create controller summary {}",
            summary_path.display()
        )
    })?;
    json::to_writer_pretty(summary_file, &summary).wrap_err_with(|| {
        format!(
            "failed to write controller summary {}",
            summary_path.display()
        )
    })?;

    fs::write(
        &markdown_path,
        render_markdown(&summary, &reconciliation_report),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write controller report {}",
            markdown_path.display()
        )
    })?;

    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    policy: &GarPolicySpec,
    policy_digest: &[u8; 32],
    action: &GarEnforcementActionV1,
    reason: &str,
    pop: &PopSpec,
    operator: &AccountId,
    now_unix: u64,
    default_expiry: Option<u64>,
) -> Result<GarEnforcementReceiptV1> {
    let receipt_id = build_receipt_id(&policy.gar_name, &pop.label, policy_digest, now_unix);
    let mut evidence = policy.evidence_uris.clone();
    if let Some(uri) = &pop.audit_uri {
        evidence.push(uri.clone());
    }
    let mut labels = policy.labels.clone();
    labels.push(format!("pop:{}", pop.label));
    labels.push(format!("gar:{}", policy.gar_name));

    Ok(GarEnforcementReceiptV1 {
        receipt_id,
        gar_name: policy.gar_name.clone(),
        canonical_host: policy.canonical_host.clone(),
        action: action.clone(),
        triggered_at_unix: now_unix,
        expires_at_unix: policy.expires_at_unix.or(default_expiry),
        policy_version: policy.policy_version.clone(),
        policy_digest: Some(*policy_digest),
        operator: operator.clone(),
        reason: reason.to_string(),
        notes: pop.audit_uri.clone(),
        evidence_uris: evidence,
        labels,
    })
}

fn write_receipt(
    receipts_dir: &Path,
    gar_name: &str,
    pop_label: &str,
    receipt: &GarEnforcementReceiptV1,
) -> Result<(PathBuf, PathBuf)> {
    let stem = format!("{}-{}", sanitize(gar_name), sanitize(pop_label));
    let json_path = receipts_dir.join(format!("{stem}.json"));
    let norito_path = receipts_dir.join(format!("{stem}.to"));

    let json_file = File::create(&json_path)
        .wrap_err_with(|| format!("failed to create receipt json {}", json_path.display()))?;
    json::to_writer_pretty(json_file, receipt)
        .wrap_err_with(|| format!("failed to write receipt json copy {}", json_path.display()))?;

    let norito_bytes =
        norito::to_bytes(receipt).wrap_err_with(|| format!("failed to encode receipt {}", stem))?;
    fs::write(&norito_path, &norito_bytes).wrap_err_with(|| {
        format!(
            "failed to write receipt norito copy {}",
            norito_path.display()
        )
    })?;

    Ok((json_path, norito_path))
}

fn write_audit_log(path: &Path, entries: &[AuditEntry]) -> Result<()> {
    let mut file = File::create(path)
        .wrap_err_with(|| format!("failed to create GAR audit log {}", path.display()))?;
    for entry in entries {
        let value = json::to_value(entry).wrap_err("failed to encode GAR audit entry payload")?;
        json::to_writer(&mut file, &value).wrap_err("failed to serialize GAR audit entry")?;
        writeln!(&mut file).wrap_err("failed to write audit log entry")?;
    }
    Ok(())
}

fn policy_digest(policy: &GarPolicyPayloadV1) -> Result<[u8; 32]> {
    let bytes = norito::to_bytes(policy).wrap_err("failed to Norito-encode GAR policy payload")?;
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(*digest.as_bytes())
}

fn derive_primary_action(policy: &GarPolicySpec) -> (GarEnforcementActionV1, String) {
    let Some(cdn) = &policy.policy.cdn_policy else {
        return (
            GarEnforcementActionV1::AuditNotice,
            "no CDN policy attached; recording audit notice".to_string(),
        );
    };

    if cdn.legal_hold {
        return (
            GarEnforcementActionV1::LegalHold,
            "legal hold active for this GAR".to_string(),
        );
    }

    if !cdn.allow_regions.is_empty() || !cdn.deny_regions.is_empty() {
        return (
            GarEnforcementActionV1::GeoFence,
            "geofence update required for regional policy".to_string(),
        );
    }

    if cdn.rate_ceiling_rps.is_some() {
        return (
            GarEnforcementActionV1::RateLimitOverride,
            "rate ceiling applied at gateway edge".to_string(),
        );
    }

    if cdn.ttl_override_secs.is_some() {
        return (
            GarEnforcementActionV1::TtlOverride,
            "ttl override applied to cached objects".to_string(),
        );
    }

    if !cdn.purge_tags.is_empty() {
        return (
            GarEnforcementActionV1::PurgeStaticZone,
            "purge tags required before serving content".to_string(),
        );
    }

    if !cdn.moderation_slugs.is_empty() {
        return (
            GarEnforcementActionV1::Moderation,
            "moderation directive applied to GAR".to_string(),
        );
    }

    (
        GarEnforcementActionV1::AuditNotice,
        "recording GAR audit notice".to_string(),
    )
}

fn action_label(action: &GarEnforcementActionV1) -> &'static str {
    match action {
        GarEnforcementActionV1::PurgeStaticZone => "purge_static_zone",
        GarEnforcementActionV1::CacheBypass => "cache_bypass",
        GarEnforcementActionV1::TtlOverride => "ttl_override",
        GarEnforcementActionV1::RateLimitOverride => "rate_limit_override",
        GarEnforcementActionV1::GeoFence => "geo_fence",
        GarEnforcementActionV1::LegalHold => "legal_hold",
        GarEnforcementActionV1::Moderation => "moderation",
        GarEnforcementActionV1::AuditNotice => "audit_notice",
        GarEnforcementActionV1::Custom(_) => "custom",
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn build_receipt_id(
    gar_name: &str,
    pop_label: &str,
    policy_digest: &[u8; 32],
    now_unix: u64,
) -> [u8; 16] {
    let mut hasher = Hasher::new();
    hasher.update(gar_name.as_bytes());
    hasher.update(pop_label.as_bytes());
    hasher.update(policy_digest);
    hasher.update(&now_unix.to_le_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

fn render_markdown(
    summary: &GarControllerSummary,
    reconciliation: &ReconciliationReport,
) -> String {
    let mut out = String::new();
    out.push_str("# GAR Controller Dispatch Report\n");
    out.push_str(&format!(
        "- Base subject: {}\n- Events: {}\n- Receipts: {}\n- Metrics: {}\n- Audit log: {}\n- Reconciliation: {}\n\n",
        summary.base_subject,
        summary.events_path,
        summary.receipts_dir,
        summary.metrics_path,
        summary.audit_log_path,
        summary.reconciliation_path
    ));
    if !summary.pops.is_empty() {
        out.push_str("## Pops\n");
        out.push_str("| Pop | Subject | Receipts | Audit URI |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for pop in &summary.pops {
            let audit = pop.audit_uri.as_ref().map_or("-", |uri| uri.as_str());
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                pop.label, pop.subject, pop.receipts, audit
            ));
        }
        out.push('\n');
    }
    if !summary.policies.is_empty() {
        out.push_str("## Policies\n");
        out.push_str(
            "| GAR | Host | Action | Reason | Expires | Digest | Pops | Labels | Evidence |\n",
        );
        out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for policy in &summary.policies {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | `{}` | {} | {} | {} |\n",
                policy.gar_name,
                policy.canonical_host,
                policy.action,
                policy.reason,
                format_opt_unix(policy.expires_at_unix),
                policy.policy_digest,
                policy.pops.join(", "),
                if policy.labels.is_empty() {
                    "-".to_string()
                } else {
                    policy.labels.join(", ")
                },
                if policy.evidence_uris.is_empty() {
                    "-".to_string()
                } else {
                    policy.evidence_uris.join(", ")
                }
            ));
        }
        out.push('\n');
    }

    if !reconciliation.policy_coverage.is_empty() {
        out.push_str("## Reconciliation\n");
        out.push_str("| GAR | Action | Missing Pops | Expires | Labels |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for coverage in &reconciliation.policy_coverage {
            let missing = if coverage.missing_pops.is_empty() {
                "-".to_string()
            } else {
                coverage.missing_pops.join(", ")
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                coverage.gar_name,
                coverage.action,
                missing,
                format_opt_unix(coverage.expires_at_unix),
                if coverage.labels.is_empty() {
                    "-".to_string()
                } else {
                    coverage.labels.join(", ")
                }
            ));
        }
        out.push('\n');
    }

    if !reconciliation.warnings.is_empty() {
        out.push_str("## Warnings\n");
        for warning in &reconciliation.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    out
}

fn format_opt_unix(value: Option<u64>) -> String {
    value
        .map(|unix| unix.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn build_reconciliation_report(
    pops: &[PopSpec],
    policies: &[PolicyDispatchSummary],
    now_unix: u64,
) -> ReconciliationReport {
    let pop_labels: Vec<String> = pops.iter().map(|pop| pop.label.clone()).collect();
    let mut warnings = BTreeSet::new();

    let pop_coverage = pops
        .iter()
        .map(|pop| {
            let missing: Vec<String> = policies
                .iter()
                .filter(|policy| !policy.pops.contains(&pop.label))
                .map(|policy| policy.gar_name.clone())
                .collect();
            let receipts = policies
                .iter()
                .filter(|policy| policy.pops.contains(&pop.label))
                .count();
            if policies.is_empty() {
                warnings.insert("no GAR policies present in controller config".to_string());
            } else if receipts == 0 {
                warnings.insert(format!("pop {} missing GAR dispatch entries", pop.label));
            }
            PopCoverage {
                label: pop.label.clone(),
                receipts,
                missing_policies: missing,
                audit_uri: pop.audit_uri.clone(),
            }
        })
        .collect();

    let policy_coverage = policies
        .iter()
        .map(|policy| {
            let missing_pops: Vec<String> = pop_labels
                .iter()
                .filter(|pop| !policy.pops.contains(*pop))
                .cloned()
                .collect();
            if let Some(expiry) = policy.expires_at_unix
                && expiry <= now_unix
            {
                warnings.insert(format!("GAR {} expired at {}", policy.gar_name, expiry));
            }
            if !missing_pops.is_empty() {
                warnings.insert(format!(
                    "GAR {} missing reconciliation for pops: {}",
                    policy.gar_name,
                    missing_pops.join(", ")
                ));
            }
            PolicyCoverage {
                gar_name: policy.gar_name.clone(),
                canonical_host: policy.canonical_host.clone(),
                action: policy.action.clone(),
                pop_count: policy.pops.len(),
                missing_pops,
                expires_at_unix: policy.expires_at_unix,
                policy_version: policy.policy_version.clone(),
                cache_version: policy.cache_version.clone(),
                labels: policy.labels.clone(),
            }
        })
        .collect();

    ReconciliationReport {
        generated_at_unix: now_unix,
        pop_coverage,
        policy_coverage,
        warnings: warnings.into_iter().collect(),
    }
}

fn render_metrics(
    policies: &[PolicyDispatchSummary],
    reconciliation: &ReconciliationReport,
) -> String {
    let mut out = String::new();
    out.push_str("# TYPE gar_policy_dispatch_total counter\n");
    for policy in policies {
        for pop in &policy.pops {
            out.push_str(&format!(
                "gar_policy_dispatch_total{{gar=\"{}\",pop=\"{}\",action=\"{}\"}} 1\n",
                policy.gar_name, pop, policy.action
            ));
        }
    }

    out.push_str("# TYPE gar_policy_missing_pops gauge\n");
    for coverage in &reconciliation.policy_coverage {
        out.push_str(&format!(
            "gar_policy_missing_pops{{gar=\"{}\"}} {}\n",
            coverage.gar_name,
            coverage.missing_pops.len()
        ));
    }

    out.push_str("# TYPE gar_policy_expiry_unix gauge\n");
    for coverage in &reconciliation.policy_coverage {
        let expiry = coverage.expires_at_unix.unwrap_or(0);
        out.push_str(&format!(
            "gar_policy_expiry_unix{{gar=\"{}\"}} {}\n",
            coverage.gar_name, expiry
        ));
    }

    out.push_str("# TYPE gar_policy_labels_total gauge\n");
    for coverage in &reconciliation.policy_coverage {
        for label in &coverage.labels {
            out.push_str(&format!(
                "gar_policy_labels_total{{gar=\"{}\",label=\"{}\"}} 1\n",
                coverage.gar_name, label
            ));
        }
    }

    out.push_str("# TYPE gar_policy_warning_total counter\n");
    out.push_str(&format!(
        "gar_policy_warning_total {}\n",
        reconciliation.warnings.len()
    ));

    out
}

fn update_pop_summary(summaries: &mut Vec<PopDispatchSummary>, pop: &PopSpec, subject: &str) {
    if let Some(existing) = summaries.iter_mut().find(|s| s.label == pop.label) {
        existing.receipts = existing.receipts.saturating_add(1);
        if existing.audit_uri.is_none() {
            existing.audit_uri = pop.audit_uri.clone();
        }
        if existing.subject != subject {
            existing.subject = subject.to_string();
        }
    } else {
        summaries.push(PopDispatchSummary {
            label: pop.label.clone(),
            subject: subject.to_string(),
            receipts: 1,
            audit_uri: pop.audit_uri.clone(),
        });
    }
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sorafs::gar::GarCdnPolicyV1;
    use norito::json;
    use tempfile::TempDir;

    use super::*;

    fn empty_cdn_policy() -> GarCdnPolicyV1 {
        GarCdnPolicyV1 {
            ttl_override_secs: None,
            purge_tags: Vec::new(),
            moderation_slugs: Vec::new(),
            rate_ceiling_rps: None,
            allow_regions: Vec::new(),
            deny_regions: Vec::new(),
            legal_hold: false,
        }
    }

    #[test]
    fn derives_actions_from_cdn_policy() {
        let mut spec = GarPolicySpec {
            gar_name: "docs.sora".to_string(),
            canonical_host: "docs.gw.sora.net".to_string(),
            policy: GarPolicyPayloadV1 {
                cdn_policy: Some(GarCdnPolicyV1 {
                    legal_hold: true,
                    ..empty_cdn_policy()
                }),
                ..GarPolicyPayloadV1::default()
            },
            policy_version: None,
            cache_version: None,
            evidence_uris: Vec::new(),
            labels: Vec::new(),
            expires_at_unix: None,
        };
        let (action, _) = derive_primary_action(&spec);
        assert!(matches!(action, GarEnforcementActionV1::LegalHold));

        spec.policy.cdn_policy = Some(GarCdnPolicyV1 {
            legal_hold: false,
            allow_regions: vec!["EU".to_string()],
            ..empty_cdn_policy()
        });
        let (action, _) = derive_primary_action(&spec);
        assert!(matches!(action, GarEnforcementActionV1::GeoFence));

        spec.policy.cdn_policy = Some(GarCdnPolicyV1 {
            allow_regions: Vec::new(),
            rate_ceiling_rps: Some(100),
            ..empty_cdn_policy()
        });
        let (action, _) = derive_primary_action(&spec);
        assert!(matches!(action, GarEnforcementActionV1::RateLimitOverride));

        spec.policy.cdn_policy = Some(GarCdnPolicyV1 {
            rate_ceiling_rps: None,
            ttl_override_secs: Some(60),
            ..empty_cdn_policy()
        });
        let (action, _) = derive_primary_action(&spec);
        assert!(matches!(action, GarEnforcementActionV1::TtlOverride));
    }

    #[test]
    fn generates_dispatch_bundle() -> Result<()> {
        let temp = TempDir::new()?;
        let config_path = temp.path().join("gar_controller.json");
        let output_dir = temp.path().join("out");
        let operator = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D";
        let config_json = format!(
            r#"{{
  "base_subject": "soranet.gar",
  "operator": "{operator}",
  "pops": [
    {{"label": "soranet-pop-sjc", "nats_subject": "soranet.gar.sjc"}},
    {{"label": "soranet-pop-ams"}}
  ],
  "policies": [
    {{
      "gar_name": "docs.sora",
      "canonical_host": "docs.gw.sora.net",
      "policy_version": "m1",
      "policy": {{
        "license_sets": [],
        "moderation_directives": [],
        "cdn_policy": {{
          "ttl_override_secs": 300,
          "purge_tags": ["rollout"],
          "moderation_slugs": [],
          "rate_ceiling_rps": 2000,
          "allow_regions": ["NA", "EU"],
          "deny_regions": [],
          "legal_hold": false
        }},
        "metrics_policy": null,
        "telemetry_labels": ["gar.docs"],
        "rpt_digest": null
      }},
      "cache_version": "m1",
      "evidence_uris": ["sora://gar/docs/log"]
    }}
  ]
}}"#
        );
        fs::write(&config_path, config_json)?;

        let summary = run_gar_controller(GarControllerOptions {
            config: config_path.clone(),
            output_dir: output_dir.clone(),
            markdown_out: None,
            now_unix: Some(1_700_000_000),
        })?;

        let events = fs::read_to_string(output_dir.join("gar_events.jsonl"))?;
        let lines: Vec<&str> = events.lines().collect();
        assert_eq!(lines.len(), 2, "one policy should emit per-pop events");

        let receipts_dir = output_dir.join("gar_receipts");
        let receipts: Vec<_> = fs::read_dir(&receipts_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect();
        assert_eq!(receipts.len(), 2, "should write two receipts");

        let first_receipt_path = receipts_dir.join("docs-sora-soranet-pop-sjc.json");
        let receipt_bytes = fs::read(&first_receipt_path)?;
        let receipt: GarEnforcementReceiptV1 = json::from_slice(&receipt_bytes)?;
        assert_eq!(receipt.action, GarEnforcementActionV1::GeoFence);
        assert_eq!(
            receipt.reason,
            "geofence update required for regional policy"
        );

        assert!(
            summary
                .policies
                .iter()
                .any(|policy| policy.gar_name == "docs.sora"),
            "summary should include the policy entry"
        );
        let metrics = fs::read_to_string(output_dir.join("gar_metrics.prom"))?;
        assert!(
            metrics.contains("gar_policy_dispatch_total"),
            "metrics snapshot should include dispatch totals"
        );

        let reconciliation = fs::read(output_dir.join("gar_reconciliation_report.json"))?;
        let reconciliation: ReconciliationReport = json::from_slice(&reconciliation)?;
        assert!(
            reconciliation.warnings.is_empty(),
            "baseline fixture should not emit warnings"
        );
        assert_eq!(
            reconciliation.policy_coverage.len(),
            1,
            "expected one policy coverage entry"
        );

        let audit_log = fs::read_to_string(output_dir.join("gar_audit_log.jsonl"))?;
        assert_eq!(
            audit_log.lines().count(),
            2,
            "audit log should include one entry per pop"
        );
        let summary_bytes = fs::read(output_dir.join("gar_controller_summary.json"))?;
        let summary_json: GarControllerSummary = json::from_slice(&summary_bytes)?;
        assert!(
            summary_json.metrics_path.ends_with("gar_metrics.prom"),
            "summary should carry metrics path"
        );
        Ok(())
    }

    #[test]
    fn reconciliation_flags_expired_policy() -> Result<()> {
        let temp = TempDir::new()?;
        let config_path = temp.path().join("gar_controller.json");
        let output_dir = temp.path().join("out");
        let operator = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D";
        let config_json = format!(
            r#"{{
  "base_subject": "soranet.gar",
  "operator": "{operator}",
  "pops": [
    {{"label": "soranet-pop-sjc"}}
  ],
  "policies": [
    {{
      "gar_name": "docs.sora",
      "canonical_host": "docs.gw.sora.net",
      "policy_version": "m1",
      "policy": {{
        "license_sets": [],
        "moderation_directives": [],
        "cdn_policy": {{
          "ttl_override_secs": 60,
          "purge_tags": [],
          "moderation_slugs": [],
          "rate_ceiling_rps": 1000,
          "allow_regions": [],
          "deny_regions": [],
          "legal_hold": false
        }},
        "metrics_policy": null,
        "telemetry_labels": [],
        "rpt_digest": null
      }},
      "cache_version": "m1",
      "evidence_uris": ["sora://gar/docs/log"],
      "expires_at_unix": 100
    }}
  ]
}}"#
        );
        fs::write(&config_path, config_json)?;

        let _summary = run_gar_controller(GarControllerOptions {
            config: config_path,
            output_dir: output_dir.clone(),
            markdown_out: None,
            now_unix: Some(1_000_000),
        })?;

        let reconciliation = fs::read(output_dir.join("gar_reconciliation_report.json"))?;
        let reconciliation: ReconciliationReport = json::from_slice(&reconciliation)?;
        assert!(
            reconciliation
                .warnings
                .iter()
                .any(|warning| warning.contains("expired")),
            "expired policy should emit a warning in reconciliation report"
        );
        Ok(())
    }
}
